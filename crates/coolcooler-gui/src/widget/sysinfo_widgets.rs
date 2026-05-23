use image::RgbaImage;

use super::text::TextState;
use super::{LcdWidget, WidgetConfig, WidgetContext, WidgetDescriptor, WidgetEdit};

pub const CPU_USAGE_DESCRIPTOR: WidgetDescriptor = WidgetDescriptor {
    name: "CPU Usage",
    category: "System Metrics",
    default_size: (50, 30),
};

pub const CPU_TEMP_DESCRIPTOR: WidgetDescriptor = WidgetDescriptor {
    name: "CPU Temp",
    category: "System Metrics",
    default_size: (50, 30),
};

pub const RAM_USAGE_DESCRIPTOR: WidgetDescriptor = WidgetDescriptor {
    name: "RAM Usage",
    category: "System Metrics",
    default_size: (80, 30),
};

pub const GPU_TEMP_DESCRIPTOR: WidgetDescriptor = WidgetDescriptor {
    name: "GPU Temp",
    category: "System Metrics",
    default_size: (50, 30),
};

pub const GPU_USAGE_DESCRIPTOR: WidgetDescriptor = WidgetDescriptor {
    name: "GPU Usage",
    category: "System Metrics",
    default_size: (50, 30),
};

macro_rules! sysinfo_text_widget {
    (
        $name:ident,
        descriptor: $descriptor:ident,
        default_color: [$r:expr, $g:expr, $b:expr, $a:expr],
        initial_text: $initial:expr,
        tick: |$ctx:ident| $tick_body:expr
    ) => {
        #[derive(Debug)]
        pub struct $name {
            text: TextState,
        }

        impl $name {
            pub fn new() -> Self {
                Self {
                    text: TextState::new($initial, [$r, $g, $b, $a]),
                }
            }
        }

        impl LcdWidget for $name {
            fn descriptor(&self) -> &WidgetDescriptor {
                &$descriptor
            }

            fn render(&self, width: u32, height: u32, _ctx: &WidgetContext) -> RgbaImage {
                self.text.render(width, height, 0.65)
            }

            fn is_dynamic(&self) -> bool {
                true
            }

            fn tick(&mut self, $ctx: &WidgetContext) -> bool {
                let new_text: String = $tick_body;
                self.text.set_text(new_text)
            }

            fn config(&self) -> WidgetConfig {
                self.text.config(false)
            }

            fn apply_config(&mut self, config: &WidgetConfig) -> Result<(), &'static str> {
                self.text.apply_config(config, false)
            }

            fn apply_edit(&mut self, edit: WidgetEdit) {
                self.text.apply_edit(edit, false);
            }
        }
    };
}

sysinfo_text_widget!(
    CpuUsage,
    descriptor: CPU_USAGE_DESCRIPTOR,
    default_color: [80, 255, 80, 255],
    initial_text: "--",
    tick: |ctx| {
        format!("{:.0}", ctx.sysinfo.cpu_usage)
    }
);

sysinfo_text_widget!(
    CpuTemp,
    descriptor: CPU_TEMP_DESCRIPTOR,
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
    descriptor: RAM_USAGE_DESCRIPTOR,
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
    descriptor: GPU_TEMP_DESCRIPTOR,
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
    descriptor: GPU_USAGE_DESCRIPTOR,
    default_color: [0, 180, 255, 255],
    initial_text: "--",
    tick: |ctx| {
        match ctx.sysinfo.gpu_usage {
            Some(u) => format!("{:.0}", u),
            None => "N/A".to_string(),
        }
    }
);
