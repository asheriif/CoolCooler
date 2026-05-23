use chrono::Local;
use image::RgbaImage;

use super::text::TextState;
use super::{LcdWidget, WidgetCapabilities, WidgetContext, WidgetDescriptor, WidgetSettings};

pub const DATE_DESCRIPTOR: WidgetDescriptor = WidgetDescriptor {
    name: "Date",
    category: "Datetime",
    default_size: (130, 35),
};

#[derive(Debug)]
pub struct DateWidget {
    text: TextState,
}

impl DateWidget {
    pub fn new() -> Self {
        Self {
            text: TextState::new(
                Local::now().format("%b %d, %Y").to_string(),
                [220, 220, 220, 255],
            ),
        }
    }
}

impl LcdWidget for DateWidget {
    fn descriptor(&self) -> &WidgetDescriptor {
        &DATE_DESCRIPTOR
    }

    fn render(&self, width: u32, height: u32, _ctx: &WidgetContext) -> RgbaImage {
        self.text.render(width, height, 0.65)
    }

    fn is_dynamic(&self) -> bool {
        true
    }

    fn tick(&mut self, _ctx: &WidgetContext) -> bool {
        let new_text = Local::now().format("%b %d, %Y").to_string();
        self.text.set_text(new_text)
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities {
            color: true,
            font: true,
            ..Default::default()
        }
    }

    fn settings(&self) -> WidgetSettings {
        self.text.settings(false)
    }

    fn apply_settings(&mut self, settings: &WidgetSettings) {
        self.text.apply_settings(settings, false);
    }
}
