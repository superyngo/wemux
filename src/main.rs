//! wemux - Windows Multi-HDMI Audio Sync CLI

use anyhow::Result;
use clap::Parser;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::error;
use tracing_subscriber::EnvFilter;

use wemux::audio::{AudioEngine, EngineConfig};
use wemux::config::{Args, Command};
use wemux::device::DeviceEnumerator;

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    init_logging(&args)?;

    // Execute command
    match args.command.unwrap_or_default() {
        Command::List {
            hdmi_only,
            show_ids,
        } => cmd_list(hdmi_only, show_ids),
        Command::Start {
            devices,
            exclude,
            buffer,
            source,
        } => cmd_start(devices, exclude, buffer, source),
        Command::Info { device_id } => cmd_info(&device_id),
    }
}

fn init_logging(args: &Args) -> Result<()> {
    let level = args.log_level();

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level.to_string()));

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false);

    if let Some(log_file) = &args.log {
        let file = std::fs::File::create(log_file)?;
        subscriber.with_writer(file).init();
    } else {
        subscriber.init();
    }

    Ok(())
}

/// List available audio devices
fn cmd_list(hdmi_only: bool, show_ids: bool) -> Result<()> {
    let enumerator = DeviceEnumerator::new()?;

    let devices = if hdmi_only {
        enumerator.enumerate_hdmi_devices().unwrap_or_default()
    } else {
        enumerator.enumerate_all_devices()?
    };

    if devices.is_empty() {
        if hdmi_only {
            println!("No HDMI audio devices found.");
        } else {
            println!("No audio devices found.");
        }
        return Ok(());
    }

    println!("Available audio devices:\n");

    for (i, device) in devices.iter().enumerate() {
        let hdmi_tag = if device.is_hdmi { " [HDMI]" } else { "" };
        let default_tag = if device.is_default { " (default)" } else { "" };

        print!("  {}. {}{}{}", i + 1, device.name, hdmi_tag, default_tag);

        if show_ids {
            println!("\n     ID: {}", device.id);
        } else {
            println!();
        }
    }

    println!();
    Ok(())
}

/// Start audio synchronization
fn cmd_start(
    devices: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    buffer_ms: u32,
    source: Option<String>,
) -> Result<()> {
    println!("wemux - Windows Multi-HDMI Audio Sync\n");

    let config = EngineConfig {
        buffer_ms,
        device_ids: devices,
        exclude_ids: exclude,
        source_device_id: source,
    };

    let mut engine = AudioEngine::new(config);

    // Setup Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    #[cfg(windows)]
    {
        let _ = ctrlc::set_handler(move || {
            println!("\nReceived Ctrl+C, stopping...");
            r.store(false, Ordering::SeqCst);
        });
    }

    // Start the engine
    match engine.start() {
        Ok(()) => {
            if let Some(format) = engine.format() {
                println!("Audio format: {}", format);
            }
            println!("\nAudio sync running. Press Ctrl+C to stop.\n");
        }
        Err(e) => {
            error!("Failed to start engine: {}", e);
            return Err(e.into());
        }
    }

    // Wait for Ctrl+C
    while running.load(Ordering::SeqCst) && engine.is_running() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Stop the engine
    engine.stop()?;
    println!("Stopped.");

    Ok(())
}

/// Show detailed device information
fn cmd_info(device_id: &str) -> Result<()> {
    let enumerator = DeviceEnumerator::new()?;
    let devices = enumerator.enumerate_all_devices()?;

    let device = devices
        .iter()
        .find(|d| d.id.contains(device_id) || d.name.contains(device_id));

    match device {
        Some(dev) => {
            println!("Device Information:\n");
            println!("  Name:     {}", dev.name);
            println!("  ID:       {}", dev.id);
            println!("  HDMI:     {}", if dev.is_hdmi { "Yes" } else { "No" });
            println!("  Default:  {}", if dev.is_default { "Yes" } else { "No" });
        }
        None => {
            println!("Device not found: {}", device_id);
            println!("\nUse 'wemux list --show-ids' to see available devices.");
        }
    }

    Ok(())
}
