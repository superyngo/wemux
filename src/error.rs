//! Unified error types for wemux

use thiserror::Error;

/// Main error type for wemux operations
#[derive(Error, Debug)]
pub enum WemuxError {
    /// COM initialization failed
    #[error("COM initialization failed: {0}")]
    ComInit(#[from] windows::core::Error),

    /// Device not found
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    /// Device operation error
    #[error("Device '{device_id}' error: {message}")]
    DeviceError {
        device_id: String,
        message: String,
    },

    /// Audio format mismatch between devices
    #[error("Format mismatch - expected: {expected}, actual: {actual}")]
    FormatMismatch {
        expected: String,
        actual: String,
    },

    /// Buffer overrun - capture producing faster than render consuming
    #[error("Buffer overrun: capture producing faster than render consuming")]
    BufferOverrun,

    /// Buffer underrun - render consuming faster than capture producing
    #[error("Buffer underrun: render consuming faster than capture producing")]
    BufferUnderrun,

    /// No HDMI devices found
    #[error("No HDMI audio devices found")]
    NoHdmiDevices,

    /// Engine not initialized
    #[error("Audio engine not initialized")]
    NotInitialized,

    /// Engine already running
    #[error("Audio engine already running")]
    AlreadyRunning,

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Thread communication error
    #[error("Thread communication error: {0}")]
    ChannelError(String),
}

/// Result type alias for wemux operations
pub type Result<T> = std::result::Result<T, WemuxError>;

impl WemuxError {
    /// Create a device error with context
    pub fn device_error(device_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::DeviceError {
            device_id: device_id.into(),
            message: message.into(),
        }
    }

    /// Check if this error is recoverable (can retry)
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            WemuxError::DeviceError { .. }
                | WemuxError::BufferOverrun
                | WemuxError::BufferUnderrun
        )
    }
}
