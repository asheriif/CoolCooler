mod canvas;
mod preset;
mod tray;
mod widget;

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use canvas::{Canvas, LayerSelection, Viewport};
use coolcooler_core::frame::{self, DEFAULT_JPEG_QUALITY};
use coolcooler_core::DeviceInfo;
use coolcooler_driver::{widgets_allowed, DisplayCapability};
use fast_image_resize as fir;
use iced::widget::{
    button, column, container, mouse_area, pick_list, row, scrollable, slider, text, text_input,
};
use iced::{
    mouse, window, Background, Border, Color, Element, Length, Point, Size, Subscription, Task,
    Theme,
};
use image::{imageops, AnimationDecoder, DynamicImage, Rgba, RgbaImage};
use widget::{sysinfo_backend::SysInfoBackend, LcdWidget, WidgetContext};

// -- Colors --

// -- Theme colors --

struct AppColors {
    bg: Color,
    card_bg: Color,
    accent: Color,
    text_dim: Color,
    green: Color,
    red: Color,
    surface: Color,
    text_primary: Color,
    danger_bg: Color,
    disabled_bg: Color,
}

const DARK: AppColors = AppColors {
    bg: Color::from_rgb(0.086, 0.086, 0.118),
    card_bg: Color::from_rgb(0.118, 0.118, 0.180),
    accent: Color::from_rgb(0.024, 0.714, 0.831),
    text_dim: Color::from_rgb(0.392, 0.392, 0.471),
    green: Color::from_rgb(0.133, 0.773, 0.369),
    red: Color::from_rgb(0.937, 0.267, 0.267),
    surface: Color::from_rgb(0.157, 0.157, 0.220),
    text_primary: Color::WHITE,
    danger_bg: Color::from_rgb(0.3, 0.08, 0.08),
    disabled_bg: Color::from_rgb(0.1, 0.1, 0.14),
};

const LIGHT: AppColors = AppColors {
    bg: Color::from_rgb(0.94, 0.94, 0.96),
    card_bg: Color::from_rgb(1.0, 1.0, 1.0),
    accent: Color::from_rgb(0.02, 0.55, 0.65),
    text_dim: Color::from_rgb(0.5, 0.5, 0.56),
    green: Color::from_rgb(0.1, 0.6, 0.3),
    red: Color::from_rgb(0.8, 0.2, 0.2),
    surface: Color::from_rgb(0.9, 0.9, 0.92),
    text_primary: Color::from_rgb(0.1, 0.1, 0.12),
    danger_bg: Color::from_rgb(1.0, 0.92, 0.92),
    disabled_bg: Color::from_rgb(0.88, 0.88, 0.90),
};

// -- Source data --

struct SourceFrame {
    rgba: RgbaImage,
    duration: Duration,
}

#[derive(Clone)]
struct LoadedData {
    frames: Arc<Vec<LoadedFrame>>,
    filename: String,
}

#[derive(Clone)]
struct LoadedFrame {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    duration: Duration,
}

impl std::fmt::Debug for LoadedData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedData")
            .field("frames", &self.frames.len())
            .field("filename", &self.filename)
            .finish()
    }
}

/// Layer option for the pick_list dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LayerOption {
    selection: LayerSelection,
    label: String,
}

impl std::fmt::Display for LayerOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

// -- App --

fn main() -> iced::Result {
    ensure_single_instance();

    iced::daemon(CoolCooler::boot, CoolCooler::update, CoolCooler::view)
        .subscription(CoolCooler::subscription)
        .theme(CoolCooler::theme)
        .title("CoolCooler")
        .antialiasing(true)
        .run()
}

fn ensure_single_instance() {
    use std::fs::File;

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

struct CoolCooler {
    dark_mode: bool,
    driver_info: DeviceInfo,
    driver_capability: DisplayCapability,
    driver_connected: bool,
    selected_path: Option<PathBuf>,

    // Source data
    source_frames: Vec<SourceFrame>,
    filename: String,
    loading: bool,

    // Animation
    current_frame: usize,
    last_advance: Instant,

    // Canvas (layers + viewports)
    canvas: Canvas,

    // Interaction
    dragging: bool,
    last_cursor: Option<Point>,

    // Widget catalog + backends
    widget_catalog: Vec<Box<dyn LcdWidget>>,
    selected_category: String,
    sysinfo_backend: SysInfoBackend,
    widget_ctx: WidgetContext,

    // Cached preview
    preview: Option<iced::widget::image::Handle>,

    // Presets
    current_preset_folder: Option<String>,
    current_preset_name: Option<String>,
    show_save_dialog: bool,
    show_load_dialog: bool,
    save_name_input: String,
    preset_list: Vec<preset::PresetEntry>,
    last_preset_click: Option<(String, Instant)>,

    status_message: String,
    display_active: bool,
    stop_signal: Arc<AtomicBool>,
    device_frame: Arc<Mutex<Vec<u8>>>,

    // Tray icon
    _tray_handle: tray::TrayHandle,
    tray_rx: Arc<Mutex<std::sync::mpsc::Receiver<tray::TrayEvent>>>,
    window_id: Option<window::Id>,
}

#[derive(Debug, Clone)]
enum Message {
    SelectFile,
    FileSelected(Option<PathBuf>),
    SourceLoaded(Result<LoadedData, String>),
    AnimationTick,
    WidgetTick,
    Scroll(mouse::ScrollDelta),
    DragStart,
    DragMove(Point),
    DragEnd,
    ResetView,
    SelectLayer(LayerOption),
    SelectCategory(String),
    AddWidget(usize),
    RemoveWidget(widget::WidgetId),
    SetWidgetOpacity(f32),
    SetWidgetTextColor([u8; 4]),
    SetWidgetFont(String),
    SetWidgetText(String),
    ShowSaveDialog,
    ShowLoadDialog,
    CloseSaveDialog,
    CloseLoadDialog,
    SaveNameChanged(String),
    SavePreset,
    SavePresetAs,
    PresetClicked(String),
    DeletePreset(String),
    PresetSourceLoaded(Result<LoadedData, String>, preset::PresetData),
    ToggleTheme,
    WindowClosed(window::Id),
    TrayPoll,
    ShowWindow,
    Quit,
}

impl CoolCooler {
    fn boot() -> (Self, Task<Message>) {
        let (connected, info, capability) = match coolcooler_driver::detect_device() {
            Some(driver) => {
                let info = driver.info().clone();
                let cap = driver.capability();
                (true, info, cap)
            }
            None => {
                // No device found — show UI in disconnected state with default info
                let info = DeviceInfo::default();
                (false, info, DisplayCapability::Streaming)
            }
        };
        let (tray_handle, tray_rx) = tray::spawn();

        let (id, open_task) = window::open(app_window_settings());

        let mut app = Self {
            dark_mode: true,
            driver_info: info,
            driver_capability: capability,
            driver_connected: connected,
            selected_path: None,
            source_frames: Vec::new(),
            filename: String::new(),
            loading: false,
            current_frame: 0,
            last_advance: Instant::now(),
            canvas: Canvas::new(),
            dragging: false,
            last_cursor: None,
            selected_category: "Static".to_string(),
            widget_catalog: widget::catalog(),
            sysinfo_backend: SysInfoBackend::new(),
            widget_ctx: WidgetContext::default(),
            preview: None,
            current_preset_folder: None,
            current_preset_name: None,
            show_save_dialog: false,
            show_load_dialog: false,
            save_name_input: String::new(),
            preset_list: Vec::new(),
            last_preset_click: None,
            status_message: String::new(),
            display_active: false,
            stop_signal: Arc::new(AtomicBool::new(false)),
            device_frame: Arc::new(Mutex::new(Vec::new())),
            _tray_handle: tray_handle,
            tray_rx: Arc::new(Mutex::new(tray_rx)),
            window_id: Some(id),
        };
        app.rebuild_preview();
        (app, open_task.discard())
    }

    fn has_source(&self) -> bool {
        !self.source_frames.is_empty()
    }

    fn is_animated(&self) -> bool {
        self.source_frames.len() > 1
    }

    fn colors(&self) -> &'static AppColors {
        if self.dark_mode {
            &DARK
        } else {
            &LIGHT
        }
    }

    fn lcd_size(&self) -> u32 {
        self.driver_info.resolution.width
    }

    /// Render the full composited 240x240 RGBA (base + widgets).
    fn render_composited(&self) -> RgbaImage {
        let lcd = self.lcd_size();
        let base = if let Some(src) = self.source_frames.get(self.current_frame) {
            let vp = &self.canvas.base_viewport;
            render_base_rgba(&src.rgba, &self.driver_info, vp.zoom, vp.pan)
        } else {
            RgbaImage::from_pixel(lcd, lcd, Rgba([0, 0, 0, 255]))
        };
        self.canvas.composite(base, &self.widget_ctx)
    }

    fn rebuild_preview(&mut self) {
        let composited = self.render_composited();
        self.preview = Some(circular_preview_from_rgba(composited));
        if self.display_active {
            self.push_device_frame();
        }
    }

    /// Build a PresetData from the current app state.
    fn build_preset_data(&self, name: &str) -> preset::PresetData {
        let bg = self.selected_path.as_ref().map(|p| {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("png");
            preset::BackgroundData {
                file: format!("background.{ext}"),
            }
        });

        let widgets = self
            .canvas
            .layers
            .iter()
            .map(|layer| preset::WidgetLayerData {
                type_id: layer.widget.type_id().to_string(),
                position: layer.position,
                size: layer.size,
                opacity: layer.opacity,
                config: layer.widget.save_config(),
            })
            .collect();

        preset::PresetData {
            version: 1,
            name: name.to_string(),
            background: bg,
            viewport: preset::ViewportData {
                zoom: self.canvas.base_viewport.zoom,
                pan: self.canvas.base_viewport.pan,
            },
            widgets,
        }
    }

    /// Apply a loaded preset's widget/viewport config to the current state.
    fn apply_preset_config(&mut self, data: &preset::PresetData) {
        // Restore base viewport
        self.canvas.base_viewport.zoom = data.viewport.zoom;
        self.canvas.base_viewport.pan = data.viewport.pan;

        // Clear existing widgets
        self.canvas.layers.clear();
        self.canvas.active_layer = canvas::LayerSelection::Base;

        // Recreate widgets from config
        for wd in &data.widgets {
            if let Some(mut w) = widget::create_by_type_id(&wd.type_id) {
                w.load_config(&wd.config);
                let id = widget::WidgetId(self.canvas.next_id());
                self.canvas.layers.push(canvas::WidgetLayer {
                    id,
                    widget: w,
                    position: wd.position,
                    size: wd.size,
                    visible: true,
                    opacity: wd.opacity,
                });
            }
        }

        self.rebuild_preview();
    }

    /// Start (or restart) the device display thread.
    fn start_display(&mut self) {
        if !self.driver_connected {
            return;
        }
        self.stop_signal.store(true, Ordering::Relaxed);

        let stop = Arc::new(AtomicBool::new(false));
        self.stop_signal = stop.clone();
        let shared = Arc::new(Mutex::new(Vec::new()));
        self.device_frame = shared.clone();

        self.push_device_frame();

        // Detect a fresh driver instance for the display thread.
        // Each display session gets its own connection.
        if let Some(driver) = coolcooler_driver::detect_device() {
            let shared_clone = shared;
            std::thread::spawn(move || {
                coolcooler_driver::run_display(driver, shared_clone, &stop);
            });
        }

        self.display_active = true;
    }

    /// Encode the current composited frame and push to the device thread.
    fn push_device_frame(&self) {
        let composited = self.render_composited();
        let encoded = match self.driver_capability {
            DisplayCapability::Streaming => {
                let rgb = DynamicImage::ImageRgba8(composited).to_rgb8();
                frame::encode_resized(&rgb, self.driver_info.rotation, DEFAULT_JPEG_QUALITY).ok()
            }
            DisplayCapability::FileTransfer => {
                // PNG encode — liquidctl handles resizing/format conversion
                let mut buf = std::io::Cursor::new(Vec::new());
                DynamicImage::ImageRgba8(composited)
                    .write_to(&mut buf, image::ImageFormat::Png)
                    .ok()
                    .map(|()| buf.into_inner())
            }
        };
        if let Some(bytes) = encoded {
            if let Ok(mut frame) = self.device_frame.lock() {
                *frame = bytes;
            }
        }
    }

    fn clamp_base_pan(&mut self) {
        if let Some(src) = self.source_frames.get(self.current_frame) {
            let (sw, sh) = (src.rgba.width() as f32, src.rgba.height() as f32);
            let short = sw.min(sh);
            let vp = &mut self.canvas.base_viewport;
            let vis = short / vp.zoom;

            if vis <= sw && vis <= sh {
                let max_pan_x = ((sw - vis) / 2.0).max(0.0);
                let max_pan_y = ((sh - vis) / 2.0).max(0.0);
                vp.pan.0 = vp.pan.0.clamp(-max_pan_x, max_pan_x);
                vp.pan.1 = vp.pan.1.clamp(-max_pan_y, max_pan_y);
            } else {
                let margin = short / vp.zoom * 0.375;
                vp.pan.0 = vp.pan.0.clamp(-margin, margin);
                vp.pan.1 = vp.pan.1.clamp(-margin, margin);
            }
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectFile => {
                return Task::perform(pick_file(), Message::FileSelected);
            }
            Message::FileSelected(Some(path)) => {
                self.selected_path = Some(path.clone());
                self.stop_signal.store(true, Ordering::Relaxed);
                self.display_active = false;
                self.source_frames.clear();
                self.preview = None;
                self.loading = true;
                self.canvas.base_viewport = Viewport::default();
                self.current_frame = 0;
                self.status_message = "Loading...".to_string();

                let filename = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                return Task::perform(
                    async move { load_source_data(&path, filename) },
                    Message::SourceLoaded,
                );
            }
            Message::FileSelected(None) => {}
            Message::SourceLoaded(result) => {
                self.loading = false;
                match result {
                    Ok(data) => {
                        let count = data.frames.len();
                        self.filename = data.filename;
                        self.source_frames = Arc::try_unwrap(data.frames)
                            .unwrap_or_else(|arc| (*arc).clone())
                            .into_iter()
                            .map(|f| SourceFrame {
                                rgba: RgbaImage::from_raw(f.width, f.height, f.pixels).unwrap(),
                                duration: f.duration,
                            })
                            .collect();

                        // On file-transfer devices, clear widgets when loading a GIF
                        if !widgets_allowed(self.driver_capability, count > 1)
                            && !self.canvas.layers.is_empty()
                        {
                            self.canvas.layers.clear();
                            self.canvas.active_layer = LayerSelection::Base;
                        }

                        let detail = if count > 1 {
                            format!(" ({count} frames)")
                        } else {
                            String::new()
                        };
                        self.status_message = format!("{}{detail}", self.filename);
                        self.current_frame = 0;
                        self.last_advance = Instant::now();
                        self.rebuild_preview();
                        self.start_display();
                    }
                    Err(e) => {
                        self.status_message = format!("Error: {e}");
                    }
                }
            }
            Message::AnimationTick => {
                if self.is_animated() {
                    let dur = self.source_frames[self.current_frame].duration;
                    if self.last_advance.elapsed() >= dur {
                        self.current_frame = (self.current_frame + 1) % self.source_frames.len();
                        self.last_advance = Instant::now();
                        self.rebuild_preview();
                        if self.display_active {
                            self.push_device_frame();
                        }
                    }
                }
            }
            Message::WidgetTick => {
                // Refresh sysinfo backend
                let has_sysinfo_widgets = self
                    .canvas
                    .layers
                    .iter()
                    .any(|l| l.widget.descriptor().category == "System Metrics");
                if has_sysinfo_widgets {
                    self.sysinfo_backend.refresh();
                    self.widget_ctx.sysinfo = self.sysinfo_backend.data().clone();
                }

                if self.canvas.tick_widgets(&self.widget_ctx) {
                    self.rebuild_preview();
                    if self.display_active {
                        self.push_device_frame();
                    }
                }
            }
            Message::Scroll(delta) => {
                let y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => y,
                    mouse::ScrollDelta::Pixels { y, .. } => y / 28.0,
                };
                let factor = 1.1_f32.powf(y);

                match self.canvas.active_layer {
                    LayerSelection::Base => {
                        if self.has_source() {
                            let vp = &mut self.canvas.base_viewport;
                            vp.zoom = (vp.zoom * factor).clamp(0.25, 10.0);
                            self.clamp_base_pan();
                            self.rebuild_preview();
                        }
                    }
                    LayerSelection::Widget(id) => {
                        if let Some(layer) = self.canvas.layers.iter_mut().find(|l| l.id == id) {
                            // Scale widget size
                            let new_w = ((layer.size.0 as f32) * factor).round() as u32;
                            let new_h = ((layer.size.1 as f32) * factor).round() as u32;
                            layer.size = (new_w.clamp(10, 240), new_h.clamp(10, 240));
                            self.rebuild_preview();
                        }
                    }
                }
            }
            Message::DragStart => {
                self.dragging = true;
                self.last_cursor = None;
            }
            Message::DragMove(pos) => {
                if self.dragging {
                    if let Some(last) = self.last_cursor {
                        let dx = pos.x - last.x;
                        let dy = pos.y - last.y;
                        let lcd = self.lcd_size();

                        match self.canvas.active_layer {
                            LayerSelection::Base => {
                                if let Some(src) = self.source_frames.get(self.current_frame) {
                                    let (sw, sh) =
                                        (src.rgba.width() as f32, src.rgba.height() as f32);
                                    let vis = sw.min(sh) / self.canvas.base_viewport.zoom;
                                    let src_per_px = vis / lcd as f32;
                                    let vp = &mut self.canvas.base_viewport;
                                    vp.pan.0 -= dx * src_per_px;
                                    vp.pan.1 -= dy * src_per_px;
                                    self.clamp_base_pan();
                                    self.rebuild_preview();
                                }
                            }
                            LayerSelection::Widget(id) => {
                                if let Some(layer) =
                                    self.canvas.layers.iter_mut().find(|l| l.id == id)
                                {
                                    let size = layer.size;
                                    let mut new_pos = (
                                        layer.position.0 + dx as i32,
                                        layer.position.1 + dy as i32,
                                    );
                                    let min_vis = 10i32;
                                    let lcd_i = lcd as i32;
                                    new_pos.0 = new_pos
                                        .0
                                        .clamp(-(size.0 as i32) + min_vis, lcd_i - min_vis);
                                    new_pos.1 = new_pos
                                        .1
                                        .clamp(-(size.1 as i32) + min_vis, lcd_i - min_vis);
                                    layer.position = new_pos;
                                    self.rebuild_preview();
                                }
                            }
                        }
                    }
                    self.last_cursor = Some(pos);
                }
            }
            Message::DragEnd => {
                self.dragging = false;
                self.last_cursor = None;
            }
            Message::ResetView => {
                match self.canvas.active_layer {
                    LayerSelection::Base => {
                        self.canvas.base_viewport = Viewport::default();
                    }
                    LayerSelection::Widget(id) => {
                        let lcd = self.lcd_size() as i32;
                        if let Some(layer) = self.canvas.layers.iter_mut().find(|l| l.id == id) {
                            let default_size = layer.widget.descriptor().default_size;
                            layer.size = default_size;
                            layer.position = (
                                (lcd - default_size.0 as i32) / 2,
                                (lcd - default_size.1 as i32) / 2,
                            );
                        }
                    }
                }
                self.rebuild_preview();
            }
            Message::SelectLayer(option) => {
                self.canvas.active_layer = option.selection;
            }
            Message::SelectCategory(cat) => {
                self.selected_category = cat;
            }
            Message::AddWidget(catalog_idx) => {
                // Block adding widgets when GIF is loaded on a file-transfer device
                if !widgets_allowed(self.driver_capability, self.is_animated()) {
                    return Task::none();
                }
                if let Some(template) = self.widget_catalog.get(catalog_idx) {
                    let instance = template.create_instance();
                    let id = self.canvas.add_widget(instance, self.lcd_size());
                    self.canvas.active_layer = LayerSelection::Widget(id);
                    self.rebuild_preview();
                }
            }
            Message::RemoveWidget(id) => {
                self.canvas.remove_widget(id);
                self.rebuild_preview();
            }
            Message::SetWidgetOpacity(val) => {
                if let LayerSelection::Widget(id) = self.canvas.active_layer {
                    if let Some(layer) = self.canvas.layers.iter_mut().find(|l| l.id == id) {
                        layer.opacity = (val * 255.0) as u8;
                        self.rebuild_preview();
                    }
                }
            }
            Message::SetWidgetTextColor(color) => {
                if let LayerSelection::Widget(id) = self.canvas.active_layer {
                    if let Some(layer) = self.canvas.layers.iter_mut().find(|l| l.id == id) {
                        layer.widget.set_text_color(color);
                        self.rebuild_preview();
                    }
                }
            }
            Message::SetWidgetFont(name) => {
                if let LayerSelection::Widget(id) = self.canvas.active_layer {
                    if let Some(layer) = self.canvas.layers.iter_mut().find(|l| l.id == id) {
                        layer.widget.set_font_name(name);
                        self.rebuild_preview();
                    }
                }
            }
            Message::SetWidgetText(text) => {
                if let LayerSelection::Widget(id) = self.canvas.active_layer {
                    if let Some(layer) = self.canvas.layers.iter_mut().find(|l| l.id == id) {
                        layer.widget.set_text_content(text);
                        self.rebuild_preview();
                    }
                }
            }
            Message::ShowSaveDialog => {
                if self.current_preset_name.is_some() {
                    // Already have a preset loaded — save in-place
                    let name = self.current_preset_name.clone().unwrap();
                    let data = self.build_preset_data(&name);
                    let composited = self.render_composited();
                    match preset::save(
                        &name,
                        self.current_preset_folder.as_deref(),
                        self.selected_path.as_deref(),
                        &composited,
                        &data,
                    ) {
                        Ok(_) => self.status_message = format!("Preset '{name}' saved"),
                        Err(e) => self.status_message = format!("Save failed: {e}"),
                    }
                } else {
                    // No preset loaded — show save dialog
                    self.save_name_input.clear();
                    self.show_save_dialog = true;
                }
            }
            Message::ShowLoadDialog => {
                self.preset_list = preset::list();
                self.show_load_dialog = true;
            }
            Message::CloseSaveDialog => {
                self.show_save_dialog = false;
            }
            Message::CloseLoadDialog => {
                self.show_load_dialog = false;
            }
            Message::SaveNameChanged(name) => {
                self.save_name_input = name;
            }
            Message::SavePreset => {
                let name = self.save_name_input.trim().to_string();
                if let Err(e) = preset::validate_name(&name) {
                    self.status_message = e.to_string();
                } else {
                    let data = self.build_preset_data(&name);
                    let composited = self.render_composited();
                    match preset::save(
                        &name,
                        None,
                        self.selected_path.as_deref(),
                        &composited,
                        &data,
                    ) {
                        Ok(folder) => {
                            self.current_preset_folder = Some(folder);
                            self.current_preset_name = Some(name.clone());
                            self.show_save_dialog = false;
                            self.status_message = format!("Preset '{name}' saved");
                        }
                        Err(e) => self.status_message = format!("Save failed: {e}"),
                    }
                }
            }
            Message::SavePresetAs => {
                self.save_name_input.clear();
                self.show_save_dialog = true;
            }
            Message::PresetClicked(folder) => {
                // Double-click detection: load if same folder clicked within 400ms
                let is_double = self
                    .last_preset_click
                    .as_ref()
                    .map(|(f, t)| f == &folder && t.elapsed() < Duration::from_millis(400))
                    .unwrap_or(false);

                if !is_double {
                    self.last_preset_click = Some((folder, Instant::now()));
                    return Task::none();
                }

                self.last_preset_click = None;
                self.show_load_dialog = false;
                match preset::load(&folder) {
                    Ok((data, bg_path)) => {
                        self.current_preset_folder = Some(folder);
                        self.current_preset_name = Some(data.name.clone());

                        if let Some(ref path) = bg_path {
                            if path.exists() {
                                let filename = path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default();
                                self.loading = true;
                                self.status_message = "Loading preset...".to_string();
                                let path_clone = path.clone();
                                return Task::perform(
                                    async move { load_source_data(&path_clone, filename) },
                                    move |result| Message::PresetSourceLoaded(result, data),
                                );
                            }
                        }

                        // No background — just apply widgets/viewport
                        self.source_frames.clear();
                        self.selected_path = None;
                        self.filename.clear();
                        self.current_frame = 0;
                        self.apply_preset_config(&data);
                        self.start_display();
                        self.status_message = format!("Loaded preset '{}'", data.name);
                    }
                    Err(e) => self.status_message = format!("Load failed: {e}"),
                }
            }
            Message::PresetSourceLoaded(result, data) => {
                self.loading = false;
                match result {
                    Ok(loaded) => {
                        self.source_frames = loaded
                            .frames
                            .iter()
                            .map(|f| SourceFrame {
                                rgba: RgbaImage::from_raw(f.width, f.height, f.pixels.clone())
                                    .unwrap(),
                                duration: f.duration,
                            })
                            .collect();
                        self.filename = loaded.filename;
                        // Set selected_path to the preset's background file
                        if let (Some(bg), Some(folder)) = (
                            data.background.as_ref(),
                            self.current_preset_folder.as_deref(),
                        ) {
                            self.selected_path = Some(preset::preset_file_path(folder, &bg.file));
                        }
                        self.current_frame = 0;
                        self.last_advance = Instant::now();
                        self.apply_preset_config(&data);
                        self.start_display();
                        self.status_message = format!("Loaded preset '{}'", data.name);
                    }
                    Err(e) => self.status_message = format!("Load failed: {e}"),
                }
            }
            Message::DeletePreset(folder) => {
                if let Err(e) = preset::delete(&folder) {
                    self.status_message = format!("Delete failed: {e}");
                } else {
                    self.preset_list = preset::list();
                    // If we deleted the current preset, clear all tracking
                    if self.current_preset_folder.as_deref() == Some(&folder) {
                        self.current_preset_folder = None;
                        self.current_preset_name = None;
                        self.selected_path = None;
                    }
                }
            }
            Message::ToggleTheme => {
                self.dark_mode = !self.dark_mode;
            }
            Message::TrayPoll => {
                let got_show = matches!(
                    self.tray_rx.lock().unwrap().try_recv(),
                    Ok(tray::TrayEvent::ShowWindow)
                );
                if got_show {
                    return self.update(Message::ShowWindow);
                }
            }
            Message::WindowClosed(id) => {
                if self.window_id == Some(id) {
                    self.window_id = None;
                }
            }
            Message::ShowWindow => {
                if let Some(window_id) = self.window_id {
                    // Window already open, just focus it
                    return window::gain_focus(window_id);
                }
                let (id, task) = window::open(app_window_settings());
                self.window_id = Some(id);
                return task.discard();
            }
            Message::Quit => {
                self.stop_signal.store(true, Ordering::Relaxed);
                return iced::exit();
            }
        }
        Task::none()
    }

    fn view(&self, _window_id: window::Id) -> Element<'_, Message> {
        let c = self.colors();

        let title_row = row![
            text("CoolCooler").size(28).color(c.accent),
            iced::widget::space().width(Length::Fill),
            iced::widget::toggler(self.dark_mode)
                .label("Dark")
                .on_toggle(|_| Message::ToggleTheme)
                .size(16)
                .text_size(12),
        ]
        .align_y(iced::Alignment::Center);

        // -- Device card --
        let device_content = if self.driver_connected {
            let info = &self.driver_info;
            let (w, h) = (info.resolution.width, info.resolution.height);
            column![
                section_label("DEVICE", c),
                row![
                    text("●").size(10).color(c.green),
                    text(info.name.as_str()).size(15),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                text(format!("{w}×{h} LCD")).size(12).color(c.text_dim),
            ]
            .spacing(6)
        } else {
            column![
                section_label("DEVICE", c),
                row![
                    text("●").size(10).color(c.red),
                    text("No cooler detected").size(15),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            ]
            .spacing(6)
        };

        // -- Preview --
        let lcd_size = self.lcd_size() as f32;

        let handle = self.preview.clone().unwrap_or_else(|| {
            // Fallback: empty black circle
            let lcd = self.lcd_size();
            circular_preview_from_rgba(RgbaImage::from_pixel(lcd, lcd, Rgba([0, 0, 0, 255])))
        });

        let is_widget_selected = matches!(self.canvas.active_layer, LayerSelection::Widget(_));
        let cursor_style = if self.dragging {
            mouse::Interaction::Grabbing
        } else if is_widget_selected {
            mouse::Interaction::Move
        } else {
            mouse::Interaction::Grab
        };

        let preview_inner: Element<Message> = row![
            iced::widget::space().width(Length::Fill),
            mouse_area(iced::widget::image(handle).width(lcd_size).height(lcd_size),)
                .on_scroll(Message::Scroll)
                .on_press(Message::DragStart)
                .on_release(Message::DragEnd)
                .on_move(Message::DragMove)
                .interaction(cursor_style),
            iced::widget::space().width(Length::Fill),
        ]
        .into();

        // -- Canvas header with zoom info + reset --
        let show_reset = match self.canvas.active_layer {
            LayerSelection::Base => {
                (self.canvas.base_viewport.zoom - 1.0).abs() > 0.01
                    || self.canvas.base_viewport.pan != (0.0, 0.0)
            }
            LayerSelection::Widget(id) => self
                .canvas
                .layers
                .iter()
                .find(|l| l.id == id)
                .map(|l| {
                    let def = l.widget.descriptor().default_size;
                    l.size != def
                        || l.position
                            != (
                                (self.lcd_size() as i32 - def.0 as i32) / 2,
                                (self.lcd_size() as i32 - def.1 as i32) / 2,
                            )
                })
                .unwrap_or(false),
        };

        let canvas_header = if show_reset {
            let info_text = match self.canvas.active_layer {
                LayerSelection::Base => {
                    format!("{}%", (self.canvas.base_viewport.zoom * 100.0) as u32)
                }
                LayerSelection::Widget(id) => self
                    .canvas
                    .layers
                    .iter()
                    .find(|l| l.id == id)
                    .map(|l| format!("{}×{}", l.size.0, l.size.1))
                    .unwrap_or_default(),
            };
            row![
                section_label("CANVAS", c),
                iced::widget::space().width(Length::Fill),
                text(info_text).size(11).color(c.text_dim),
                button(text("Reset").size(10).color(c.accent))
                    .padding([2, 8])
                    .style(|_: &Theme, _| button::Style {
                        background: None,
                        text_color: c.accent,
                        ..Default::default()
                    })
                    .on_press(Message::ResetView),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center)
        } else {
            row![section_label("CANVAS", c)]
        };

        // -- Layer selector --
        let layer_options: Vec<LayerOption> = self
            .canvas
            .layer_options()
            .into_iter()
            .map(|(sel, label)| LayerOption {
                selection: sel,
                label,
            })
            .collect();

        let active_option = layer_options
            .iter()
            .find(|o| o.selection == self.canvas.active_layer)
            .cloned();

        let layer_picker = pick_list(layer_options, active_option, Message::SelectLayer)
            .width(Length::Fill)
            .text_size(13);

        // -- Layer selector with label + optional remove button --
        let layer_controls: Element<Message> =
            if let LayerSelection::Widget(id) = self.canvas.active_layer {
                row![
                    text("Layer").size(12).color(c.text_dim),
                    layer_picker,
                    button(text("Remove").size(11).color(c.red))
                        .padding([6, 12])
                        .style({
                            let danger_bg = c.danger_bg;
                            let red = c.red;
                            move |_: &Theme, _| button::Style {
                                background: Some(Background::Color(danger_bg)),
                                text_color: red,
                                border: Border {
                                    radius: 6.0.into(),
                                    ..Default::default()
                                },
                                ..Default::default()
                            }
                        })
                        .on_press(Message::RemoveWidget(id)),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into()
            } else {
                row![text("Layer").size(12).color(c.text_dim), layer_picker]
                    .spacing(8)
                    .align_y(iced::Alignment::Center)
                    .into()
            };

        let preview_card = card(
            column![canvas_header, preview_inner, layer_controls].spacing(12),
            c,
        );

        // -- Widget config panel (only when a widget layer is selected) --
        let widget_config: Option<Element<Message>> =
            if let LayerSelection::Widget(id) = self.canvas.active_layer {
                self.canvas.layers.iter().find(|l| l.id == id).map(|layer| {
                    let opacity_val = layer.opacity as f32 / 255.0;
                    let mut config_items = column![row![
                        text("Opacity").size(12).color(c.text_dim).width(60),
                        slider(0.0..=1.0, opacity_val, Message::SetWidgetOpacity)
                            .step(0.01)
                            .width(Length::Fill),
                        text(format!("{}%", (opacity_val * 100.0) as u32))
                            .size(11)
                            .color(c.text_dim)
                            .width(35),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center)]
                    .spacing(8);

                    // Text color swatches (only for widgets that support it)
                    if layer.widget.supports_text_color() {
                        let current_color = layer.widget.text_color();
                        let colors: Vec<([u8; 4], &str)> = vec![
                            ([255, 255, 255, 255], "White"),
                            ([220, 220, 220, 255], "Light Gray"),
                            ([80, 255, 80, 255], "Green"),
                            ([255, 80, 60, 255], "Red"),
                            ([0, 180, 255, 255], "Cyan"),
                            ([255, 200, 50, 255], "Gold"),
                            ([255, 140, 0, 255], "Orange"),
                            ([200, 80, 255, 255], "Purple"),
                            ([255, 105, 180, 255], "Pink"),
                        ];

                        let mut swatches = row![text("Color").size(12).color(c.text_dim).width(60)]
                            .spacing(4)
                            .align_y(iced::Alignment::Center);

                        for (color, _name) in &colors {
                            let c = *color;
                            let is_selected = current_color[0..3] == c[0..3];
                            let border_color = if is_selected {
                                Color::WHITE
                            } else {
                                Color::TRANSPARENT
                            };
                            let swatch_color = Color::from_rgb8(c[0], c[1], c[2]);

                            swatches = swatches.push(
                                button(text("").width(14).height(14))
                                    .padding(0)
                                    .width(18)
                                    .height(18)
                                    .style(move |_: &Theme, _| button::Style {
                                        background: Some(Background::Color(swatch_color)),
                                        border: Border {
                                            radius: 3.0.into(),
                                            width: 2.0,
                                            color: border_color,
                                        },
                                        ..Default::default()
                                    })
                                    .on_press(Message::SetWidgetTextColor(c)),
                            );
                        }

                        config_items = config_items.push(swatches);
                    }

                    // Font dropdown (for widgets that support it)
                    if layer.widget.supports_font() {
                        let font_names: Vec<String> = widget::fonts::font_names()
                            .into_iter()
                            .map(|s| s.to_string())
                            .collect();
                        let current_font = layer.widget.font_name().to_string();
                        config_items = config_items.push(
                            row![
                                text("Font").size(12).color(c.text_dim).width(60),
                                pick_list(font_names, Some(current_font), Message::SetWidgetFont)
                                    .width(Length::Fill)
                                    .text_size(12),
                            ]
                            .spacing(8)
                            .align_y(iced::Alignment::Center),
                        );
                    }

                    // Text content input (for FreeText widget)
                    if layer.widget.supports_text_edit() {
                        let current_text = layer.widget.text_content().to_string();
                        config_items = config_items.push(
                            row![
                                text("Text").size(12).color(c.text_dim).width(60),
                                iced::widget::text_input("Enter text...", &current_text)
                                    .on_input(Message::SetWidgetText)
                                    .size(12)
                                    .width(Length::Fill),
                            ]
                            .spacing(8)
                            .align_y(iced::Alignment::Center),
                        );
                    }

                    card(config_items, c).into()
                })
            } else {
                None
            };

        // -- Status --
        let status = text(&self.status_message).size(12).color(c.text_dim);

        // -- Buttons --
        let select_btn = if self.loading {
            styled_button("Loading...", ButtonKind::Disabled, c)
        } else {
            styled_button("Select Image", ButtonKind::Default, c).on_press(Message::SelectFile)
        };

        // Preset buttons
        let save_label = if self.current_preset_name.is_some() {
            "Save"
        } else {
            "Save Preset"
        };
        let save_btn =
            styled_button(save_label, ButtonKind::Default, c).on_press(Message::ShowSaveDialog);

        let mut preset_buttons = row![save_btn].spacing(8);
        if self.current_preset_name.is_some() {
            preset_buttons = preset_buttons.push(
                styled_button("Save As", ButtonKind::Default, c).on_press(Message::SavePresetAs),
            );
        }
        preset_buttons = preset_buttons.push(
            styled_button("Load Preset", ButtonKind::Default, c).on_press(Message::ShowLoadDialog),
        );

        let quit_btn = styled_button("Quit", ButtonKind::Danger, c).on_press(Message::Quit);

        // -- Left panel (below title) --
        let mut left_panel = column![card(device_content, c), preview_card,]
            .spacing(16)
            .width(Length::FillPortion(7))
            .height(Length::Fill);

        if let Some(config) = widget_config {
            left_panel = left_panel.push(config);
        }

        left_panel = left_panel
            .push(status)
            .push(iced::widget::space().height(Length::Fill))
            .push(select_btn)
            .push(preset_buttons)
            .push(quit_btn);

        // -- Right panel: widget catalog --
        let right_panel = if !widgets_allowed(self.driver_capability, self.is_animated()) {
            card(
                column![
                    text("Widgets").size(18).color(c.text_primary),
                    text("Widgets with animated backgrounds are not supported on this device.")
                        .size(13)
                        .color(c.text_dim),
                ]
                .spacing(12),
                c,
            )
            .width(Length::FillPortion(3))
            .height(Length::Fill)
        } else {
            let categories: Vec<String> = widget::categories(&self.widget_catalog)
                .into_iter()
                .map(|s| s.to_string())
                .collect();

            let category_picker = pick_list(
                categories,
                Some(self.selected_category.clone()),
                Message::SelectCategory,
            )
            .width(Length::Fill)
            .text_size(13);

            let mut catalog_items = column![].spacing(6);
            for (i, w) in self.widget_catalog.iter().enumerate() {
                let desc = w.descriptor();
                if desc.category != self.selected_category {
                    continue;
                }
                catalog_items = catalog_items.push(
                    button(text(desc.name).size(13).color(c.text_primary).center())
                        .padding([8, 12])
                        .width(Length::Fill)
                        .style(|_: &Theme, _| button::Style {
                            background: Some(Background::Color(c.surface)),
                            text_color: c.text_primary,
                            border: Border {
                                radius: 8.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .on_press(Message::AddWidget(i)),
                );
            }

            card(
                column![
                    text("Widgets").size(18).color(c.text_primary),
                    category_picker,
                    catalog_items,
                ]
                .spacing(12),
                c,
            )
            .width(Length::FillPortion(3))
            .height(Length::Fill)
        };

        // -- Footer --
        let footer = text("Made with ♥ by asheriif")
            .size(11)
            .color(c.text_dim)
            .center()
            .width(Length::Fill);

        // -- Dialog views (replace main content when open) --
        if self.show_save_dialog {
            return container(card(
                column![
                    text("Save Preset").size(22).color(c.text_primary),
                    text_input("Preset name...", &self.save_name_input)
                        .on_input(Message::SaveNameChanged)
                        .on_submit(Message::SavePreset)
                        .size(14)
                        .padding(10),
                    row![
                        styled_button("Save", ButtonKind::Default, c).on_press(Message::SavePreset),
                        styled_button("Cancel", ButtonKind::Danger, c)
                            .on_press(Message::CloseSaveDialog),
                    ]
                    .spacing(8),
                ]
                .spacing(16)
                .width(350),
                c,
            ))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Shrink)
            .center_y(Length::Shrink)
            .padding(24)
            .into();
        }

        if self.show_load_dialog {
            let mut preset_grid = column![].spacing(12);

            if self.preset_list.is_empty() {
                preset_grid = preset_grid.push(
                    container(
                        text("No presets saved yet")
                            .size(14)
                            .color(c.text_dim)
                            .center()
                            .width(Length::Fill),
                    )
                    .padding(40),
                );
            } else {
                let mut grid_row = row![].spacing(12);
                for (i, entry) in self.preset_list.iter().enumerate() {
                    let folder = entry.folder.clone();
                    let folder_del = entry.folder.clone();

                    let preview_el: Element<Message> = if let Some(ref handle) = entry.preview {
                        container(iced::widget::image(handle.clone()).width(120).height(120))
                            .style(|_: &Theme| container::Style {
                                border: Border {
                                    radius: 8.0.into(),
                                    ..Default::default()
                                },
                                ..Default::default()
                            })
                            .into()
                    } else {
                        container(text("No preview").size(10).color(c.text_dim))
                            .width(120)
                            .height(120)
                            .center_x(Length::Shrink)
                            .center_y(Length::Shrink)
                            .style(|_: &Theme| container::Style {
                                background: Some(Background::Color(c.surface)),
                                border: Border {
                                    radius: 8.0.into(),
                                    ..Default::default()
                                },
                                ..Default::default()
                            })
                            .into()
                    };

                    let preset_card: Element<Message> = mouse_area(
                        container(
                            column![
                                preview_el,
                                text(&entry.name)
                                    .size(12)
                                    .color(c.text_primary)
                                    .center()
                                    .width(Length::Fill),
                                button(text("Delete").size(9).color(c.red).center())
                                    .padding([2, 8])
                                    .width(Length::Fill)
                                    .style(|_: &Theme, _| button::Style {
                                        background: None,
                                        text_color: c.red,
                                        ..Default::default()
                                    })
                                    .on_press(Message::DeletePreset(folder_del)),
                            ]
                            .spacing(6)
                            .align_x(iced::Alignment::Center),
                        )
                        .padding(10)
                        .style(|_: &Theme| container::Style {
                            background: Some(Background::Color(c.card_bg)),
                            border: Border {
                                radius: 10.0.into(),
                                width: 1.0,
                                color: Color::from_rgb(0.2, 0.2, 0.28),
                            },
                            ..Default::default()
                        }),
                    )
                    .on_press(Message::PresetClicked(folder))
                    .into();

                    grid_row = grid_row.push(preset_card);

                    if (i + 1) % 4 == 0 {
                        preset_grid = preset_grid.push(grid_row);
                        grid_row = row![].spacing(12);
                    }
                }
                if !self.preset_list.len().is_multiple_of(4) {
                    preset_grid = preset_grid.push(grid_row);
                }
            }

            return column![
                row![
                    text("Load Preset").size(22).color(c.text_primary),
                    iced::widget::space().width(Length::Fill),
                    styled_button("Back", ButtonKind::Default, c)
                        .on_press(Message::CloseLoadDialog),
                ]
                .align_y(iced::Alignment::Center),
                scrollable(preset_grid).height(Length::Fill),
            ]
            .spacing(16)
            .padding(24)
            .height(Length::Fill)
            .into();
        }

        // -- Main layout --
        column![
            title_row,
            row![left_panel, right_panel]
                .spacing(16)
                .height(Length::Fill),
            footer,
        ]
        .spacing(16)
        .padding(24)
        .height(Length::Fill)
        .into()
    }

    fn theme(&self, _window_id: window::Id) -> Theme {
        let c = self.colors();
        Theme::custom(
            "CoolCooler".to_string(),
            iced::theme::Palette {
                background: c.bg,
                text: c.text_primary,
                primary: c.accent,
                success: c.green,
                warning: Color::from_rgb(0.718, 0.494, 0.204),
                danger: c.red,
            },
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = Vec::new();

        // Track when window is closed (minimize to tray)
        subs.push(window::close_events().map(Message::WindowClosed));

        // Poll tray icon events (250ms is responsive enough for a click-to-show)
        subs.push(iced::time::every(Duration::from_millis(250)).map(|_| Message::TrayPoll));

        // 30ms tick for GIF animation
        if self.is_animated() {
            subs.push(iced::time::every(Duration::from_millis(30)).map(|_| Message::AnimationTick));
        }

        // 1s tick for dynamic widgets (clock, date, sysinfo)
        let has_dynamic = self.canvas.layers.iter().any(|l| l.widget.is_dynamic());
        if has_dynamic {
            subs.push(iced::time::every(Duration::from_secs(1)).map(|_| Message::WidgetTick));
        }

        Subscription::batch(subs)
    }
}

// -- UI helpers --

fn section_label<'a>(label: &'a str, c: &AppColors) -> iced::widget::Text<'a> {
    text(label).size(11).color(c.text_dim)
}

fn card<'a>(
    content: impl Into<Element<'a, Message>>,
    c: &AppColors,
) -> container::Container<'a, Message> {
    let bg = c.card_bg;
    container(content)
        .padding(16)
        .width(Length::Fill)
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                radius: 12.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
}

enum ButtonKind {
    Default,
    Danger,
    Disabled,
}

fn styled_button<'a>(
    label: &'a str,
    kind: ButtonKind,
    c: &AppColors,
) -> button::Button<'a, Message> {
    let (bg, text_color) = match kind {
        ButtonKind::Default => (c.surface, c.text_primary),
        ButtonKind::Danger => (c.danger_bg, c.red),
        ButtonKind::Disabled => (c.disabled_bg, c.text_dim),
    };

    button(text(label).size(14).color(text_color).center())
        .padding([10, 20])
        .width(Length::Fill)
        .style(move |_theme: &Theme, _status| button::Style {
            background: Some(Background::Color(bg)),
            text_color,
            border: Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
}

// -- Viewport rendering --

/// Render the base layer at its viewport. Returns a 240x240 RGBA image.
fn render_base_rgba(
    source: &RgbaImage,
    info: &DeviceInfo,
    zoom: f32,
    pan: (f32, f32),
) -> RgbaImage {
    let (sw, sh) = (source.width() as f32, source.height() as f32);
    let res = info.resolution;
    let short = sw.min(sh);
    let vis = short / zoom;

    if vis <= sw && vis <= sh {
        let cx = (sw / 2.0 + pan.0).clamp(vis / 2.0, sw - vis / 2.0);
        let cy = (sh / 2.0 + pan.1).clamp(vis / 2.0, sh - vis / 2.0);

        let x0 = (cx - vis / 2.0).max(0.0) as u32;
        let y0 = (cy - vis / 2.0).max(0.0) as u32;
        let side = (vis as u32)
            .min(source.width() - x0)
            .min(source.height() - y0);

        let cropped = imageops::crop_imm(source, x0, y0, side, side).to_image();
        resize_rgba(&cropped, res.width, res.height)
    } else {
        let scale = res.width as f32 * zoom / short;
        let scaled_w = (sw * scale).round() as u32;
        let scaled_h = (sh * scale).round() as u32;

        let scaled = resize_rgba(source, scaled_w.max(1), scaled_h.max(1));

        let pan_scale = res.width as f32 / short;
        let pan_ox = (pan.0 * pan_scale).round() as i64;
        let pan_oy = (pan.1 * pan_scale).round() as i64;

        let mut canvas = RgbaImage::from_pixel(res.width, res.height, Rgba([0, 0, 0, 255]));
        let base_ox = (res.width.saturating_sub(scaled_w)) as i64 / 2;
        let base_oy = (res.height.saturating_sub(scaled_h)) as i64 / 2;
        imageops::overlay(&mut canvas, &scaled, base_ox - pan_ox, base_oy - pan_oy);
        canvas
    }
}

// -- Source loading --

fn load_source_data(path: &Path, filename: String) -> Result<LoadedData, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let frames = if ext == "gif" {
        load_gif_source_data(path)?
    } else {
        let img = image::open(path).map_err(|e| e.to_string())?;
        let rgba = img.to_rgba8();
        vec![LoadedFrame {
            width: rgba.width(),
            height: rgba.height(),
            pixels: rgba.into_raw(),
            duration: Duration::MAX,
        }]
    };

    Ok(LoadedData {
        frames: Arc::new(frames),
        filename,
    })
}

fn load_gif_source_data(path: &Path) -> Result<Vec<LoadedFrame>, String> {
    let file = BufReader::new(File::open(path).map_err(|e| e.to_string())?);
    let decoder = image::codecs::gif::GifDecoder::new(file).map_err(|e| e.to_string())?;
    let raw_frames = decoder
        .into_frames()
        .collect_frames()
        .map_err(|e| e.to_string())?;

    if raw_frames.is_empty() {
        return Err("GIF has no frames".to_string());
    }

    Ok(raw_frames
        .into_iter()
        .map(|raw| {
            let (n, d) = raw.delay().numer_denom_ms();
            let ms = if d == 0 { 100 } else { n / d };
            let duration = Duration::from_millis((ms).max(20) as u64);
            let rgba = raw.into_buffer();
            LoadedFrame {
                width: rgba.width(),
                height: rgba.height(),
                pixels: rgba.into_raw(),
                duration,
            }
        })
        .collect())
}

fn resize_rgba(source: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let (sw, sh) = (source.width(), source.height());
    if sw == width && sh == height {
        return source.clone();
    }
    let src_img =
        fir::images::Image::from_vec_u8(sw, sh, source.as_raw().clone(), fir::PixelType::U8x4)
            .unwrap();
    let mut dst_img = fir::images::Image::new(width, height, fir::PixelType::U8x4);
    let mut resizer = fir::Resizer::new();
    resizer
        .resize(
            &src_img,
            &mut dst_img,
            &fir::ResizeOptions::new()
                .resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::CatmullRom)),
        )
        .unwrap();
    RgbaImage::from_raw(width, height, dst_img.into_vec()).unwrap()
}

// -- Circular preview --

fn circular_preview_from_rgba(mut rgba: RgbaImage) -> iced::widget::image::Handle {
    let (w, h) = (rgba.width(), rgba.height());
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    let r = cx.min(cy);
    let aa_width = 1.5;

    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 - cx + 0.5;
            let dy = y as f32 - cy + 0.5;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist > r {
                rgba.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            } else if dist > r - aa_width {
                let alpha = ((r - dist) / aa_width * 255.0).clamp(0.0, 255.0) as u8;
                let p = *rgba.get_pixel(x, y);
                rgba.put_pixel(x, y, Rgba([p[0], p[1], p[2], alpha]));
            }
        }
    }

    iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw())
}

// -- Window --

fn app_window_settings() -> window::Settings {
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

async fn pick_file() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp"])
        .pick_file()
        .await
        .map(|h| h.path().to_path_buf())
}
