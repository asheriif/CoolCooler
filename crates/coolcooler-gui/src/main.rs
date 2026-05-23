mod canvas;
mod canvas_editor;
mod composition;
mod display_flow;
mod display_session;
mod preset;
mod preset_flow;
mod rendering;
mod source;
mod source_flow;
mod style;
mod tray;
mod update;
mod view;
mod viewport;
mod widget;
mod window_flow;
mod windowing;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use canvas::{Canvas, LayerSelection};
use display_session::DisplayController;
use iced::{mouse, window, Color, Element, Point, Subscription, Task, Theme};
use source::{LoadedData, SourceFrame};
use style::{AppColors, DARK, LIGHT};
use widget::{sysinfo_backend::SysInfoBackend, WidgetContext, WidgetSpec};
use windowing::{app_window_settings, ensure_single_instance};

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

    fn colors(&self) -> &'static AppColors {
        if self.dark_mode {
            &DARK
        } else {
            &LIGHT
        }
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
