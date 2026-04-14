use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use clap::{Parser, Subcommand};
use coolcooler_core::frame::{self, DEFAULT_JPEG_QUALITY};
use coolcooler_driver::{DisplayCapability, DisplayDriver};
use image::{DynamicImage, Rgb, RgbImage};

#[derive(Parser)]
#[command(name = "coolcooler", about = "AIO cooler LCD test CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Test connection (send a keepalive or verify liquidctl)
    Test,
    /// Display a static image
    Image {
        /// Path to image file
        path: String,
        /// JPEG quality (0-100)
        #[arg(long, default_value_t = DEFAULT_JPEG_QUALITY)]
        quality: u8,
    },
    /// Display a solid color
    Color {
        /// Color: name ("red"), hex ("#FF6600"), or R,G,B ("255,128,0")
        color: String,
    },
}

fn parse_color(s: &str) -> Result<Rgb<u8>, String> {
    let s = s.trim();

    // R,G,B
    if s.contains(',') {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != 3 {
            return Err(format!("expected R,G,B, got {s}"));
        }
        let r = parts[0].trim().parse::<u8>().map_err(|e| e.to_string())?;
        let g = parts[1].trim().parse::<u8>().map_err(|e| e.to_string())?;
        let b = parts[2].trim().parse::<u8>().map_err(|e| e.to_string())?;
        return Ok(Rgb([r, g, b]));
    }

    // #RRGGBB
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() != 6 {
            return Err(format!("expected #RRGGBB, got {s}"));
        }
        let r = u8::from_str_radix(&hex[0..2], 16).map_err(|e| e.to_string())?;
        let g = u8::from_str_radix(&hex[2..4], 16).map_err(|e| e.to_string())?;
        let b = u8::from_str_radix(&hex[4..6], 16).map_err(|e| e.to_string())?;
        return Ok(Rgb([r, g, b]));
    }

    // Named colors
    match s.to_lowercase().as_str() {
        "red" => Ok(Rgb([255, 0, 0])),
        "green" => Ok(Rgb([0, 255, 0])),
        "blue" => Ok(Rgb([0, 0, 255])),
        "white" => Ok(Rgb([255, 255, 255])),
        "black" => Ok(Rgb([0, 0, 0])),
        "yellow" => Ok(Rgb([255, 255, 0])),
        "cyan" => Ok(Rgb([0, 255, 255])),
        "magenta" => Ok(Rgb([255, 0, 255])),
        "orange" => Ok(Rgb([255, 165, 0])),
        "purple" => Ok(Rgb([128, 0, 128])),
        _ => Err(format!("unknown color: {s}")),
    }
}

/// Streaming display loop for native devices. Sends the same JPEG at target FPS.
/// Takes ownership of the driver (it's moved into the display thread).
/// Blocks until `running` is set to false (Ctrl+C).
fn streaming_display_loop(
    driver: DisplayDriver,
    jpeg: Vec<u8>,
    running: &AtomicBool,
) {
    let shared = Arc::new(Mutex::new(jpeg));
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();

    std::thread::spawn(move || {
        coolcooler_driver::run_display(driver, shared, &stop_clone);
    });

    while running.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    stop.store(true, Ordering::Relaxed);
}

/// Single-shot send for file-transfer (liquidctl) devices.
fn file_transfer_send(lc: &coolcooler_liquidctl::LiquidctlDriver, img: &DynamicImage) -> Result<(), Box<dyn std::error::Error>> {
    let temp_path = lc.temp_file_path().to_path_buf();
    img.save(&temp_path)?;
    lc.send_image(&temp_path)?;
    println!("Image sent to device.");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let mut driver = coolcooler_driver::detect_device()
        .ok_or("No supported cooler detected")?;

    println!("Detected: {}", driver.info().name);
    driver.connect()?;
    println!("Connected.");

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        eprintln!("\nStopping...");
        r.store(false, Ordering::Relaxed);
    })?;

    let capability = driver.capability();

    match cli.command {
        Command::Test => {
            println!("Device is responding ({}).", match capability {
                DisplayCapability::Streaming => "native streaming",
                DisplayCapability::FileTransfer => "liquidctl file-transfer",
            });
            driver.disconnect();
        }
        Command::Image { path, quality } => {
            let img = image::open(&path)?;
            match capability {
                DisplayCapability::Streaming => {
                    let jpeg = frame::prepare(&img, driver.info(), quality)?;
                    println!("Displaying {path} ({} bytes JPEG, Ctrl+C to stop)", jpeg.len());
                    streaming_display_loop(driver, jpeg, &running);
                    // driver moved into thread — disconnect happens on thread exit
                }
                DisplayCapability::FileTransfer => {
                    if let DisplayDriver::Liquidctl(ref lc) = driver {
                        file_transfer_send(lc, &img)?;
                    }
                    driver.disconnect();
                }
            }
        }
        Command::Color { color } => {
            let rgb = parse_color(&color)?;
            let res = driver.info().resolution;
            let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(res.width, res.height, rgb));
            match capability {
                DisplayCapability::Streaming => {
                    let jpeg = frame::prepare(&img, driver.info(), DEFAULT_JPEG_QUALITY)?;
                    println!("Displaying color {color} (Ctrl+C to stop)");
                    streaming_display_loop(driver, jpeg, &running);
                }
                DisplayCapability::FileTransfer => {
                    if let DisplayDriver::Liquidctl(ref lc) = driver {
                        file_transfer_send(lc, &img)?;
                    }
                    driver.disconnect();
                }
            }
        }
    }

    println!("Done.");
    Ok(())
}
