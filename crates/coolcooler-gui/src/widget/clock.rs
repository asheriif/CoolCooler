use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use chrono::Local;
use image::{Rgba, RgbaImage};
use imageproc::drawing::draw_text_mut;
use serde_json::{json, Value};

use super::fonts;
use super::{LcdWidget, WidgetContext, WidgetDescriptor};

const DESCRIPTOR: WidgetDescriptor = WidgetDescriptor {
    name: "Time",
    category: "Datetime",
    default_size: (120, 40),
};

#[derive(Debug)]
pub struct Clock {
    text: String,
    color: [u8; 4],
    font_name: String,
}

impl Clock {
    pub fn new() -> Self {
        Self {
            text: Local::now().format("%H:%M:%S").to_string(),
            color: [255, 255, 255, 255],
            font_name: fonts::DEFAULT_FONT.to_string(),
        }
    }
}

impl LcdWidget for Clock {
    fn descriptor(&self) -> &WidgetDescriptor {
        &DESCRIPTOR
    }

    fn render(&self, width: u32, height: u32, _ctx: &WidgetContext) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
        let font_data = fonts::font_data(&self.font_name);
        let font = FontRef::try_from_slice(font_data).unwrap();
        let scale = PxScale::from(height as f32 * 0.7);
        let color = Rgba(self.color);

        let metrics = ab_glyph::Font::as_scaled(&font, scale);
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

    fn is_dynamic(&self) -> bool {
        true
    }

    fn tick(&mut self, _ctx: &WidgetContext) -> bool {
        let new_text = Local::now().format("%H:%M:%S").to_string();
        if new_text != self.text {
            self.text = new_text;
            true
        } else {
            false
        }
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
        "clock"
    }

    fn save_config(&self) -> Value {
        json!({ "color": self.color, "font": self.font_name })
    }

    fn load_config(&mut self, config: &Value) {
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

    fn create_instance(&self) -> Box<dyn LcdWidget> {
        Box::new(Clock::new())
    }
}
