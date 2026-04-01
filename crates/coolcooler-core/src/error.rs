use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("device not found (vendor={vendor_id:#06x}, product={product_id:#06x})")]
    DeviceNotFound { vendor_id: u16, product_id: u16 },

    #[error("not connected")]
    NotConnected,

    #[error("connection failed: {0}")]
    Connection(String),

    #[error("USB transfer failed: {0}")]
    Transfer(String),

    #[error("image processing error: {0}")]
    Image(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
