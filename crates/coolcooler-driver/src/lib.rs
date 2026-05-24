mod detect;
mod display_loop;

use coolcooler_core::frame::{self, DEFAULT_JPEG_QUALITY};
use coolcooler_core::{CoolerLcd, DeviceInfo, Error, Result};
use coolcooler_idcooling::Fx360;
use coolcooler_liquidctl::LiquidctlDriver;
use image::{DynamicImage, RgbaImage};

pub use detect::{detect_device, match_liquidctl_device};
pub use display_loop::{run_display, run_display_with_events, DisplayLoopEvent};

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

/// Frame bytes prepared for a specific display backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayFrame {
    StreamingJpeg(Vec<u8>),
    FileTransferPng(Vec<u8>),
}

/// Driver-owned encoder for converting an RGBA canvas into backend frame bytes.
#[derive(Debug, Clone)]
pub struct DisplayFrameEncoder {
    info: DeviceInfo,
    capability: DisplayCapability,
}

impl DisplayFrameEncoder {
    pub fn new(info: DeviceInfo, capability: DisplayCapability) -> Self {
        Self { info, capability }
    }

    pub fn from_driver(driver: &DisplayDriver) -> Self {
        Self::new(driver.info().clone(), driver.capability())
    }

    pub fn prepare(&self, composited: &RgbaImage) -> Result<DisplayFrame> {
        match self.capability {
            DisplayCapability::Streaming => {
                let rgb = DynamicImage::ImageRgba8(composited.clone()).to_rgb8();
                frame::encode_resized(&rgb, self.info.rotation, DEFAULT_JPEG_QUALITY)
                    .map(DisplayFrame::StreamingJpeg)
            }
            DisplayCapability::FileTransfer => {
                let mut buf = std::io::Cursor::new(Vec::new());
                DynamicImage::ImageRgba8(composited.clone())
                    .write_to(&mut buf, image::ImageFormat::Png)
                    .map_err(|e| Error::Image(e.to_string()))?;
                Ok(DisplayFrame::FileTransferPng(buf.into_inner()))
            }
        }
    }
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
    use image::{Rgba, RgbaImage};

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
    fn frame_encoder_prepares_typed_streaming_frame() {
        let driver = DisplayDriver::Native(Fx360::new());
        let encoder = DisplayFrameEncoder::from_driver(&driver);
        let composited = RgbaImage::from_pixel(240, 240, Rgba([255, 0, 0, 255]));

        let frame = encoder.prepare(&composited).unwrap();

        let DisplayFrame::StreamingJpeg(bytes) = frame else {
            panic!("native encoder should produce JPEG streaming frames");
        };
        assert!(bytes.starts_with(&[0xFF, 0xD8]));
    }

    #[test]
    fn frame_encoder_prepares_typed_file_transfer_frame() {
        let def = &DEVICE_REGISTRY[0];
        let driver = DisplayDriver::Liquidctl(LiquidctlDriver::new(def));
        let encoder = DisplayFrameEncoder::from_driver(&driver);
        let composited = RgbaImage::from_pixel(320, 320, Rgba([0, 0, 255, 255]));

        let frame = encoder.prepare(&composited).unwrap();

        let DisplayFrame::FileTransferPng(bytes) = frame else {
            panic!("liquidctl encoder should produce PNG file-transfer frames");
        };
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1A\n"));
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
}
