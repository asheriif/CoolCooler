use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};
use imageproc::drawing::draw_text_mut;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use super::{fonts, TextWidgetConfig, WidgetControl, WidgetEdit};

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

    pub(super) fn config_value(&self, include_text: bool) -> Value {
        to_value(TextWidgetConfig {
            text: include_text.then(|| self.text.clone()),
            color: self.color,
            font_name: self.font_name.clone(),
        })
    }

    pub(super) fn controls(&self, allow_text: bool) -> Vec<WidgetControl<'_>> {
        let mut controls = vec![
            WidgetControl::Color(self.color),
            WidgetControl::Font(&self.font_name),
        ];
        if allow_text {
            controls.push(WidgetControl::Text(self.text.as_str()));
        }
        controls
    }

    pub(super) fn apply_preset_config(
        &mut self,
        config: &Value,
        allow_text: bool,
    ) -> Result<(), &'static str> {
        let config: TextWidgetConfig = from_value(config)?;

        if allow_text {
            if let Some(text) = config.text.as_ref() {
                self.text = text.clone();
            }
        }
        self.color = config.color;
        self.font_name = config.font_name.clone();
        Ok(())
    }

    pub(super) fn apply_edit(&mut self, edit: WidgetEdit, allow_text: bool) {
        match edit {
            WidgetEdit::Color(color) => {
                self.color = color;
            }
            WidgetEdit::Font(font_name) => {
                self.font_name = font_name;
            }
            WidgetEdit::Text(text) => {
                if allow_text {
                    self.text = text;
                }
            }
            WidgetEdit::Thickness(_) => {}
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

pub(super) fn to_value(config: impl Serialize) -> Value {
    serde_json::to_value(config).unwrap_or(Value::Null)
}

pub(super) fn from_value<T: DeserializeOwned>(config: &Value) -> Result<T, &'static str> {
    serde_json::from_value(config.clone()).map_err(|_| "invalid widget config")
}
