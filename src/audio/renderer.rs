//! WASAPI render client for audio output to HDMI devices

use crate::audio::AudioFormat;
use crate::error::{Result, WemuxError};
use std::ptr;
use tracing::{debug, info, trace, warn};
use windows::{
    core::PCWSTR,
    Win32::{
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
        Foundation::{HANDLE, WAIT_OBJECT_0},
        Media::Audio::{
            IAudioClient, IAudioRenderClient, IMMDevice, AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
        },
        System::{
            Com::STGM_READ,
            Threading::{CreateEventW, WaitForSingleObject},
        },
    },
};

/// PROPVARIANT type for wide string pointers
const VT_LPWSTR: u16 = 31;

/// State of an HDMI renderer
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererState {
    /// Not started
    Idle,
    /// Running normally
    Running,
    /// Error occurred
    Error(String),
    /// Attempting to reconnect
    Reconnecting,
}

/// WASAPI render client for a single HDMI device
pub struct HdmiRenderer {
    device_id: String,
    device_name: String,
    audio_client: IAudioClient,
    render_client: IAudioRenderClient,
    format: AudioFormat,
    event: HANDLE,
    buffer_frames: u32,
    state: RendererState,
}

// SAFETY: HdmiRenderer is Send because WASAPI uses MTA (Multi-Threaded Apartment)
// and each thread initializes COM with COINIT_MULTITHREADED
unsafe impl Send for HdmiRenderer {}

impl HdmiRenderer {
    /// Create a new renderer for the given device
    pub fn new(device: &IMMDevice) -> Result<Self> {
        unsafe {
            // Get device ID
            let device_id = {
                let id_ptr = device.GetId()?;
                let id = PCWSTR(id_ptr.0).to_string().unwrap_or_default();
                windows::Win32::System::Com::CoTaskMemFree(Some(id_ptr.0 as *const _));
                id
            };

            // Get device name
            let device_name =
                Self::get_device_name(device).unwrap_or_else(|| "Unknown".to_string());

            debug!("Creating renderer for: {} ({})", device_name, device_id);

            // Activate audio client
            let audio_client: IAudioClient =
                device.Activate(windows::Win32::System::Com::CLSCTX_ALL, None)?;

            // Get mix format
            let format_ptr = audio_client.GetMixFormat()?;
            let format_ref = &*format_ptr;

            let format = AudioFormat {
                sample_rate: format_ref.nSamplesPerSec,
                channels: format_ref.nChannels,
                bits_per_sample: format_ref.wBitsPerSample,
                block_align: format_ref.nBlockAlign,
            };

            info!("Renderer format for {}: {}", device_name, format);

            // Create event for buffer notification
            let event = CreateEventW(None, false, false, None)?;

            // Auto-calculate optimal buffer duration based on hardware capabilities
            let buffer_duration = crate::audio::HardwareCapabilities::query(&audio_client)
                .map(|caps| caps.optimal_buffer_duration())
                .unwrap_or_else(|e| {
                    debug!(
                        "Failed to query hardware capabilities: {}, using default 35ms",
                        e
                    );
                    350_000i64 // 35ms fallback
                });

            audio_client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                buffer_duration,
                0,
                format_ptr,
                None,
            )?;

            // Set event handle
            audio_client.SetEventHandle(event)?;

            // Get buffer size
            let buffer_frames = audio_client.GetBufferSize()?;
            debug!(
                "Renderer {} buffer size: {} frames",
                device_name, buffer_frames
            );

            // Get render client
            let render_client: IAudioRenderClient = audio_client.GetService()?;

            // Free format memory
            windows::Win32::System::Com::CoTaskMemFree(Some(format_ptr as *const _ as *const _));

            Ok(Self {
                device_id,
                device_name,
                audio_client,
                render_client,
                format,
                event,
                buffer_frames,
                state: RendererState::Idle,
            })
        }
    }

    fn get_device_name(device: &IMMDevice) -> Option<String> {
        unsafe {
            let store = device.OpenPropertyStore(STGM_READ).ok()?;
            let prop = store.GetValue(&PKEY_Device_FriendlyName).ok()?;

            // Extract string from PROPVARIANT using repr(C) struct
            #[repr(C)]
            struct PropVariantRaw {
                vt: u16,
                w_reserved1: u16,
                w_reserved2: u16,
                w_reserved3: u16,
                data: *const u16,
            }

            let raw = &*((&prop) as *const windows_core::PROPVARIANT as *const PropVariantRaw);
            if raw.vt == VT_LPWSTR && !raw.data.is_null() {
                return PCWSTR(raw.data).to_string().ok();
            }
            None
        }
    }

    /// Get device ID
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// Get device name
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Get the audio format
    pub fn format(&self) -> &AudioFormat {
        &self.format
    }

    /// Get current state
    pub fn state(&self) -> &RendererState {
        &self.state
    }

    /// Get buffer size in frames
    pub fn buffer_frames(&self) -> u32 {
        self.buffer_frames
    }

    /// Start rendering
    pub fn start(&mut self) -> Result<()> {
        if self.state == RendererState::Running {
            return Ok(());
        }

        unsafe {
            self.audio_client.Start()?;
            self.state = RendererState::Running;
            info!("Renderer started: {}", self.device_name);
            Ok(())
        }
    }

    /// Stop rendering
    pub fn stop(&mut self) -> Result<()> {
        if self.state != RendererState::Running {
            return Ok(());
        }

        unsafe {
            self.audio_client.Stop()?;
            self.state = RendererState::Idle;
            info!("Renderer stopped: {}", self.device_name);
            Ok(())
        }
    }

    /// Wait for buffer space and write frames
    ///
    /// Returns the number of frames written
    pub fn write_frames(&mut self, data: &[u8], timeout_ms: u32) -> Result<u32> {
        if self.state != RendererState::Running {
            return Err(WemuxError::device_error(
                &self.device_id,
                "Renderer not running",
            ));
        }

        unsafe {
            // Wait for buffer event
            let wait_result = WaitForSingleObject(self.event, timeout_ms);
            if wait_result != WAIT_OBJECT_0 {
                trace!("Renderer {} wait timeout", self.device_name);
                return Ok(0);
            }

            // Get padding (frames already in buffer)
            let padding = self.audio_client.GetCurrentPadding()?;
            let available_frames = self.buffer_frames - padding;

            if available_frames == 0 {
                return Ok(0);
            }

            // Calculate how many frames we can write
            let data_frames = self.format.bytes_to_frames(data.len());
            let frames_to_write = data_frames.min(available_frames);

            if frames_to_write == 0 {
                return Ok(0);
            }

            // Get buffer
            let buffer_ptr = self.render_client.GetBuffer(frames_to_write)?;

            // Copy data
            let bytes_to_write = self.format.frames_to_bytes(frames_to_write);
            ptr::copy_nonoverlapping(data.as_ptr(), buffer_ptr, bytes_to_write);

            // Release buffer
            self.render_client.ReleaseBuffer(frames_to_write, 0)?;

            trace!(
                "Renderer {} wrote {} frames",
                self.device_name,
                frames_to_write
            );

            Ok(frames_to_write)
        }
    }

    /// Write silence to fill the buffer
    pub fn write_silence(&mut self, frames: u32) -> Result<()> {
        if self.state != RendererState::Running {
            return Ok(());
        }

        unsafe {
            let padding = self.audio_client.GetCurrentPadding()?;
            let available = self.buffer_frames - padding;
            let to_write = frames.min(available);

            if to_write == 0 {
                return Ok(());
            }

            let buffer_ptr = self.render_client.GetBuffer(to_write)?;
            ptr::write_bytes(buffer_ptr, 0, self.format.frames_to_bytes(to_write));
            self.render_client.ReleaseBuffer(
                to_write,
                windows::Win32::Media::Audio::AUDCLNT_BUFFERFLAGS_SILENT.0 as u32,
            )?;

            Ok(())
        }
    }

    /// Get current buffer position for synchronization
    pub fn get_buffer_position(&self) -> Result<u64> {
        unsafe {
            let mut _position: u64 = 0;
            let mut _qpc: u64 = 0;

            // Note: This requires AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM to be useful
            // For now, we use padding as a proxy
            let padding = self.audio_client.GetCurrentPadding()?;
            Ok(padding as u64)
        }
    }

    /// Set error state
    pub fn set_error(&mut self, message: &str) {
        warn!("Renderer {} error: {}", self.device_name, message);
        self.state = RendererState::Error(message.to_string());
    }

    /// Set reconnecting state
    pub fn set_reconnecting(&mut self) {
        info!("Renderer {} reconnecting...", self.device_name);
        self.state = RendererState::Reconnecting;
    }
}

impl Drop for HdmiRenderer {
    fn drop(&mut self) {
        let _ = self.stop();
        unsafe {
            if !self.event.is_invalid() {
                let _ = windows::Win32::Foundation::CloseHandle(self.event);
            }
        }
    }
}
