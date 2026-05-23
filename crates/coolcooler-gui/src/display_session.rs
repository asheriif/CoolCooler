use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use coolcooler_core::{DeviceInfo, Resolution};
use coolcooler_driver::{DisplayCapability, DisplayDriver, DisplayFrame, DisplayFrameEncoder};
use image::RgbaImage;

pub(crate) struct DisplayController {
    state: DisplayState,
    status: DisplayStatus,
}

const FALLBACK_PREVIEW_RESOLUTION: Resolution = Resolution::new(240, 240);

enum DisplayState {
    Idle,
    Running(SessionHandle),
    Stopping {
        session: SessionHandle,
        pending_restart: Option<PendingStart>,
    },
}

type SessionHandle = Box<dyn DisplaySessionHandle>;

trait DisplaySessionHandle: Send {
    fn submit_frame(&self, frame: DisplayFrame);
    fn request_stop(&self);
    fn join_if_finished(&mut self) -> bool;
}

struct PendingStart {
    driver: DisplayDriver,
    frame: DisplayFrame,
}

#[derive(Debug, Clone)]
pub(crate) enum DisplayStatus {
    Disconnected,
    Connected {
        info: DeviceInfo,
        capability: DisplayCapability,
    },
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

    pub(crate) fn restart(&mut self, composited: &RgbaImage) {
        let Some(pending_start) = self.detect_pending_start(composited) else {
            self.stop();
            return;
        };
        self.restart_pending(pending_start);
    }

    fn restart_pending(&mut self, pending_start: PendingStart) {
        match std::mem::replace(&mut self.state, DisplayState::Idle) {
            DisplayState::Idle => self.start(pending_start),
            DisplayState::Running(session) => {
                session.request_stop();
                self.state = DisplayState::Stopping {
                    session,
                    pending_restart: Some(pending_start),
                };
            }
            DisplayState::Stopping { session, .. } => {
                self.state = DisplayState::Stopping {
                    session,
                    pending_restart: Some(pending_start),
                };
            }
        }
    }

    pub(crate) fn stop(&mut self) {
        match std::mem::replace(&mut self.state, DisplayState::Idle) {
            DisplayState::Idle => {
                self.state = DisplayState::Idle;
            }
            DisplayState::Running(session) => {
                session.request_stop();
                self.state = DisplayState::Stopping {
                    session,
                    pending_restart: None,
                };
            }
            DisplayState::Stopping { session, .. } => {
                self.state = DisplayState::Stopping {
                    session,
                    pending_restart: None,
                };
            }
        }
    }

    pub(crate) fn submit_frame(&self, composited: &RgbaImage) {
        if let DisplayState::Running(session) = &self.state {
            if let Some(bytes) = self
                .status
                .encoder()
                .and_then(|encoder| encoder.prepare(composited).ok())
            {
                session.submit_frame(bytes);
            }
        }
    }

    pub(crate) fn join_finished(&mut self) {
        match std::mem::replace(&mut self.state, DisplayState::Idle) {
            DisplayState::Running(mut session) => {
                if session.join_if_finished() {
                    self.state = DisplayState::Idle;
                } else {
                    self.state = DisplayState::Running(session);
                }
            }
            DisplayState::Stopping {
                mut session,
                pending_restart,
            } => {
                if session.join_if_finished() {
                    if let Some(pending_start) = pending_restart {
                        self.start(pending_start);
                    } else {
                        self.state = DisplayState::Idle;
                    }
                } else {
                    self.state = DisplayState::Stopping {
                        session,
                        pending_restart,
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

    fn start(&mut self, pending_start: PendingStart) {
        self.state = DisplayState::Running(Box::new(DisplaySession::start(
            pending_start.driver,
            pending_start.frame,
        )));
    }

    fn refresh_status(&mut self) {
        self.status = coolcooler_driver::detect_device()
            .as_ref()
            .map(DisplayStatus::from_driver)
            .unwrap_or_else(DisplayStatus::disconnected);
    }

    fn detect_pending_start(&mut self, composited: &RgbaImage) -> Option<PendingStart> {
        let Some(driver) = coolcooler_driver::detect_device() else {
            self.status = DisplayStatus::disconnected();
            return None;
        };
        self.status = DisplayStatus::from_driver(&driver);
        let frame = DisplayFrameEncoder::from_driver(&driver)
            .prepare(composited)
            .ok()?;
        Some(PendingStart { driver, frame })
    }
}

pub(crate) struct DisplaySession {
    stop: Arc<AtomicBool>,
    shared_frame: Arc<Mutex<DisplayFrame>>,
    join: Option<thread::JoinHandle<()>>,
}

impl DisplaySession {
    pub(crate) fn start(driver: DisplayDriver, initial_frame: DisplayFrame) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let shared_frame = Arc::new(Mutex::new(initial_frame));
        let thread_frame = Arc::clone(&shared_frame);
        let thread_stop = Arc::clone(&stop);
        let join = thread::spawn(move || {
            coolcooler_driver::run_display(driver, thread_frame, &thread_stop);
        });

        Self {
            stop,
            shared_frame,
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

    fn join_in_background(&mut self) {
        if let Some(join) = self.join.take() {
            let _ = thread::spawn(move || {
                let _ = join.join();
            });
        }
    }
}

impl DisplaySessionHandle for DisplaySession {
    fn submit_frame(&self, bytes: DisplayFrame) {
        Self::submit_frame(self, bytes);
    }

    fn request_stop(&self) {
        Self::request_stop(self);
    }

    fn join_if_finished(&mut self) -> bool {
        Self::join_if_finished(self)
    }
}

impl Drop for DisplaySession {
    fn drop(&mut self) {
        self.request_stop();
        self.join_in_background();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    struct FakeSession {
        finished: bool,
        stop_requests: Arc<AtomicUsize>,
        joined: Arc<AtomicUsize>,
        submitted: Arc<Mutex<Vec<DisplayFrame>>>,
    }

    impl FakeSession {
        fn new(finished: bool) -> Self {
            Self {
                finished,
                stop_requests: Arc::new(AtomicUsize::new(0)),
                joined: Arc::new(AtomicUsize::new(0)),
                submitted: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl DisplaySessionHandle for FakeSession {
        fn submit_frame(&self, frame: DisplayFrame) {
            self.submitted.lock().unwrap().push(frame);
        }

        fn request_stop(&self) {
            self.stop_requests.fetch_add(1, Ordering::Relaxed);
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

    #[test]
    fn disconnected_controller_has_no_device_info() {
        let controller = controller_with(DisplayState::Idle);

        assert!(controller.device_info().is_none());
        assert_eq!(controller.capability(), None);
        assert_eq!(controller.resolution(), FALLBACK_PREVIEW_RESOLUTION);
    }

    #[test]
    fn finished_running_session_is_reaped() {
        let session = FakeSession::new(true);
        let joined = Arc::clone(&session.joined);
        let mut controller = controller_with(DisplayState::Running(Box::new(session)));

        controller.join_finished();

        assert!(matches!(controller.state, DisplayState::Idle));
        assert_eq!(joined.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn unfinished_running_session_stays_running() {
        let session = FakeSession::new(false);
        let joined = Arc::clone(&session.joined);
        let mut controller = controller_with(DisplayState::Running(Box::new(session)));

        controller.join_finished();

        assert!(matches!(controller.state, DisplayState::Running(_)));
        assert_eq!(joined.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn stopping_session_without_restart_goes_idle_after_join() {
        let session = FakeSession::new(true);
        let joined = Arc::clone(&session.joined);
        let mut controller = controller_with(DisplayState::Stopping {
            session: Box::new(session),
            pending_restart: None,
        });

        controller.join_finished();

        assert!(matches!(controller.state, DisplayState::Idle));
        assert_eq!(joined.load(Ordering::Relaxed), 1);
    }
}
