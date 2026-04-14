use std::time::Duration;

use coolcooler_core::*;
use hidapi::HidDevice;

use crate::protocol::{self, PACKET_SIZE};

const VENDOR_ID: u16 = 0x2000;
const PRODUCT_ID: u16 = 0x3000;

/// HID report ID. This device uses a single report (ID 0).
/// hidapi requires the report ID as the first byte of every write;
/// the HID layer strips it before the USB transfer hits the wire.
const REPORT_ID: u8 = 0x00;

/// ID-Cooling FX360 LCD controller.
///
/// Communicates over USB HID, sending 240x240 JPEG frames via
/// the CRT/DRA protocol.
pub struct Fx360 {
    info: DeviceInfo,
    device: Option<HidDevice>,
}

impl Fx360 {
    pub fn new() -> Self {
        Self {
            info: DeviceInfo {
                name: "ID-Cooling FX360".to_string(),
                resolution: Resolution::new(240, 240),
                rotation: Rotation::Deg180,
                target_fps: 20.0,
                keepalive_interval: Duration::from_secs(8),
            },
            device: None,
        }
    }

    /// Write a 1024-byte protocol packet, prepending the HID report ID.
    fn write_packet(&self, data: &[u8; PACKET_SIZE]) -> Result<()> {
        let device = self.device.as_ref().ok_or(Error::NotConnected)?;
        let mut buf = [0u8; 1 + PACKET_SIZE];
        buf[0] = REPORT_ID;
        buf[1..].copy_from_slice(data);
        device
            .write(&buf)
            .map_err(|e| Error::Transfer(e.to_string()))?;
        Ok(())
    }
}

impl Default for Fx360 {
    fn default() -> Self {
        Self::new()
    }
}

impl CoolerLcd for Fx360 {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn connect(&mut self) -> Result<()> {
        let api = hidapi::HidApi::new().map_err(|e| Error::Connection(format!("HID init: {e}")))?;

        let device = api
            .open(VENDOR_ID, PRODUCT_ID)
            .map_err(|_| Error::DeviceNotFound {
                vendor_id: VENDOR_ID,
                product_id: PRODUCT_ID,
            })?;

        self.device = Some(device);
        Ok(())
    }

    fn disconnect(&mut self) {
        self.device.take(); // HidDevice closes on drop
    }

    fn is_connected(&self) -> bool {
        self.device.is_some()
    }

    fn send_frame(&mut self, jpeg_data: &[u8]) -> Result<()> {
        for pkt in &protocol::build_frame_packets(jpeg_data) {
            self.write_packet(pkt)?;
        }
        Ok(())
    }

    fn send_keepalive(&mut self) -> Result<()> {
        self.write_packet(&protocol::build_connect_packet())
    }
}

impl Drop for Fx360 {
    fn drop(&mut self) {
        self.disconnect();
    }
}
