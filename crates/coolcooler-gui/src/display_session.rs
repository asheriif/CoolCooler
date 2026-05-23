use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use coolcooler_core::frame::{self, DEFAULT_JPEG_QUALITY};
use coolcooler_core::DeviceInfo;
use coolcooler_driver::{DisplayCapability, DisplayDriver};
use image::{DynamicImage, RgbaImage};

pub(crate) struct DisplayController {
    state: DisplayState,
    status: DisplayStatus,
}

enum DisplayState {
    Idle,
    Running(DisplaySession),
    Stopping {
        session: DisplaySession,
        pending_restart: Option<PendingStart>,
    },
}

struct PendingStart {
    driver: DisplayDriver,
    frame: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct DisplayStatus {
    connected: bool,
    info: DeviceInfo,
    capability: DisplayCapability,
}

impl DisplayStatus {
    fn disconnected() -> Self {
        Self {
            connected: false,
            info: DeviceInfo::default(),
            capability: DisplayCapability::Streaming,
        }
    }

    fn from_driver(driver: &DisplayDriver) -> Self {
        Self {
            connected: true,
            info: driver.info().clone(),
            capability: driver.capability(),
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

    pub(crate) fn info(&self) -> &DeviceInfo {
        &self.status.info
    }

    pub(crate) fn capability(&self) -> DisplayCapability {
        self.status.capability
    }

    pub(crate) fn is_connected(&self) -> bool {
        self.status.connected
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
            if let Some(bytes) = encode_frame(composited, &self.status.info, self.status.capability)
            {
                session.submit_frame(bytes);
            }
        }
    }

    pub(crate) fn join_finished(&mut self) {
        match std::mem::replace(&mut self.state, DisplayState::Idle) {
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

    pub(crate) fn is_stopping(&self) -> bool {
        matches!(self.state, DisplayState::Stopping { .. })
    }

    fn start(&mut self, pending_start: PendingStart) {
        self.state = DisplayState::Running(DisplaySession::start(
            pending_start.driver,
            pending_start.frame,
        ));
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
        let frame = encode_frame(composited, &self.status.info, self.status.capability)?;
        Some(PendingStart { driver, frame })
    }
}

fn encode_frame(
    composited: &RgbaImage,
    info: &DeviceInfo,
    capability: DisplayCapability,
) -> Option<Vec<u8>> {
    match capability {
        DisplayCapability::Streaming => {
            let rgb = DynamicImage::ImageRgba8(composited.clone()).to_rgb8();
            frame::encode_resized(&rgb, info.rotation, DEFAULT_JPEG_QUALITY).ok()
        }
        DisplayCapability::FileTransfer => {
            let mut buf = std::io::Cursor::new(Vec::new());
            DynamicImage::ImageRgba8(composited.clone())
                .write_to(&mut buf, image::ImageFormat::Png)
                .ok()
                .map(|()| buf.into_inner())
        }
    }
}

pub(crate) struct DisplaySession {
    stop: Arc<AtomicBool>,
    shared_frame: Arc<Mutex<Vec<u8>>>,
    join: Option<thread::JoinHandle<()>>,
}

impl DisplaySession {
    pub(crate) fn start(driver: DisplayDriver, initial_frame: Vec<u8>) -> Self {
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

    pub(crate) fn submit_frame(&self, bytes: Vec<u8>) {
        if let Ok(mut frame) = self.shared_frame.lock() {
            *frame = bytes;
        }
    }

    pub(crate) fn request_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.join.as_ref().is_none_or(|join| join.is_finished())
    }

    pub(crate) fn join_if_finished(&mut self) -> bool {
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

impl Drop for DisplaySession {
    fn drop(&mut self) {
        self.request_stop();
        self.join_in_background();
    }
}
