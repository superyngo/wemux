//! Windows Service runner implementation

use crate::audio::AudioEngine;
use crate::service::config::ServiceConfig;
use crate::service::{SERVICE_DISPLAY_NAME, SERVICE_NAME};
use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

/// Run the Windows service
///
/// This is the main entry point called from the service binary.
/// It registers with the Service Control Manager and starts the service dispatcher.
pub fn run_service() -> Result<(), windows_service::Error> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
}

// Generate the Windows service entry point
define_windows_service!(ffi_service_main, service_main);

/// Service main function called by the Windows Service Control Manager
fn service_main(arguments: Vec<OsString>) {
    if let Err(e) = run_service_main(arguments) {
        error!("Service error: {}", e);
    }
}

fn run_service_main(_arguments: Vec<OsString>) -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = ServiceConfig::load_default().unwrap_or_else(|e| {
        eprintln!("Warning: Failed to load config: {}, using defaults", e);
        ServiceConfig::default()
    });

    // Initialize logging
    init_logging(&config);

    info!("Starting {} service", SERVICE_DISPLAY_NAME);

    // Create stop flag
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_handler = stop_flag.clone();

    // Register service control handler
    let status_handle = service_control_handler::register(
        SERVICE_NAME,
        move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    info!("Received stop/shutdown signal");
                    stop_flag_handler.store(true, Ordering::SeqCst);
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        },
    )?;

    // Report service starting
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(10),
        process_id: None,
    })?;

    // Create and start the audio engine
    let engine_config = config.to_engine_config();
    let mut engine = AudioEngine::new(engine_config);

    match engine.start() {
        Ok(()) => {
            info!("Audio engine started successfully");

            // Report service running
            status_handle.set_service_status(ServiceStatus {
                service_type: SERVICE_TYPE,
                current_state: ServiceState::Running,
                controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
                exit_code: ServiceExitCode::Win32(0),
                checkpoint: 0,
                wait_hint: Duration::default(),
                process_id: None,
            })?;

            // Main service loop
            while !stop_flag.load(Ordering::SeqCst) && engine.is_running() {
                std::thread::sleep(Duration::from_millis(100));
            }

            // Report service stopping
            status_handle.set_service_status(ServiceStatus {
                service_type: SERVICE_TYPE,
                current_state: ServiceState::StopPending,
                controls_accepted: ServiceControlAccept::empty(),
                exit_code: ServiceExitCode::Win32(0),
                checkpoint: 0,
                wait_hint: Duration::from_secs(5),
                process_id: None,
            })?;

            // Stop the engine
            if let Err(e) = engine.stop() {
                warn!("Error stopping engine: {}", e);
            }
        }
        Err(e) => {
            error!("Failed to start audio engine: {}", e);

            // Report service stopped with error
            status_handle.set_service_status(ServiceStatus {
                service_type: SERVICE_TYPE,
                current_state: ServiceState::Stopped,
                controls_accepted: ServiceControlAccept::empty(),
                exit_code: ServiceExitCode::Win32(1),
                checkpoint: 0,
                wait_hint: Duration::default(),
                process_id: None,
            })?;

            return Err(e.into());
        }
    }

    // Report service stopped
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    info!("Service stopped");
    Ok(())
}

/// Initialize logging for service mode
fn init_logging(config: &ServiceConfig) {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    if !config.log_file.is_empty() {
        // Log to file
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.log_file);

        match file {
            Ok(file) => {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(fmt::layer().with_writer(file).with_ansi(false))
                    .init();
                return;
            }
            Err(e) => {
                eprintln!("Warning: Failed to open log file: {}", e);
            }
        }
    }

    // Default: no output in service mode (services don't have console)
    // But we still set up the subscriber for potential debugging
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::sink))
        .init();
}
