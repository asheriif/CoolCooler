mod canvas;
mod composition;
mod display_session;
mod preset;
mod rendering;
mod source;
mod style;
mod tray;
mod view;
mod widget;
mod windowing;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use canvas::{Canvas, LayerSelection, Viewport};
use composition::{CanvasPolicy, SourceKind};
use coolcooler_core::frame::{self, DEFAULT_JPEG_QUALITY};
use coolcooler_core::DeviceInfo;
use coolcooler_driver::DisplayCapability;
use display_session::DisplaySession;
use iced::{mouse, window, Color, Element, Point, Subscription, Task, Theme};
use image::{DynamicImage, Rgba, RgbaImage};
use rendering::{circular_preview_from_rgba, render_base_rgba};
use source::{load_source_data, LoadedData, SourceFrame};
use style::{AppColors, DARK, LIGHT};
use widget::{sysinfo_backend::SysInfoBackend, WidgetContext, WidgetSpec};
use windowing::{app_window_settings, ensure_single_instance, pick_file};

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
    widget_catalog: &'static [WidgetSpec],
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
    display_session: Option<DisplaySession>,

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
    LoadLastPreset(String),
    PresetClicked(String),
    DeletePreset(String),
    PresetSourceLoaded {
        result: Result<LoadedData, String>,
        data: preset::PresetData,
        folder: String,
        silent: bool,
    },
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
            display_session: None,
            _tray_handle: tray_handle,
            tray_rx: Arc::new(Mutex::new(tray_rx)),
            window_id: Some(id),
        };
        app.rebuild_preview();

        let startup_task = preset::last_used_folder()
            .map(|folder| Task::done(Message::LoadLastPreset(folder)))
            .unwrap_or_else(Task::none);

        (app, Task::batch([open_task.discard(), startup_task]))
    }

    fn has_source(&self) -> bool {
        !self.source_frames.is_empty()
    }

    fn is_animated(&self) -> bool {
        self.source_frames.len() > 1
    }

    fn source_kind(&self) -> SourceKind {
        SourceKind::from_frame_count(self.source_frames.len())
    }

    fn canvas_policy(&self) -> CanvasPolicy {
        CanvasPolicy::for_content(self.driver_capability, self.source_kind())
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
        if self.display_session.is_some() {
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
                type_id: layer.type_id.to_string(),
                position: layer.position,
                size: layer.size,
                opacity: layer.opacity,
                config: layer.widget.settings(),
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
            if let Some(spec) = widget::spec_by_type_id(&wd.type_id) {
                let mut w = spec.create();
                w.apply_settings(&wd.config);
                let id = widget::WidgetId(self.canvas.next_id());
                self.canvas.layers.push(canvas::WidgetLayer {
                    id,
                    type_id: spec.type_id,
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

    fn load_preset_folder(&mut self, folder: String, silent: bool) -> Task<Message> {
        let (data, bg_path) = match preset::load(&folder) {
            Ok(loaded) => loaded,
            Err(e) => {
                if !silent {
                    self.status_message = format!("Load failed: {e}");
                }
                return Task::none();
            }
        };

        if let Some(path) = bg_path {
            if !path.exists() {
                if !silent {
                    self.status_message = "Load failed: background file missing".to_string();
                }
                return Task::none();
            }

            let filename = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let path_clone = path.clone();

            self.loading = true;
            if !silent {
                self.status_message = "Loading preset...".to_string();
            }

            return Task::perform(
                async move { load_source_data(&path_clone, filename) },
                move |result| Message::PresetSourceLoaded {
                    result,
                    data,
                    folder,
                    silent,
                },
            );
        }

        self.apply_loaded_preset(folder, data, None, silent);
        Task::none()
    }

    fn apply_loaded_preset(
        &mut self,
        folder: String,
        data: preset::PresetData,
        loaded_source: Option<LoadedData>,
        silent: bool,
    ) {
        let name = data.name.clone();

        if let Some(loaded) = loaded_source {
            let LoadedData { frames, filename } = loaded;
            self.filename = filename;
            self.source_frames = Arc::try_unwrap(frames)
                .unwrap_or_else(|arc| (*arc).clone())
                .into_iter()
                .map(|f| SourceFrame {
                    rgba: RgbaImage::from_raw(f.width, f.height, f.pixels).unwrap(),
                    duration: f.duration,
                })
                .collect();

            if let Some(bg) = data.background.as_ref() {
                self.selected_path = Some(preset::preset_file_path(&folder, &bg.file));
            }
        } else {
            self.source_frames.clear();
            self.selected_path = None;
            self.filename.clear();
        }

        self.current_preset_folder = Some(folder.clone());
        self.current_preset_name = Some(name.clone());
        self.current_frame = 0;
        self.last_advance = Instant::now();
        self.apply_preset_config(&data);
        self.start_display();
        preset::remember_last_used(&folder);

        if !silent || self.status_message.is_empty() {
            self.status_message = format!("Loaded preset '{name}'");
        }
    }

    /// Start (or restart) the device display thread.
    fn start_display(&mut self) {
        self.stop_display();
        if !self.driver_connected {
            return;
        }

        if let (Some(driver), Some(frame)) = (
            coolcooler_driver::detect_device(),
            self.encode_device_frame(),
        ) {
            self.display_session = Some(DisplaySession::start(driver, frame));
        }
    }

    fn stop_display(&mut self) {
        if let Some(session) = self.display_session.take() {
            session.stop();
        }
    }

    fn encode_device_frame(&self) -> Option<Vec<u8>> {
        let composited = self.render_composited();
        match self.driver_capability {
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
        }
    }

    /// Encode the current composited frame and push to the device thread.
    fn push_device_frame(&self) {
        if let (Some(session), Some(bytes)) =
            (self.display_session.as_ref(), self.encode_device_frame())
        {
            session.submit_frame(bytes);
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
                self.stop_display();
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
                        let policy = CanvasPolicy::for_content(
                            self.driver_capability,
                            SourceKind::from_frame_count(count),
                        );
                        if !policy.widgets_allowed() && !self.canvas.layers.is_empty() {
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
                        if self.display_session.is_some() {
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
                    if self.display_session.is_some() {
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
                if !self.canvas_policy().widgets_allowed() {
                    return Task::none();
                }
                if let Some(spec) = self.widget_catalog.get(catalog_idx) {
                    let id = self.canvas.add_widget(spec, self.lcd_size());
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
                        let mut settings = layer.widget.settings();
                        settings.color = Some(color);
                        layer.widget.apply_settings(&settings);
                        self.rebuild_preview();
                    }
                }
            }
            Message::SetWidgetFont(name) => {
                if let LayerSelection::Widget(id) = self.canvas.active_layer {
                    if let Some(layer) = self.canvas.layers.iter_mut().find(|l| l.id == id) {
                        let mut settings = layer.widget.settings();
                        settings.font_name = Some(name);
                        layer.widget.apply_settings(&settings);
                        self.rebuild_preview();
                    }
                }
            }
            Message::SetWidgetText(text) => {
                if let LayerSelection::Widget(id) = self.canvas.active_layer {
                    if let Some(layer) = self.canvas.layers.iter_mut().find(|l| l.id == id) {
                        let mut settings = layer.widget.settings();
                        settings.text = Some(text);
                        layer.widget.apply_settings(&settings);
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
                        Ok(folder) => {
                            self.current_preset_folder = Some(folder.clone());
                            self.current_preset_name = Some(name.clone());
                            preset::remember_last_used(&folder);
                            self.status_message = format!("Preset '{name}' saved");
                        }
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
                            preset::remember_last_used(&folder);
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
            Message::LoadLastPreset(folder) => {
                return self.load_preset_folder(folder, true);
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
                return self.load_preset_folder(folder, false);
            }
            Message::PresetSourceLoaded {
                result,
                data,
                folder,
                silent,
            } => {
                self.loading = false;
                match result {
                    Ok(loaded) => {
                        self.apply_loaded_preset(folder, data, Some(loaded), silent);
                    }
                    Err(e) => {
                        if !silent {
                            self.status_message = format!("Load failed: {e}");
                        }
                    }
                }
            }
            Message::DeletePreset(folder) => {
                if let Err(e) = preset::delete(&folder) {
                    self.status_message = format!("Delete failed: {e}");
                } else {
                    preset::forget_last_used_if(&folder);
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
                self.stop_display();
                return iced::exit();
            }
        }
        Task::none()
    }

    fn view(&self, window_id: window::Id) -> Element<'_, Message> {
        view::view(self, window_id)
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
