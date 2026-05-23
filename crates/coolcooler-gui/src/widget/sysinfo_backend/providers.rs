mod cpu;
mod gpu;
mod ram;

pub(super) use cpu::CpuProvider;
pub(super) use gpu::{GpuSensorFallback, NvidiaProvider};
pub(super) use ram::RamProvider;

#[cfg(target_os = "linux")]
pub(super) use gpu::AmdDrmProvider;
