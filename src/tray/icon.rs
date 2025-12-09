//! Icon management for tray application

use anyhow::Result;
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
        // Generate simple programmatic icons
        let idle_icon = Self::create_simple_icon([128, 128, 128, 255])?; // Gray
        let active_icon = Self::create_simple_icon([0, 200, 0, 255])?; // Green
        let error_icon = Self::create_simple_icon([200, 0, 0, 255])?; // Red

        Ok(Self {
            idle_icon,
            active_icon,
            error_icon,
        })
    }

    /// Create a simple solid color icon
    fn create_simple_icon(color: [u8; 4]) -> Result<Icon> {
        // Create a 16x16 solid color icon with a border
        let size = 16;
        let mut rgba = Vec::with_capacity(size * size * 4);

        for y in 0..size {
            for x in 0..size {
                // Create a border
                if x == 0 || y == 0 || x == size - 1 || y == size - 1 {
                    // White border
                    rgba.extend_from_slice(&[255, 255, 255, 255]);
                } else {
                    // Fill color
                    rgba.extend_from_slice(&color);
                }
            }
        }

        Ok(Icon::from_rgba(rgba, size as u32, size as u32)?)
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
