pub mod clock;
pub mod date;
pub mod fonts;
pub mod static_widgets;
pub mod sysinfo_backend;
pub mod sysinfo_widgets;

use image::RgbaImage;
use serde_json::Value;
use std::fmt;

/// Unique identifier for a widget instance on the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WidgetId(pub usize);

/// Metadata describing a widget type for the catalog UI.
pub struct WidgetDescriptor {
    /// Human-readable name shown in the catalog.
    pub name: &'static str,
    /// Category for the catalog dropdown.
    pub category: &'static str,
    /// Default size in LCD pixels when first placed.
    pub default_size: (u32, u32),
}

/// Shared context passed to widgets during render and tick.
/// Carries data from shared backends (sysinfo, etc.).
#[derive(Debug, Default)]
pub struct WidgetContext {
    pub sysinfo: sysinfo_backend::SysInfoData,
}

/// Trait for LCD canvas widgets.
///
/// A widget renders a rectangular RGBA buffer that gets composited
/// onto the LCD canvas. Widgets can be static (render is constant)
/// or dynamic (backed by a data source that updates via `tick`).
pub trait LcdWidget: fmt::Debug + Send {
    /// Descriptor for the widget catalog.
    fn descriptor(&self) -> &WidgetDescriptor;

    /// Render the widget at the given dimensions.
    /// Called on every preview rebuild — must be fast.
    fn render(&self, width: u32, height: u32, ctx: &WidgetContext) -> RgbaImage;

    /// Whether this widget has a dynamic backend that needs periodic updates.
    fn is_dynamic(&self) -> bool {
        false
    }

    /// Called periodically for dynamic widgets to refresh data.
    /// Returns true if the rendered output changed.
    fn tick(&mut self, ctx: &WidgetContext) -> bool {
        let _ = ctx;
        false
    }

    /// Whether this widget supports configurable text styling.
    fn supports_text_color(&self) -> bool {
        false
    }

    fn text_color(&self) -> [u8; 4] {
        [255, 255, 255, 255]
    }

    fn set_text_color(&mut self, _color: [u8; 4]) {}

    /// Whether this widget supports a configurable font.
    fn supports_font(&self) -> bool {
        false
    }

    /// Get the current font name.
    fn font_name(&self) -> &str {
        fonts::DEFAULT_FONT
    }

    /// Set the font by name.
    fn set_font_name(&mut self, _name: String) {}

    /// Whether this widget has editable text content (e.g., free text).
    fn supports_text_edit(&self) -> bool {
        false
    }

    /// Get the current editable text content.
    fn text_content(&self) -> &str {
        ""
    }

    /// Set the editable text content.
    fn set_text_content(&mut self, _text: String) {}

    /// Unique type identifier for serialization (e.g., "clock", "cpu_usage").
    fn type_id(&self) -> &'static str;

    /// Serialize widget-specific configuration to JSON.
    fn save_config(&self) -> Value {
        Value::Object(serde_json::Map::new())
    }

    /// Restore widget-specific configuration from JSON.
    fn load_config(&mut self, _config: &Value) {}

    /// Create a new independent instance of this widget type.
    fn create_instance(&self) -> Box<dyn LcdWidget>;
}

/// Create a widget instance from a type_id string.
/// Returns None if the type_id is unknown.
pub fn create_by_type_id(type_id: &str) -> Option<Box<dyn LcdWidget>> {
    match type_id {
        "free_text" => Some(Box::new(static_widgets::FreeText::new())),
        "h_line" => Some(Box::new(static_widgets::HorizontalLine::new())),
        "v_line" => Some(Box::new(static_widgets::VerticalLine::new())),
        "circle" => Some(Box::new(static_widgets::CircleGauge::new())),
        "filled_circle" => Some(Box::new(static_widgets::FilledCircle::new())),
        "clock" => Some(Box::new(clock::Clock::new())),
        "date" => Some(Box::new(date::DateWidget::new())),
        "cpu_usage" => Some(Box::new(sysinfo_widgets::CpuUsage::new())),
        "cpu_temp" => Some(Box::new(sysinfo_widgets::CpuTemp::new())),
        "ram_usage" => Some(Box::new(sysinfo_widgets::RamUsage::new())),
        "gpu_temp" => Some(Box::new(sysinfo_widgets::GpuTemp::new())),
        "gpu_usage" => Some(Box::new(sysinfo_widgets::GpuUsage::new())),
        _ => None,
    }
}

/// All available widget types for the catalog.
pub fn catalog() -> Vec<Box<dyn LcdWidget>> {
    vec![
        Box::new(static_widgets::FreeText::new()),
        Box::new(static_widgets::HorizontalLine::new()),
        Box::new(static_widgets::VerticalLine::new()),
        Box::new(static_widgets::CircleGauge::new()),
        Box::new(static_widgets::FilledCircle::new()),
        Box::new(clock::Clock::new()),
        Box::new(date::DateWidget::new()),
        Box::new(sysinfo_widgets::CpuUsage::new()),
        Box::new(sysinfo_widgets::CpuTemp::new()),
        Box::new(sysinfo_widgets::RamUsage::new()),
        Box::new(sysinfo_widgets::GpuTemp::new()),
        Box::new(sysinfo_widgets::GpuUsage::new()),
    ]
}

/// All unique category names, in display order.
pub fn categories(catalog: &[Box<dyn LcdWidget>]) -> Vec<&'static str> {
    let mut cats = Vec::new();
    for w in catalog {
        let cat = w.descriptor().category;
        if !cats.contains(&cat) {
            cats.push(cat);
        }
    }
    cats
}
