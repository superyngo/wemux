//! Device settings persistence using TOML format

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Device setting entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSetting {
    /// Device name (for reference only)
    pub name: String,
    /// Whether the device is enabled
    pub enabled: bool,
}

/// Settings structure for persistence
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraySettings {
    /// Device settings keyed by device ID
    #[serde(default)]
    pub devices: HashMap<String, DeviceSetting>,
}

impl TraySettings {
    /// Load settings from file, returns default if file doesn't exist
    pub fn load() -> Self {
        let path = Self::settings_path();

        if !path.exists() {
            debug!("Settings file not found, using defaults");
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(settings) => {
                    info!("Loaded settings from {:?}", path);
                    settings
                }
                Err(e) => {
                    warn!("Failed to parse settings file: {}", e);
                    Self::default()
                }
            },
            Err(e) => {
                warn!("Failed to read settings file: {}", e);
                Self::default()
            }
        }
    }

    /// Save settings to file
    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::settings_path();

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        fs::write(&path, content)?;
        info!("Saved settings to {:?}", path);
        Ok(())
    }

    /// Get settings file path (MSIX-compatible)
    ///
    /// When running as MSIX package, settings are stored in LocalAppData.
    /// For standalone executable, settings are stored alongside the executable.
    fn settings_path() -> PathBuf {
        // Check if running as MSIX package
        if std::env::var("MSIX_PACKAGE_FAMILY_NAME").is_ok() {
            // Use LocalAppData for MSIX (e.g., %LOCALAPPDATA%\wemux\wemux.toml)
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("wemux")
                .join("wemux.toml")
        } else {
            // Use executable directory for non-MSIX (current behavior)
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .unwrap_or_else(|| PathBuf::from("."))
                .join("wemux.toml")
        }
    }

    /// Check if a device is enabled in settings
    /// Returns true if not found (default enabled)
    pub fn is_device_enabled(&self, device_id: &str) -> bool {
        self.devices
            .get(device_id)
            .map(|s| s.enabled)
            .unwrap_or(true) // Default to enabled if not in settings
    }

    /// Set device enabled state
    pub fn set_device_enabled(&mut self, device_id: &str, name: &str, enabled: bool) {
        self.devices.insert(
            device_id.to_string(),
            DeviceSetting {
                name: name.to_string(),
                enabled,
            },
        );
    }

    /// Update settings from device list, adding new devices as enabled
    pub fn update_from_devices(&mut self, devices: &[(String, String)]) {
        for (id, name) in devices {
            if !self.devices.contains_key(id) {
                // New device, add as enabled by default
                self.devices.insert(
                    id.clone(),
                    DeviceSetting {
                        name: name.clone(),
                        enabled: true,
                    },
                );
            } else {
                // Update name in case it changed
                if let Some(setting) = self.devices.get_mut(id) {
                    setting.name = name.clone();
                }
            }
        }
    }
}
