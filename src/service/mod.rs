//! Windows Service support for wemux
//!
//! This module provides Windows Service functionality, allowing wemux to run
//! as a background service managed by the Windows Service Control Manager.

#[cfg(all(windows, feature = "service"))]
mod runner;

pub mod config;

#[cfg(all(windows, feature = "service"))]
pub use runner::run_service;

/// Service name used for registration
pub const SERVICE_NAME: &str = "wemux";

/// Service display name shown in services.msc
pub const SERVICE_DISPLAY_NAME: &str = "Wemux Audio Sync";

/// Service description
pub const SERVICE_DESCRIPTION: &str =
    "Duplicates system audio output to multiple HDMI devices for synchronized playback";
