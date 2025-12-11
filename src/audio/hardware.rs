//! Hardware capability detection for auto-calculating optimal buffer sizes

use crate::error::Result;
use tracing::{debug, info};
use windows::Win32::Media::Audio::IAudioClient;

/// Latency classification based on device characteristics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatencyClass {
    /// Low latency devices (professional/gaming cards): 20-30ms buffer
    LowLatency,
    /// Standard consumer devices: 30-40ms buffer
    Standard,
    /// High latency devices (USB/Bluetooth): 40-50ms buffer
    HighLatency,
}

impl LatencyClass {
    /// Get the base WASAPI buffer duration in milliseconds for this latency class
    pub fn wasapi_buffer_ms(&self) -> u32 {
        match self {
            LatencyClass::LowLatency => 25,
            LatencyClass::Standard => 35,
            LatencyClass::HighLatency => 50,
        }
    }

    /// Get the base ring buffer duration in milliseconds for this latency class
    pub fn ring_buffer_base_ms(&self) -> u32 {
        match self {
            LatencyClass::LowLatency => 200,
            LatencyClass::Standard => 300,
            LatencyClass::HighLatency => 400,
        }
    }
}

/// Hardware capabilities queried from WASAPI
#[derive(Debug, Clone)]
pub struct HardwareCapabilities {
    /// Minimum period supported by device (100-nanosecond units)
    pub min_period: i64,
    /// Default period for shared mode (100-nanosecond units)
    pub default_period: i64,
    /// Detected latency class
    pub latency_class: LatencyClass,
}

impl HardwareCapabilities {
    /// Query hardware capabilities from an audio client
    pub fn query(audio_client: &IAudioClient) -> Result<Self> {
        unsafe {
            let mut default_period: i64 = 0;
            let mut min_period: i64 = 0;

            audio_client.GetDevicePeriod(Some(&mut default_period), Some(&mut min_period))?;

            // Convert from 100-nanosecond units to milliseconds for classification
            let min_period_ms = (min_period as f64) / 10_000.0;
            let default_period_ms = (default_period as f64) / 10_000.0;

            debug!(
                "Device period: min={:.2}ms, default={:.2}ms",
                min_period_ms, default_period_ms
            );

            // Classify based on minimum period
            // Low latency: < 5ms minimum period (professional/gaming cards)
            // Standard: 5-15ms minimum period (most consumer devices)
            // High latency: > 15ms minimum period (USB/Bluetooth)
            let latency_class = if min_period_ms < 5.0 {
                LatencyClass::LowLatency
            } else if min_period_ms < 15.0 {
                LatencyClass::Standard
            } else {
                LatencyClass::HighLatency
            };

            info!("Detected latency class: {:?} (min period: {:.2}ms)", latency_class, min_period_ms);

            Ok(Self {
                min_period,
                default_period,
                latency_class,
            })
        }
    }

    /// Get the optimal WASAPI buffer duration in 100-nanosecond units
    ///
    /// This calculates the optimal buffer size based on hardware capabilities,
    /// balancing latency against stability.
    pub fn optimal_buffer_duration(&self) -> i64 {
        // Use a buffer that's at least 2x the minimum period for stability,
        // but constrained by the latency class recommendations
        let min_safe_buffer = self.min_period * 2;
        let class_recommended = (self.latency_class.wasapi_buffer_ms() as i64) * 10_000;

        // Take the larger of the two for stability
        let optimal = min_safe_buffer.max(class_recommended);

        debug!(
            "Optimal buffer duration: {}ms (min_safe={}ms, class_recommended={}ms)",
            optimal / 10_000,
            min_safe_buffer / 10_000,
            class_recommended / 10_000
        );

        optimal
    }

    /// Calculate optimal ring buffer size in milliseconds
    ///
    /// Takes into account the number of renderers to add safety margin.
    pub fn optimal_ring_buffer_ms(&self, num_renderers: usize) -> u32 {
        let base_ms = self.latency_class.ring_buffer_base_ms();
        // Add 25ms per renderer for safety margin
        let renderer_margin = (num_renderers as u32) * 25;
        let total = base_ms + renderer_margin;

        debug!(
            "Optimal ring buffer: {}ms (base={}ms, margin={}ms for {} renderers)",
            total, base_ms, renderer_margin, num_renderers
        );

        total
    }

    /// Get the minimum period in milliseconds
    pub fn min_period_ms(&self) -> f64 {
        (self.min_period as f64) / 10_000.0
    }

    /// Get the default period in milliseconds
    pub fn default_period_ms(&self) -> f64 {
        (self.default_period as f64) / 10_000.0
    }
}

impl Default for HardwareCapabilities {
    /// Default capabilities (conservative values for when detection fails)
    fn default() -> Self {
        Self {
            min_period: 100_000,    // 10ms
            default_period: 100_000, // 10ms
            latency_class: LatencyClass::Standard,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_class_wasapi_buffer() {
        assert_eq!(LatencyClass::LowLatency.wasapi_buffer_ms(), 25);
        assert_eq!(LatencyClass::Standard.wasapi_buffer_ms(), 35);
        assert_eq!(LatencyClass::HighLatency.wasapi_buffer_ms(), 50);
    }

    #[test]
    fn test_latency_class_ring_buffer() {
        assert_eq!(LatencyClass::LowLatency.ring_buffer_base_ms(), 200);
        assert_eq!(LatencyClass::Standard.ring_buffer_base_ms(), 300);
        assert_eq!(LatencyClass::HighLatency.ring_buffer_base_ms(), 400);
    }

    #[test]
    fn test_default_capabilities() {
        let caps = HardwareCapabilities::default();
        assert_eq!(caps.latency_class, LatencyClass::Standard);
        assert_eq!(caps.min_period_ms(), 10.0);
    }

    #[test]
    fn test_ring_buffer_with_renderers() {
        let caps = HardwareCapabilities {
            min_period: 50_000,  // 5ms
            default_period: 100_000, // 10ms
            latency_class: LatencyClass::Standard,
        };

        assert_eq!(caps.optimal_ring_buffer_ms(0), 300);
        assert_eq!(caps.optimal_ring_buffer_ms(2), 350);
        assert_eq!(caps.optimal_ring_buffer_ms(4), 400);
    }
}
