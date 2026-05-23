use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_circle_mut, draw_hollow_circle_mut, draw_text_mut};

use super::fonts;
use super::{LcdWidget, WidgetCapabilities, WidgetContext, WidgetDescriptor, WidgetSettings};

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
    text: String,
    color: [u8; 4],
    font_name: String,
}

impl FreeText {
    pub fn new() -> Self {
        Self {
            text: "Text".to_string(),
            color: [255, 255, 255, 255],
            font_name: fonts::DEFAULT_FONT.to_string(),
        }
    }
}

impl LcdWidget for FreeText {
    fn descriptor(&self) -> &WidgetDescriptor {
        &FREE_TEXT_DESCRIPTOR
    }

    fn render(&self, width: u32, height: u32, _ctx: &WidgetContext) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
        let font_data = fonts::font_data(&self.font_name);
        let font = FontRef::try_from_slice(font_data).unwrap();
        let scale = PxScale::from(height as f32 * 0.7);
        let color = Rgba(self.color);

        let metrics = font.as_scaled(scale);
        let text_width: f32 = self
            .text
            .chars()
            .map(|c| metrics.h_advance(font.glyph_id(c)))
            .sum();
        let x = ((width as f32 - text_width) / 2.0).max(0.0) as i32;
        let y = ((height as f32 - height as f32 * 0.7) / 2.0) as i32;

        draw_text_mut(&mut img, color, x, y, scale, &font, &self.text);
        img
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities {
            color: true,
            font: true,
            text: true,
        }
    }

    fn settings(&self) -> WidgetSettings {
        WidgetSettings {
            text: Some(self.text.clone()),
            color: Some(self.color),
            font_name: Some(self.font_name.clone()),
            thickness: None,
        }
    }

    fn apply_settings(&mut self, settings: &WidgetSettings) {
        if let Some(text) = settings.text.as_ref() {
            self.text = text.clone();
        }
        if let Some(color) = settings.color {
            self.color = color;
        }
        if let Some(font_name) = settings.font_name.as_ref() {
            self.font_name = font_name.clone();
        }
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

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities {
            color: true,
            ..Default::default()
        }
    }

    fn settings(&self) -> WidgetSettings {
        WidgetSettings {
            color: Some(self.color),
            ..Default::default()
        }
    }

    fn apply_settings(&mut self, settings: &WidgetSettings) {
        if let Some(color) = settings.color {
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

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities {
            color: true,
            ..Default::default()
        }
    }

    fn settings(&self) -> WidgetSettings {
        WidgetSettings {
            color: Some(self.color),
            ..Default::default()
        }
    }

    fn apply_settings(&mut self, settings: &WidgetSettings) {
        if let Some(color) = settings.color {
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

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities {
            color: true,
            ..Default::default()
        }
    }

    fn settings(&self) -> WidgetSettings {
        WidgetSettings {
            color: Some(self.color),
            thickness: Some(self.thickness),
            ..Default::default()
        }
    }

    fn apply_settings(&mut self, settings: &WidgetSettings) {
        if let Some(color) = settings.color {
            self.color = color;
        }
        if let Some(thickness) = settings.thickness {
            self.thickness = thickness;
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

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities {
            color: true,
            ..Default::default()
        }
    }

    fn settings(&self) -> WidgetSettings {
        WidgetSettings {
            color: Some(self.color),
            ..Default::default()
        }
    }

    fn apply_settings(&mut self, settings: &WidgetSettings) {
        if let Some(color) = settings.color {
            self.color = color;
        }
    }
}
