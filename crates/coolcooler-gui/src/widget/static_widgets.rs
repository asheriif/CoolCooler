use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_circle_mut, draw_hollow_circle_mut, draw_text_mut};
use serde_json::{json, Value};

use super::fonts;
use super::{LcdWidget, WidgetContext, WidgetDescriptor};

// =============================================================================
// Free Text
// =============================================================================

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
        const DESC: WidgetDescriptor = WidgetDescriptor {
            name: "Free Text",
            category: "Static",
            default_size: (80, 28),
        };
        &DESC
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

    fn supports_text_color(&self) -> bool {
        true
    }

    fn text_color(&self) -> [u8; 4] {
        self.color
    }

    fn set_text_color(&mut self, color: [u8; 4]) {
        self.color = color;
    }

    fn supports_font(&self) -> bool {
        true
    }

    fn font_name(&self) -> &str {
        &self.font_name
    }

    fn set_font_name(&mut self, name: String) {
        self.font_name = name;
    }

    fn type_id(&self) -> &'static str {
        "free_text"
    }

    fn save_config(&self) -> Value {
        json!({ "text": self.text, "color": self.color, "font": self.font_name })
    }

    fn load_config(&mut self, config: &Value) {
        if let Some(t) = config.get("text").and_then(|v| v.as_str()) {
            self.text = t.to_string();
        }
        if let Some(arr) = config.get("color").and_then(|v| v.as_array()) {
            if arr.len() == 4 {
                self.color = [
                    arr[0].as_u64().unwrap_or(255) as u8,
                    arr[1].as_u64().unwrap_or(255) as u8,
                    arr[2].as_u64().unwrap_or(255) as u8,
                    arr[3].as_u64().unwrap_or(255) as u8,
                ];
            }
        }
        if let Some(f) = config.get("font").and_then(|v| v.as_str()) {
            self.font_name = f.to_string();
        }
    }

    /// Free text has editable content.
    fn supports_text_edit(&self) -> bool {
        true
    }

    fn text_content(&self) -> &str {
        &self.text
    }

    fn set_text_content(&mut self, text: String) {
        self.text = text;
    }

    fn create_instance(&self) -> Box<dyn LcdWidget> {
        Box::new(FreeText::new())
    }
}

// =============================================================================
// Horizontal Line
// =============================================================================

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
        const DESC: WidgetDescriptor = WidgetDescriptor {
            name: "Horizontal Line",
            category: "Static",
            default_size: (120, 3),
        };
        &DESC
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

    fn supports_text_color(&self) -> bool {
        true
    }

    fn text_color(&self) -> [u8; 4] {
        self.color
    }

    fn set_text_color(&mut self, color: [u8; 4]) {
        self.color = color;
    }

    fn type_id(&self) -> &'static str {
        "h_line"
    }

    fn save_config(&self) -> Value {
        json!({ "color": self.color })
    }

    fn load_config(&mut self, config: &Value) {
        if let Some(arr) = config.get("color").and_then(|v| v.as_array()) {
            if arr.len() == 4 {
                self.color = [
                    arr[0].as_u64().unwrap_or(255) as u8,
                    arr[1].as_u64().unwrap_or(255) as u8,
                    arr[2].as_u64().unwrap_or(255) as u8,
                    arr[3].as_u64().unwrap_or(200) as u8,
                ];
            }
        }
    }

    fn create_instance(&self) -> Box<dyn LcdWidget> {
        Box::new(HorizontalLine::new())
    }
}

// =============================================================================
// Vertical Line
// =============================================================================

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
        const DESC: WidgetDescriptor = WidgetDescriptor {
            name: "Vertical Line",
            category: "Static",
            default_size: (3, 120),
        };
        &DESC
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

    fn supports_text_color(&self) -> bool {
        true
    }

    fn text_color(&self) -> [u8; 4] {
        self.color
    }

    fn set_text_color(&mut self, color: [u8; 4]) {
        self.color = color;
    }

    fn type_id(&self) -> &'static str {
        "v_line"
    }

    fn save_config(&self) -> Value {
        json!({ "color": self.color })
    }

    fn load_config(&mut self, config: &Value) {
        if let Some(arr) = config.get("color").and_then(|v| v.as_array()) {
            if arr.len() == 4 {
                self.color = [
                    arr[0].as_u64().unwrap_or(255) as u8,
                    arr[1].as_u64().unwrap_or(255) as u8,
                    arr[2].as_u64().unwrap_or(255) as u8,
                    arr[3].as_u64().unwrap_or(200) as u8,
                ];
            }
        }
    }

    fn create_instance(&self) -> Box<dyn LcdWidget> {
        Box::new(VerticalLine::new())
    }
}

// =============================================================================
// Circle (Gauge Ring)
// =============================================================================

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
        const DESC: WidgetDescriptor = WidgetDescriptor {
            name: "Circle",
            category: "Static",
            default_size: (60, 60),
        };
        &DESC
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

    fn supports_text_color(&self) -> bool {
        true
    }

    fn text_color(&self) -> [u8; 4] {
        self.color
    }

    fn set_text_color(&mut self, color: [u8; 4]) {
        self.color = color;
    }

    fn type_id(&self) -> &'static str {
        "circle"
    }

    fn save_config(&self) -> Value {
        json!({ "color": self.color, "thickness": self.thickness })
    }

    fn load_config(&mut self, config: &Value) {
        if let Some(arr) = config.get("color").and_then(|v| v.as_array()) {
            if arr.len() == 4 {
                self.color = [
                    arr[0].as_u64().unwrap_or(0) as u8,
                    arr[1].as_u64().unwrap_or(180) as u8,
                    arr[2].as_u64().unwrap_or(255) as u8,
                    arr[3].as_u64().unwrap_or(220) as u8,
                ];
            }
        }
        if let Some(t) = config.get("thickness").and_then(|v| v.as_u64()) {
            self.thickness = t as u32;
        }
    }

    fn create_instance(&self) -> Box<dyn LcdWidget> {
        Box::new(CircleGauge::new())
    }
}

// =============================================================================
// Filled Circle (Dot)
// =============================================================================

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
        const DESC: WidgetDescriptor = WidgetDescriptor {
            name: "Filled Circle",
            category: "Static",
            default_size: (40, 40),
        };
        &DESC
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

    fn supports_text_color(&self) -> bool {
        true
    }

    fn text_color(&self) -> [u8; 4] {
        self.color
    }

    fn set_text_color(&mut self, color: [u8; 4]) {
        self.color = color;
    }

    fn type_id(&self) -> &'static str {
        "filled_circle"
    }

    fn save_config(&self) -> Value {
        json!({ "color": self.color })
    }

    fn load_config(&mut self, config: &Value) {
        if let Some(arr) = config.get("color").and_then(|v| v.as_array()) {
            if arr.len() == 4 {
                self.color = [
                    arr[0].as_u64().unwrap_or(255) as u8,
                    arr[1].as_u64().unwrap_or(255) as u8,
                    arr[2].as_u64().unwrap_or(255) as u8,
                    arr[3].as_u64().unwrap_or(200) as u8,
                ];
            }
        }
    }

    fn create_instance(&self) -> Box<dyn LcdWidget> {
        Box::new(FilledCircle::new())
    }
}
