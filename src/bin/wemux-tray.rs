//! wemux System Tray Application
//!
//! Windows system tray application for controlling audio synchronization
//! to multiple HDMI devices.
//!
//! Run with `--debug` to show console window and enable stdout/stderr output.

// Hide console window in release mode unless --debug is passed
// This is handled at runtime via Windows API
#![windows_subsystem = "windows"]

use anyhow::Result;
use std::env;
use tracing_subscriber::EnvFilter;
use wemux::tray::{TrayApp, TrayConfig};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let debug_mode = args.iter().any(|arg| arg == "--debug" || arg == "-d");

    // In debug mode, allocate a console window for stdout/stderr
    if debug_mode {
        unsafe {
            windows::Win32::System::Console::AllocConsole()?;
        }

        // Initialize logging only in debug mode
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .init();

        println!("Starting wemux system tray application (debug mode)...");
    }

    // Initialize COM (required for Windows audio)
    unsafe {
        windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_MULTITHREADED,
        )
        .ok()?;
    }

    // Create and run tray app
    let config = TrayConfig {
        auto_start: true,
        show_notifications: true,
    };

    let mut app = TrayApp::new(config)?;
    app.run()?;

    Ok(())
}
