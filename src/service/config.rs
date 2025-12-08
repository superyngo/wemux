//! Service configuration file support

use crate::audio::EngineConfig;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Service configuration loaded from TOML file
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServiceConfig {
    /// Audio buffer size in milliseconds
    pub buffer_ms: u32,

    /// Specific device IDs to use (empty = auto-detect all HDMI)
    #[serde(default)]
    pub device_ids: Vec<String>,

    /// Device IDs to exclude
    #[serde(default)]
    pub exclude_ids: Vec<String>,

    /// Source device ID for loopback (empty = system default)
    #[serde(default)]
    pub source_device_id: String,

    /// Log level (trace, debug, info, warn, error)
    pub log_level: String,

    /// Log file path (empty = no file logging)
    #[serde(default)]
    pub log_file: String,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            buffer_ms: 50,
            device_ids: Vec::new(),
            exclude_ids: Vec::new(),
            source_device_id: String::new(),
            log_level: "info".to_string(),
            log_file: String::new(),
        }
    }
}

impl ServiceConfig {
    /// Load configuration from a TOML file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(|e| ConfigError::Io {
            path: path.as_ref().to_string_lossy().to_string(),
            source: e,
        })?;

        toml::from_str(&content).map_err(|e| ConfigError::Parse {
            path: path.as_ref().to_string_lossy().to_string(),
            source: e,
        })
    }

    /// Load configuration from default locations
    ///
    /// Searches in order:
    /// 1. Same directory as executable: wemux.toml
    /// 2. %PROGRAMDATA%\wemux\config.toml
    pub fn load_default() -> Result<Self, ConfigError> {
        // Try executable directory first
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let config_path = exe_dir.join("wemux.toml");
                if config_path.exists() {
                    return Self::load(&config_path);
                }
            }
        }

        // Try ProgramData
        if let Ok(program_data) = std::env::var("PROGRAMDATA") {
            let config_path = Path::new(&program_data).join("wemux").join("config.toml");
            if config_path.exists() {
                return Self::load(&config_path);
            }
        }

        // Return default config if no file found
        Ok(Self::default())
    }

    /// Save configuration to a TOML file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), ConfigError> {
        let content = toml::to_string_pretty(self).map_err(ConfigError::Serialize)?;

        // Create parent directories if needed
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(|e| ConfigError::Io {
                path: parent.to_string_lossy().to_string(),
                source: e,
            })?;
        }

        std::fs::write(path.as_ref(), content).map_err(|e| ConfigError::Io {
            path: path.as_ref().to_string_lossy().to_string(),
            source: e,
        })
    }

    /// Convert to EngineConfig
    pub fn to_engine_config(&self) -> EngineConfig {
        EngineConfig {
            buffer_ms: self.buffer_ms,
            device_ids: if self.device_ids.is_empty() {
                None
            } else {
                Some(self.device_ids.clone())
            },
            exclude_ids: if self.exclude_ids.is_empty() {
                None
            } else {
                Some(self.exclude_ids.clone())
            },
            source_device_id: if self.source_device_id.is_empty() {
                None
            } else {
                Some(self.source_device_id.clone())
            },
        }
    }

    /// Generate a sample configuration file content
    pub fn sample_config() -> String {
        r#"# Wemux Service Configuration
# This file configures the wemux audio sync service

# Audio buffer size in milliseconds (default: 50)
buffer_ms = 50

# Specific device IDs to use (empty = auto-detect all HDMI)
# Example: device_ids = ["Device1", "Device2"]
device_ids = []

# Device IDs to exclude from auto-detection
# Example: exclude_ids = ["SomeDevice"]
exclude_ids = []

# Source device ID for loopback capture (empty = system default)
source_device_id = ""

# Log level: trace, debug, info, warn, error (default: info)
log_level = "info"

# Log file path (empty = no file logging)
# Example: log_file = "C:\\ProgramData\\wemux\\wemux.log"
log_file = ""
"#
        .to_string()
    }
}

/// Configuration error types
#[derive(Debug)]
pub enum ConfigError {
    /// IO error reading/writing config file
    Io {
        path: String,
        source: std::io::Error,
    },
    /// Error parsing TOML
    Parse {
        path: String,
        source: toml::de::Error,
    },
    /// Error serializing config
    Serialize(toml::ser::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io { path, source } => {
                write!(f, "Failed to read config file '{}': {}", path, source)
            }
            ConfigError::Parse { path, source } => {
                write!(f, "Failed to parse config file '{}': {}", path, source)
            }
            ConfigError::Serialize(e) => write!(f, "Failed to serialize config: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Io { source, .. } => Some(source),
            ConfigError::Parse { source, .. } => Some(source),
            ConfigError::Serialize(e) => Some(e),
        }
    }
}
