//! Audio engine - main controller coordinating capture and renderers

use crate::audio::buffer::ReaderState;
use crate::audio::volume::{apply_volume_f32, VolumeLevel, VolumeTracker};
use crate::audio::{AudioFormat, HdmiRenderer, LoopbackCapture, RingBuffer};
use crate::device::{DeviceEnumerator, DeviceEvent, DeviceInfo, DeviceMonitor};
use crate::error::{Result, WemuxError};
use crate::sync::ClockSync;
use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Device status for external control
#[derive(Debug, Clone)]
pub struct DeviceStatus {
    /// Device ID
    pub id: String,
    /// Device name
    pub name: String,
    /// Whether the device is enabled for rendering
    pub is_enabled: bool,
    /// Whether the device is paused by user
    pub is_paused: bool,
    /// Whether this device is the current system default output (auto-paused, cannot be controlled)
    pub is_system_default: bool,
}

/// Engine configuration
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Buffer size in milliseconds
    pub buffer_ms: u32,
    /// Specific device IDs to use (None = auto-detect all output devices)
    pub device_ids: Option<Vec<String>>,
    /// Device IDs to exclude (system default will be auto-excluded)
    pub exclude_ids: Option<Vec<String>>,
    /// Source device ID for loopback (None = system default)
    pub source_device_id: Option<String>,
    /// Device IDs that should start paused (disabled in settings)
    pub paused_device_ids: Option<Vec<String>>,
    /// Use all output devices instead of HDMI only
    pub use_all_devices: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            buffer_ms: 50,
            device_ids: None,
            exclude_ids: None,
            source_device_id: None,
            paused_device_ids: None,
            use_all_devices: false,
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

/// Command sent to capture thread
enum CaptureCommand {
    /// Reinitialize capture to current default device
    Reinitialize,
}

/// Control for individual renderer threads
#[derive(Clone)]
struct RendererControl {
    /// Flag to pause this renderer (keeps thread alive but silent)
    paused: Arc<AtomicBool>,
}

/// Events from the engine that external controllers might care about
#[derive(Debug, Clone)]
pub enum EngineEvent {
    /// Default audio device changed - UI should refresh
    DefaultDeviceChanged,
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
    volume_level: Arc<VolumeLevel>,
    volume_handle: Option<JoinHandle<()>>,
    // Device monitoring
    device_monitor: Option<DeviceMonitor>,
    monitor_handle: Option<JoinHandle<()>>,
    renderer_controls: Arc<Mutex<HashMap<String, RendererControl>>>,
    capture_cmd_tx: Option<Sender<CaptureCommand>>,
    // Track current default device and device names for external control
    current_default_id: Arc<Mutex<Option<String>>>,
    device_names: Arc<Mutex<HashMap<String, String>>>,
    // Event notification channel for external listeners
    event_tx: Option<Sender<EngineEvent>>,
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
            volume_level: Arc::new(VolumeLevel::new()),
            volume_handle: None,
            device_monitor: None,
            monitor_handle: None,
            renderer_controls: Arc::new(Mutex::new(HashMap::new())),
            capture_cmd_tx: None,
            current_default_id: Arc::new(Mutex::new(None)),
            device_names: Arc::new(Mutex::new(HashMap::new())),
            event_tx: None,
        }
    }

    /// Set an event notification channel
    /// Events will be sent when things like default device changes occur
    pub fn set_event_channel(&mut self, tx: Sender<EngineEvent>) {
        self.event_tx = Some(tx);
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

        // Create loopback capture (just to get format, will be recreated in thread)
        let capture = LoopbackCapture::from_default_device()?;
        let format = capture.format().clone();
        self.format = Some(format.clone());
        drop(capture); // Release the capture, thread will create its own

        info!("Capture format: {}", format);

        // Create ring buffer (enough for 500ms of audio)
        let buffer_size = format.buffer_size_for_ms(500);
        let buffer = Arc::new(RingBuffer::new(buffer_size));
        self.buffer = Some(buffer.clone());

        // Enumerate and create renderers
        let enumerator = DeviceEnumerator::new()?;
        let target_devices = self.get_target_devices(&enumerator)?;

        if target_devices.is_empty() {
            return Err(WemuxError::NoHdmiDevices);
        }

        let device_type = if self.config.use_all_devices {
            "output"
        } else {
            "HDMI"
        };
        info!("Found {} {} devices:", target_devices.len(), device_type);
        for device in &target_devices {
            info!("  - {}", device.name);
        }

        // Create clock sync
        let clock_sync = Arc::new(Mutex::new(ClockSync::new(format.sample_rate)));

        // Create command channel
        let (cmd_tx, _cmd_rx) = bounded::<EngineCommand>(16);
        self.command_tx = Some(cmd_tx);

        // Create capture command channel
        let (capture_cmd_tx, capture_cmd_rx) = bounded::<CaptureCommand>(16);
        self.capture_cmd_tx = Some(capture_cmd_tx.clone());

        // Start capture thread
        let capture_buffer = buffer.clone();
        let capture_stop = self.stop_flag.clone();

        self.capture_handle = Some(thread::spawn(move || {
            capture_thread(capture_buffer, capture_stop, capture_cmd_rx);
        }));

        // Create device monitor
        let (device_event_tx, device_event_rx) = bounded::<DeviceEvent>(64);
        self.device_monitor = Some(DeviceMonitor::new(device_event_tx)?);
        info!("Device enumerator initialized");

        // Create channel for volume tracker device events
        let (volume_event_tx, volume_event_rx) = bounded::<DeviceEvent>(16);

        // Start volume tracking thread
        let volume_level = self.volume_level.clone();
        let volume_stop = self.stop_flag.clone();

        self.volume_handle = Some(thread::spawn(move || {
            volume_tracking_thread(volume_level, volume_stop, volume_event_rx);
        }));

        // Clear renderer controls and device names
        self.renderer_controls.lock().clear();
        self.device_names.lock().clear();

        // Get current default device ID for checking during renderer setup
        let default_device_id = enumerator
            .get_default_render_device()
            .ok()
            .and_then(|d| unsafe {
                d.GetId().ok().and_then(|id_ptr| {
                    let id = windows::core::PCWSTR(id_ptr.0).to_string().ok();
                    windows::Win32::System::Com::CoTaskMemFree(Some(id_ptr.0 as *const _));
                    id
                })
            });

        // Store current default device ID
        *self.current_default_id.lock() = default_device_id.clone();

        // Start renderer threads
        let mut first_device = true;
        for device_info in target_devices {
            let device = enumerator.get_device_by_id(&device_info.id)?;
            let renderer = HdmiRenderer::new(&device)?;

            // Set first device as master
            if first_device {
                clock_sync.lock().set_master(&device_info.id);
                first_device = false;
            } else {
                clock_sync.lock().register_slave(&device_info.id);
            }

            // Create renderer control - start paused if:
            // 1. This device is the default output (to prevent feedback)
            // 2. This device is in the paused_device_ids list (from settings)
            let is_default = default_device_id
                .as_ref()
                .map(|id| id == &device_info.id)
                .unwrap_or(false);

            let should_pause_from_config = self.should_device_start_paused(&device_info.id);
            let should_start_paused = is_default || should_pause_from_config;

            if is_default {
                info!(
                    "Device {} is the default output, starting paused",
                    device_info.name
                );
            } else if should_pause_from_config {
                info!(
                    "Device {} is disabled in settings, starting paused",
                    device_info.name
                );
            }

            let paused_flag = Arc::new(AtomicBool::new(should_start_paused));
            let renderer_control = RendererControl {
                paused: paused_flag.clone(),
            };
            self.renderer_controls
                .lock()
                .insert(device_info.id.clone(), renderer_control);

            // Store device name for external control
            self.device_names
                .lock()
                .insert(device_info.id.clone(), device_info.name.clone());

            let render_buffer = buffer.clone();
            let render_stop = self.stop_flag.clone();
            let render_clock = clock_sync.clone();
            let render_format = format.clone();
            let render_volume = self.volume_level.clone();

            let handle = thread::spawn(move || {
                render_thread(
                    renderer,
                    render_buffer,
                    render_stop,
                    paused_flag,
                    render_clock,
                    render_format,
                    render_volume,
                );
            });

            self.render_handles.push(handle);
        }

        // Start device monitor thread
        let monitor_controls = self.renderer_controls.clone();
        let monitor_stop = self.stop_flag.clone();
        let monitor_default_id = self.current_default_id.clone();
        let monitor_event_tx = self.event_tx.clone();

        self.monitor_handle = Some(thread::spawn(move || {
            device_monitor_thread(
                device_event_rx,
                monitor_controls,
                capture_cmd_tx,
                volume_event_tx,
                monitor_stop,
                monitor_default_id,
                monitor_event_tx,
            );
        }));

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

        // Drop device monitor first (unregisters COM callback)
        // This must happen before waiting for monitor thread
        self.device_monitor = None;

        // Wait for capture thread
        if let Some(handle) = self.capture_handle.take() {
            let _ = handle.join();
        }

        // Wait for volume tracking thread
        if let Some(handle) = self.volume_handle.take() {
            let _ = handle.join();
        }

        // Wait for device monitor thread
        if let Some(handle) = self.monitor_handle.take() {
            let _ = handle.join();
        }

        // Wait for render threads
        for handle in self.render_handles.drain(..) {
            let _ = handle.join();
        }

        // Clear renderer controls and device names
        self.renderer_controls.lock().clear();
        self.device_names.lock().clear();

        // Clear channels
        self.command_tx = None;
        self.capture_cmd_tx = None;

        // Clear buffer and format
        self.buffer = None;
        self.format = None;

        // Clear current default device
        *self.current_default_id.lock() = None;

        *self.state.lock() = EngineState::Stopped;
        info!("Audio engine stopped");

        Ok(())
    }

    /// Get target devices based on configuration
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
        } else if self.config.use_all_devices {
            // Use all output devices
            enumerator.enumerate_all_devices()?
        } else {
            // Auto-detect HDMI devices only (legacy behavior)
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

    /// Check if a device should start paused based on config
    fn should_device_start_paused(&self, device_id: &str) -> bool {
        if let Some(paused_ids) = &self.config.paused_device_ids {
            paused_ids.iter().any(|id| id == device_id)
        } else {
            false
        }
    }

    /// Check if engine is running
    pub fn is_running(&self) -> bool {
        *self.state.lock() == EngineState::Running
    }

    /// Get status of all active renderers
    pub fn get_device_statuses(&self) -> Vec<DeviceStatus> {
        let controls = self.renderer_controls.lock();
        let names = self.device_names.lock();
        let current_default = self.current_default_id.lock();

        controls
            .iter()
            .map(|(id, control)| {
                let is_system_default = current_default.as_ref().map(|d| d == id).unwrap_or(false);
                DeviceStatus {
                    id: id.clone(),
                    name: names.get(id).cloned().unwrap_or_else(|| id.clone()),
                    is_enabled: true, // In active renderers = enabled
                    is_paused: control.paused.load(Ordering::Relaxed),
                    is_system_default,
                }
            })
            .collect()
    }

    /// Pause a specific renderer
    pub fn pause_renderer(&self, device_id: &str) -> Result<()> {
        let controls = self.renderer_controls.lock();
        if let Some(control) = controls.get(device_id) {
            control.paused.store(true, Ordering::SeqCst);
            debug!("Paused renderer: {}", device_id);
            Ok(())
        } else {
            Err(WemuxError::DeviceNotFound(device_id.to_string()))
        }
    }

    /// Resume a specific renderer
    pub fn resume_renderer(&self, device_id: &str) -> Result<()> {
        let controls = self.renderer_controls.lock();
        if let Some(control) = controls.get(device_id) {
            control.paused.store(false, Ordering::SeqCst);
            debug!("Resumed renderer: {}", device_id);
            Ok(())
        } else {
            Err(WemuxError::DeviceNotFound(device_id.to_string()))
        }
    }

    /// Check if a device is the current default output
    pub fn is_device_default(&self, device_id: &str) -> bool {
        self.current_default_id
            .lock()
            .as_ref()
            .map(|id| id == device_id)
            .unwrap_or(false)
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Capture thread function
fn capture_thread(
    buffer: Arc<RingBuffer>,
    stop_flag: Arc<AtomicBool>,
    command_rx: Receiver<CaptureCommand>,
) {
    info!("Capture thread started");

    let mut capture = match LoopbackCapture::from_default_device() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create capture: {}", e);
            return;
        }
    };

    if let Err(e) = capture.start() {
        error!("Failed to start capture: {}", e);
        return;
    }

    let mut temp_buffer = vec![0u8; 4096];

    while !stop_flag.load(Ordering::Relaxed) {
        // Check for commands (non-blocking)
        if let Ok(cmd) = command_rx.try_recv() {
            match cmd {
                CaptureCommand::Reinitialize => {
                    info!("Reinitializing capture for new default device...");
                    let _ = capture.stop();

                    // Small delay to let Windows settle
                    thread::sleep(Duration::from_millis(100));

                    match LoopbackCapture::from_default_device() {
                        Ok(new_capture) => {
                            capture = new_capture;
                            if let Err(e) = capture.start() {
                                error!("Failed to start new capture: {}", e);
                                // Try to recover by sleeping and retrying
                                thread::sleep(Duration::from_millis(500));
                                continue;
                            }
                            info!("Capture reinitialized successfully");
                        }
                        Err(e) => {
                            error!("Failed to reinitialize capture: {}", e);
                            // Try to recover by recreating with old device
                            thread::sleep(Duration::from_millis(500));
                            continue;
                        }
                    }
                }
            }
        }

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

/// Volume tracking thread function
fn volume_tracking_thread(
    volume_level: Arc<VolumeLevel>,
    stop_flag: Arc<AtomicBool>,
    device_event_rx: Receiver<DeviceEvent>,
) {
    info!("Volume tracking thread started");

    // Initialize volume tracker
    let mut tracker = match VolumeTracker::from_default_device() {
        Ok(t) => t,
        Err(e) => {
            error!("Failed to initialize volume tracker: {}", e);
            return;
        }
    };

    while !stop_flag.load(Ordering::Relaxed) {
        // Check for device change events (non-blocking)
        if let Ok(DeviceEvent::DefaultChanged { .. }) = device_event_rx.try_recv() {
            info!("Reinitializing volume tracker for new default device...");
            // Small delay to let Windows settle
            thread::sleep(Duration::from_millis(100));
            match VolumeTracker::from_default_device() {
                Ok(new_tracker) => {
                    tracker = new_tracker;
                    info!("Volume tracker reinitialized successfully");
                }
                Err(e) => {
                    warn!("Failed to reinitialize volume tracker: {}", e);
                }
            }
        }

        let volume = tracker.get_effective_volume();
        volume_level.set(volume);

        // Poll every 100ms
        thread::sleep(Duration::from_millis(100));
    }

    info!("Volume tracking thread stopped");
}

/// Device monitor thread function
fn device_monitor_thread(
    event_rx: Receiver<DeviceEvent>,
    renderer_controls: Arc<Mutex<HashMap<String, RendererControl>>>,
    capture_cmd_tx: Sender<CaptureCommand>,
    volume_event_tx: Sender<DeviceEvent>,
    stop_flag: Arc<AtomicBool>,
    current_default_id: Arc<Mutex<Option<String>>>,
    engine_event_tx: Option<Sender<EngineEvent>>,
) {
    info!("Device monitor thread started");

    while !stop_flag.load(Ordering::Relaxed) {
        match event_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(event) => {
                if let DeviceEvent::DefaultChanged {
                    data_flow,
                    device_id,
                    ..
                } = &event
                {
                    // Only care about render devices (data_flow = 0 = eRender)
                    if *data_flow == 0 {
                        info!("Default render device changed to: {}", device_id);

                        // Update current default device ID
                        *current_default_id.lock() = Some(device_id.clone());

                        // 1. Notify capture to reinitialize
                        if let Err(e) = capture_cmd_tx.send(CaptureCommand::Reinitialize) {
                            warn!("Failed to send reinitialize command: {}", e);
                        }

                        // 2. Notify volume tracker to reinitialize
                        let _ = volume_event_tx.send(event.clone());

                        // 3. Check if new default is one of our HDMI renderers
                        let controls = renderer_controls.lock();
                        let mut found_match = false;

                        for (id, control) in controls.iter() {
                            if id == device_id {
                                // This renderer's device is now the default output
                                // Pause it to avoid echo/feedback
                                info!("Pausing renderer for device: {} (now default output)", id);
                                control.paused.store(true, Ordering::SeqCst);
                                found_match = true;
                            } else {
                                // Resume other renderers that were auto-paused due to being system default
                                // Note: We don't resume here as we want user-paused devices to stay paused
                                // The paused flag is only auto-set when device becomes default
                            }
                        }

                        if !found_match {
                            // Default changed to non-HDMI device, resume all renderers
                            debug!("Default device is not an HDMI renderer, all renderers active");
                        }

                        // 4. Notify external listeners (UI) to refresh
                        if let Some(ref tx) = engine_event_tx {
                            let _ = tx.send(EngineEvent::DefaultDeviceChanged);
                        }
                    }
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                // Normal timeout, continue loop
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                info!("Device monitor channel disconnected");
                break;
            }
        }
    }

    info!("Device monitor thread stopped");
}

/// Render thread function
fn render_thread(
    mut renderer: HdmiRenderer,
    buffer: Arc<RingBuffer>,
    stop_flag: Arc<AtomicBool>,
    paused_flag: Arc<AtomicBool>,
    clock_sync: Arc<Mutex<ClockSync>>,
    format: AudioFormat,
    volume_level: Arc<VolumeLevel>,
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
        // Check if paused (when this device is the default output)
        if paused_flag.load(Ordering::Relaxed) {
            // Write silence to keep device happy, but don't read from buffer
            let _ = renderer.write_silence(480); // 10ms of silence
            thread::sleep(Duration::from_millis(50));
            // Keep reader caught up to avoid buffer overrun when resuming
            reader.catch_up(&buffer);
            continue;
        }

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
            let (start, end) = if correction > 0 {
                let skip_bytes = (correction as usize * format.block_align as usize).min(read);
                (skip_bytes, read)
            } else {
                (0, read)
            };

            // Apply volume scaling
            let volume = volume_level.get();
            apply_volume_f32(&mut render_buffer[start..end], volume);

            match renderer.write_frames(&render_buffer[start..end], 50) {
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
