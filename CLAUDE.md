# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

wemux is a Windows-only system tray application that captures system audio via WASAPI loopback and synchronously duplicates it to multiple HDMI audio devices. It's useful for multi-room audio or playing the same audio through multiple TVs/monitors.

## Build Commands

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Format code
cargo fmt

# Lint
cargo clippy
```

## Running

```bash
# Run in debug mode with console window
cargo run -- --debug

# Run normally (no console window)
cargo run
```

The application runs as a system tray icon. Right-click the tray icon to access:
- Enable/disable individual HDMI devices
- Start/stop audio synchronization
- Refresh device list
- Exit the application

## Architecture

### Module Structure

- **`src/audio/`** - Core audio processing
  - `engine.rs` - Main `AudioEngine` coordinating capture and renderers
  - `capture.rs` - WASAPI loopback capture from default output device
  - `renderer.rs` - `HdmiRenderer` for outputting to HDMI devices
  - `buffer.rs` - Lock-free ring buffer for inter-thread audio data
  - `volume.rs` - Volume tracking and scaling

- **`src/device/`** - Device management
  - `enumerator.rs` - `DeviceEnumerator` for listing audio devices
  - `monitor.rs` - `DeviceMonitor` for hot-plug and default device change events
  - `filter.rs` - HDMI device detection heuristics

- **`src/sync/`** - Clock synchronization
  - `clock.rs` - `ClockSync` for master-slave synchronization across renderers

- **`src/tray/`** - System tray application
  - `app.rs` - Main tray application loop with Windows message pump
  - `controller.rs` - Engine controller thread managing AudioEngine lifecycle
  - `icon.rs` - Icon management and state-based icon updates
  - `menu.rs` - Dynamic context menu with device toggles
  - `settings.rs` - Device settings persistence (wemux.toml)

### Threading Model

The `AudioEngine` spawns multiple threads:
1. **Capture thread** - Reads from WASAPI loopback, writes to shared ring buffer
2. **Volume tracking thread** - Polls system volume for output scaling
3. **Device monitor thread** - Handles device change events (hot-plug, default change)
4. **Renderer threads** (one per HDMI device) - Read from ring buffer, write to HDMI output

### Key Behaviors

- When the default audio output changes to an HDMI device that wemux is outputting to, that renderer is auto-paused to prevent feedback loops
- Ring buffer uses lock-free design with per-reader state tracking
- Clock sync uses master-slave model where first HDMI device is master

## Dependencies

- `windows` crate (0.58) for WASAPI and COM APIs
- `tray-icon` and `muda` for system tray support
- `image` for icon loading
- `crossbeam-channel` for inter-thread communication
- `parking_lot` for mutexes
- `serde` and `toml` for settings persistence
