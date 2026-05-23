use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};
use imageproc::drawing::draw_text_mut;

use super::{fonts, WidgetSettings};

#[derive(Debug, Clone)]
pub(super) struct TextState {
    text: String,
    color: [u8; 4],
    font_name: String,
}

impl TextState {
    pub(super) fn new(text: impl Into<String>, color: [u8; 4]) -> Self {
        Self {
            text: text.into(),
            color,
            font_name: fonts::DEFAULT_FONT.to_string(),
        }
    }

    pub(super) fn render(&self, width: u32, height: u32, scale_factor: f32) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
        let font_data = fonts::font_data(&self.font_name);
        let font = FontRef::try_from_slice(font_data).unwrap();
        let scale = PxScale::from(height as f32 * scale_factor);

        let metrics = font.as_scaled(scale);
        let text_width: f32 = self
            .text
            .chars()
            .map(|c| metrics.h_advance(font.glyph_id(c)))
            .sum();
        let x = ((width as f32 - text_width) / 2.0).max(0.0) as i32;
        let y = ((height as f32 - height as f32 * scale_factor) / 2.0) as i32;

        draw_text_mut(&mut img, Rgba(self.color), x, y, scale, &font, &self.text);
        img
    }

    pub(super) fn settings(&self, include_text: bool) -> WidgetSettings {
        WidgetSettings {
            text: include_text.then(|| self.text.clone()),
            color: Some(self.color),
            font_name: Some(self.font_name.clone()),
            thickness: None,
        }
    }

    pub(super) fn apply_settings(&mut self, settings: &WidgetSettings, allow_text: bool) {
        if allow_text {
            if let Some(text) = settings.text.as_ref() {
                self.text = text.clone();
            }
        }
        if let Some(color) = settings.color {
            self.color = color;
        }
        if let Some(font_name) = settings.font_name.as_ref() {
            self.font_name = font_name.clone();
        }
    }

    pub(super) fn set_text(&mut self, text: String) -> bool {
        if text == self.text {
            false
        } else {
            self.text = text;
            true
        }
    }
}
