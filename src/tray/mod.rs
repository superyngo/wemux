//! System tray application for wemux
//!
//! This module provides Windows system tray functionality for controlling
//! audio synchronization to multiple HDMI devices.

#[cfg(feature = "tray")]
mod app;
#[cfg(feature = "tray")]
mod controller;
#[cfg(feature = "tray")]
mod icon;
#[cfg(feature = "tray")]
mod menu;

#[cfg(feature = "tray")]
pub use app::{TrayApp, TrayConfig};
#[cfg(feature = "tray")]
pub use controller::{EngineController, EngineStatus, TrayCommand};
