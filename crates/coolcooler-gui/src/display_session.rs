use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use coolcooler_driver::DisplayDriver;

pub(crate) struct DisplayController {
    state: DisplayState,
}

enum DisplayState {
    Idle,
    Running(DisplaySession),
    Stopping {
        session: DisplaySession,
        pending_restart: Option<Vec<u8>>,
    },
}

impl DisplayController {
    pub(crate) fn new() -> Self {
        Self {
            state: DisplayState::Idle,
        }
    }

    pub(crate) fn restart(&mut self, initial_frame: Option<Vec<u8>>) {
        match std::mem::replace(&mut self.state, DisplayState::Idle) {
            DisplayState::Idle => self.start(initial_frame),
            DisplayState::Running(session) => {
                session.request_stop();
                self.state = DisplayState::Stopping {
                    session,
                    pending_restart: initial_frame,
                };
            }
            DisplayState::Stopping { session, .. } => {
                self.state = DisplayState::Stopping {
                    session,
                    pending_restart: initial_frame,
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

    pub(crate) fn submit_frame(&self, bytes: Vec<u8>) {
        if let DisplayState::Running(session) = &self.state {
            session.submit_frame(bytes);
        }
    }

    pub(crate) fn join_finished(&mut self) {
        match std::mem::replace(&mut self.state, DisplayState::Idle) {
            DisplayState::Stopping {
                mut session,
                pending_restart,
            } => {
                if session.join_if_finished() {
                    self.start(pending_restart);
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

    fn start(&mut self, initial_frame: Option<Vec<u8>>) {
        if let Some((driver, frame)) = initial_frame
            .and_then(|frame| coolcooler_driver::detect_device().map(|driver| (driver, frame)))
        {
            self.state = DisplayState::Running(DisplaySession::start(driver, frame));
        } else {
            self.state = DisplayState::Idle;
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
