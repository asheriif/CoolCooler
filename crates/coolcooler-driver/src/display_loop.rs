use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use coolcooler_core::CoolerLcd;

use crate::{DisplayDriver, DisplayFrame};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayLoopEvent {
    Started,
    Reconnecting(String),
    TransferFailed(String),
    Failed(String),
    Stopped,
}

/// Run the appropriate display loop for the given driver.
///
/// This function blocks until `stop` is set to `true`.
pub fn run_display(
    driver: DisplayDriver,
    shared_frame: Arc<Mutex<DisplayFrame>>,
    stop: &AtomicBool,
) {
    run_display_with_events(driver, shared_frame, stop, |_| {});
}

pub fn run_display_with_events<F>(
    mut driver: DisplayDriver,
    shared_frame: Arc<Mutex<DisplayFrame>>,
    stop: &AtomicBool,
    mut emit: F,
) where
    F: FnMut(DisplayLoopEvent),
{
    match &mut driver {
        DisplayDriver::Native(lcd) => streaming_loop(lcd, shared_frame, stop, &mut emit),
        DisplayDriver::Liquidctl(lc) => file_transfer_loop(lc, shared_frame, stop, &mut emit),
    }
    emit(DisplayLoopEvent::Stopped);
}

/// How long to wait between reconnection attempts after a USB error.
const RECONNECT_DELAY: Duration = Duration::from_secs(2);

/// High-FPS streaming loop for native devices (e.g. FX360 at 20 FPS).
///
/// On USB errors (e.g. after system suspend/resume), the loop disconnects,
/// waits, and attempts to reconnect rather than silently dying.
fn streaming_loop<F>(
    lcd: &mut impl CoolerLcd,
    shared_frame: Arc<Mutex<DisplayFrame>>,
    stop: &AtomicBool,
    emit: &mut F,
) where
    F: FnMut(DisplayLoopEvent),
{
    if let Err(e) = lcd.connect() {
        emit(DisplayLoopEvent::Failed(format!("connect failed: {e}")));
        return;
    }
    emit(DisplayLoopEvent::Started);

    let device_interval = Duration::from_secs_f64(1.0 / lcd.info().target_fps);
    let keepalive_interval = lcd.info().keepalive_interval;
    let mut last_keepalive = Instant::now();
    let mut current_jpeg = Vec::new();

    while !stop.load(Ordering::Relaxed) {
        if let Ok(frame) = shared_frame.lock() {
            if let DisplayFrame::StreamingJpeg(bytes) = &*frame {
                if !bytes.is_empty() && bytes.as_slice() != current_jpeg.as_slice() {
                    current_jpeg = bytes.clone();
                }
            }
        }

        if !current_jpeg.is_empty() {
            if let Err(e) = lcd.send_frame(&current_jpeg) {
                emit(DisplayLoopEvent::Reconnecting(format!(
                    "frame transfer failed: {e}"
                )));
                if !reconnect(lcd, stop) {
                    return;
                }
                last_keepalive = Instant::now();
                continue;
            }
        }

        if last_keepalive.elapsed() >= keepalive_interval {
            if let Err(e) = lcd.send_keepalive() {
                emit(DisplayLoopEvent::Reconnecting(format!(
                    "keepalive failed: {e}"
                )));
                if !reconnect(lcd, stop) {
                    return;
                }
                last_keepalive = Instant::now();
                continue;
            }
            last_keepalive = Instant::now();
        }

        std::thread::sleep(device_interval);
    }
}

/// Attempt to reconnect after a USB error. Returns `true` on success,
/// `false` if the stop signal was raised while waiting.
fn reconnect(lcd: &mut impl CoolerLcd, stop: &AtomicBool) -> bool {
    lcd.disconnect();
    loop {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(RECONNECT_DELAY);
        if lcd.connect().is_ok() {
            return true;
        }
    }
}

/// On-change file-transfer loop for liquidctl devices.
///
/// Polls the shared buffer at ~200ms intervals. When the PNG content changes,
/// writes it to a temp file and sends via liquidctl subprocess.
fn file_transfer_loop<F>(
    lc: &mut coolcooler_liquidctl::LiquidctlDriver,
    shared_frame: Arc<Mutex<DisplayFrame>>,
    stop: &AtomicBool,
    emit: &mut F,
) where
    F: FnMut(DisplayLoopEvent),
{
    let poll_interval = Duration::from_millis(200);
    let mut last_sent: Vec<u8> = Vec::new();
    let temp_path = lc.temp_file_path().to_path_buf();
    emit(DisplayLoopEvent::Started);

    while !stop.load(Ordering::Relaxed) {
        let current = {
            match shared_frame.lock() {
                Ok(frame) => match &*frame {
                    DisplayFrame::FileTransferPng(png_data)
                        if !png_data.is_empty() && png_data.as_slice() != last_sent.as_slice() =>
                    {
                        Some(png_data.clone())
                    }
                    _ => None,
                },
                Err(_) => None,
            }
        };

        if let Some(png_data) = current {
            match std::fs::write(&temp_path, &png_data) {
                Ok(()) => {
                    if let Err(e) = lc.send_image(&temp_path) {
                        emit(DisplayLoopEvent::TransferFailed(format!(
                            "liquidctl transfer failed: {e}"
                        )));
                    }
                    last_sent = png_data;
                }
                Err(e) => {
                    emit(DisplayLoopEvent::TransferFailed(format!(
                        "failed to write liquidctl frame: {e}"
                    )));
                }
            }
        }

        std::thread::sleep(poll_interval);
    }
}
