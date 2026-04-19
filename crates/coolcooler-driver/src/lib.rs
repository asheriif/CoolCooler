mod detect;
mod display_loop;

use coolcooler_core::{CoolerLcd, DeviceInfo, Result};
use coolcooler_idcooling::Fx360;
use coolcooler_liquidctl::LiquidctlDriver;

pub use detect::{detect_device, match_liquidctl_device};
pub use display_loop::run_display;

/// What display update strategy a device supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayCapability {
    /// Native USB streaming: supports high-FPS frame pushing (e.g. 20 FPS).
    /// GIF backgrounds with widget overlays work fine.
    Streaming,
    /// File-based update via liquidctl: updates are expensive (~1/sec).
    /// GIF backgrounds with widget overlays are NOT supported.
    FileTransfer,
}

/// Whether widget overlays are allowed for the given device capability and content.
///
/// Widgets are blocked when the background is animated (GIF) on a file-transfer
/// device, because those devices can't stream fast enough for smooth playback.
pub fn widgets_allowed(capability: DisplayCapability, is_animated: bool) -> bool {
    !(capability == DisplayCapability::FileTransfer && is_animated)
}

/// Unified device driver.
///
/// Enum dispatch over native (streaming) and liquidctl (file-transfer) backends.
/// The GUI uses this as its sole device interface.
pub enum DisplayDriver {
    Native(Fx360),
    Liquidctl(LiquidctlDriver),
}

impl DisplayDriver {
    pub fn info(&self) -> &DeviceInfo {
        match self {
            Self::Native(d) => d.info(),
            Self::Liquidctl(d) => d.info(),
        }
    }

    pub fn capability(&self) -> DisplayCapability {
        match self {
            Self::Native(_) => DisplayCapability::Streaming,
            Self::Liquidctl(_) => DisplayCapability::FileTransfer,
        }
    }

    pub fn connect(&mut self) -> Result<()> {
        match self {
            Self::Native(d) => d.connect(),
            Self::Liquidctl(_) => {
                LiquidctlDriver::check_available()?;
                Ok(())
            }
        }
    }

    pub fn disconnect(&mut self) {
        match self {
            Self::Native(d) => d.disconnect(),
            Self::Liquidctl(_) => {}
        }
    }

    pub fn is_connected(&self) -> bool {
        match self {
            Self::Native(d) => d.is_connected(),
            // liquidctl devices don't hold a persistent connection
            Self::Liquidctl(_) => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coolcooler_liquidctl::DEVICE_REGISTRY;

    // -- DisplayDriver enum tests --

    #[test]
    fn liquidctl_driver_returns_file_transfer_capability() {
        let def = &DEVICE_REGISTRY[0];
        let driver = DisplayDriver::Liquidctl(LiquidctlDriver::new(def));
        assert_eq!(driver.capability(), DisplayCapability::FileTransfer);
    }

    #[test]
    fn native_driver_returns_streaming_capability() {
        let driver = DisplayDriver::Native(Fx360::new());
        assert_eq!(driver.capability(), DisplayCapability::Streaming);
    }

    #[test]
    fn liquidctl_driver_info_matches_registry() {
        for def in DEVICE_REGISTRY {
            let driver = DisplayDriver::Liquidctl(LiquidctlDriver::new(def));
            let info = driver.info();
            assert_eq!(info.name, def.name);
            assert_eq!(info.resolution, def.resolution);
            assert_eq!(info.rotation, def.rotation);
        }
    }

    #[test]
    fn native_driver_info() {
        let driver = DisplayDriver::Native(Fx360::new());
        let info = driver.info();
        assert_eq!(info.name, "ID-Cooling FX360");
        assert_eq!(info.resolution.width, 240);
        assert_eq!(info.resolution.height, 240);
    }

    #[test]
    fn liquidctl_driver_is_always_connected() {
        let def = &DEVICE_REGISTRY[0];
        let driver = DisplayDriver::Liquidctl(LiquidctlDriver::new(def));
        assert!(driver.is_connected());
    }

    #[test]
    fn native_driver_not_connected_initially() {
        let driver = DisplayDriver::Native(Fx360::new());
        assert!(!driver.is_connected());
    }

    // -- Detection matching tests --

    #[test]
    fn match_known_kraken_z_vid_pid() {
        let def = match_liquidctl_device(0x1E71, 0x3008);
        assert!(def.is_some());
        assert_eq!(def.unwrap().name, "NZXT Kraken Z");
    }

    #[test]
    fn match_known_kraken_2023_vid_pid() {
        let def = match_liquidctl_device(0x1E71, 0x300E);
        assert!(def.is_some());
        assert_eq!(def.unwrap().name, "NZXT Kraken 2023");
    }

    #[test]
    fn match_known_msi_vid_pid() {
        let def = match_liquidctl_device(0x0DB0, 0xB130);
        assert!(def.is_some());
        assert_eq!(def.unwrap().name, "MSI MPG CoreLiquid K360");
    }

    #[test]
    fn no_match_for_unknown_vid_pid() {
        assert!(match_liquidctl_device(0xDEAD, 0xBEEF).is_none());
    }

    #[test]
    fn no_match_for_correct_vid_wrong_pid() {
        // NZXT vendor ID but wrong product ID
        assert!(match_liquidctl_device(0x1E71, 0x0000).is_none());
    }

    #[test]
    fn all_registry_entries_are_matchable() {
        for def in DEVICE_REGISTRY {
            let matched = match_liquidctl_device(def.vendor_id, def.product_id);
            assert!(
                matched.is_some(),
                "registry entry '{}' should be matchable",
                def.name
            );
            assert_eq!(matched.unwrap().name, def.name);
        }
    }

    // -- DisplayCapability equality --

    #[test]
    fn capability_equality() {
        assert_eq!(DisplayCapability::Streaming, DisplayCapability::Streaming);
        assert_eq!(
            DisplayCapability::FileTransfer,
            DisplayCapability::FileTransfer
        );
        assert_ne!(
            DisplayCapability::Streaming,
            DisplayCapability::FileTransfer
        );
    }

    // -- widgets_allowed tests --

    #[test]
    fn widgets_blocked_on_file_transfer_with_animated_background() {
        assert!(!widgets_allowed(DisplayCapability::FileTransfer, true));
    }

    #[test]
    fn widgets_allowed_on_file_transfer_with_static_background() {
        assert!(widgets_allowed(DisplayCapability::FileTransfer, false));
    }

    #[test]
    fn widgets_allowed_on_streaming_with_animated_background() {
        assert!(widgets_allowed(DisplayCapability::Streaming, true));
    }

    #[test]
    fn widgets_allowed_on_streaming_with_static_background() {
        assert!(widgets_allowed(DisplayCapability::Streaming, false));
    }
}
