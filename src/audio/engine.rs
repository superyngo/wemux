//! Audio engine - main controller coordinating capture and renderers

use crate::audio::buffer::ReaderState;
use crate::audio::{AudioFormat, HdmiRenderer, LoopbackCapture, RingBuffer};
use crate::device::{DeviceEnumerator, DeviceInfo};
use crate::error::{Result, WemuxError};
use crate::sync::ClockSync;
use crossbeam_channel::{bounded, Sender};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tracing::{error, info, warn};

/// Engine configuration
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Buffer size in milliseconds
    pub buffer_ms: u32,
    /// Specific device IDs to use (None = auto-detect all HDMI)
    pub device_ids: Option<Vec<String>>,
    /// Device IDs to exclude
    pub exclude_ids: Option<Vec<String>>,
    /// Source device ID for loopback (None = system default)
    pub source_device_id: Option<String>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            buffer_ms: 50,
            device_ids: None,
            exclude_ids: None,
            source_device_id: None,
        }
    }
}

/// Engine state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    /// Not initialized
    Uninitialized,
    /// Initialized but not running
    Stopped,
    /// Running
    Running,
    /// Shutting down
    ShuttingDown,
}

/// Command sent to worker threads
enum EngineCommand {
    Stop,
}

/// Audio engine coordinating capture and multiple renderers
pub struct AudioEngine {
    config: EngineConfig,
    state: Arc<Mutex<EngineState>>,
    stop_flag: Arc<AtomicBool>,
    capture_handle: Option<JoinHandle<()>>,
    render_handles: Vec<JoinHandle<()>>,
    command_tx: Option<Sender<EngineCommand>>,
    buffer: Option<Arc<RingBuffer>>,
    format: Option<AudioFormat>,
}

impl AudioEngine {
    /// Create a new audio engine with the given configuration
    pub fn new(config: EngineConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(EngineState::Uninitialized)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            capture_handle: None,
            render_handles: Vec::new(),
            command_tx: None,
            buffer: None,
            format: None,
        }
    }

    /// Get current engine state
    pub fn state(&self) -> EngineState {
        *self.state.lock()
    }

    /// Get the audio format (available after initialization)
    pub fn format(&self) -> Option<&AudioFormat> {
        self.format.as_ref()
    }

    /// Initialize and start the engine
    pub fn start(&mut self) -> Result<()> {
        {
            let state = self.state.lock();
            if *state == EngineState::Running {
                return Err(WemuxError::AlreadyRunning);
            }
        }

        info!("Starting audio engine...");

        // Reset stop flag
        self.stop_flag.store(false, Ordering::SeqCst);

        // Create loopback capture
        let capture = LoopbackCapture::from_default_device()?;
        let format = capture.format().clone();
        self.format = Some(format.clone());

        info!("Capture format: {}", format);

        // Create ring buffer (enough for 500ms of audio)
        let buffer_size = format.buffer_size_for_ms(500);
        let buffer = Arc::new(RingBuffer::new(buffer_size));
        self.buffer = Some(buffer.clone());

        // Enumerate and create renderers
        let enumerator = DeviceEnumerator::new()?;
        let hdmi_devices = self.get_target_devices(&enumerator)?;

        if hdmi_devices.is_empty() {
            return Err(WemuxError::NoHdmiDevices);
        }

        info!("Found {} HDMI devices:", hdmi_devices.len());
        for device in &hdmi_devices {
            info!("  - {}", device.name);
        }

        // Create clock sync
        let clock_sync = Arc::new(Mutex::new(ClockSync::new(format.sample_rate)));

        // Create command channel
        let (cmd_tx, _cmd_rx) = bounded::<EngineCommand>(16);
        self.command_tx = Some(cmd_tx);

        // Start capture thread
        let capture_buffer = buffer.clone();
        let capture_stop = self.stop_flag.clone();
        let capture_state = self.state.clone();

        self.capture_handle = Some(thread::spawn(move || {
            capture_thread(capture, capture_buffer, capture_stop, capture_state);
        }));

        // Start renderer threads
        let mut first_device = true;
        for device_info in hdmi_devices {
            let device = enumerator.get_device_by_id(&device_info.id)?;
            let renderer = HdmiRenderer::new(&device)?;

            // Set first device as master
            if first_device {
                clock_sync.lock().set_master(&device_info.id);
                first_device = false;
            } else {
                clock_sync.lock().register_slave(&device_info.id);
            }

            let render_buffer = buffer.clone();
            let render_stop = self.stop_flag.clone();
            let render_clock = clock_sync.clone();
            let render_format = format.clone();

            let handle = thread::spawn(move || {
                render_thread(
                    renderer,
                    render_buffer,
                    render_stop,
                    render_clock,
                    render_format,
                );
            });

            self.render_handles.push(handle);
        }

        *self.state.lock() = EngineState::Running;
        info!("Audio engine started");

        Ok(())
    }

    /// Stop the engine
    pub fn stop(&mut self) -> Result<()> {
        {
            let mut state = self.state.lock();
            if *state != EngineState::Running {
                return Ok(());
            }
            *state = EngineState::ShuttingDown;
        }

        info!("Stopping audio engine...");

        // Signal threads to stop
        self.stop_flag.store(true, Ordering::SeqCst);

        // Send stop command
        if let Some(tx) = &self.command_tx {
            let _ = tx.send(EngineCommand::Stop);
        }

        // Wait for capture thread
        if let Some(handle) = self.capture_handle.take() {
            let _ = handle.join();
        }

        // Wait for render threads
        for handle in self.render_handles.drain(..) {
            let _ = handle.join();
        }

        *self.state.lock() = EngineState::Stopped;
        info!("Audio engine stopped");

        Ok(())
    }

    /// Get target HDMI devices based on configuration
    fn get_target_devices(&self, enumerator: &DeviceEnumerator) -> Result<Vec<DeviceInfo>> {
        let mut devices = if let Some(ids) = &self.config.device_ids {
            // Use specified devices
            let all_devices = enumerator.enumerate_all_devices()?;
            all_devices
                .into_iter()
                .filter(|d| {
                    ids.iter()
                        .any(|id| d.id.contains(id) || d.name.contains(id))
                })
                .collect()
        } else {
            // Auto-detect HDMI devices
            enumerator.enumerate_hdmi_devices().unwrap_or_default()
        };

        // Apply exclusions
        if let Some(excludes) = &self.config.exclude_ids {
            devices.retain(|d| {
                !excludes
                    .iter()
                    .any(|ex| d.id.contains(ex) || d.name.contains(ex))
            });
        }

        Ok(devices)
    }

    /// Check if engine is running
    pub fn is_running(&self) -> bool {
        *self.state.lock() == EngineState::Running
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Capture thread function
fn capture_thread(
    mut capture: LoopbackCapture,
    buffer: Arc<RingBuffer>,
    stop_flag: Arc<AtomicBool>,
    _state: Arc<Mutex<EngineState>>,
) {
    info!("Capture thread started");

    if let Err(e) = capture.start() {
        error!("Failed to start capture: {}", e);
        return;
    }

    let mut temp_buffer = vec![0u8; 4096];

    while !stop_flag.load(Ordering::Relaxed) {
        match capture.read_frames(100) {
            Ok(frames) => {
                if !frames.is_empty() {
                    let bytes = frames.copy_to(&mut temp_buffer);
                    buffer.write(&temp_buffer[..bytes]);
                }
            }
            Err(e) => {
                warn!("Capture error: {}", e);
                // Brief pause before retry
                thread::sleep(Duration::from_millis(10));
            }
        }
    }

    let _ = capture.stop();
    info!("Capture thread stopped");
}

/// Render thread function
fn render_thread(
    mut renderer: HdmiRenderer,
    buffer: Arc<RingBuffer>,
    stop_flag: Arc<AtomicBool>,
    clock_sync: Arc<Mutex<ClockSync>>,
    format: AudioFormat,
) {
    let device_name = renderer.device_name().to_string();
    let device_id = renderer.device_id().to_string();
    info!("Render thread started for: {}", device_name);

    if let Err(e) = renderer.start() {
        error!("Failed to start renderer {}: {}", device_name, e);
        return;
    }

    // Create reader state for this renderer
    let mut reader = ReaderState::new(&buffer);
    let mut render_buffer = vec![0u8; format.buffer_size_for_ms(50)];

    // Pre-fill with silence to establish latency buffer
    let _ =
        renderer.write_silence(format.buffer_size_for_ms(20) as u32 / format.block_align as u32);

    while !stop_flag.load(Ordering::Relaxed) {
        // Check for buffer underrun/overrun
        if reader.is_lagging(&buffer) {
            warn!("Renderer {} buffer overrun, catching up", device_name);
            reader.catch_up(&buffer);
        }

        // Read available data
        let available = reader.available(&buffer);
        if available == 0 {
            // No data available, write silence
            let _ = renderer.write_silence(480); // 10ms of silence
            thread::sleep(Duration::from_millis(5));
            continue;
        }

        // Read and write
        let to_read = available.min(render_buffer.len());
        let read = reader.read(&buffer, &mut render_buffer[..to_read]);

        if read > 0 {
            // Apply clock sync correction
            let correction = clock_sync.lock().get_correction(&device_id);

            // For now, skip samples if ahead (positive correction)
            // In a more sophisticated implementation, we'd do sample rate conversion
            let adjusted_data = if correction > 0 {
                let skip_bytes = (correction as usize * format.block_align as usize).min(read);
                &render_buffer[skip_bytes..read]
            } else {
                &render_buffer[..read]
            };

            match renderer.write_frames(adjusted_data, 50) {
                Ok(_frames) => {
                    // Update clock sync
                    if let Ok(pos) = renderer.get_buffer_position() {
                        let mut sync = clock_sync.lock();
                        if sync.is_master(&device_id) {
                            sync.update_master(pos);
                        } else {
                            sync.update_slave(&device_id, pos);
                        }
                    }
                }
                Err(e) => {
                    warn!("Renderer {} write error: {}", device_name, e);
                    renderer.set_error(&e.to_string());
                    // Brief pause before retry
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }

    let _ = renderer.stop();
    info!("Render thread stopped for: {}", device_name);
}
