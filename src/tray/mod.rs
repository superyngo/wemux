//! System tray application for wemux
//!
//! This module provides Windows system tray functionality for controlling
//! audio synchronization to multiple HDMI devices.

mod app;
mod controller;
mod icon;
mod menu;
mod settings;

pub use app::{TrayApp, TrayConfig};
pub use controller::{EngineController, EngineStatus, TrayCommand};
pub use settings::TraySettings;
