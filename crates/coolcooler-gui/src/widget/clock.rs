use chrono::Local;
use image::RgbaImage;

use super::text::TextWidgetCore;
use super::{LcdWidget, WidgetContext, WidgetControl, WidgetDescriptor, WidgetEdit};

pub const CLOCK_DESCRIPTOR: WidgetDescriptor = WidgetDescriptor {
    name: "Time",
    category: "Datetime",
    default_size: (120, 40),
};

#[derive(Debug)]
pub struct Clock {
    text: TextWidgetCore,
}

impl Clock {
    pub fn new() -> Self {
        Self {
            text: TextWidgetCore::new(
                &CLOCK_DESCRIPTOR,
                Local::now().format("%H:%M:%S").to_string(),
                [255, 255, 255, 255],
                0.7,
                false,
                true,
            ),
        }
    }
}

impl LcdWidget for Clock {
    fn descriptor(&self) -> &WidgetDescriptor {
        self.text.descriptor()
    }

    fn render(&self, width: u32, height: u32, _ctx: &WidgetContext) -> RgbaImage {
        self.text.render(width, height)
    }

    fn is_dynamic(&self) -> bool {
        self.text.is_dynamic()
    }

    fn tick(&mut self, _ctx: &WidgetContext) -> bool {
        let new_text = Local::now().format("%H:%M:%S").to_string();
        self.text.set_text(new_text)
    }

    fn preset_config(&self) -> serde_json::Value {
        self.text.preset_config()
    }

    fn controls(&self) -> Vec<WidgetControl<'_>> {
        self.text.controls()
    }

    fn apply_preset_config(&mut self, config: &serde_json::Value) -> Result<(), &'static str> {
        self.text.apply_preset_config(config)
    }

    fn apply_edit(&mut self, edit: WidgetEdit) {
        self.text.apply_edit(edit);
    }
}
