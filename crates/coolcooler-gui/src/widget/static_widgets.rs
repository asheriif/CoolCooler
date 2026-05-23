use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_circle_mut, draw_hollow_circle_mut};

use super::text::{from_value, to_value, TextState};
use super::{
    CircleWidgetConfig, ColorWidgetConfig, LcdWidget, WidgetContext, WidgetControl,
    WidgetDescriptor, WidgetEdit,
};

const CIRCLE_MIN_THICKNESS: u32 = 1;
const CIRCLE_MAX_THICKNESS: u32 = 20;

// =============================================================================
// Free Text
// =============================================================================

pub const FREE_TEXT_DESCRIPTOR: WidgetDescriptor = WidgetDescriptor {
    name: "Free Text",
    category: "Static",
    default_size: (80, 28),
};

#[derive(Debug)]
pub struct FreeText {
    text: TextState,
}

impl FreeText {
    pub fn new() -> Self {
        Self {
            text: TextState::new("Text", [255, 255, 255, 255]),
        }
    }
}

impl LcdWidget for FreeText {
    fn descriptor(&self) -> &WidgetDescriptor {
        &FREE_TEXT_DESCRIPTOR
    }

    fn render(&self, width: u32, height: u32, _ctx: &WidgetContext) -> RgbaImage {
        self.text.render(width, height, 0.7)
    }

    fn preset_config(&self) -> serde_json::Value {
        self.text.config_value(true)
    }

    fn controls(&self) -> Vec<WidgetControl<'_>> {
        self.text.controls(true)
    }

    fn apply_preset_config(&mut self, config: &serde_json::Value) -> Result<(), &'static str> {
        self.text.apply_preset_config(config, true)
    }

    fn apply_edit(&mut self, edit: WidgetEdit) {
        self.text.apply_edit(edit, true);
    }
}

// =============================================================================
// Horizontal Line
// =============================================================================

pub const HORIZONTAL_LINE_DESCRIPTOR: WidgetDescriptor = WidgetDescriptor {
    name: "Horizontal Line",
    category: "Static",
    default_size: (120, 3),
};

#[derive(Debug)]
pub struct HorizontalLine {
    color: [u8; 4],
}

impl HorizontalLine {
    pub fn new() -> Self {
        Self {
            color: [255, 255, 255, 200],
        }
    }
}

impl LcdWidget for HorizontalLine {
    fn descriptor(&self) -> &WidgetDescriptor {
        &HORIZONTAL_LINE_DESCRIPTOR
    }

    fn render(&self, width: u32, height: u32, _ctx: &WidgetContext) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
        let color = Rgba(self.color);
        let mid_y = height / 2;
        for x in 0..width {
            img.put_pixel(x, mid_y, color);
            if height > 1 && mid_y > 0 {
                img.put_pixel(x, mid_y - 1, color);
            }
        }
        img
    }

    fn preset_config(&self) -> serde_json::Value {
        to_value(ColorWidgetConfig { color: self.color })
    }

    fn controls(&self) -> Vec<WidgetControl<'_>> {
        vec![WidgetControl::Color(self.color)]
    }

    fn apply_preset_config(&mut self, config: &serde_json::Value) -> Result<(), &'static str> {
        let config: ColorWidgetConfig = from_value(config)?;
        self.color = config.color;
        Ok(())
    }

    fn apply_edit(&mut self, edit: WidgetEdit) {
        if let WidgetEdit::Color(color) = edit {
            self.color = color;
        }
    }
}

// =============================================================================
// Vertical Line
// =============================================================================

pub const VERTICAL_LINE_DESCRIPTOR: WidgetDescriptor = WidgetDescriptor {
    name: "Vertical Line",
    category: "Static",
    default_size: (3, 120),
};

#[derive(Debug)]
pub struct VerticalLine {
    color: [u8; 4],
}

impl VerticalLine {
    pub fn new() -> Self {
        Self {
            color: [255, 255, 255, 200],
        }
    }
}

impl LcdWidget for VerticalLine {
    fn descriptor(&self) -> &WidgetDescriptor {
        &VERTICAL_LINE_DESCRIPTOR
    }

    fn render(&self, width: u32, height: u32, _ctx: &WidgetContext) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
        let color = Rgba(self.color);
        let mid_x = width / 2;
        for y in 0..height {
            img.put_pixel(mid_x, y, color);
            if width > 1 && mid_x > 0 {
                img.put_pixel(mid_x - 1, y, color);
            }
        }
        img
    }

    fn preset_config(&self) -> serde_json::Value {
        to_value(ColorWidgetConfig { color: self.color })
    }

    fn controls(&self) -> Vec<WidgetControl<'_>> {
        vec![WidgetControl::Color(self.color)]
    }

    fn apply_preset_config(&mut self, config: &serde_json::Value) -> Result<(), &'static str> {
        let config: ColorWidgetConfig = from_value(config)?;
        self.color = config.color;
        Ok(())
    }

    fn apply_edit(&mut self, edit: WidgetEdit) {
        if let WidgetEdit::Color(color) = edit {
            self.color = color;
        }
    }
}

// =============================================================================
// Circle (Gauge Ring)
// =============================================================================

pub const CIRCLE_GAUGE_DESCRIPTOR: WidgetDescriptor = WidgetDescriptor {
    name: "Circle",
    category: "Static",
    default_size: (60, 60),
};

#[derive(Debug)]
pub struct CircleGauge {
    color: [u8; 4],
    thickness: u32,
}

impl CircleGauge {
    pub fn new() -> Self {
        Self {
            color: [0, 180, 255, 220],
            thickness: 3,
        }
    }
}

impl LcdWidget for CircleGauge {
    fn descriptor(&self) -> &WidgetDescriptor {
        &CIRCLE_GAUGE_DESCRIPTOR
    }

    fn render(&self, width: u32, height: u32, _ctx: &WidgetContext) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
        let color = Rgba(self.color);
        let cx = width as i32 / 2;
        let cy = height as i32 / 2;
        let radius = (width.min(height) as i32 / 2) - 1;

        if radius <= 0 {
            return img;
        }

        // Draw concentric circles for thickness
        for t in 0..self.thickness as i32 {
            let r = radius - t;
            if r > 0 {
                draw_hollow_circle_mut(&mut img, (cx, cy), r, color);
            }
        }

        img
    }

    fn preset_config(&self) -> serde_json::Value {
        to_value(CircleWidgetConfig {
            color: self.color,
            thickness: self.thickness,
        })
    }

    fn controls(&self) -> Vec<WidgetControl<'_>> {
        vec![
            WidgetControl::Color(self.color),
            WidgetControl::Thickness {
                value: self.thickness,
                min: CIRCLE_MIN_THICKNESS,
                max: CIRCLE_MAX_THICKNESS,
            },
        ]
    }

    fn apply_preset_config(&mut self, config: &serde_json::Value) -> Result<(), &'static str> {
        let config: CircleWidgetConfig = from_value(config)?;
        self.color = config.color;
        self.thickness = config
            .thickness
            .clamp(CIRCLE_MIN_THICKNESS, CIRCLE_MAX_THICKNESS);
        Ok(())
    }

    fn apply_edit(&mut self, edit: WidgetEdit) {
        match edit {
            WidgetEdit::Color(color) => {
                self.color = color;
            }
            WidgetEdit::Thickness(thickness) => {
                self.thickness = thickness.clamp(CIRCLE_MIN_THICKNESS, CIRCLE_MAX_THICKNESS);
            }
            WidgetEdit::Font(_) | WidgetEdit::Text(_) => {}
        }
    }
}

// =============================================================================
// Filled Circle (Dot)
// =============================================================================

pub const FILLED_CIRCLE_DESCRIPTOR: WidgetDescriptor = WidgetDescriptor {
    name: "Filled Circle",
    category: "Static",
    default_size: (40, 40),
};

#[derive(Debug)]
pub struct FilledCircle {
    color: [u8; 4],
}

impl FilledCircle {
    pub fn new() -> Self {
        Self {
            color: [255, 255, 255, 200],
        }
    }
}

impl LcdWidget for FilledCircle {
    fn descriptor(&self) -> &WidgetDescriptor {
        &FILLED_CIRCLE_DESCRIPTOR
    }

    fn render(&self, width: u32, height: u32, _ctx: &WidgetContext) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
        let color = Rgba(self.color);
        let cx = width as i32 / 2;
        let cy = height as i32 / 2;
        let radius = (width.min(height) as i32 / 2) - 1;

        if radius > 0 {
            draw_filled_circle_mut(&mut img, (cx, cy), radius, color);
        }

        img
    }

    fn preset_config(&self) -> serde_json::Value {
        to_value(ColorWidgetConfig { color: self.color })
    }

    fn controls(&self) -> Vec<WidgetControl<'_>> {
        vec![WidgetControl::Color(self.color)]
    }

    fn apply_preset_config(&mut self, config: &serde_json::Value) -> Result<(), &'static str> {
        let config: ColorWidgetConfig = from_value(config)?;
        self.color = config.color;
        Ok(())
    }

    fn apply_edit(&mut self, edit: WidgetEdit) {
        if let WidgetEdit::Color(color) = edit {
            self.color = color;
        }
    }
}
