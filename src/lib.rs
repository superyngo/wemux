//! wemux - Windows Multi-HDMI Audio Sync
//!
//! This library provides functionality to capture system audio output
//! and synchronously play it to multiple HDMI audio devices.
//!
//! # Example
//!
//! ```no_run
//! use wemux::audio::{AudioEngine, EngineConfig};
//!
//! let config = EngineConfig::default();
//! let mut engine = AudioEngine::new(config);
//!
//! // Start audio synchronization
//! engine.start().expect("Failed to start engine");
//!
//! // ... engine runs until stopped
//!
//! engine.stop().expect("Failed to stop engine");
//! ```

pub mod audio;
pub mod config;
pub mod device;
pub mod error;
pub mod sync;

pub use error::{Result, WemuxError};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
