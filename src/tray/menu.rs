//! Menu management for tray application

use crate::audio::DeviceStatus;
use muda::{CheckMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use std::collections::HashMap;

/// Menu actions
#[derive(Debug, Clone)]
pub enum MenuAction {
    ToggleDevice(String),
    RefreshDevices,
    StartEngine,
    StopEngine,
    Exit,
}

/// Menu manager for tray application
pub struct MenuManager {
    menu: Menu,
    device_submenu: Submenu,
    device_items: HashMap<MenuId, String>, // MenuId -> device_id
    actions: HashMap<MenuId, MenuAction>,
    default_output_item: MenuItem,
    status_item: MenuItem,
    start_item: MenuItem,
    stop_item: MenuItem,
    // Cached state for menu rebuilds
    cached_default_output: String,
    cached_devices: Vec<DeviceStatus>,
    cached_engine_running: bool,
}

impl MenuManager {
    /// Create a new menu manager
    pub fn new() -> Self {
        let menu = Menu::new();
        let device_submenu = Submenu::new("Output Devices", true);

        // Create placeholder items
        let default_output_item = MenuItem::new("System Output: Unknown", false, None);
        let status_item = MenuItem::new("wemux: Stopped", false, None);
        let start_item = MenuItem::new("Start", true, None);
        let stop_item = MenuItem::new("Stop", false, None);

        Self {
            menu,
            device_submenu,
            device_items: HashMap::new(),
            actions: HashMap::new(),
            default_output_item,
            status_item,
            start_item,
            stop_item,
            cached_default_output: "Unknown".to_string(),
            cached_devices: Vec::new(),
            cached_engine_running: false,
        }
    }

    /// Build the initial menu structure
    pub fn build_initial_menu(&mut self) -> Result<Menu, muda::Error> {
        // Clear existing
        self.device_items.clear();
        self.actions.clear();

        let menu = Menu::new();

        // System Output display (non-clickable) - use cached value
        let output_text = format!("System Output: {}", self.cached_default_output);
        self.default_output_item = MenuItem::new(&output_text, false, None);
        menu.append(&self.default_output_item)?;

        menu.append(&PredefinedMenuItem::separator())?;

        // Output Devices submenu - use cached devices
        self.device_submenu = Submenu::new("Output Devices", true);
        if self.cached_devices.is_empty() {
            let no_devices = MenuItem::new("Not found", false, None);
            self.device_submenu.append(&no_devices)?;
        } else {
            for device in &self.cached_devices {
                let label = self.format_device_label(device);
                // System default devices are greyed out (disabled) and cannot be toggled
                // Other devices can be toggled between Active and Disabled
                let can_toggle = !device.is_system_default;
                let is_active = !device.is_paused && !device.is_system_default;
                let item = CheckMenuItem::new(&label, can_toggle, is_active, None);
                let item_id = item.id().clone();
                self.device_items.insert(item_id.clone(), device.id.clone());
                self.actions
                    .insert(item_id, MenuAction::ToggleDevice(device.id.clone()));
                self.device_submenu.append(&item)?;
            }
        }
        menu.append(&self.device_submenu)?;

        menu.append(&PredefinedMenuItem::separator())?;

        // Control items - use cached engine state
        self.start_item = MenuItem::new("Start", !self.cached_engine_running, None);
        let start_id = self.start_item.id().clone();
        self.actions.insert(start_id, MenuAction::StartEngine);
        menu.append(&self.start_item)?;

        self.stop_item = MenuItem::new("Stop", self.cached_engine_running, None);
        let stop_id = self.stop_item.id().clone();
        self.actions.insert(stop_id, MenuAction::StopEngine);
        menu.append(&self.stop_item)?;

        let refresh_item = MenuItem::new("Refresh Devices", true, None);
        let refresh_id = refresh_item.id().clone();
        self.actions.insert(refresh_id, MenuAction::RefreshDevices);
        menu.append(&refresh_item)?;

        menu.append(&PredefinedMenuItem::separator())?;

        // Version info (non-clickable)
        self.status_item = MenuItem::new("wemux v0.1.1 by wen", false, None);
        menu.append(&self.status_item)?;

        // Exit
        let exit_item = MenuItem::new("Exit", true, None);
        let exit_id = exit_item.id().clone();
        self.actions.insert(exit_id, MenuAction::Exit);
        menu.append(&exit_item)?;

        self.menu = menu.clone();
        Ok(menu)
    }

    /// Update device menu with current device list
    pub fn update_device_menu(&mut self, devices: &[DeviceStatus]) -> Result<(), muda::Error> {
        // Cache the devices for menu rebuilds
        self.cached_devices = devices.to_vec();
        Ok(())
    }

    fn format_device_label(&self, device: &DeviceStatus) -> String {
        let mut label = device.name.clone();

        if device.is_system_default {
            // System default device - auto-paused to prevent feedback
            label.push_str(" (System Default)");
        } else if device.is_paused {
            // User manually disabled this device
            label.push_str(" [Disabled]");
        } else if device.is_enabled {
            // Active and outputting audio
            label.push_str(" [Active]");
        }

        label
    }

    /// Update engine state in status item
    pub fn update_engine_state(&mut self, running: bool) -> Result<(), muda::Error> {
        // Cache engine state for menu rebuilds
        self.cached_engine_running = running;

        let text = if running {
            "wemux: Running"
        } else {
            "wemux: Stopped"
        };

        // Update status item text
        self.status_item.set_text(text);

        // Update Start/Stop button states
        self.start_item.set_enabled(!running);
        self.stop_item.set_enabled(running);

        Ok(())
    }

    /// Get action for a menu ID
    pub fn get_action(&self, id: &MenuId) -> Option<&MenuAction> {
        self.actions.get(id)
    }

    /// Get the current menu
    #[allow(dead_code)]
    pub fn get_menu(&self) -> &Menu {
        &self.menu
    }

    /// Get device submenu
    #[allow(dead_code)]
    pub fn get_device_submenu(&self) -> &Submenu {
        &self.device_submenu
    }

    /// Update the system default output device display
    pub fn update_default_output(&mut self, device_name: &str) -> Result<(), muda::Error> {
        // Cache the default output for menu rebuilds
        self.cached_default_output = device_name.to_string();
        // Also update current menu item
        let text = format!("System Output: {}", device_name);
        self.default_output_item.set_text(&text);
        Ok(())
    }
}

impl Default for MenuManager {
    fn default() -> Self {
        Self::new()
    }
}
