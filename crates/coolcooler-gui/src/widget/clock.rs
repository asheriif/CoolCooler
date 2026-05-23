use chrono::Local;
use image::RgbaImage;

use super::text::TextState;
use super::{LcdWidget, WidgetContext, WidgetControl, WidgetDescriptor, WidgetEdit};

pub const CLOCK_DESCRIPTOR: WidgetDescriptor = WidgetDescriptor {
    name: "Time",
    category: "Datetime",
    default_size: (120, 40),
};

#[derive(Debug)]
pub struct Clock {
    text: TextState,
}

impl Clock {
    pub fn new() -> Self {
        Self {
            text: TextState::new(
                Local::now().format("%H:%M:%S").to_string(),
                [255, 255, 255, 255],
            ),
        }
    }
}

impl LcdWidget for Clock {
    fn descriptor(&self) -> &WidgetDescriptor {
        &CLOCK_DESCRIPTOR
    }

    fn render(&self, width: u32, height: u32, _ctx: &WidgetContext) -> RgbaImage {
        self.text.render(width, height, 0.7)
    }

    fn is_dynamic(&self) -> bool {
        true
    }

    fn tick(&mut self, _ctx: &WidgetContext) -> bool {
        let new_text = Local::now().format("%H:%M:%S").to_string();
        self.text.set_text(new_text)
    }

    fn preset_config(&self) -> serde_json::Value {
        self.text.config_value(false)
    }

    fn controls(&self) -> Vec<WidgetControl<'_>> {
        self.text.controls(false)
    }

    fn apply_preset_config(&mut self, config: &serde_json::Value) -> Result<(), &'static str> {
        self.text.apply_preset_config(config, false)
    }

    fn apply_edit(&mut self, edit: WidgetEdit) {
        self.text.apply_edit(edit, false);
    }
}
