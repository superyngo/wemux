//! Master-slave clock synchronization for multiple renderers

use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, trace};

/// Threshold in samples before applying drift correction
const DRIFT_THRESHOLD_SAMPLES: i64 = 240; // ~5ms at 48kHz (tighter sync)

/// Maximum correction per update (to avoid audible glitches)
const MAX_CORRECTION_SAMPLES: i64 = 48; // ~1ms at 48kHz

/// Clock synchronization state for master-slave model
pub struct ClockSync {
    /// Master device ID
    master_id: Option<String>,
    /// Master's reference position
    master_position: u64,
    /// Last update time
    last_update: Instant,
    /// Per-slave state
    slaves: HashMap<String, SlaveState>,
    /// Sample rate for calculations
    sample_rate: u32,
}

struct SlaveState {
    /// Position at last sync
    last_position: u64,
    /// Accumulated drift in samples (positive = ahead of master, negative = behind)
    drift_samples: i64,
    /// Last sync time
    last_sync: Instant,
    /// Pending correction to apply
    pending_correction: i64,
}

impl ClockSync {
    /// Create a new clock sync instance
    pub fn new(sample_rate: u32) -> Self {
        Self {
            master_id: None,
            master_position: 0,
            last_update: Instant::now(),
            slaves: HashMap::new(),
            sample_rate,
        }
    }

    /// Set the master device
    pub fn set_master(&mut self, device_id: &str) {
        self.master_id = Some(device_id.to_string());
        self.master_position = 0;
        self.last_update = Instant::now();
        debug!("Clock sync master set to: {}", device_id);
    }

    /// Register a slave device
    pub fn register_slave(&mut self, device_id: &str) {
        if Some(device_id.to_string()) == self.master_id {
            return; // Don't register master as slave
        }

        self.slaves.insert(
            device_id.to_string(),
            SlaveState {
                last_position: 0,
                drift_samples: 0,
                last_sync: Instant::now(),
                pending_correction: 0,
            },
        );
        debug!("Registered clock sync slave: {}", device_id);
    }

    /// Remove a slave device
    pub fn remove_slave(&mut self, device_id: &str) {
        self.slaves.remove(device_id);
    }

    /// Update master position
    pub fn update_master(&mut self, position: u64) {
        self.master_position = position;
        self.last_update = Instant::now();
    }

    /// Update slave position and calculate drift
    pub fn update_slave(&mut self, device_id: &str, position: u64) {
        if let Some(slave) = self.slaves.get_mut(device_id) {
            let now = Instant::now();
            let elapsed = now.duration_since(slave.last_sync);

            // Calculate expected position based on elapsed time
            let elapsed_samples = (elapsed.as_secs_f64() * self.sample_rate as f64) as i64;

            // Calculate actual movement
            let actual_movement = position.wrapping_sub(slave.last_position) as i64;

            // Drift is difference between actual and expected
            // Positive drift = slave is ahead, negative = slave is behind
            let drift_delta = actual_movement - elapsed_samples;

            // Accumulate drift with some smoothing
            slave.drift_samples = (slave.drift_samples * 7 + drift_delta) / 8;

            slave.last_position = position;
            slave.last_sync = now;

            trace!(
                "Slave {} drift: {} samples ({:.2}ms)",
                device_id,
                slave.drift_samples,
                slave.drift_samples as f64 * 1000.0 / self.sample_rate as f64
            );

            // Calculate correction if drift exceeds threshold
            if slave.drift_samples.abs() > DRIFT_THRESHOLD_SAMPLES {
                let correction = slave.drift_samples.signum()
                    * slave.drift_samples.abs().min(MAX_CORRECTION_SAMPLES);
                slave.pending_correction = correction;

                debug!(
                    "Slave {} needs correction: {} samples",
                    device_id, correction
                );
            } else {
                slave.pending_correction = 0;
            }
        }
    }

    /// Get the pending correction for a slave (read-only, does not modify state)
    ///
    /// Returns number of samples to skip (positive) or duplicate (negative)
    /// This is the non-mutating version for use in hot paths.
    pub fn get_correction_readonly(&self, device_id: &str) -> i64 {
        self.slaves
            .get(device_id)
            .map(|slave| slave.pending_correction)
            .unwrap_or(0)
    }

    /// Apply correction and reset pending state
    ///
    /// Should be called after get_correction_readonly() to mark correction as applied.
    pub fn apply_correction(&mut self, device_id: &str) {
        if let Some(slave) = self.slaves.get_mut(device_id) {
            if slave.pending_correction != 0 {
                slave.drift_samples -= slave.pending_correction;
                slave.pending_correction = 0;
            }
        }
    }

    /// Get the pending correction for a slave (legacy, mutating version)
    ///
    /// Returns number of samples to skip (positive) or duplicate (negative)
    /// Prefer using get_correction_readonly() + apply_correction() for better performance.
    pub fn get_correction(&mut self, device_id: &str) -> i64 {
        if let Some(slave) = self.slaves.get_mut(device_id) {
            let correction = slave.pending_correction;
            if correction != 0 {
                // Apply correction to drift tracking
                slave.drift_samples -= correction;
                slave.pending_correction = 0;
            }
            correction
        } else {
            0
        }
    }

    /// Check if a device is the master
    pub fn is_master(&self, device_id: &str) -> bool {
        self.master_id.as_ref().is_some_and(|m| m == device_id)
    }

    /// Get current drift for a slave (for monitoring)
    pub fn get_drift_ms(&self, device_id: &str) -> Option<f64> {
        self.slaves
            .get(device_id)
            .map(|slave| slave.drift_samples as f64 * 1000.0 / self.sample_rate as f64)
    }

    /// Get all slave drift values for monitoring
    pub fn get_all_drifts(&self) -> Vec<(String, f64)> {
        self.slaves
            .iter()
            .map(|(id, slave)| {
                (
                    id.clone(),
                    slave.drift_samples as f64 * 1000.0 / self.sample_rate as f64,
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_master_slave_basic() {
        let mut sync = ClockSync::new(48000);
        sync.set_master("master");
        sync.register_slave("slave1");

        assert!(sync.is_master("master"));
        assert!(!sync.is_master("slave1"));
    }

    #[test]
    fn test_drift_calculation() {
        let mut sync = ClockSync::new(48000);
        sync.set_master("master");
        sync.register_slave("slave1");

        // Update slave with matching rate - should have no drift
        sync.update_slave("slave1", 0);
        sleep(Duration::from_millis(10));
        sync.update_slave("slave1", 480); // 10ms worth of samples

        let drift = sync.get_drift_ms("slave1").unwrap();
        // Should be close to 0 (within tolerance for timing)
        assert!(drift.abs() < 5.0, "Drift was {}", drift);
    }
}
