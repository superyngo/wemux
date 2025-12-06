//! CLI argument parsing using clap

use clap::{Parser, Subcommand};

/// wemux - Windows Multi-HDMI Audio Sync
///
/// Duplicate system audio to multiple HDMI audio devices simultaneously
#[derive(Parser, Debug)]
#[command(name = "wemux")]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Verbose output (can be repeated for more verbosity)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Quiet mode - only show errors
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Log output to file
    #[arg(long, global = true)]
    pub log: Option<String>,
}

/// Available commands
#[derive(Subcommand, Debug)]
pub enum Command {
    /// List all available audio devices
    List {
        /// Show only HDMI devices
        #[arg(long)]
        hdmi_only: bool,

        /// Show device IDs (useful for scripting)
        #[arg(long)]
        show_ids: bool,
    },

    /// Start audio synchronization
    Start {
        /// Specify HDMI device IDs to use (comma-separated)
        /// If not specified, all HDMI devices will be used
        #[arg(short, long, value_delimiter = ',')]
        devices: Option<Vec<String>>,

        /// Exclude specific device IDs (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        exclude: Option<Vec<String>>,

        /// Buffer size in milliseconds (default: 50)
        #[arg(short, long, default_value = "50")]
        buffer: u32,

        /// Source device ID for loopback capture
        /// If not specified, uses system default output
        #[arg(long)]
        source: Option<String>,
    },

    /// Show detailed device information
    Info {
        /// Device ID to show info for
        device_id: String,
    },
}

impl Args {
    /// Get the log level based on verbose/quiet flags
    pub fn log_level(&self) -> tracing::Level {
        if self.quiet {
            tracing::Level::ERROR
        } else {
            match self.verbose {
                0 => tracing::Level::INFO,
                1 => tracing::Level::DEBUG,
                _ => tracing::Level::TRACE,
            }
        }
    }
}

impl Default for Command {
    fn default() -> Self {
        // Default to start with auto-detect
        Command::Start {
            devices: None,
            exclude: None,
            buffer: 50,
            source: None,
        }
    }
}
