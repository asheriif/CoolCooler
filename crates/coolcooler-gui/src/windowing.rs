use std::fs::File;
use std::path::PathBuf;

use iced::{window, Size};

pub(crate) fn ensure_single_instance() {
    let dir = std::env::var("XDG_RUNTIME_DIR")
        .or_else(|_| std::env::var("TEMP"))
        .unwrap_or_else(|_| "/tmp".to_string());
    let lock = File::create(format!("{dir}/coolcooler.lock")).expect("failed to create lock file");

    if lock.try_lock().is_err() {
        eprintln!("CoolCooler is already running.");
        std::process::exit(0);
    }

    // Leak the handle so the lock persists until process exit.
    // The OS releases it automatically on crash/exit.
    std::mem::forget(lock);
}

pub(crate) fn app_window_settings() -> window::Settings {
    let icon = window::icon::from_file_data(
        include_bytes!("../../../assets/icon.png"),
        Some(image::ImageFormat::Png),
    )
    .ok();

    window::Settings {
        size: Size::new(820.0, 1000.0),
        icon,
        platform_specific: window::settings::PlatformSpecific {
            application_id: "coolcooler".to_string(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub(crate) async fn pick_file() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp"])
        .pick_file()
        .await
        .map(|h| h.path().to_path_buf())
}
