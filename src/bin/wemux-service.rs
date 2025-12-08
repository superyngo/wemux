//! Wemux Windows Service executable
//!
//! This binary is designed to be run by the Windows Service Control Manager.
//! Do not run this directly - use the service installer instead.
//!
//! To install the service:
//!   wemux service install
//!
//! To start the service:
//!   net start wemux
//!   or: sc start wemux
//!
//! To stop the service:
//!   net stop wemux
//!   or: sc stop wemux
//!
//! To uninstall the service:
//!   wemux service uninstall

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // This binary should only be started by the Windows Service Control Manager
    wemux::service::run_service().map_err(|e| {
        eprintln!("Failed to run service: {}", e);
        e
    })?;
    Ok(())
}
