use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use coolcooler_core::{DeviceInfo, Resolution};
use coolcooler_driver::{
    DisplayCapability, DisplayDriver, DisplayFrame, DisplayFrameEncoder, DisplayLoopEvent,
};
use image::RgbaImage;

pub(crate) struct DisplayController {
    state: DisplayState,
    status: DisplayStatus,
}

const FALLBACK_PREVIEW_RESOLUTION: Resolution = Resolution::new(240, 240);

enum DisplayState {
    Idle,
    Running(WorkerHandle),
    Stopping {
        worker: WorkerHandle,
        queued_start: Option<WorkerStart>,
    },
}

type WorkerHandle = Box<dyn DisplayWorkerHandle>;

trait DisplayWorkerHandle: Send {
    fn submit_frame(&self, frame: DisplayFrame);
    fn request_stop(&self);
    fn drain_events(&mut self) -> Vec<DisplayLoopEvent>;
    fn join_if_finished(&mut self) -> bool;
}

enum WorkerStart {
    Detected {
        driver: DisplayDriver,
        frame: DisplayFrame,
    },
    Reopen {
        info: DeviceInfo,
        capability: DisplayCapability,
        frame: DisplayFrame,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum DisplayStatus {
    Disconnected,
    Connected {
        info: DeviceInfo,
        capability: DisplayCapability,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DisplayNotice {
    Started(String),
    Reconnecting(String),
    TransferFailed(String),
    Failed(String),
    Stopped,
}

impl DisplayStatus {
    fn disconnected() -> Self {
        Self::Disconnected
    }

    fn from_driver(driver: &DisplayDriver) -> Self {
        Self::Connected {
            info: driver.info().clone(),
            capability: driver.capability(),
        }
    }

    fn info(&self) -> Option<&DeviceInfo> {
        match self {
            Self::Connected { info, .. } => Some(info),
            Self::Disconnected => None,
        }
    }

    fn capability(&self) -> Option<DisplayCapability> {
        match self {
            Self::Connected { capability, .. } => Some(*capability),
            Self::Disconnected => None,
        }
    }

    fn encoder(&self) -> Option<DisplayFrameEncoder> {
        match self {
            Self::Connected { info, capability } => {
                Some(DisplayFrameEncoder::new(info.clone(), *capability))
            }
            Self::Disconnected => None,
        }
    }
}

impl DisplayController {
    pub(crate) fn new() -> Self {
        let mut controller = Self {
            state: DisplayState::Idle,
            status: DisplayStatus::disconnected(),
        };
        controller.refresh_status();
        controller
    }

    pub(crate) fn device_info(&self) -> Option<&DeviceInfo> {
        self.status.info()
    }

    pub(crate) fn resolution(&self) -> Resolution {
        self.status
            .info()
            .map(|info| info.resolution)
            .unwrap_or(FALLBACK_PREVIEW_RESOLUTION)
    }

    pub(crate) fn capability(&self) -> Option<DisplayCapability> {
        self.status.capability()
    }

    pub(crate) fn present_with<F>(&mut self, render: F)
    where
        F: FnOnce(Resolution) -> RgbaImage,
    {
        if matches!(self.state, DisplayState::Running(_)) {
            match self.prepare_frame_from_status(render) {
                Some(frame) => {
                    if let DisplayState::Running(worker) = &self.state {
                        worker.submit_frame(frame);
                    }
                }
                None => self.stop(),
            }
            return;
        }

        if matches!(self.state, DisplayState::Stopping { .. }) {
            match self.prepare_reopen_from_status(render) {
                Some(worker_start) => self.replace_queued_start(worker_start),
                None => self.stop(),
            }
            return;
        }

        let Some(worker_start) = self.detect_worker_start(render) else {
            self.stop();
            return;
        };
        self.start(worker_start);
    }

    fn replace_queued_start(&mut self, worker_start: WorkerStart) {
        match std::mem::replace(&mut self.state, DisplayState::Idle) {
            DisplayState::Stopping { worker, .. } => {
                self.state = DisplayState::Stopping {
                    worker,
                    queued_start: Some(worker_start),
                };
            }
            state => self.state = state,
        }
    }

    pub(crate) fn stop(&mut self) {
        match std::mem::replace(&mut self.state, DisplayState::Idle) {
            DisplayState::Idle => {
                self.state = DisplayState::Idle;
            }
            DisplayState::Running(worker) => {
                worker.request_stop();
                self.state = DisplayState::Stopping {
                    worker,
                    queued_start: None,
                };
            }
            DisplayState::Stopping { worker, .. } => {
                self.state = DisplayState::Stopping {
                    worker,
                    queued_start: None,
                };
            }
        }
    }

    pub(crate) fn submit_frame(&self, composited: &RgbaImage) {
        if let DisplayState::Running(worker) = &self.state {
            if let Some(bytes) = self
                .status
                .encoder()
                .and_then(|encoder| encoder.prepare(composited).ok())
            {
                worker.submit_frame(bytes);
            }
        }
    }

    pub(crate) fn poll_lifecycle(&mut self) -> Vec<DisplayNotice> {
        let events = self.drain_worker_events();
        self.join_finished();
        events
            .into_iter()
            .map(|event| self.notice_for(event))
            .collect()
    }

    fn join_finished(&mut self) {
        match std::mem::replace(&mut self.state, DisplayState::Idle) {
            DisplayState::Running(mut worker) => {
                if worker.join_if_finished() {
                    self.state = DisplayState::Idle;
                } else {
                    self.state = DisplayState::Running(worker);
                }
            }
            DisplayState::Stopping {
                mut worker,
                queued_start,
            } => {
                if worker.join_if_finished() {
                    if let Some(worker_start) = queued_start {
                        self.start(worker_start);
                    } else {
                        self.state = DisplayState::Idle;
                    }
                } else {
                    self.state = DisplayState::Stopping {
                        worker,
                        queued_start,
                    };
                }
            }
            state => {
                self.state = state;
            }
        }
    }

    pub(crate) fn needs_lifecycle_poll(&self) -> bool {
        matches!(
            self.state,
            DisplayState::Running(_) | DisplayState::Stopping { .. }
        )
    }

    fn start(&mut self, worker_start: WorkerStart) {
        let (driver, frame) = match worker_start {
            WorkerStart::Detected { driver, frame } => {
                self.status = DisplayStatus::from_driver(&driver);
                (driver, frame)
            }
            WorkerStart::Reopen {
                info,
                capability,
                frame,
            } => {
                let Some(driver) = self.detect_matching_driver(&info, capability) else {
                    self.state = DisplayState::Idle;
                    return;
                };
                (driver, frame)
            }
        };

        self.state = DisplayState::Running(Box::new(DisplayWorker::start(driver, frame)));
    }

    fn refresh_status(&mut self) {
        self.status = coolcooler_driver::detect_device()
            .as_ref()
            .map(DisplayStatus::from_driver)
            .unwrap_or_else(DisplayStatus::disconnected);
    }

    fn detect_worker_start<F>(&mut self, render: F) -> Option<WorkerStart>
    where
        F: FnOnce(Resolution) -> RgbaImage,
    {
        let Some(driver) = coolcooler_driver::detect_device() else {
            self.status = DisplayStatus::disconnected();
            return None;
        };
        self.status = DisplayStatus::from_driver(&driver);
        let composited = render(driver.info().resolution);
        let frame = DisplayFrameEncoder::from_driver(&driver)
            .prepare(&composited)
            .ok()?;
        Some(WorkerStart::Detected { driver, frame })
    }

    fn prepare_frame_from_status<F>(&self, render: F) -> Option<DisplayFrame>
    where
        F: FnOnce(Resolution) -> RgbaImage,
    {
        let (info, capability) = match &self.status {
            DisplayStatus::Connected { info, capability } => (info, *capability),
            DisplayStatus::Disconnected => return None,
        };
        DisplayFrameEncoder::new(info.clone(), capability)
            .prepare(&render(info.resolution))
            .ok()
    }

    fn prepare_reopen_from_status<F>(&self, render: F) -> Option<WorkerStart>
    where
        F: FnOnce(Resolution) -> RgbaImage,
    {
        let (info, capability) = match &self.status {
            DisplayStatus::Connected { info, capability } => (info.clone(), *capability),
            DisplayStatus::Disconnected => return None,
        };
        let frame = DisplayFrameEncoder::new(info.clone(), capability)
            .prepare(&render(info.resolution))
            .ok()?;
        Some(WorkerStart::Reopen {
            info,
            capability,
            frame,
        })
    }

    fn detect_matching_driver(
        &mut self,
        expected_info: &DeviceInfo,
        expected_capability: DisplayCapability,
    ) -> Option<DisplayDriver> {
        let Some(driver) = coolcooler_driver::detect_device() else {
            self.status = DisplayStatus::disconnected();
            return None;
        };
        self.status = DisplayStatus::from_driver(&driver);
        if driver.capability() == expected_capability
            && driver.info().name == expected_info.name
            && driver.info().resolution == expected_info.resolution
            && driver.info().rotation == expected_info.rotation
        {
            Some(driver)
        } else {
            None
        }
    }

    fn drain_worker_events(&mut self) -> Vec<DisplayLoopEvent> {
        match &mut self.state {
            DisplayState::Running(worker) => worker.drain_events(),
            DisplayState::Stopping { worker, .. } => worker.drain_events(),
            DisplayState::Idle => Vec::new(),
        }
    }

    fn notice_for(&self, event: DisplayLoopEvent) -> DisplayNotice {
        match event {
            DisplayLoopEvent::Started => {
                let name = self
                    .device_info()
                    .map(|info| info.name.clone())
                    .unwrap_or_else(|| "display".to_string());
                DisplayNotice::Started(name)
            }
            DisplayLoopEvent::Reconnecting(message) => DisplayNotice::Reconnecting(message),
            DisplayLoopEvent::TransferFailed(message) => DisplayNotice::TransferFailed(message),
            DisplayLoopEvent::Failed(message) => DisplayNotice::Failed(message),
            DisplayLoopEvent::Stopped => DisplayNotice::Stopped,
        }
    }
}

pub(crate) struct DisplayWorker {
    stop: Arc<AtomicBool>,
    shared_frame: Arc<Mutex<DisplayFrame>>,
    event_rx: mpsc::Receiver<DisplayLoopEvent>,
    join: Option<thread::JoinHandle<()>>,
}

impl DisplayWorker {
    pub(crate) fn start(driver: DisplayDriver, initial_frame: DisplayFrame) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let shared_frame = Arc::new(Mutex::new(initial_frame));
        let (event_tx, event_rx) = mpsc::channel();
        let thread_frame = Arc::clone(&shared_frame);
        let thread_stop = Arc::clone(&stop);
        let join = thread::spawn(move || {
            coolcooler_driver::run_display_with_events(
                driver,
                thread_frame,
                &thread_stop,
                |event| {
                    let _ = event_tx.send(event);
                },
            );
        });

        Self {
            stop,
            shared_frame,
            event_rx,
            join: Some(join),
        }
    }

    fn submit_frame(&self, bytes: DisplayFrame) {
        if let Ok(mut frame) = self.shared_frame.lock() {
            *frame = bytes;
        }
    }

    fn request_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    fn is_finished(&self) -> bool {
        self.join.as_ref().is_none_or(|join| join.is_finished())
    }

    fn join_if_finished(&mut self) -> bool {
        if !self.is_finished() {
            return false;
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
        true
    }

    fn drain_events(&mut self) -> Vec<DisplayLoopEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        events
    }

    fn join_in_background(&mut self) {
        if let Some(join) = self.join.take() {
            let _ = thread::spawn(move || {
                let _ = join.join();
            });
        }
    }
}

impl DisplayWorkerHandle for DisplayWorker {
    fn submit_frame(&self, bytes: DisplayFrame) {
        Self::submit_frame(self, bytes);
    }

    fn request_stop(&self) {
        Self::request_stop(self);
    }

    fn drain_events(&mut self) -> Vec<DisplayLoopEvent> {
        Self::drain_events(self)
    }

    fn join_if_finished(&mut self) -> bool {
        Self::join_if_finished(self)
    }
}

impl Drop for DisplayWorker {
    fn drop(&mut self) {
        self.request_stop();
        self.join_in_background();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coolcooler_core::Rotation;
    use image::{Rgba, RgbaImage};
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    struct FakeWorker {
        finished: bool,
        stop_requests: Arc<AtomicUsize>,
        joined: Arc<AtomicUsize>,
        submitted: Arc<Mutex<Vec<DisplayFrame>>>,
    }

    impl FakeWorker {
        fn new(finished: bool) -> Self {
            Self {
                finished,
                stop_requests: Arc::new(AtomicUsize::new(0)),
                joined: Arc::new(AtomicUsize::new(0)),
                submitted: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl DisplayWorkerHandle for FakeWorker {
        fn submit_frame(&self, frame: DisplayFrame) {
            self.submitted.lock().unwrap().push(frame);
        }

        fn request_stop(&self) {
            self.stop_requests.fetch_add(1, Ordering::Relaxed);
        }

        fn drain_events(&mut self) -> Vec<DisplayLoopEvent> {
            Vec::new()
        }

        fn join_if_finished(&mut self) -> bool {
            if self.finished {
                self.joined.fetch_add(1, Ordering::Relaxed);
                true
            } else {
                false
            }
        }
    }

    fn controller_with(state: DisplayState) -> DisplayController {
        DisplayController {
            state,
            status: DisplayStatus::disconnected(),
        }
    }

    fn fake_info() -> DeviceInfo {
        DeviceInfo {
            name: "Fake cooler".to_string(),
            resolution: Resolution::new(320, 240),
            rotation: Rotation::None,
            target_fps: 20.0,
            keepalive_interval: Duration::from_secs(1),
        }
    }

    fn connected_controller_with(state: DisplayState) -> DisplayController {
        DisplayController {
            state,
            status: DisplayStatus::Connected {
                info: fake_info(),
                capability: DisplayCapability::FileTransfer,
            },
        }
    }

    #[test]
    fn disconnected_controller_has_no_device_info() {
        let controller = controller_with(DisplayState::Idle);

        assert!(controller.device_info().is_none());
        assert_eq!(controller.capability(), None);
        assert_eq!(controller.resolution(), FALLBACK_PREVIEW_RESOLUTION);
    }

    #[test]
    fn finished_running_worker_is_reaped() {
        let worker = FakeWorker::new(true);
        let joined = Arc::clone(&worker.joined);
        let mut controller = controller_with(DisplayState::Running(Box::new(worker)));

        controller.join_finished();

        assert!(matches!(controller.state, DisplayState::Idle));
        assert_eq!(joined.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn unfinished_running_worker_stays_running() {
        let worker = FakeWorker::new(false);
        let joined = Arc::clone(&worker.joined);
        let mut controller = controller_with(DisplayState::Running(Box::new(worker)));

        controller.join_finished();

        assert!(matches!(controller.state, DisplayState::Running(_)));
        assert_eq!(joined.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn present_while_running_submits_frame_without_stopping_worker() {
        let worker = FakeWorker::new(false);
        let stop_requests = Arc::clone(&worker.stop_requests);
        let submitted = Arc::clone(&worker.submitted);
        let mut controller = connected_controller_with(DisplayState::Running(Box::new(worker)));

        controller.present_with(|resolution| {
            assert_eq!(resolution, Resolution::new(320, 240));
            RgbaImage::from_pixel(resolution.width, resolution.height, Rgba([1, 2, 3, 255]))
        });

        assert!(matches!(controller.state, DisplayState::Running(_)));
        assert_eq!(stop_requests.load(Ordering::Relaxed), 0);
        let frames = submitted.lock().unwrap();
        assert_eq!(frames.len(), 1);
        assert!(matches!(&frames[0], DisplayFrame::FileTransferPng(_)));
    }

    #[test]
    fn present_while_stopping_queues_reopen_without_extra_stop_request() {
        let worker = FakeWorker::new(false);
        let stop_requests = Arc::clone(&worker.stop_requests);
        let mut controller = connected_controller_with(DisplayState::Stopping {
            worker: Box::new(worker),
            queued_start: None,
        });

        controller.present_with(|resolution| {
            assert_eq!(resolution, Resolution::new(320, 240));
            RgbaImage::from_pixel(resolution.width, resolution.height, Rgba([4, 5, 6, 255]))
        });

        assert_eq!(stop_requests.load(Ordering::Relaxed), 0);
        let DisplayState::Stopping {
            queued_start: Some(WorkerStart::Reopen { frame, .. }),
            ..
        } = &controller.state
        else {
            panic!("stopping worker should keep a queued start");
        };
        assert!(matches!(frame, DisplayFrame::FileTransferPng(_)));
    }

    #[test]
    fn stopping_worker_without_queued_start_goes_idle_after_join() {
        let worker = FakeWorker::new(true);
        let joined = Arc::clone(&worker.joined);
        let mut controller = controller_with(DisplayState::Stopping {
            worker: Box::new(worker),
            queued_start: None,
        });

        controller.join_finished();

        assert!(matches!(controller.state, DisplayState::Idle));
        assert_eq!(joined.load(Ordering::Relaxed), 1);
    }
}
