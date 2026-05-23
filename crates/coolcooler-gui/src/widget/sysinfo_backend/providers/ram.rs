use sysinfo::System;

pub(crate) struct RamProvider;

impl RamProvider {
    pub(crate) fn usage(system: &System) -> (f32, f32, f32) {
        let total = system.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
        let used = system.used_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
        let percent = if total > 0.0 {
            used / total * 100.0
        } else {
            0.0
        };
        (used as f32, total as f32, percent as f32)
    }
}
