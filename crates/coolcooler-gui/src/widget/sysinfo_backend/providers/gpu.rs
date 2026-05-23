use nvml_wrapper::Nvml;
use sysinfo::Components;

#[cfg(target_os = "linux")]
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct GpuMetrics {
    pub(crate) temp: Option<f32>,
    pub(crate) usage: Option<f32>,
}

pub(crate) struct NvidiaProvider {
    nvml: Option<Nvml>,
}

impl NvidiaProvider {
    pub(crate) fn new() -> Self {
        Self {
            nvml: Nvml::init().ok(),
        }
    }

    pub(crate) fn metrics(&self) -> Option<GpuMetrics> {
        let nvml = self.nvml.as_ref()?;
        let device = nvml.device_by_index(0).ok()?;
        let temp = device
            .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
            .ok()
            .map(|temp| temp as f32);
        let usage = device
            .utilization_rates()
            .ok()
            .map(|utilization| utilization.gpu as f32);

        Some(GpuMetrics { temp, usage })
    }
}

pub(crate) struct GpuSensorFallback;

impl GpuSensorFallback {
    pub(crate) fn temperature(components: &Components) -> Option<f32> {
        components
            .iter()
            .filter_map(|comp| {
                let label = comp.label().to_lowercase();
                is_normal_gpu_temp_label(&label)
                    .then(|| comp.temperature())
                    .flatten()
            })
            .max_by(|a, b| a.total_cmp(b))
    }
}

#[cfg(target_os = "linux")]
pub(crate) struct AmdDrmProvider;

#[cfg(target_os = "linux")]
impl AmdDrmProvider {
    pub(crate) fn metrics() -> GpuMetrics {
        read_linux_gpu_metrics_from_drm(Path::new("/sys/class/drm"))
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
