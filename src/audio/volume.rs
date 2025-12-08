//! System volume tracking for volume-following feature

use crate::error::Result;
use std::sync::atomic::{AtomicU32, Ordering};
use tracing::{debug, warn};
use windows::Win32::{
    Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator},
    Media::Audio::Endpoints::IAudioEndpointVolume,
    System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED},
};

/// Atomic volume level stored as u32 bits of an f32 (0.0-1.0)
///
/// Using AtomicU32 with f32 bit representation for lock-free sharing
/// between the volume polling thread and render threads.
pub struct VolumeLevel(AtomicU32);

impl VolumeLevel {
    /// Create with default volume of 1.0 (full)
    pub fn new() -> Self {
        Self(AtomicU32::new(1.0f32.to_bits()))
    }

    /// Get current volume level (0.0 - 1.0)
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    /// Set volume level (0.0 - 1.0)
    pub fn set(&self, volume: f32) {
        let clamped = volume.clamp(0.0, 1.0);
        self.0.store(clamped.to_bits(), Ordering::Relaxed);
    }
}

impl Default for VolumeLevel {
    fn default() -> Self {
        Self::new()
    }
}

/// Tracks system volume from the default render device
pub struct VolumeTracker {
    endpoint_volume: IAudioEndpointVolume,
}

impl VolumeTracker {
    /// Create a new volume tracker for the default render device
    pub fn from_default_device() -> Result<Self> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

            let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;

            let endpoint_volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;

            debug!("Volume tracker initialized for default device");

            Ok(Self { endpoint_volume })
        }
    }

    /// Get current master volume level (0.0 - 1.0)
    pub fn get_volume(&self) -> f32 {
        unsafe {
            match self.endpoint_volume.GetMasterVolumeLevelScalar() {
                Ok(volume) => volume,
                Err(e) => {
                    warn!("Failed to get volume: {}", e);
                    1.0 // Default to full volume on error
                }
            }
        }
    }

    /// Check if the device is muted
    pub fn is_muted(&self) -> bool {
        unsafe {
            self.endpoint_volume
                .GetMute()
                .map(|m| m.as_bool())
                .unwrap_or(false)
        }
    }

    /// Get effective volume (0.0 if muted, otherwise master volume)
    pub fn get_effective_volume(&self) -> f32 {
        if self.is_muted() {
            0.0
        } else {
            self.get_volume()
        }
    }
}

// SAFETY: VolumeTracker uses COM interfaces that are safe to use
// from any thread when initialized with COINIT_MULTITHREADED
unsafe impl Send for VolumeTracker {}

/// Apply volume scaling to 32-bit float audio samples in-place
///
/// # Arguments
/// * `data` - Byte slice containing f32 samples (must be aligned to 4 bytes)
/// * `volume` - Volume level 0.0 - 1.0
#[inline]
pub fn apply_volume_f32(data: &mut [u8], volume: f32) {
    // Early exit for full volume
    if (volume - 1.0).abs() < f32::EPSILON {
        return;
    }

    // Process as f32 samples
    // SAFETY: Audio data is always 4-byte aligned (32-bit float format)
    let samples = unsafe {
        std::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut f32, data.len() / 4)
    };

    // Apply volume with SIMD-friendly loop
    for sample in samples.iter_mut() {
        *sample *= volume;
    }
}
