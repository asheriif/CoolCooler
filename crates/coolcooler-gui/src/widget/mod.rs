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
#[derive(Debug, Clone, Copy)]
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

    /// Serialize widget-specific configuration to JSON.
    fn save_config(&self) -> Value {
        Value::Object(serde_json::Map::new())
    }

    /// Restore widget-specific configuration from JSON.
    fn load_config(&mut self, _config: &Value) {}
}

pub type WidgetFactory = fn() -> Box<dyn LcdWidget>;

/// Canonical registry entry for a widget type.
#[derive(Clone, Copy)]
pub struct WidgetSpec {
    pub type_id: &'static str,
    pub descriptor: WidgetDescriptor,
    factory: WidgetFactory,
}

impl WidgetSpec {
    pub fn create(&self) -> Box<dyn LcdWidget> {
        (self.factory)()
    }
}

fn free_text() -> Box<dyn LcdWidget> {
    Box::new(static_widgets::FreeText::new())
}

fn horizontal_line() -> Box<dyn LcdWidget> {
    Box::new(static_widgets::HorizontalLine::new())
}

fn vertical_line() -> Box<dyn LcdWidget> {
    Box::new(static_widgets::VerticalLine::new())
}

fn circle_gauge() -> Box<dyn LcdWidget> {
    Box::new(static_widgets::CircleGauge::new())
}

fn filled_circle() -> Box<dyn LcdWidget> {
    Box::new(static_widgets::FilledCircle::new())
}

fn clock() -> Box<dyn LcdWidget> {
    Box::new(clock::Clock::new())
}

fn date() -> Box<dyn LcdWidget> {
    Box::new(date::DateWidget::new())
}

fn cpu_usage() -> Box<dyn LcdWidget> {
    Box::new(sysinfo_widgets::CpuUsage::new())
}

fn cpu_temp() -> Box<dyn LcdWidget> {
    Box::new(sysinfo_widgets::CpuTemp::new())
}

fn ram_usage() -> Box<dyn LcdWidget> {
    Box::new(sysinfo_widgets::RamUsage::new())
}

fn gpu_temp() -> Box<dyn LcdWidget> {
    Box::new(sysinfo_widgets::GpuTemp::new())
}

fn gpu_usage() -> Box<dyn LcdWidget> {
    Box::new(sysinfo_widgets::GpuUsage::new())
}

pub static WIDGET_REGISTRY: &[WidgetSpec] = &[
    WidgetSpec {
        type_id: "free_text",
        descriptor: static_widgets::FREE_TEXT_DESCRIPTOR,
        factory: free_text,
    },
    WidgetSpec {
        type_id: "h_line",
        descriptor: static_widgets::HORIZONTAL_LINE_DESCRIPTOR,
        factory: horizontal_line,
    },
    WidgetSpec {
        type_id: "v_line",
        descriptor: static_widgets::VERTICAL_LINE_DESCRIPTOR,
        factory: vertical_line,
    },
    WidgetSpec {
        type_id: "circle",
        descriptor: static_widgets::CIRCLE_GAUGE_DESCRIPTOR,
        factory: circle_gauge,
    },
    WidgetSpec {
        type_id: "filled_circle",
        descriptor: static_widgets::FILLED_CIRCLE_DESCRIPTOR,
        factory: filled_circle,
    },
    WidgetSpec {
        type_id: "clock",
        descriptor: clock::CLOCK_DESCRIPTOR,
        factory: clock,
    },
    WidgetSpec {
        type_id: "date",
        descriptor: date::DATE_DESCRIPTOR,
        factory: date,
    },
    WidgetSpec {
        type_id: "cpu_usage",
        descriptor: sysinfo_widgets::CPU_USAGE_DESCRIPTOR,
        factory: cpu_usage,
    },
    WidgetSpec {
        type_id: "cpu_temp",
        descriptor: sysinfo_widgets::CPU_TEMP_DESCRIPTOR,
        factory: cpu_temp,
    },
    WidgetSpec {
        type_id: "ram_usage",
        descriptor: sysinfo_widgets::RAM_USAGE_DESCRIPTOR,
        factory: ram_usage,
    },
    WidgetSpec {
        type_id: "gpu_temp",
        descriptor: sysinfo_widgets::GPU_TEMP_DESCRIPTOR,
        factory: gpu_temp,
    },
    WidgetSpec {
        type_id: "gpu_usage",
        descriptor: sysinfo_widgets::GPU_USAGE_DESCRIPTOR,
        factory: gpu_usage,
    },
];

pub fn spec_by_type_id(type_id: &str) -> Option<&'static WidgetSpec> {
    WIDGET_REGISTRY.iter().find(|spec| spec.type_id == type_id)
}

/// All available widget types for the catalog.
pub fn catalog() -> &'static [WidgetSpec] {
    WIDGET_REGISTRY
}

/// All unique category names, in display order.
pub fn categories(catalog: &[WidgetSpec]) -> Vec<&'static str> {
    let mut cats = Vec::new();
    for spec in catalog {
        let cat = spec.descriptor.category;
        if !cats.contains(&cat) {
            cats.push(cat);
        }
    }
    cats
}
