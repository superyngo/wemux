//! Audio capture, rendering, and synchronization

mod buffer;
mod capture;
mod engine;
mod renderer;

pub use buffer::RingBuffer;
pub use capture::LoopbackCapture;
pub use engine::{AudioEngine, EngineConfig, EngineState};
pub use renderer::{HdmiRenderer, RendererState};

/// Audio format information
#[derive(Debug, Clone)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub block_align: u16,
}

impl AudioFormat {
    /// Calculate bytes per second
    pub fn bytes_per_second(&self) -> u32 {
        self.sample_rate * self.block_align as u32
    }

    /// Calculate buffer size in bytes for given milliseconds
    pub fn buffer_size_for_ms(&self, ms: u32) -> usize {
        ((self.bytes_per_second() as u64 * ms as u64) / 1000) as usize
    }

    /// Calculate number of frames for given bytes
    pub fn bytes_to_frames(&self, bytes: usize) -> u32 {
        (bytes / self.block_align as usize) as u32
    }

    /// Calculate bytes for given number of frames
    pub fn frames_to_bytes(&self, frames: u32) -> usize {
        frames as usize * self.block_align as usize
    }
}

impl std::fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}Hz {}ch {}bit",
            self.sample_rate, self.channels, self.bits_per_sample
        )
    }
}
