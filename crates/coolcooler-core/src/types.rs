use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Resolution {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn aspect_ratio(&self) -> f64 {
        self.width as f64 / self.height as f64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Rotation {
    #[default]
    None,
    Deg90,
    Deg180,
    Deg270,
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub resolution: Resolution,
    pub rotation: Rotation,
    pub target_fps: f64,
    pub keepalive_interval: Duration,
}
