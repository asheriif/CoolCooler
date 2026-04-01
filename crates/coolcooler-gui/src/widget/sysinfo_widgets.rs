use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};
use imageproc::drawing::draw_text_mut;
use serde_json::{json, Value};

use super::fonts;
use super::{LcdWidget, WidgetContext, WidgetDescriptor};

fn render_text_widget(
    text: &str,
    width: u32,
    height: u32,
    color: Rgba<u8>,
    font_name: &str,
) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
    let font_data = fonts::font_data(font_name);
    let font = FontRef::try_from_slice(font_data).unwrap();
    let scale = PxScale::from(height as f32 * 0.65);

    let metrics = font.as_scaled(scale);
    let text_width: f32 = text
        .chars()
        .map(|c| metrics.h_advance(font.glyph_id(c)))
        .sum();
    let x = ((width as f32 - text_width) / 2.0).max(0.0) as i32;
    let y = ((height as f32 - height as f32 * 0.65) / 2.0) as i32;

    draw_text_mut(&mut img, color, x, y, scale, &font, text);
    img
}

macro_rules! sysinfo_text_widget {
    (
        $name:ident,
        type_id: $tid:expr,
        desc_name: $desc_name:expr,
        default_size: ($dw:expr, $dh:expr),
        default_color: [$r:expr, $g:expr, $b:expr, $a:expr],
        initial_text: $initial:expr,
        tick: |$ctx:ident| $tick_body:expr
    ) => {
        #[derive(Debug)]
        pub struct $name {
            text: String,
            color: [u8; 4],
            font_name: String,
        }

        impl $name {
            pub fn new() -> Self {
                Self {
                    text: $initial.to_string(),
                    color: [$r, $g, $b, $a],
                    font_name: fonts::DEFAULT_FONT.to_string(),
                }
            }
        }

        impl LcdWidget for $name {
            fn descriptor(&self) -> &WidgetDescriptor {
                const DESC: WidgetDescriptor = WidgetDescriptor {
                    name: $desc_name,
                    category: "System Metrics",
                    default_size: ($dw, $dh),
                };
                &DESC
            }

            fn render(&self, width: u32, height: u32, _ctx: &WidgetContext) -> RgbaImage {
                render_text_widget(&self.text, width, height, Rgba(self.color), &self.font_name)
            }

            fn is_dynamic(&self) -> bool {
                true
            }

            fn tick(&mut self, $ctx: &WidgetContext) -> bool {
                let new_text: String = $tick_body;
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
                $tid
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
                Box::new($name::new())
            }
        }
    };
}

sysinfo_text_widget!(
    CpuUsage,
    type_id: "cpu_usage",
    desc_name: "CPU Usage",
    default_size: (50, 30),
    default_color: [80, 255, 80, 255],
    initial_text: "--",
    tick: |ctx| {
        format!("{:.0}", ctx.sysinfo.cpu_usage)
    }
);

sysinfo_text_widget!(
    CpuTemp,
    type_id: "cpu_temp",
    desc_name: "CPU Temp",
    default_size: (50, 30),
    default_color: [255, 80, 60, 255],
    initial_text: "--",
    tick: |ctx| {
        match ctx.sysinfo.cpu_temp {
            Some(t) => format!("{:.0}", t),
            None => "N/A".to_string(),
        }
    }
);

sysinfo_text_widget!(
    RamUsage,
    type_id: "ram_usage",
    desc_name: "RAM Usage",
    default_size: (80, 30),
    default_color: [255, 200, 50, 255],
    initial_text: "--/--",
    tick: |ctx| {
        format!(
            "{:.1}/{:.0}",
            ctx.sysinfo.ram_used_gb, ctx.sysinfo.ram_total_gb
        )
    }
);

sysinfo_text_widget!(
    GpuTemp,
    type_id: "gpu_temp",
    desc_name: "GPU Temp",
    default_size: (50, 30),
    default_color: [0, 180, 255, 255],
    initial_text: "--",
    tick: |ctx| {
        match ctx.sysinfo.gpu_temp {
            Some(t) => format!("{:.0}", t),
            None => "N/A".to_string(),
        }
    }
);

sysinfo_text_widget!(
    GpuUsage,
    type_id: "gpu_usage",
    desc_name: "GPU Usage",
    default_size: (50, 30),
    default_color: [0, 180, 255, 255],
    initial_text: "--",
    tick: |ctx| {
        match ctx.sysinfo.gpu_usage {
            Some(u) => format!("{:.0}", u),
            None => "N/A".to_string(),
        }
    }
);
