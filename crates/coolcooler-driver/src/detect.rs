use coolcooler_idcooling::Fx360;
use coolcooler_liquidctl::{LiquidctlDeviceDef, LiquidctlDriver, DEVICE_REGISTRY};

use crate::DisplayDriver;

/// Scan for a supported LCD cooler device.
///
/// Returns the first device found, prioritizing native drivers
/// (which provide the best experience with streaming support).
pub fn detect_device() -> Option<DisplayDriver> {
    let api = hidapi::HidApi::new().ok()?;

    // 1. Match native drivers first (higher quality: streaming, keepalives).
    if api
        .device_list()
        .any(|dev| Fx360::matches_device(dev.vendor_id(), dev.product_id()))
    {
        return Some(DisplayDriver::Native(Fx360::new()));
    }

    // 2. Match devices handled through liquidctl.
    for dev in api.device_list() {
        if let Some(def) = match_liquidctl_device(dev.vendor_id(), dev.product_id()) {
            return Some(DisplayDriver::Liquidctl(LiquidctlDriver::new(def)));
        }
    }

    None
}

/// Match a VID/PID pair against the liquidctl device registry.
/// Returns the first matching device definition, or `None`.
pub fn match_liquidctl_device(
    vendor_id: u16,
    product_id: u16,
) -> Option<&'static LiquidctlDeviceDef> {
    DEVICE_REGISTRY
        .iter()
        .find(|def| def.vendor_id == vendor_id && def.product_id == product_id)
}
