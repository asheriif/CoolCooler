mod providers;

use std::time::Instant;

use providers::{CpuProvider, GpuSensorFallback, NvidiaProvider, RamProvider};
use sysinfo::{Components, System};

#[cfg(target_os = "linux")]
use providers::AmdDrmProvider;

/// Shared system info readings, refreshed once per tick cycle.
#[derive(Debug, Default, Clone)]
pub struct SysInfoData {
    pub cpu_usage: f32,        // 0-100%
    pub cpu_temp: Option<f32>, // °C
    pub ram_used_gb: f32,
    pub ram_total_gb: f32,
    pub ram_percent: f32,       // 0-100%
    pub gpu_temp: Option<f32>,  // °C
    pub gpu_usage: Option<f32>, // 0-100%
}

/// Backend that owns the sysinfo::System and refreshes on demand.
pub struct SysInfoBackend {
    system: System,
    components: Components,
    nvidia: NvidiaProvider,
    last_refresh: Instant,
    data: SysInfoData,
}

impl SysInfoBackend {
    pub fn new() -> Self {
        let mut system = System::new();
        system.refresh_cpu_usage();
        system.refresh_memory();
        let components = Components::new_with_refreshed_list();

        // Initial CPU reading needs two samples.
        std::thread::sleep(std::time::Duration::from_millis(100));
        system.refresh_cpu_usage();

        let mut backend = Self {
            system,
            components,
            nvidia: NvidiaProvider::new(),
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

        self.data.cpu_usage = CpuProvider::usage(&self.system);
        self.data.cpu_temp = CpuProvider::temperature(&self.components);

        let nvidia_metrics = self.nvidia.metrics();
        self.data.gpu_temp = nvidia_metrics.and_then(|metrics| metrics.temp);
        self.data.gpu_usage = nvidia_metrics.and_then(|metrics| metrics.usage);

        #[cfg(target_os = "linux")]
        if nvidia_metrics.is_none()
            && (self.data.gpu_temp.is_none() || self.data.gpu_usage.is_none())
        {
            let metrics = AmdDrmProvider::metrics();
            if self.data.gpu_temp.is_none() {
                self.data.gpu_temp = metrics.temp;
            }
            if self.data.gpu_usage.is_none() {
                self.data.gpu_usage = metrics.usage;
            }
        }

        if self.data.gpu_temp.is_none() {
            self.data.gpu_temp = GpuSensorFallback::temperature(&self.components);
        }

        let (ram_used_gb, ram_total_gb, ram_percent) = RamProvider::usage(&self.system);
        self.data.ram_used_gb = ram_used_gb;
        self.data.ram_total_gb = ram_total_gb;
        self.data.ram_percent = ram_percent;

        self.last_refresh = Instant::now();

        (self.data.cpu_usage - old.cpu_usage).abs() > 0.5
            || self.data.cpu_temp != old.cpu_temp
            || self.data.gpu_temp != old.gpu_temp
            || self.data.gpu_usage != old.gpu_usage
            || (self.data.ram_percent - old.ram_percent).abs() > 0.1
    }

    pub fn data(&self) -> &SysInfoData {
        &self.data
    }
}
