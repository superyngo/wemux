//! Bridge between UI and AudioEngine

use crate::audio::{AudioEngine, DeviceStatus, EngineConfig, EngineEvent, EngineState};
use crate::device::DeviceEnumerator;
use crate::tray::settings::TraySettings;
use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::Mutex;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tracing::{error, info, warn};

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
        // Create channel for engine events
        let (engine_event_tx, engine_event_rx) = bounded::<EngineEvent>(64);

        thread::spawn(move || {
            // Initialize COM for this thread - required for audio API calls
            unsafe {
                let _ = windows::Win32::System::Com::CoInitializeEx(
                    None,
                    windows::Win32::System::Com::COINIT_MULTITHREADED,
                );
            }

            // Load settings at startup
            let settings = Arc::new(Mutex::new(TraySettings::load()));

            // Create engine inside the thread to avoid Send issues with COM objects
            let mut engine: Option<AudioEngine> = None;
            Self::run_loop(
                command_rx,
                status_tx,
                &mut engine,
                &engine_event_rx,
                engine_event_tx,
                &settings,
            );

            // Uninitialize COM when thread exits
            unsafe {
                windows::Win32::System::Com::CoUninitialize();
            }
        })
    }

    fn run_loop(
        command_rx: Receiver<TrayCommand>,
        status_tx: Sender<EngineStatus>,
        engine: &mut Option<AudioEngine>,
        engine_event_rx: &Receiver<EngineEvent>,
        engine_event_tx: Sender<EngineEvent>,
        settings: &Arc<Mutex<TraySettings>>,
    ) {
        loop {
            // Check for commands (non-blocking with timeout)
            match command_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(command) => {
                    if !Self::handle_command(
                        command,
                        &status_tx,
                        engine,
                        &engine_event_tx,
                        settings,
                    ) {
                        break;
                    }
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                    // No command, continue to check events
                }
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    info!("Command channel disconnected");
                    break;
                }
            }

            // Check for engine events (non-blocking)
            while let Ok(event) = engine_event_rx.try_recv() {
                match event {
                    EngineEvent::DefaultDeviceChanged => {
                        info!("Default device changed, refreshing device list");
                        Self::refresh_devices(&status_tx, engine, settings);
                    }
                }
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
        engine_event_tx: &Sender<EngineEvent>,
        settings: &Arc<Mutex<TraySettings>>,
    ) -> bool {
        match command {
            TrayCommand::Start => {
                Self::start_engine(status_tx, engine, engine_event_tx, settings);
            }
            TrayCommand::Stop => {
                Self::stop_engine(status_tx, engine, settings);
            }
            TrayCommand::ToggleDevice { device_id } => {
                Self::toggle_device(&device_id, status_tx, engine, settings);
            }
            TrayCommand::SetDeviceEnabled { device_id, enabled } => {
                Self::set_device_enabled(&device_id, enabled, status_tx, engine, settings);
            }
            TrayCommand::RefreshDevices => {
                Self::refresh_devices(status_tx, engine, settings);
            }
            TrayCommand::Shutdown => {
                return false; // Signal to exit loop
            }
        }
        true
    }

    fn start_engine(
        status_tx: &Sender<EngineStatus>,
        engine: &mut Option<AudioEngine>,
        engine_event_tx: &Sender<EngineEvent>,
        settings: &Arc<Mutex<TraySettings>>,
    ) {
        if engine.is_some() {
            return;
        }

        // Build config from settings
        let config = Self::build_engine_config(settings);
        let mut eng = AudioEngine::new(config);

        // Set up event channel so engine can notify us of device changes
        eng.set_event_channel(engine_event_tx.clone());

        match eng.start() {
            Ok(()) => {
                info!("Engine started from tray controller");
                let _ = status_tx.send(EngineStatus::EngineStateChanged(EngineState::Running));
                *engine = Some(eng);
                Self::refresh_devices(status_tx, engine, settings);
            }
            Err(e) => {
                error!("Failed to start engine: {}", e);
                let _ = status_tx.send(EngineStatus::Error(e.to_string()));
            }
        }
    }

    fn stop_engine(
        status_tx: &Sender<EngineStatus>,
        engine: &mut Option<AudioEngine>,
        settings: &Arc<Mutex<TraySettings>>,
    ) {
        if let Some(ref mut eng) = engine {
            let _ = eng.stop();
            let _ = status_tx.send(EngineStatus::EngineStateChanged(EngineState::Stopped));
        }
        *engine = None;

        // Refresh to show device list based on settings
        Self::refresh_devices(status_tx, engine, settings);
    }

    fn toggle_device(
        device_id: &str,
        status_tx: &Sender<EngineStatus>,
        engine: &mut Option<AudioEngine>,
        settings: &Arc<Mutex<TraySettings>>,
    ) {
        if let Some(ref eng) = engine {
            // Engine is running, toggle renderer state
            let statuses = eng.get_device_statuses();
            if let Some(status) = statuses.iter().find(|s| s.id == device_id) {
                // Don't allow toggling system default devices
                if status.is_system_default {
                    info!("Cannot toggle system default device: {}", device_id);
                    return;
                }

                let new_enabled = status.is_paused;
                if new_enabled {
                    let _ = eng.resume_renderer(device_id);
                } else {
                    let _ = eng.pause_renderer(device_id);
                }

                // Also save to settings
                Self::save_device_setting(device_id, &status.name, new_enabled, settings);

                Self::refresh_devices(status_tx, engine, settings);
            }
        } else {
            // Engine not running, just toggle setting
            Self::toggle_device_setting(device_id, settings);
            Self::refresh_devices(status_tx, engine, settings);
        }
    }

    fn set_device_enabled(
        device_id: &str,
        enabled: bool,
        status_tx: &Sender<EngineStatus>,
        engine: &mut Option<AudioEngine>,
        settings: &Arc<Mutex<TraySettings>>,
    ) {
        if let Some(ref eng) = engine {
            if enabled {
                let _ = eng.resume_renderer(device_id);
            } else {
                let _ = eng.pause_renderer(device_id);
            }
        }

        // Get device name from settings
        let name = settings
            .lock()
            .devices
            .get(device_id)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| device_id.to_string());

        Self::save_device_setting(device_id, &name, enabled, settings);
        Self::refresh_devices(status_tx, engine, settings);
    }

    fn refresh_devices(
        status_tx: &Sender<EngineStatus>,
        engine: &mut Option<AudioEngine>,
        settings: &Arc<Mutex<TraySettings>>,
    ) {
        // Get default device info first
        if let Ok(enumerator) = DeviceEnumerator::new() {
            if let Ok(default_name) = enumerator.get_default_device_name() {
                let _ = status_tx.send(EngineStatus::DefaultDeviceChanged(default_name));
            }
        }

        if let Some(ref eng) = engine {
            let statuses = eng.get_device_statuses();
            let _ = status_tx.send(EngineStatus::DevicesUpdated(statuses));
        } else {
            // Engine not running, enumerate ALL available output devices
            if let Ok(enumerator) = DeviceEnumerator::new() {
                match enumerator.enumerate_all_devices() {
                    Ok(devices) => {
                        // Load settings and update with new devices
                        let mut settings_guard = settings.lock();

                        // Update settings with device list
                        let device_list: Vec<(String, String)> = devices
                            .iter()
                            .map(|d| (d.id.clone(), d.name.clone()))
                            .collect();
                        settings_guard.update_from_devices(&device_list);

                        // Create device statuses based on settings
                        // System default devices are always paused (disabled)
                        let statuses: Vec<DeviceStatus> = devices
                            .iter()
                            .map(|d| {
                                let enabled_in_settings = settings_guard.is_device_enabled(&d.id);
                                // If it's system default, it cannot be enabled
                                let is_paused = d.is_default || !enabled_in_settings;

                                DeviceStatus {
                                    id: d.id.clone(),
                                    name: d.name.clone(),
                                    is_enabled: !is_paused,
                                    is_paused,
                                    is_system_default: d.is_default,
                                }
                            })
                            .collect();

                        // Save settings
                        if let Err(e) = settings_guard.save() {
                            warn!("Failed to save settings: {}", e);
                        }

                        let _ = status_tx.send(EngineStatus::DevicesUpdated(statuses));
                    }
                    Err(_) => {
                        // Error enumerating, send empty list
                        let _ = status_tx.send(EngineStatus::DevicesUpdated(Vec::new()));
                    }
                }
            }
        }
    }

    fn toggle_device_setting(device_id: &str, settings: &Arc<Mutex<TraySettings>>) {
        let mut settings_guard = settings.lock();

        // Get current state and toggle
        let current_enabled = settings_guard.is_device_enabled(device_id);
        let new_enabled = !current_enabled;

        // Get name from existing setting or use device_id as fallback
        let name = settings_guard
            .devices
            .get(device_id)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| device_id.to_string());

        settings_guard.set_device_enabled(device_id, &name, new_enabled);

        // Save settings
        if let Err(e) = settings_guard.save() {
            warn!("Failed to save settings: {}", e);
        }

        info!(
            "Toggled device {} to {}",
            device_id,
            if new_enabled { "enabled" } else { "disabled" }
        );
    }

    fn save_device_setting(
        device_id: &str,
        name: &str,
        enabled: bool,
        settings: &Arc<Mutex<TraySettings>>,
    ) {
        let mut settings_guard = settings.lock();
        settings_guard.set_device_enabled(device_id, name, enabled);

        if let Err(e) = settings_guard.save() {
            warn!("Failed to save settings: {}", e);
        }
    }

    /// Build engine config from settings
    fn build_engine_config(settings: &Arc<Mutex<TraySettings>>) -> EngineConfig {
        let settings_guard = settings.lock();

        // Collect device IDs that are disabled in settings
        let paused_ids: Vec<String> = settings_guard
            .devices
            .iter()
            .filter(|(_, setting)| !setting.enabled)
            .map(|(id, _)| id.clone())
            .collect();

        info!(
            "Building engine config: {} devices disabled in settings",
            paused_ids.len()
        );

        EngineConfig {
            buffer_ms: 50,
            device_ids: None,
            exclude_ids: None,
            source_device_id: None,
            paused_device_ids: if paused_ids.is_empty() {
                None
            } else {
                Some(paused_ids)
            },
            use_all_devices: true, // Use all output devices, not just HDMI
        }
    }
}
