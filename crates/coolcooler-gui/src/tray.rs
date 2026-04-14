use std::sync::mpsc;

const APP_ICON_NAME: &str = "coolcooler";

/// Messages sent from the tray icon to the iced app.
pub enum TrayEvent {
    ShowWindow,
}

/// Handle to the running tray icon. Drop it to shut down.
pub struct TrayHandle {
    _inner: PlatformHandle,
}

/// Spawn the tray icon and return a handle + receiver for events.
pub fn spawn() -> (TrayHandle, mpsc::Receiver<TrayEvent>) {
    let (tx, rx) = mpsc::channel();
    let handle = platform_spawn(tx);
    (TrayHandle { _inner: handle }, rx)
}

// -- Linux: ksni (D-Bus StatusNotifierItem) --

#[cfg(target_os = "linux")]
type PlatformHandle = ksni::blocking::Handle<CoolCoolerTray>;

#[cfg(target_os = "linux")]
struct CoolCoolerTray {
    tx: mpsc::Sender<TrayEvent>,
}

#[cfg(target_os = "linux")]
impl ksni::Tray for CoolCoolerTray {
    fn id(&self) -> String {
        "coolcooler".to_string()
    }

    fn title(&self) -> String {
        "CoolCooler".to_string()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::Hardware
    }

    fn icon_name(&self) -> String {
        APP_ICON_NAME.to_string()
    }

    fn icon_theme_path(&self) -> String {
        appimage_icon_theme_path().unwrap_or_default()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![load_icon()]
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            icon_name: APP_ICON_NAME.to_string(),
            title: "CoolCooler".to_string(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.send(TrayEvent::ShowWindow);
    }
}

#[cfg(target_os = "linux")]
fn appimage_icon_theme_path() -> Option<String> {
    let appdir = std::env::var_os("APPDIR")?;
    let path = std::path::PathBuf::from(appdir).join("usr/share/icons");
    Some(path.to_string_lossy().into_owned())
}

#[cfg(target_os = "linux")]
fn load_icon() -> ksni::Icon {
    let png_bytes = include_bytes!("../../../assets/tray_icon.png");
    let img = image::load_from_memory(png_bytes).expect("invalid tray icon PNG");
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width() as i32, rgba.height() as i32);
    // Convert RGBA to ARGB (network byte order)
    let mut argb = Vec::with_capacity((w * h * 4) as usize);
    for pixel in rgba.pixels() {
        argb.push(pixel[3]); // A
        argb.push(pixel[0]); // R
        argb.push(pixel[1]); // G
        argb.push(pixel[2]); // B
    }
    ksni::Icon {
        width: w,
        height: h,
        data: argb,
    }
}

#[cfg(target_os = "linux")]
fn platform_spawn(tx: mpsc::Sender<TrayEvent>) -> PlatformHandle {
    use ksni::blocking::TrayMethods;

    CoolCoolerTray { tx }
        .spawn()
        .expect("failed to create tray icon")
}

// -- Windows: tray-icon --

#[cfg(target_os = "windows")]
type PlatformHandle = WindowsTrayHandle;

#[cfg(target_os = "windows")]
pub struct WindowsTrayHandle {
    _join: std::thread::JoinHandle<()>,
}

#[cfg(target_os = "windows")]
fn platform_spawn(tx: mpsc::Sender<TrayEvent>) -> WindowsTrayHandle {
    use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let join = std::thread::spawn(move || {
        let _tray = TrayIconBuilder::new()
            .with_tooltip("CoolCooler")
            .build()
            .expect("failed to create tray icon");

        loop {
            if let Ok(event) = TrayIconEvent::receiver().recv() {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    let _ = tx.send(TrayEvent::ShowWindow);
                }
            }
        }
    });

    WindowsTrayHandle { _join: join }
}
