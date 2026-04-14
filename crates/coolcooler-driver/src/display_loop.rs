use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use coolcooler_core::CoolerLcd;

use crate::DisplayDriver;

/// Run the appropriate display loop for the given driver.
///
/// For native (streaming) devices, the shared buffer carries JPEG bytes.
/// For liquidctl (file-transfer) devices, the shared buffer carries PNG bytes.
///
/// This function blocks until `stop` is set to `true`.
pub fn run_display(mut driver: DisplayDriver, shared_frame: Arc<Mutex<Vec<u8>>>, stop: &AtomicBool) {
    match &mut driver {
        DisplayDriver::Native(lcd) => streaming_loop(lcd, shared_frame, stop),
        DisplayDriver::Liquidctl(lc) => file_transfer_loop(lc, shared_frame, stop),
    }
}

/// How long to wait between reconnection attempts after a USB error.
const RECONNECT_DELAY: Duration = Duration::from_secs(2);

/// High-FPS streaming loop for native devices (e.g. FX360 at 20 FPS).
///
/// On USB errors (e.g. after system suspend/resume), the loop disconnects,
/// waits, and attempts to reconnect rather than silently dying.
fn streaming_loop(
    lcd: &mut impl CoolerLcd,
    shared_frame: Arc<Mutex<Vec<u8>>>,
    stop: &AtomicBool,
) {
    if lcd.connect().is_err() {
        return;
    }

    let device_interval = Duration::from_secs_f64(1.0 / lcd.info().target_fps);
    let keepalive_interval = lcd.info().keepalive_interval;
    let mut last_keepalive = Instant::now();
    let mut current_jpeg = Vec::new();

    while !stop.load(Ordering::Relaxed) {
        if let Ok(frame) = shared_frame.lock() {
            if !frame.is_empty() && *frame != current_jpeg {
                current_jpeg = frame.clone();
            }
        }

        if !current_jpeg.is_empty() && lcd.send_frame(&current_jpeg).is_err() {
            if !reconnect(lcd, stop) {
                return;
            }
            last_keepalive = Instant::now();
            continue;
        }

        if last_keepalive.elapsed() >= keepalive_interval {
            if lcd.send_keepalive().is_err() {
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
fn file_transfer_loop(
    lc: &mut coolcooler_liquidctl::LiquidctlDriver,
    shared_frame: Arc<Mutex<Vec<u8>>>,
    stop: &AtomicBool,
) {
    let poll_interval = Duration::from_millis(200);
    let mut last_sent: Vec<u8> = Vec::new();
    let temp_path = lc.temp_file_path().to_path_buf();

    while !stop.load(Ordering::Relaxed) {
        let current = {
            match shared_frame.lock() {
                Ok(frame) => {
                    if frame.is_empty() || *frame == last_sent {
                        None
                    } else {
                        Some(frame.clone())
                    }
                }
                Err(_) => None,
            }
        };

        if let Some(png_data) = current {
            if std::fs::write(&temp_path, &png_data).is_ok() {
                if lc.send_image(&temp_path).is_err() {
                    // Log but continue — transient liquidctl errors shouldn't kill the loop
                }
                last_sent = png_data;
            }
        }

        std::thread::sleep(poll_interval);
    }
}
