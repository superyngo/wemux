//! wemux - Windows Multi-HDMI Audio Sync
//!
//! System tray application for duplicating system audio output
//! to multiple HDMI audio devices.

pub mod audio;
pub mod device;
pub mod error;
pub mod sync;
pub mod tray;

pub use error::{Result, WemuxError};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
