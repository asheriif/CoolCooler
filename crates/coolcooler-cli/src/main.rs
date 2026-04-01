use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use clap::{Parser, Subcommand};
use coolcooler_core::frame::{self, DEFAULT_JPEG_QUALITY};
use coolcooler_core::CoolerLcd;
use coolcooler_idcooling::Fx360;
use image::{DynamicImage, Rgb, RgbImage};

#[derive(Parser)]
#[command(name = "coolcooler", about = "ID-Cooling FX360 LCD test CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Test USB connection (send a keepalive)
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

fn display_loop(lcd: &mut Fx360, jpeg: &[u8], running: &AtomicBool) {
    let info = lcd.info().clone();
    let frame_interval = std::time::Duration::from_secs_f64(1.0 / info.target_fps);
    let mut last_keepalive = Instant::now();
    let mut frames = 0u64;
    let start = Instant::now();

    while running.load(Ordering::Relaxed) {
        if let Err(e) = lcd.send_frame(jpeg) {
            eprintln!("send error: {e}");
            break;
        }
        frames += 1;

        if last_keepalive.elapsed() >= info.keepalive_interval {
            if let Err(e) = lcd.send_keepalive() {
                eprintln!("keepalive error: {e}");
                break;
            }
            last_keepalive = Instant::now();
        }

        std::thread::sleep(frame_interval);
    }

    let elapsed = start.elapsed().as_secs_f64();
    if elapsed > 0.0 {
        eprintln!("{frames} frames in {elapsed:.1}s ({:.1} FPS)", frames as f64 / elapsed);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let mut lcd = Fx360::new();
    println!("Connecting to {}...", lcd.info().name);
    lcd.connect()?;
    println!("Connected.");

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        eprintln!("\nStopping...");
        r.store(false, Ordering::Relaxed);
    })?;

    match cli.command {
        Command::Test => {
            lcd.send_keepalive()?;
            println!("Keepalive sent. Device is responding.");
        }
        Command::Image { path, quality } => {
            let img = image::open(&path)?;
            let jpeg = frame::prepare(&img, lcd.info(), quality)?;
            println!("Displaying {path} ({} bytes JPEG, Ctrl+C to stop)", jpeg.len());
            display_loop(&mut lcd, &jpeg, &running);
        }
        Command::Color { color } => {
            let rgb = parse_color(&color)?;
            let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(240, 240, rgb));
            let jpeg = frame::prepare(&img, lcd.info(), DEFAULT_JPEG_QUALITY)?;
            println!("Displaying color {color} (Ctrl+C to stop)");
            display_loop(&mut lcd, &jpeg, &running);
        }
    }

    lcd.disconnect();
    println!("Disconnected.");
    Ok(())
}
