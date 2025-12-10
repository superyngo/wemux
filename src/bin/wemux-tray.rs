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
use std::sync::atomic::{AtomicBool, Ordering};
use tracing_subscriber::EnvFilter;
use wemux::tray::{TrayApp, TrayConfig};

// Global flag for console control handler
static CONSOLE_EXIT_FLAG: AtomicBool = AtomicBool::new(false);

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let debug_mode = args.iter().any(|arg| arg == "--debug" || arg == "-d");

    // In debug mode, allocate a console window for stdout/stderr
    if debug_mode {
        unsafe {
            windows::Win32::System::Console::AllocConsole()?;

            // Set up console control handler to handle Ctrl+C and console close events
            use windows::Win32::System::Console::{SetConsoleCtrlHandler, CTRL_C_EVENT, CTRL_CLOSE_EVENT, CTRL_BREAK_EVENT};

            unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> windows::Win32::Foundation::BOOL {
                match ctrl_type {
                    x if x == CTRL_C_EVENT || x == CTRL_CLOSE_EVENT || x == CTRL_BREAK_EVENT => {
                        CONSOLE_EXIT_FLAG.store(true, Ordering::SeqCst);
                        // Return TRUE to indicate we handled it (prevents immediate termination)
                        // But for CTRL_CLOSE_EVENT, Windows will still terminate after a timeout
                        windows::Win32::Foundation::TRUE
                    }
                    _ => windows::Win32::Foundation::FALSE,
                }
            }

            let _ = SetConsoleCtrlHandler(Some(console_ctrl_handler), true);
        }

        // Initialize logging only in debug mode
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .init();

        println!("Starting wemux system tray application (debug mode)...");
        println!("Use the system tray Exit menu or Ctrl+C to exit cleanly.");
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

    if debug_mode {
        println!("Application exited cleanly.");
    }

    Ok(())
}
