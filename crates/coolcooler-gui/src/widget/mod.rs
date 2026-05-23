pub mod clock;
pub mod date;
pub mod fonts;
pub mod static_widgets;
pub mod sysinfo_backend;
pub mod sysinfo_widgets;
mod text;

use image::RgbaImage;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetControl<'a> {
    Color([u8; 4]),
    Font(&'a str),
    Text(&'a str),
    Thickness { value: u32, min: u32, max: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextWidgetConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub color: [u8; 4],
    #[serde(rename = "font")]
    pub font_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorWidgetConfig {
    pub color: [u8; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CircleWidgetConfig {
    pub color: [u8; 4],
    pub thickness: u32,
}

#[derive(Debug, Clone)]
pub enum WidgetEdit {
    Color([u8; 4]),
    Font(String),
    Text(String),
    Thickness(u32),
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

    fn preset_config(&self) -> Value {
        Value::Null
    }

    fn controls(&self) -> Vec<WidgetControl<'_>> {
        Vec::new()
    }

    fn apply_preset_config(&mut self, config: &Value) -> Result<(), &'static str> {
        if config.is_null() {
            Ok(())
        } else {
            Err("widget does not accept this config")
        }
    }

    fn apply_edit(&mut self, _edit: WidgetEdit) {}
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn text_widget_config_keeps_existing_preset_shape() {
        let config = TextWidgetConfig {
            text: Some("Label".to_string()),
            color: [255, 255, 255, 255],
            font_name: "default".to_string(),
        };

        let value = serde_json::to_value(&config).unwrap();
        assert_eq!(
            value,
            json!({
                "text": "Label",
                "color": [255, 255, 255, 255],
                "font": "default"
            })
        );
    }

    #[test]
    fn widget_config_is_owned_by_widget_type() {
        let line = static_widgets::HorizontalLine::new();
        assert_eq!(
            line.preset_config(),
            json!({
                "color": [255, 255, 255, 200]
            })
        );

        let circle = static_widgets::CircleGauge::new();
        assert_eq!(
            circle.preset_config(),
            json!({
                "color": [0, 180, 255, 220],
                "thickness": 3
            })
        );
    }

    #[test]
    fn widget_rejects_mismatched_config_shape() {
        let mut line = static_widgets::HorizontalLine::new();

        assert!(line
            .apply_preset_config(&json!({
                "text": "wrong",
                "color": [255, 255, 255, 255],
                "font": "default"
            }))
            .is_err());
    }

    #[test]
    fn widget_rejects_malformed_raw_config() {
        let mut line = static_widgets::HorizontalLine::new();

        assert!(line
            .apply_preset_config(&json!({
                "color": [255, 255, 255, 255],
                "future": true
            }))
            .is_err());
    }

    #[test]
    fn widget_controls_are_explicit_for_dynamic_text() {
        let clock = clock::Clock::new();
        let controls = clock.controls();

        assert_eq!(
            controls,
            vec![
                WidgetControl::Color([255, 255, 255, 255]),
                WidgetControl::Font(fonts::DEFAULT_FONT)
            ]
        );
        assert_eq!(
            clock.preset_config(),
            json!({
                "color": [255, 255, 255, 255],
                "font": fonts::DEFAULT_FONT
            })
        );
    }

    #[test]
    fn free_text_exposes_editable_text_control() {
        let free_text = static_widgets::FreeText::new();
        let controls = free_text.controls();

        assert!(controls.contains(&WidgetControl::Text("Text")));
    }

    #[test]
    fn circle_exposes_thickness_control() {
        let circle = static_widgets::CircleGauge::new();
        let controls = circle.controls();

        assert!(controls.contains(&WidgetControl::Thickness {
            value: 3,
            min: 1,
            max: 20,
        }));
    }
}
