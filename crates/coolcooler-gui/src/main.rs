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
use display_session::DisplayController;
use iced::{mouse, window, Color, Element, Point, Subscription, Task, Theme};
use image::{Rgba, RgbaImage};
use rendering::{circular_preview_from_rgba, render_base_rgba};
use source::{load_source_data, LoadedData, SourceFrame};
use style::{AppColors, DARK, LIGHT};
use widget::{sysinfo_backend::SysInfoBackend, WidgetContext, WidgetEdit, WidgetSpec};
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
    current_preset_folder: Option<preset::PresetFolder>,
    current_preset_name: Option<String>,
    show_save_dialog: bool,
    show_load_dialog: bool,
    save_name_input: String,
    preset_list: Vec<preset::PresetEntry>,
    last_preset_click: Option<(preset::PresetFolder, Instant)>,

    status_message: String,
    display: DisplayController,

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
    LoadLastPreset(preset::PresetFolder),
    PresetClicked(preset::PresetFolder),
    DeletePreset(preset::PresetFolder),
    PresetSourceLoaded {
        result: Result<LoadedData, String>,
        data: preset::PresetData,
        folder: preset::PresetFolder,
        background_path: Option<PathBuf>,
        silent: bool,
    },
    ToggleTheme,
    WindowClosed(window::Id),
    TrayPoll,
    DisplaySessionPoll,
    ShowWindow,
    Quit,
}

impl CoolCooler {
    fn boot() -> (Self, Task<Message>) {
        let (tray_handle, tray_rx) = tray::spawn();

        let (id, open_task) = window::open(app_window_settings());

        let mut app = Self {
            dark_mode: true,
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
            display: DisplayController::new(),
            _tray_handle: tray_handle,
            tray_rx: Arc::new(Mutex::new(tray_rx)),
            window_id: Some(id),
        };
        app.rebuild_preview();
        preset::cleanup_stale_internal_dirs();

        let startup_task = preset::last_used_folder()
            .map(|folder| Task::done(Message::LoadLastPreset(folder)))
            .unwrap_or_else(Task::none);

        (app, Task::batch([open_task.discard(), startup_task]))
    }

    fn is_animated(&self) -> bool {
        self.source_frames.len() > 1
    }

    fn source_kind(&self) -> SourceKind {
        SourceKind::from_frame_count(self.source_frames.len())
    }

    fn current_source_size(&self) -> Option<(u32, u32)> {
        self.source_frames
            .get(self.current_frame)
            .map(|src| (src.rgba.width(), src.rgba.height()))
    }

    fn canvas_policy(&self) -> CanvasPolicy {
        CanvasPolicy::for_content(self.display.capability(), self.source_kind())
    }

    fn colors(&self) -> &'static AppColors {
        if self.dark_mode {
            &DARK
        } else {
            &LIGHT
        }
    }

    fn lcd_size(&self) -> u32 {
        self.display.info().resolution.width
    }

    /// Render the full composited 240x240 RGBA (base + widgets).
    fn render_composited(&self) -> RgbaImage {
        let lcd = self.lcd_size();
        let base = if let Some(src) = self.source_frames.get(self.current_frame) {
            let vp = self.canvas.base_viewport();
            render_base_rgba(&src.rgba, self.display.info(), vp.zoom, vp.pan)
        } else {
            RgbaImage::from_pixel(lcd, lcd, Rgba([0, 0, 0, 255]))
        };
        self.canvas.composite(base, &self.widget_ctx)
    }

    fn rebuild_preview(&mut self) {
        let composited = self.render_composited();
        self.preview = Some(circular_preview_from_rgba(composited));
    }

    fn commit_frame(&mut self) {
        let composited = self.render_composited();
        self.display.submit_frame(&composited);
        self.preview = Some(circular_preview_from_rgba(composited));
    }

    fn edit_active_widget(&mut self, edit: WidgetEdit) {
        if self.canvas.edit_active_widget(edit) {
            self.commit_frame();
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
            .layers()
            .iter()
            .map(|layer| preset::WidgetLayerData {
                type_id: layer.type_id.to_string(),
                position: layer.position,
                size: layer.size,
                opacity: layer.opacity,
                config: serde_json::to_value(layer.widget.config())
                    .unwrap_or(serde_json::Value::Null),
            })
            .collect();

        preset::PresetData {
            version: 1,
            name: name.to_string(),
            background: bg,
            viewport: preset::ViewportData {
                zoom: self.canvas.base_viewport().zoom,
                pan: self.canvas.base_viewport().pan,
            },
            widgets,
        }
    }

    /// Apply a loaded preset's widget/viewport config to the current state.
    fn apply_preset_config(&mut self, data: &preset::PresetData) -> usize {
        // Restore base viewport
        self.canvas.set_base_viewport(Viewport {
            zoom: data.viewport.zoom,
            pan: data.viewport.pan,
        });

        // Clear existing widgets
        self.canvas.clear_widgets();

        // Recreate widgets from config
        let mut skipped_widgets = 0;
        for wd in &data.widgets {
            let Some(spec) = widget::spec_by_type_id(&wd.type_id) else {
                skipped_widgets += 1;
                continue;
            };
            let mut w = spec.create();
            if w.apply_config_value(&wd.config).is_err() {
                skipped_widgets += 1;
                continue;
            }
            self.canvas
                .add_configured_widget(spec, w, wd.position, wd.size, wd.opacity);
        }

        self.rebuild_preview();
        skipped_widgets
    }

    fn load_preset_folder(&mut self, folder: preset::PresetFolder, silent: bool) -> Task<Message> {
        let preset::LoadedPreset {
            data,
            background_path,
        } = match preset::load(&folder) {
            Ok(loaded) => loaded,
            Err(e) => {
                if !silent {
                    self.status_message = format!("Load failed: {e}");
                }
                return Task::none();
            }
        };

        if let Some(path) = background_path {
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
                    background_path: Some(path),
                    silent,
                },
            );
        }

        self.apply_loaded_preset(folder, data, None, None, silent);
        Task::none()
    }

    fn apply_loaded_preset(
        &mut self,
        folder: preset::PresetFolder,
        data: preset::PresetData,
        loaded_source: Option<LoadedData>,
        background_path: Option<PathBuf>,
        silent: bool,
    ) {
        let name = data.name.clone();

        if let Some(loaded) = loaded_source {
            let (frames, filename) = loaded.into_parts();
            self.filename = filename;
            self.source_frames = frames;

            self.selected_path = background_path;
        } else {
            self.source_frames.clear();
            self.selected_path = None;
            self.filename.clear();
        }

        self.current_preset_folder = Some(folder.clone());
        self.current_preset_name = Some(name.clone());
        self.current_frame = 0;
        self.last_advance = Instant::now();
        let skipped_widgets = self.apply_preset_config(&data);
        self.start_display();
        preset::remember_last_used(&folder);

        if !silent || self.status_message.is_empty() {
            self.status_message = if skipped_widgets == 0 {
                format!("Loaded preset '{name}'")
            } else {
                format!("Loaded preset '{name}' ({skipped_widgets} incompatible widget(s) skipped)")
            };
        }
    }

    /// Start (or restart) the device display thread.
    fn start_display(&mut self) {
        let composited = self.render_composited();
        self.display.restart(&composited);
    }

    fn stop_display(&mut self) {
        self.display.stop();
    }

    fn reap_display_session(&mut self) {
        self.display.join_finished();
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
                self.canvas.set_base_viewport(Viewport::default());
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
                        let count = data.frame_count();
                        let (frames, filename) = data.into_parts();
                        self.filename = filename;
                        self.source_frames = frames;

                        // On file-transfer devices, clear widgets when loading a GIF
                        let policy = CanvasPolicy::for_content(
                            self.display.capability(),
                            SourceKind::from_frame_count(count),
                        );
                        if !policy.widgets_allowed() && self.canvas.has_widgets() {
                            self.canvas.clear_widgets();
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
                        self.commit_frame();
                    }
                }
            }
            Message::WidgetTick => {
                // Refresh sysinfo backend
                let has_sysinfo_widgets = self.canvas.has_widgets_in_category("System Metrics");
                if has_sysinfo_widgets {
                    self.sysinfo_backend.refresh();
                    self.widget_ctx.sysinfo = self.sysinfo_backend.data().clone();
                }

                if self.canvas.tick_widgets(&self.widget_ctx) {
                    self.commit_frame();
                }
            }
            Message::Scroll(delta) => {
                let y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => y,
                    mouse::ScrollDelta::Pixels { y, .. } => y / 28.0,
                };
                let factor = 1.1_f32.powf(y);

                if self
                    .canvas
                    .zoom_active_layer(factor, self.current_source_size())
                {
                    self.commit_frame();
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
                        if self.canvas.drag_active_layer(
                            (dx, dy),
                            self.lcd_size(),
                            self.current_source_size(),
                        ) {
                            self.commit_frame();
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
                if self.canvas.reset_active_layer(self.lcd_size()) {
                    self.commit_frame();
                }
            }
            Message::SelectLayer(option) => {
                self.canvas.select_layer(option.selection);
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
                    self.canvas.select_layer(LayerSelection::Widget(id));
                    self.commit_frame();
                }
            }
            Message::RemoveWidget(id) => {
                self.canvas.remove_widget(id);
                self.commit_frame();
            }
            Message::SetWidgetOpacity(val) => {
                if self.canvas.set_active_widget_opacity((val * 255.0) as u8) {
                    self.commit_frame();
                }
            }
            Message::SetWidgetTextColor(color) => {
                self.edit_active_widget(WidgetEdit::Color(color));
            }
            Message::SetWidgetFont(name) => {
                self.edit_active_widget(WidgetEdit::Font(name));
            }
            Message::SetWidgetText(text) => {
                self.edit_active_widget(WidgetEdit::Text(text));
            }
            Message::ShowSaveDialog => {
                if self.current_preset_name.is_some() {
                    // Already have a preset loaded — save in-place
                    let name = self.current_preset_name.clone().unwrap();
                    let data = self.build_preset_data(&name);
                    let composited = self.render_composited();
                    match preset::save(
                        &name,
                        self.current_preset_folder.as_ref(),
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
                background_path,
                silent,
            } => {
                self.loading = false;
                match result {
                    Ok(loaded) => {
                        self.apply_loaded_preset(
                            folder,
                            data,
                            Some(loaded),
                            background_path,
                            silent,
                        );
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
                    if self.current_preset_folder.as_ref() == Some(&folder) {
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
            Message::DisplaySessionPoll => {
                self.reap_display_session();
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
        let has_dynamic = self
            .canvas
            .layers()
            .iter()
            .any(|layer| layer.widget.is_dynamic());
        if has_dynamic {
            subs.push(iced::time::every(Duration::from_secs(1)).map(|_| Message::WidgetTick));
        }

        if self.display.needs_lifecycle_poll() {
            subs.push(
                iced::time::every(Duration::from_millis(100)).map(|_| Message::DisplaySessionPoll),
            );
        }

        Subscription::batch(subs)
    }
}
