//! Main tray application

use crate::audio::EngineState;
use crate::tray::controller::{EngineController, EngineStatus, TrayCommand};
use crate::tray::icon::IconManager;
use crate::tray::menu::{MenuAction, MenuManager};
use anyhow::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use muda::MenuEvent;
use std::time::Duration;
use tracing::{error, info};
use tray_icon::{MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
};

/// Configuration for tray application
#[derive(Debug, Clone)]
pub struct TrayConfig {
    /// Auto-start engine on application launch
    pub auto_start: bool,
    /// Show notifications for errors
    pub show_notifications: bool,
}

impl Default for TrayConfig {
    fn default() -> Self {
        Self {
            auto_start: true,
            show_notifications: true,
        }
    }
}

/// Main tray application
pub struct TrayApp {
    config: TrayConfig,
    tray_icon: Option<TrayIcon>,
    menu_manager: MenuManager,
    icon_manager: IconManager,
    command_tx: Sender<TrayCommand>,
    status_rx: Receiver<EngineStatus>,
}

impl TrayApp {
    /// Create a new tray application
    pub fn new(config: TrayConfig) -> Result<Self> {
        let (command_tx, command_rx) = bounded(64);
        let (status_tx, status_rx) = bounded(64);

        // Start engine controller in background
        EngineController::start(command_rx, status_tx);

        let icon_manager = IconManager::new()?;
        let menu_manager = MenuManager::new();

        Ok(Self {
            config,
            tray_icon: None,
            menu_manager,
            icon_manager,
            command_tx,
            status_rx,
        })
    }

    /// Run the tray application
    pub fn run(&mut self) -> Result<()> {
        // Build initial menu
        let menu = self.menu_manager.build_initial_menu()?;

        // Create tray icon
        let icon = self.icon_manager.get_idle_icon()?;
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("wemux - Audio Sync")
            .with_icon(icon)
            .build()?;

        self.tray_icon = Some(tray_icon);

        // Auto-start engine if configured
        if self.config.auto_start {
            info!("Auto-starting engine");
            self.command_tx.send(TrayCommand::Start)?;
        }

        // Request initial device list
        self.command_tx.send(TrayCommand::RefreshDevices)?;

        // Run event loop
        self.run_event_loop()
    }

    fn run_event_loop(&mut self) -> Result<()> {
        info!("Tray application event loop started");

        // Windows message loop - required for tray icon and menu to work
        loop {
            unsafe {
                let mut msg: MSG = std::mem::zeroed();

                // Process all pending Windows messages (non-blocking)
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            // Process tray icon events
            if let Ok(event) = TrayIconEvent::receiver().try_recv() {
                if let Err(e) = self.handle_tray_event(event) {
                    error!("Error handling tray event: {}", e);
                }
            }

            // Process menu events
            if let Ok(event) = MenuEvent::receiver().try_recv() {
                if let Err(e) = self.handle_menu_event(event) {
                    error!("Error handling menu event: {}", e);
                }
            }

            // Process status updates from engine
            while let Ok(status) = self.status_rx.try_recv() {
                if let Err(e) = self.handle_status_update(status) {
                    error!("Error handling status update: {}", e);
                }
            }

            // Small sleep to avoid busy-waiting
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn handle_tray_event(&mut self, event: TrayIconEvent) -> Result<()> {
        match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } => {
                // Left click - could show a popup or do nothing
                info!("Tray icon left clicked");
            }
            TrayIconEvent::DoubleClick { .. } => {
                // Double click - could toggle engine
                info!("Tray icon double clicked");
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_menu_event(&mut self, event: MenuEvent) -> Result<()> {
        let id = event.id();

        if let Some(action) = self.menu_manager.get_action(id).cloned() {
            match action {
                MenuAction::ToggleDevice(device_id) => {
                    info!("Toggle device: {}", device_id);
                    self.command_tx
                        .send(TrayCommand::ToggleDevice { device_id })?;
                }
                MenuAction::RefreshDevices => {
                    info!("Refresh devices");
                    self.command_tx.send(TrayCommand::RefreshDevices)?;
                }
                MenuAction::StartEngine => {
                    info!("Start engine");
                    self.command_tx.send(TrayCommand::Start)?;
                }
                MenuAction::StopEngine => {
                    info!("Stop engine");
                    self.command_tx.send(TrayCommand::Stop)?;
                }
                MenuAction::Exit => {
                    info!("Exit application");
                    self.command_tx.send(TrayCommand::Shutdown)?;
                    // Give controller time to shutdown
                    std::thread::sleep(Duration::from_millis(100));
                    std::process::exit(0);
                }
            }
        }

        Ok(())
    }

    fn handle_status_update(&mut self, status: EngineStatus) -> Result<()> {
        match status {
            EngineStatus::DevicesUpdated(devices) => {
                info!("Devices updated: {} devices", devices.len());
                self.menu_manager.update_device_menu(&devices)?;

                // Rebuild complete menu with updated devices
                let menu = self.menu_manager.build_initial_menu()?;

                if let Some(ref tray) = self.tray_icon {
                    tray.set_menu(Some(Box::new(menu)));
                }
            }
            EngineStatus::DefaultDeviceChanged(device_name) => {
                info!("Default device changed to: {}", device_name);
                // Update system output display
                self.menu_manager.update_default_output(&device_name)?;
            }
            EngineStatus::EngineStateChanged(state) => {
                info!("Engine state changed: {:?}", state);
                self.menu_manager
                    .update_engine_state(state == EngineState::Running)?;

                let icon = match state {
                    EngineState::Running => self.icon_manager.get_active_icon()?,
                    EngineState::Stopped => self.icon_manager.get_idle_icon()?,
                    _ => self.icon_manager.get_idle_icon()?,
                };

                if let Some(ref tray) = self.tray_icon {
                    tray.set_icon(Some(icon))?;
                }
            }
            EngineStatus::Error(msg) => {
                error!("Engine error: {}", msg);

                if self.config.show_notifications {
                    // Could show Windows toast notification here
                    // For now, just log the error
                }

                if let Some(ref tray) = self.tray_icon {
                    let icon = self.icon_manager.get_error_icon()?;
                    tray.set_icon(Some(icon))?;
                }
            }
        }

        Ok(())
    }
}
