//! Icon management for tray application

use anyhow::{Context, Result};
use image::GenericImageView;
use std::path::PathBuf;
use tray_icon::Icon;

/// Icon manager for different application states
pub struct IconManager {
    idle_icon: Icon,
    active_icon: Icon,
    error_icon: Icon,
}

impl IconManager {
    /// Create a new icon manager
    pub fn new() -> Result<Self> {
        let idle_icon = Self::load_icon_from_file("assets/icons/tray/idle.png")?;
        let active_icon = Self::load_icon_from_file("assets/icons/tray/active.png")?;
        let error_icon = Self::load_icon_from_file("assets/icons/tray/error.png")?;

        Ok(Self {
            idle_icon,
            active_icon,
            error_icon,
        })
    }

    /// Get asset path relative to executable
    ///
    /// Searches in order:
    /// 1. Executable directory (production/MSIX)
    /// 2. Current working directory (development)
    fn get_asset_path(relative_path: &str) -> Result<PathBuf> {
        // Try executable directory first (production/MSIX)
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let path = exe_dir.join(relative_path);
                if path.exists() {
                    return Ok(path);
                }
            }
        }

        // Fall back to current working directory (development)
        let cwd_path = std::env::current_dir()
            .context("Failed to get current directory")?
            .join(relative_path);

        if cwd_path.exists() {
            return Ok(cwd_path);
        }

        anyhow::bail!(
            "Asset not found: {} (searched in exe dir and current dir)",
            relative_path
        )
    }

    /// Load icon from PNG file
    fn load_icon_from_file(path: &str) -> Result<Icon> {
        let full_path = Self::get_asset_path(path)?;
        let img = image::open(&full_path)
            .with_context(|| format!("Failed to load icon: {:?}", full_path))?;
        let (width, height) = img.dimensions();
        let rgba = img.into_rgba8().into_raw();
        Ok(Icon::from_rgba(rgba, width, height)?)
    }

    /// Get icon for idle state
    pub fn get_idle_icon(&self) -> Result<Icon> {
        Ok(self.idle_icon.clone())
    }

    /// Get icon for active/running state
    pub fn get_active_icon(&self) -> Result<Icon> {
        Ok(self.active_icon.clone())
    }

    /// Get icon for error state
    pub fn get_error_icon(&self) -> Result<Icon> {
        Ok(self.error_icon.clone())
    }
}

impl Default for IconManager {
    fn default() -> Self {
        Self::new().expect("Failed to create icon manager")
    }
}
