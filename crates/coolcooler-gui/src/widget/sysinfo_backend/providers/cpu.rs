use sysinfo::{Components, System};

pub(crate) struct CpuProvider;

impl CpuProvider {
    pub(crate) fn usage(system: &System) -> f32 {
        let cpus = system.cpus();
        if cpus.is_empty() {
            0.0
        } else {
            cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpus.len() as f32
        }
    }

    pub(crate) fn temperature(components: &Components) -> Option<f32> {
        components
            .iter()
            .filter_map(|comp| {
                let label = comp.label().to_lowercase();
                (label.contains("core")
                    || label.contains("tctl")
                    || label.contains("cpu")
                    || label.contains("package"))
                .then(|| comp.temperature())
                .flatten()
            })
            .max_by(|a, b| a.total_cmp(b))
    }
}
