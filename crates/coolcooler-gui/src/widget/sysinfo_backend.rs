use nvml_wrapper::Nvml;
use std::time::Instant;
use sysinfo::{Components, System};

#[cfg(target_os = "linux")]
use std::{
    fs,
    path::{Path, PathBuf},
};

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
            self.data.cpu_usage =
                cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpus.len() as f32;
        }

        // CPU temp — look for coretemp, k10temp, or generic CPU sensor
        self.data.cpu_temp = None;
        for comp in self.components.iter() {
            let label = comp.label().to_lowercase();
            if label.contains("core")
                || label.contains("tctl")
                || label.contains("cpu")
                || label.contains("package")
            {
                if let Some(temp) = comp.temperature() {
                    if self.data.cpu_temp.is_none_or(|t| temp > t) {
                        self.data.cpu_temp = Some(temp);
                    }
                }
            }
        }

        self.data.gpu_temp = None;
        self.data.gpu_usage = None;

        // NVIDIA GPU metrics via NVML (temp + usage)
        #[cfg(target_os = "linux")]
        let mut nvidia_device_found = false;
        if let Some(ref nvml) = self.nvml {
            if let Ok(device) = nvml.device_by_index(0) {
                #[cfg(target_os = "linux")]
                {
                    nvidia_device_found = true;
                }
                if let Ok(temp) =
                    device.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
                {
                    self.data.gpu_temp = Some(temp as f32);
                }
                if let Ok(utilization) = device.utilization_rates() {
                    self.data.gpu_usage = Some(utilization.gpu as f32);
                }
            }
        }

        // AMD GPU metrics via Linux DRM/sysfs. This keeps temp and usage tied to
        // the same adapter and uses edge temp instead of hotspot/junction temp.
        #[cfg(target_os = "linux")]
        if !nvidia_device_found && (self.data.gpu_temp.is_none() || self.data.gpu_usage.is_none()) {
            let metrics = read_linux_gpu_metrics();
            if self.data.gpu_temp.is_none() {
                self.data.gpu_temp = metrics.temp;
            }
            if self.data.gpu_usage.is_none() {
                self.data.gpu_usage = metrics.usage;
            }
        }

        // GPU temp fallback — use sysinfo hwmon labels, but do not treat AMD
        // junction/hotspot/memory sensors as the generic GPU temperature.
        if self.data.gpu_temp.is_none() {
            for comp in self.components.iter() {
                let label = comp.label().to_lowercase();
                if is_normal_gpu_temp_label(&label) {
                    if let Some(temp) = comp.temperature() {
                        if self.data.gpu_temp.is_none_or(|t| temp > t) {
                            self.data.gpu_temp = Some(temp);
                        }
                    }
                }
            }
        }

        // RAM
        let total = self.system.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
        let used = self.system.used_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
        self.data.ram_total_gb = total as f32;
        self.data.ram_used_gb = used as f32;
        self.data.ram_percent = if total > 0.0 {
            (used / total * 100.0) as f32
        } else {
            0.0
        };

        self.last_refresh = Instant::now();

        // Check if anything changed meaningfully
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

fn is_normal_gpu_temp_label(label: &str) -> bool {
    (label.contains("gpu") || label.contains("edge"))
        && !label.contains("junction")
        && !label.contains("hotspot")
        && !label.contains("hot spot")
        && !label.contains("memory")
        && !label.contains("mem")
}

#[cfg(target_os = "linux")]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
struct GpuMetrics {
    temp: Option<f32>,
    usage: Option<f32>,
}

#[cfg(target_os = "linux")]
fn read_linux_gpu_metrics() -> GpuMetrics {
    read_linux_gpu_metrics_from_drm(Path::new("/sys/class/drm"))
}

#[cfg(target_os = "linux")]
fn read_linux_gpu_metrics_from_drm(drm_path: &Path) -> GpuMetrics {
    let mut amd_devices = Vec::new();

    for card_path in sorted_child_paths(drm_path) {
        let Some(name) = card_path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !is_drm_card_name(name) {
            continue;
        }

        let device_path = card_path.join("device");
        if read_trimmed(device_path.join("vendor")).as_deref() == Some("0x1002") {
            amd_devices.push(device_path);
        }
    }

    for device_path in amd_devices {
        let metrics = read_amd_gpu_metrics_from_device(&device_path);
        if metrics.temp.is_some() || metrics.usage.is_some() {
            return metrics;
        }
    }

    GpuMetrics::default()
}

#[cfg(target_os = "linux")]
fn is_drm_card_name(name: &str) -> bool {
    name.strip_prefix("card")
        .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(target_os = "linux")]
fn read_amd_gpu_metrics_from_device(device_path: &Path) -> GpuMetrics {
    GpuMetrics {
        temp: read_amd_gpu_edge_temp(device_path),
        usage: read_percent_file(device_path.join("gpu_busy_percent")),
    }
}

#[cfg(target_os = "linux")]
fn read_amd_gpu_edge_temp(device_path: &Path) -> Option<f32> {
    let hwmon_root = device_path.join("hwmon");
    let mut edge = None;
    let mut temp1 = None;
    let mut gpu = None;
    let mut fallback = None;

    for hwmon_path in sorted_child_paths(&hwmon_root) {
        if !hwmon_path.is_dir() {
            continue;
        }

        for input_path in sorted_child_paths(&hwmon_path) {
            let Some(file_name) = input_path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some(temp_id) = temp_input_id(file_name) else {
                continue;
            };
            let Some(temp) = read_milli_celsius_file(&input_path) else {
                continue;
            };

            let label = read_trimmed(hwmon_path.join(format!("temp{temp_id}_label")))
                .unwrap_or_default()
                .to_lowercase();

            if label == "edge" {
                edge = Some(temp);
            } else if temp_id == "1" && !is_hotspot_or_memory_label(&label) {
                temp1 = Some(temp);
            } else if is_normal_gpu_temp_label(&label) {
                gpu = gpu.or(Some(temp));
            } else if label.is_empty() {
                fallback = fallback.or(Some(temp));
            }
        }
    }

    edge.or(temp1).or(gpu).or(fallback)
}

#[cfg(target_os = "linux")]
fn temp_input_id(file_name: &str) -> Option<&str> {
    let (id, suffix) = file_name.strip_prefix("temp")?.split_once('_')?;
    if suffix == "input" && !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
        Some(id)
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
fn is_hotspot_or_memory_label(label: &str) -> bool {
    label.contains("junction")
        || label.contains("hotspot")
        || label.contains("hot spot")
        || label.contains("memory")
        || label.contains("mem")
}

#[cfg(target_os = "linux")]
fn sorted_child_paths(path: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(path) else {
        return Vec::new();
    };
    let mut paths = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

#[cfg(target_os = "linux")]
fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
}

#[cfg(target_os = "linux")]
fn read_percent_file(path: impl AsRef<Path>) -> Option<f32> {
    let value = read_trimmed(path)?.parse::<f32>().ok()?;
    (0.0..=100.0).contains(&value).then_some(value)
}

#[cfg(target_os = "linux")]
fn read_milli_celsius_file(path: impl AsRef<Path>) -> Option<f32> {
    let value = read_trimmed(path)?.parse::<f32>().ok()? / 1000.0;
    value.is_finite().then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_gpu_temp_label_excludes_hotspot_sensors() {
        assert!(is_normal_gpu_temp_label("amdgpu edge"));
        assert!(is_normal_gpu_temp_label("gpu temp"));
        assert!(!is_normal_gpu_temp_label("amdgpu junction"));
        assert!(!is_normal_gpu_temp_label("gpu hotspot"));
        assert!(!is_normal_gpu_temp_label("amdgpu mem"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn amd_metrics_prefer_edge_temp_over_junction() {
        let root = temp_test_dir("amd_metrics_prefer_edge_temp_over_junction");
        let device = root.join("card1/device");
        let hwmon = device.join("hwmon/hwmon0");
        fs::create_dir_all(&hwmon).unwrap();
        fs::write(device.join("gpu_busy_percent"), "42\n").unwrap();
        fs::write(hwmon.join("name"), "amdgpu\n").unwrap();
        fs::write(hwmon.join("temp1_label"), "edge\n").unwrap();
        fs::write(hwmon.join("temp1_input"), "53000\n").unwrap();
        fs::write(hwmon.join("temp2_label"), "junction\n").unwrap();
        fs::write(hwmon.join("temp2_input"), "96000\n").unwrap();

        let metrics = read_amd_gpu_metrics_from_device(&device);
        let _ = fs::remove_dir_all(&root);

        assert_eq!(metrics.temp, Some(53.0));
        assert_eq!(metrics.usage, Some(42.0));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_gpu_metrics_select_amd_card_from_drm_root() {
        let root = temp_test_dir("linux_gpu_metrics_select_amd_card_from_drm_root");
        let intel = root.join("card0/device");
        let amd = root.join("card1/device");
        let hwmon = amd.join("hwmon/hwmon0");
        fs::create_dir_all(&intel).unwrap();
        fs::create_dir_all(&hwmon).unwrap();
        fs::write(intel.join("vendor"), "0x8086\n").unwrap();
        fs::write(amd.join("vendor"), "0x1002\n").unwrap();
        fs::write(amd.join("gpu_busy_percent"), "17\n").unwrap();
        fs::write(hwmon.join("temp1_label"), "edge\n").unwrap();
        fs::write(hwmon.join("temp1_input"), "61000\n").unwrap();

        let metrics = read_linux_gpu_metrics_from_drm(&root);
        let _ = fs::remove_dir_all(&root);

        assert_eq!(metrics.temp, Some(61.0));
        assert_eq!(metrics.usage, Some(17.0));
    }

    #[cfg(target_os = "linux")]
    fn temp_test_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "coolcooler-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
