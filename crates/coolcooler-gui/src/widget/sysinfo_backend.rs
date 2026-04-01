use nvml_wrapper::Nvml;
use sysinfo::{Components, System};
use std::time::Instant;

/// Shared system info readings, refreshed once per tick cycle.
#[derive(Debug, Default, Clone)]
pub struct SysInfoData {
    pub cpu_usage: f32,          // 0-100%
    pub cpu_temp: Option<f32>,   // °C
    pub ram_used_gb: f32,
    pub ram_total_gb: f32,
    pub ram_percent: f32,        // 0-100%
    pub gpu_temp: Option<f32>,   // °C
    pub gpu_usage: Option<f32>,  // 0-100%
}

/// Backend that owns the sysinfo::System and refreshes on demand.
pub struct SysInfoBackend {
    system: System,
    components: Components,
    nvml: Option<Nvml>,
    last_refresh: Instant,
    data: SysInfoData,
}

impl SysInfoBackend {
    pub fn new() -> Self {
        let mut system = System::new();
        system.refresh_cpu_usage();
        system.refresh_memory();
        let components = Components::new_with_refreshed_list();

        // Try to initialize NVML (fails gracefully if no NVIDIA GPU)
        let nvml = Nvml::init().ok();

        // Initial CPU reading needs two samples — do an early refresh
        std::thread::sleep(std::time::Duration::from_millis(100));
        system.refresh_cpu_usage();

        let mut backend = Self {
            system,
            components,
            nvml,
            last_refresh: Instant::now(),
            data: SysInfoData::default(),
        };
        backend.refresh();
        backend
    }

    /// Refresh all metrics. Returns true if data changed meaningfully.
    pub fn refresh(&mut self) -> bool {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.components.refresh(true);

        let old = self.data.clone();

        // CPU usage (average across all cores)
        let cpus = self.system.cpus();
        if !cpus.is_empty() {
            self.data.cpu_usage = cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpus.len() as f32;
        }

        // CPU temp — look for coretemp, k10temp, or generic CPU sensor
        self.data.cpu_temp = None;
        for comp in self.components.iter() {
            let label = comp.label().to_lowercase();
            if label.contains("core") || label.contains("tctl") || label.contains("cpu") || label.contains("package") {
                if let Some(temp) = comp.temperature() {
                    if self.data.cpu_temp.map_or(true, |t| temp > t) {
                        self.data.cpu_temp = Some(temp);
                    }
                }
            }
        }

        // GPU temp — first try sysinfo (works for AMD GPUs via hwmon)
        self.data.gpu_temp = None;
        for comp in self.components.iter() {
            let label = comp.label().to_lowercase();
            if label.contains("gpu") || label.contains("edge") || label.contains("junction") {
                if let Some(temp) = comp.temperature() {
                    if self.data.gpu_temp.map_or(true, |t| temp > t) {
                        self.data.gpu_temp = Some(temp);
                    }
                }
            }
        }

        // NVIDIA GPU metrics via NVML (temp + usage)
        if let Some(ref nvml) = self.nvml {
            if let Ok(device) = nvml.device_by_index(0) {
                if self.data.gpu_temp.is_none() {
                    if let Ok(temp) = device.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu) {
                        self.data.gpu_temp = Some(temp as f32);
                    }
                }
                if let Ok(utilization) = device.utilization_rates() {
                    self.data.gpu_usage = Some(utilization.gpu as f32);
                }
            }
        }

        // RAM
        let total = self.system.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
        let used = self.system.used_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
        self.data.ram_total_gb = total as f32;
        self.data.ram_used_gb = used as f32;
        self.data.ram_percent = if total > 0.0 { (used / total * 100.0) as f32 } else { 0.0 };

        self.last_refresh = Instant::now();

        // Check if anything changed meaningfully
        (self.data.cpu_usage - old.cpu_usage).abs() > 0.5
            || self.data.cpu_temp != old.cpu_temp
            || self.data.gpu_temp != old.gpu_temp
            || (self.data.ram_percent - old.ram_percent).abs() > 0.1
    }

    pub fn data(&self) -> &SysInfoData {
        &self.data
    }
}
