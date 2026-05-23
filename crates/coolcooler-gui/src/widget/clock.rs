use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use chrono::Local;
use image::{Rgba, RgbaImage};
use imageproc::drawing::draw_text_mut;

use super::fonts;
use super::{LcdWidget, WidgetCapabilities, WidgetContext, WidgetDescriptor, WidgetSettings};

pub const CLOCK_DESCRIPTOR: WidgetDescriptor = WidgetDescriptor {
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
        &CLOCK_DESCRIPTOR
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

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities {
            color: true,
            font: true,
            ..Default::default()
        }
    }

    fn settings(&self) -> WidgetSettings {
        WidgetSettings {
            color: Some(self.color),
            font_name: Some(self.font_name.clone()),
            ..Default::default()
        }
    }

    fn apply_settings(&mut self, settings: &WidgetSettings) {
        if let Some(color) = settings.color {
            self.color = color;
        }
        if let Some(font_name) = settings.font_name.as_ref() {
            self.font_name = font_name.clone();
        }
    }
}
