use crate::{DeviceInfo, Result};

/// Trait for controlling an AIO cooler's LCD screen.
///
/// Implementors handle device-specific USB communication and protocol details.
/// The caller is responsible for image preparation (see [`crate::frame::prepare`]).
pub trait CoolerLcd {
    /// Device metadata (name, resolution, rotation, timing).
    fn info(&self) -> &DeviceInfo;

    /// Open a connection to the physical device.
    fn connect(&mut self) -> Result<()>;

    /// Close the connection and release resources.
    fn disconnect(&mut self);

    /// Whether the device is currently connected.
    fn is_connected(&self) -> bool;

    /// Send a JPEG-encoded frame to the LCD.
    fn send_frame(&mut self, jpeg_data: &[u8]) -> Result<()>;

    /// Send a keepalive packet to maintain the connection.
    fn send_keepalive(&mut self) -> Result<()>;
}
