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
use std::time::Duration;

use canvas::{Canvas, LayerSelection};
use canvas_editor::CanvasInteraction;
use display_session::DisplayController;
use iced::widget::image::Handle;
use iced::{mouse, window, Color, Element, Point, Subscription, Task, Theme};
use preset_flow::PresetState;
use source::{LoadedData, SourceLoadRequest, SourceState};
use style::{AppColors, DARK, LIGHT};
use widget::WidgetRuntime;
use window_flow::WindowState;
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
    ui: UiState,

    // Source data
    source: SourceState,

    // Canvas (layers + viewports)
    canvas: Canvas,

    // Interaction
    interaction: CanvasInteraction,

    // Widget catalog + backends
    widgets: WidgetRuntime,

    // Presets
    presets: PresetState,

    display: DisplayController,

    windows: WindowState,
}

struct UiState {
    dark_mode: bool,
    preview: Option<Handle>,
    status_message: String,
}

impl UiState {
    fn new() -> Self {
        Self {
            dark_mode: true,
            preview: None,
            status_message: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    SelectFile,
    FileSelected(Option<PathBuf>),
    SourceLoaded {
        request: SourceLoadRequest,
        result: Result<LoadedData, String>,
    },
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
    SetWidgetThickness(u32),
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
        request: SourceLoadRequest,
        result: Result<LoadedData, String>,
        job: preset_flow::PresetLoadJob,
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
            ui: UiState::new(),
            source: SourceState::new(),
            canvas: Canvas::new(),
            interaction: CanvasInteraction::new(),
            widgets: WidgetRuntime::new(),
            presets: PresetState::new(),
            display: DisplayController::new(),
            windows: WindowState::new(id, tray_handle, tray_rx),
        };
        app.rebuild_preview();
        preset::cleanup_stale_internal_dirs();

        let startup_task = preset::last_used_folder()
            .map(|folder| Task::done(Message::LoadLastPreset(folder)))
            .unwrap_or_else(Task::none);

        (app, Task::batch([open_task.discard(), startup_task]))
    }

    fn colors(&self) -> &'static AppColors {
        if self.ui.dark_mode {
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
        if self.source.is_animated() {
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
