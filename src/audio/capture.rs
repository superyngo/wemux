//! WASAPI loopback capture from system audio output

use crate::audio::AudioFormat;
use crate::error::Result;
use std::ptr;
use tracing::{debug, info, trace};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{HANDLE, WAIT_OBJECT_0},
        Media::Audio::{
            eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDevice, IMMDeviceEnumerator,
            MMDeviceEnumerator, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK,
        },
        System::{
            Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED},
            Threading::{CreateEventW, WaitForSingleObject},
        },
    },
};

/// WASAPI loopback capture for capturing system audio output
pub struct LoopbackCapture {
    audio_client: IAudioClient,
    capture_client: IAudioCaptureClient,
    format: AudioFormat,
    event: HANDLE,
    buffer_frames: u32,
    started: bool,
}

// SAFETY: LoopbackCapture is Send because WASAPI uses MTA (Multi-Threaded Apartment)
// and each thread initializes COM with COINIT_MULTITHREADED
unsafe impl Send for LoopbackCapture {}

impl LoopbackCapture {
    /// Create a loopback capture from the system default render device
    pub fn from_default_device() -> Result<Self> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

            let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;

            Self::from_device(&device)
        }
    }

    /// Create a loopback capture from a specific device
    pub fn from_device(device: &IMMDevice) -> Result<Self> {
        unsafe {
            // Get device ID for logging
            let device_id = {
                let id_ptr = device.GetId()?;
                let id = PCWSTR(id_ptr.0).to_string().unwrap_or_default();
                windows::Win32::System::Com::CoTaskMemFree(Some(id_ptr.0 as *const _));
                id
            };
            debug!("Creating loopback capture for device: {}", device_id);

            // Activate audio client
            let audio_client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;

            // Get mix format
            let format_ptr = audio_client.GetMixFormat()?;
            let format_ref = &*format_ptr;

            let format = AudioFormat {
                sample_rate: format_ref.nSamplesPerSec,
                channels: format_ref.nChannels,
                bits_per_sample: format_ref.wBitsPerSample,
                block_align: format_ref.nBlockAlign,
            };

            info!("Capture format: {}", format);

            // Create event for buffer notification
            let event = CreateEventW(None, false, false, None)?;

            // Initialize audio client in shared mode with loopback
            let buffer_duration = 500_000i64; // 50ms in 100-nanosecond units

            audio_client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                buffer_duration,
                0,
                format_ptr,
                None,
            )?;

            // Set event handle
            audio_client.SetEventHandle(event)?;

            // Get buffer size
            let buffer_frames = audio_client.GetBufferSize()?;
            debug!("Capture buffer size: {} frames", buffer_frames);

            // Get capture client
            let capture_client: IAudioCaptureClient = audio_client.GetService()?;

            // Free format memory
            windows::Win32::System::Com::CoTaskMemFree(Some(format_ptr as *const _ as *const _));

            Ok(Self {
                audio_client,
                capture_client,
                format,
                event,
                buffer_frames,
                started: false,
            })
        }
    }

    /// Get the audio format
    pub fn format(&self) -> &AudioFormat {
        &self.format
    }

    /// Get buffer size in frames
    pub fn buffer_frames(&self) -> u32 {
        self.buffer_frames
    }

    /// Start capturing
    pub fn start(&mut self) -> Result<()> {
        if self.started {
            return Ok(());
        }

        unsafe {
            self.audio_client.Start()?;
            self.started = true;
            info!("Loopback capture started");
            Ok(())
        }
    }

    /// Stop capturing
    pub fn stop(&mut self) -> Result<()> {
        if !self.started {
            return Ok(());
        }

        unsafe {
            self.audio_client.Stop()?;
            self.started = false;
            info!("Loopback capture stopped");
            Ok(())
        }
    }

    /// Wait for and read available frames
    ///
    /// Returns the audio data as a byte slice.
    /// The data is only valid until the next call to `read_frames` or `release_buffer`.
    pub fn read_frames(&self, timeout_ms: u32) -> Result<CapturedFrames<'_>> {
        unsafe {
            // Wait for buffer event
            let wait_result = WaitForSingleObject(self.event, timeout_ms);
            if wait_result != WAIT_OBJECT_0 {
                return Ok(CapturedFrames::empty());
            }

            // Get buffer
            let mut data_ptr: *mut u8 = ptr::null_mut();
            let mut num_frames: u32 = 0;
            let mut flags: u32 = 0;
            let mut device_position: u64 = 0;
            let mut qpc_position: u64 = 0;

            self.capture_client.GetBuffer(
                &mut data_ptr,
                &mut num_frames,
                &mut flags,
                Some(&mut device_position),
                Some(&mut qpc_position),
            )?;

            if num_frames == 0 {
                return Ok(CapturedFrames::empty());
            }

            let is_silent = (flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0;
            let byte_count = num_frames as usize * self.format.block_align as usize;

            trace!(
                "Captured {} frames ({} bytes), silent={}",
                num_frames,
                byte_count,
                is_silent
            );

            Ok(CapturedFrames {
                capture_client: &self.capture_client,
                data: if is_silent {
                    None
                } else {
                    Some(std::slice::from_raw_parts(data_ptr, byte_count))
                },
                num_frames,
                is_silent,
                block_align: self.format.block_align,
            })
        }
    }

    /// Check if capture is running
    pub fn is_running(&self) -> bool {
        self.started
    }
}

impl Drop for LoopbackCapture {
    fn drop(&mut self) {
        let _ = self.stop();
        unsafe {
            if !self.event.is_invalid() {
                let _ = windows::Win32::Foundation::CloseHandle(self.event);
            }
        }
    }
}

/// Captured audio frames with automatic buffer release
pub struct CapturedFrames<'a> {
    capture_client: &'a IAudioCaptureClient,
    data: Option<&'a [u8]>,
    num_frames: u32,
    is_silent: bool,
    block_align: u16,
}

impl<'a> CapturedFrames<'a> {
    #[allow(invalid_value)]
    fn empty() -> Self {
        Self {
            // SAFETY: This is only used when data is None and will never be dereferenced
            capture_client: unsafe { std::mem::zeroed() },
            data: None,
            num_frames: 0,
            is_silent: true,
            block_align: 0,
        }
    }

    /// Get the captured audio data
    ///
    /// Returns None if the buffer was silent (data would be all zeros)
    pub fn data(&self) -> Option<&[u8]> {
        self.data
    }

    /// Get number of frames captured
    pub fn num_frames(&self) -> u32 {
        self.num_frames
    }

    /// Check if this buffer contains silence
    pub fn is_silent(&self) -> bool {
        self.is_silent
    }

    /// Check if this is an empty (no data) result
    pub fn is_empty(&self) -> bool {
        self.num_frames == 0
    }

    /// Get byte count
    pub fn byte_count(&self) -> usize {
        self.num_frames as usize * self.block_align as usize
    }

    /// Copy data to a buffer, filling with zeros if silent
    pub fn copy_to(&self, dest: &mut [u8]) -> usize {
        let count = dest.len().min(self.byte_count());
        if let Some(data) = self.data {
            dest[..count].copy_from_slice(&data[..count]);
        } else {
            dest[..count].fill(0);
        }
        count
    }
}

impl<'a> Drop for CapturedFrames<'a> {
    fn drop(&mut self) {
        if self.num_frames > 0 {
            unsafe {
                let _ = self.capture_client.ReleaseBuffer(self.num_frames);
            }
        }
    }
}
