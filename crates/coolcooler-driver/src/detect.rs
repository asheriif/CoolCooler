use coolcooler_core::CoolerLcd;
use coolcooler_idcooling::Fx360;
use coolcooler_liquidctl::{LiquidctlDeviceDef, LiquidctlDriver, DEVICE_REGISTRY};

use crate::DisplayDriver;

/// Scan for a supported LCD cooler device.
///
/// Returns the first device found, prioritizing native drivers
/// (which provide the best experience with streaming support).
pub fn detect_device() -> Option<DisplayDriver> {
    // 1. Try native drivers first (higher quality: streaming, keepalives)
    if let Some(driver) = try_native_fx360() {
        return Some(driver);
    }

    // 2. Scan liquidctl device registry via USB enumeration
    if let Some(driver) = try_liquidctl_devices() {
        return Some(driver);
    }

    None
}

/// Match a VID/PID pair against the liquidctl device registry.
/// Returns the first matching device definition, or `None`.
pub fn match_liquidctl_device(vendor_id: u16, product_id: u16) -> Option<&'static LiquidctlDeviceDef> {
    DEVICE_REGISTRY
        .iter()
        .find(|def| def.vendor_id == vendor_id && def.product_id == product_id)
}

fn try_native_fx360() -> Option<DisplayDriver> {
    let mut fx360 = Fx360::new();
    if fx360.connect().is_ok() {
        fx360.disconnect();
        return Some(DisplayDriver::Native(fx360));
    }
    None
}

fn try_liquidctl_devices() -> Option<DisplayDriver> {
    let api = hidapi::HidApi::new().ok()?;
    for dev in api.device_list() {
        if let Some(def) = match_liquidctl_device(dev.vendor_id(), dev.product_id()) {
            return Some(DisplayDriver::Liquidctl(LiquidctlDriver::new(def)));
        }
    }
    None
}
