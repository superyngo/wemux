//! wemux - Windows Multi-HDMI Audio Sync CLI

use anyhow::Result;
use clap::Parser;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::error;
use tracing_subscriber::EnvFilter;

use wemux::audio::{AudioEngine, EngineConfig};
use wemux::config::{Args, Command, ServiceAction};
use wemux::device::DeviceEnumerator;
use wemux::service::{
    config::ServiceConfig, SERVICE_DESCRIPTION, SERVICE_DISPLAY_NAME, SERVICE_NAME,
};

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
        Command::Service { action } => cmd_service(action),
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

/// Windows Service management
fn cmd_service(action: ServiceAction) -> Result<()> {
    use std::process::Command as ProcessCommand;

    match action {
        ServiceAction::Install => {
            println!("Installing {} service...\n", SERVICE_DISPLAY_NAME);

            // Get path to service executable
            let exe_path = std::env::current_exe()?;
            let service_exe = exe_path.with_file_name("wemux-service.exe");

            if !service_exe.exists() {
                eprintln!("Error: Service executable not found at:");
                eprintln!("  {}", service_exe.display());
                eprintln!("\nPlease build the service binary first:");
                eprintln!("  cargo build --release --features service --bin wemux-service");
                return Err(anyhow::anyhow!("Service executable not found"));
            }

            // Use sc.exe to install the service
            let output = ProcessCommand::new("sc")
                .args([
                    "create",
                    SERVICE_NAME,
                    &format!("binPath={}", service_exe.display()),
                    &format!("DisplayName={}", SERVICE_DISPLAY_NAME),
                    "start=auto",
                ])
                .output()?;

            if output.status.success() {
                println!("Service installed successfully!");

                // Set description
                let _ = ProcessCommand::new("sc")
                    .args(["description", SERVICE_NAME, SERVICE_DESCRIPTION])
                    .output();

                println!("\nTo start the service:");
                println!("  net start {}", SERVICE_NAME);
                println!("\nOr use Services (services.msc) to manage the service.");
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("Failed to install service: {}", stderr);
                eprintln!("\nNote: You may need to run this command as Administrator.");
                return Err(anyhow::anyhow!("Service installation failed"));
            }
        }

        ServiceAction::Uninstall => {
            println!("Uninstalling {} service...\n", SERVICE_DISPLAY_NAME);

            // Stop the service first (ignore errors if not running)
            let _ = ProcessCommand::new("sc")
                .args(["stop", SERVICE_NAME])
                .output();

            // Give it a moment to stop
            std::thread::sleep(std::time::Duration::from_secs(1));

            // Delete the service
            let output = ProcessCommand::new("sc")
                .args(["delete", SERVICE_NAME])
                .output()?;

            if output.status.success() {
                println!("Service uninstalled successfully!");
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("Failed to uninstall service: {}", stderr);
                eprintln!("\nNote: You may need to run this command as Administrator.");
                return Err(anyhow::anyhow!("Service uninstallation failed"));
            }
        }

        ServiceAction::Status => {
            let output = ProcessCommand::new("sc")
                .args(["query", SERVICE_NAME])
                .output()?;

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                println!("{} Service Status:\n", SERVICE_DISPLAY_NAME);

                // Parse and display status
                for line in stdout.lines() {
                    let line = line.trim();
                    if line.starts_with("STATE")
                        || line.starts_with("SERVICE_NAME")
                        || line.starts_with("TYPE")
                        || line.starts_with("PID")
                    {
                        println!("  {}", line);
                    }
                }
            } else {
                println!("Service '{}' is not installed.", SERVICE_NAME);
                println!("\nTo install the service:");
                println!("  wemux service install");
            }
        }

        ServiceAction::Config { output } => {
            println!("Generating sample configuration file: {}\n", output);

            let config_content = ServiceConfig::sample_config();
            std::fs::write(&output, config_content)?;

            println!("Configuration file created: {}", output);
            println!("\nEdit this file and place it in one of these locations:");
            println!("  1. Same directory as wemux-service.exe");
            println!("  2. %PROGRAMDATA%\\wemux\\config.toml");
        }
    }

    Ok(())
}
