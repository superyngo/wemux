//! Bridge between UI and AudioEngine

use crate::audio::{AudioEngine, DeviceStatus, EngineConfig, EngineState};
use crate::device::DeviceEnumerator;
use crossbeam_channel::{Receiver, Sender};
use std::thread::{self, JoinHandle};
use tracing::{error, info};

/// Commands sent from UI to Engine
#[derive(Debug, Clone)]
pub enum TrayCommand {
    /// Start the audio engine
    Start,
    /// Stop the audio engine
    Stop,
    /// Toggle device enabled/paused state
    ToggleDevice { device_id: String },
    /// Set device enabled state explicitly
    SetDeviceEnabled { device_id: String, enabled: bool },
    /// Refresh device list
    RefreshDevices,
    /// Shutdown the controller
    Shutdown,
}

/// Status updates sent from Engine to UI
#[derive(Debug, Clone)]
pub enum EngineStatus {
    /// Device list updated
    DevicesUpdated(Vec<DeviceStatus>),
    /// Default device changed
    DefaultDeviceChanged(String),
    /// Engine state changed
    EngineStateChanged(EngineState),
    /// Error occurred
    Error(String),
}

/// Controller that bridges UI and AudioEngine
pub struct EngineController;

impl EngineController {
    /// Create a new controller and start it on a background thread
    pub fn start(
        command_rx: Receiver<TrayCommand>,
        status_tx: Sender<EngineStatus>,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            // Create engine inside the thread to avoid Send issues with COM objects
            let mut engine: Option<AudioEngine> = None;
            Self::run_loop(command_rx, status_tx, &mut engine);
        })
    }

    fn run_loop(
        command_rx: Receiver<TrayCommand>,
        status_tx: Sender<EngineStatus>,
        engine: &mut Option<AudioEngine>,
    ) {
        while let Ok(command) = command_rx.recv() {
            if !Self::handle_command(command, &status_tx, engine) {
                break;
            }
        }

        // Cleanup
        if let Some(ref mut eng) = engine {
            let _ = eng.stop();
        }
    }

    fn handle_command(
        command: TrayCommand,
        status_tx: &Sender<EngineStatus>,
        engine: &mut Option<AudioEngine>,
    ) -> bool {
        match command {
            TrayCommand::Start => {
                Self::start_engine(status_tx, engine);
            }
            TrayCommand::Stop => {
                Self::stop_engine(status_tx, engine);
            }
            TrayCommand::ToggleDevice { device_id } => {
                Self::toggle_device(&device_id, status_tx, engine);
            }
            TrayCommand::SetDeviceEnabled { device_id, enabled } => {
                Self::set_device_enabled(&device_id, enabled, status_tx, engine);
            }
            TrayCommand::RefreshDevices => {
                Self::refresh_devices(status_tx, engine);
            }
            TrayCommand::Shutdown => {
                return false; // Signal to exit loop
            }
        }
        true
    }

    fn start_engine(status_tx: &Sender<EngineStatus>, engine: &mut Option<AudioEngine>) {
        if engine.is_some() {
            return;
        }

        let config = EngineConfig::default();
        let mut eng = AudioEngine::new(config);

        match eng.start() {
            Ok(()) => {
                info!("Engine started from tray controller");
                let _ = status_tx.send(EngineStatus::EngineStateChanged(EngineState::Running));
                *engine = Some(eng);
                Self::refresh_devices(status_tx, engine);
            }
            Err(e) => {
                error!("Failed to start engine: {}", e);
                let _ = status_tx.send(EngineStatus::Error(e.to_string()));
            }
        }
    }

    fn stop_engine(status_tx: &Sender<EngineStatus>, engine: &mut Option<AudioEngine>) {
        if let Some(ref mut eng) = engine {
            let _ = eng.stop();
            let _ = status_tx.send(EngineStatus::EngineStateChanged(EngineState::Stopped));
        }
        *engine = None;
    }

    fn toggle_device(
        device_id: &str,
        status_tx: &Sender<EngineStatus>,
        engine: &mut Option<AudioEngine>,
    ) {
        if let Some(ref eng) = engine {
            // Get current state and toggle
            let statuses = eng.get_device_statuses();
            if let Some(status) = statuses.iter().find(|s| s.id == device_id) {
                if status.is_paused {
                    let _ = eng.resume_renderer(device_id);
                } else {
                    let _ = eng.pause_renderer(device_id);
                }
                Self::refresh_devices(status_tx, engine);
            }
        }
    }

    fn set_device_enabled(
        device_id: &str,
        enabled: bool,
        status_tx: &Sender<EngineStatus>,
        engine: &mut Option<AudioEngine>,
    ) {
        if let Some(ref eng) = engine {
            if enabled {
                let _ = eng.resume_renderer(device_id);
            } else {
                let _ = eng.pause_renderer(device_id);
            }
            Self::refresh_devices(status_tx, engine);
        }
    }

    fn refresh_devices(status_tx: &Sender<EngineStatus>, engine: &mut Option<AudioEngine>) {
        // Get default device name first
        if let Ok(enumerator) = DeviceEnumerator::new() {
            if let Ok(default_name) = enumerator.get_default_device_name() {
                let _ = status_tx.send(EngineStatus::DefaultDeviceChanged(default_name));
            }
        }

        if let Some(ref eng) = engine {
            let statuses = eng.get_device_statuses();
            let _ = status_tx.send(EngineStatus::DevicesUpdated(statuses));
        } else {
            // Engine not running, enumerate available devices
            if let Ok(enumerator) = DeviceEnumerator::new() {
                match enumerator.enumerate_hdmi_devices() {
                    Ok(devices) => {
                        let statuses: Vec<DeviceStatus> = devices
                            .iter()
                            .map(|d| DeviceStatus {
                                id: d.id.clone(),
                                name: d.name.clone(),
                                is_enabled: false,
                                is_paused: d.is_default,
                            })
                            .collect();
                        let _ = status_tx.send(EngineStatus::DevicesUpdated(statuses));
                    }
                    Err(crate::error::WemuxError::NoHdmiDevices) => {
                        // No HDMI devices found, send empty list
                        let _ = status_tx.send(EngineStatus::DevicesUpdated(Vec::new()));
                    }
                    Err(_) => {
                        // Other errors, send empty list
                        let _ = status_tx.send(EngineStatus::DevicesUpdated(Vec::new()));
                    }
                }
            }
        }
    }
}
