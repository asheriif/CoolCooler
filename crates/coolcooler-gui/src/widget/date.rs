use chrono::Local;
use image::RgbaImage;

use super::text::TextWidgetCore;
use super::{LcdWidget, WidgetContext, WidgetControl, WidgetDescriptor, WidgetEdit};

pub const DATE_DESCRIPTOR: WidgetDescriptor = WidgetDescriptor {
    name: "Date",
    category: "Datetime",
    default_size: (130, 35),
};

#[derive(Debug)]
pub struct DateWidget {
    text: TextWidgetCore,
}

impl DateWidget {
    pub fn new() -> Self {
        Self {
            text: TextWidgetCore::new(
                &DATE_DESCRIPTOR,
                Local::now().format("%b %d, %Y").to_string(),
                [220, 220, 220, 255],
                0.65,
                false,
                true,
            ),
        }
    }
}

impl LcdWidget for DateWidget {
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
        let new_text = Local::now().format("%b %d, %Y").to_string();
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
