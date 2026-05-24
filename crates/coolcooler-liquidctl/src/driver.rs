use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use coolcooler_core::{DeviceInfo, Error, Result};

use crate::LiquidctlDeviceDef;

static NEXT_TEMP_FILE_ID: AtomicU64 = AtomicU64::new(0);

/// A display driver that delegates to the `liquidctl` CLI tool.
pub struct LiquidctlDriver {
    def: &'static LiquidctlDeviceDef,
    info: DeviceInfo,
    temp_file: PathBuf,
}

impl LiquidctlDriver {
    pub fn new(def: &'static LiquidctlDeviceDef) -> Self {
        Self {
            def,
            info: DeviceInfo {
                name: def.name.to_string(),
                resolution: def.resolution,
                rotation: def.rotation,
                // File-transfer devices don't stream; this is a nominal value
                // used only if something queries target_fps.
                target_fps: 1.0,
                keepalive_interval: Duration::from_secs(3600),
            },
            temp_file: unique_temp_file_path(),
        }
    }

    pub fn info(&self) -> &DeviceInfo {
        &self.info
    }

    pub fn temp_file_path(&self) -> &Path {
        &self.temp_file
    }

    /// Verify that the `liquidctl` binary is available on PATH.
    pub fn check_available() -> Result<()> {
        Command::new("liquidctl")
            .arg("--version")
            .output()
            .map_err(|e| {
                Error::Connection(format!(
                    "liquidctl not found. Install it to use this device: {e}"
                ))
            })?;
        Ok(())
    }

    /// Send an image file to the device via liquidctl.
    pub fn send_image(&self, path: &Path) -> Result<()> {
        let path_str = path
            .to_str()
            .ok_or_else(|| Error::Other("image path is not valid UTF-8".to_string()))?;

        let args: Vec<String> = self
            .def
            .set_screen_args
            .iter()
            .map(|arg| arg.replace("{path}", path_str))
            .collect();

        let output = Command::new("liquidctl")
            .args(&args)
            .output()
            .map_err(|e| Error::Transfer(format!("failed to run liquidctl: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Transfer(format!("liquidctl failed: {stderr}")));
        }

        Ok(())
    }
}

impl Drop for LiquidctlDriver {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.temp_file);
    }
}

/// Build the liquidctl CLI args for a given device definition and image path.
/// Exposed for testing — this is the same logic used by `send_image`.
pub fn build_liquidctl_args(def: &LiquidctlDeviceDef, path: &str) -> Vec<String> {
    def.set_screen_args
        .iter()
        .map(|arg| arg.replace("{path}", path))
        .collect()
}

fn unique_temp_file_path() -> PathBuf {
    let id = NEXT_TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "coolcooler_lcd_frame_{}_{}.png",
        std::process::id(),
        id
    ))
}
