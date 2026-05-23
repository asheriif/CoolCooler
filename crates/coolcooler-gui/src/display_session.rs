use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use coolcooler_driver::DisplayDriver;

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

    pub(crate) fn stop(mut self) {
        self.request_stop();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }

    fn request_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for DisplaySession {
    fn drop(&mut self) {
        self.request_stop();
        if self.join.as_ref().is_some_and(|join| join.is_finished()) {
            if let Some(join) = self.join.take() {
                let _ = join.join();
            }
        }
    }
}
