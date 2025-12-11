# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

wemux is a Windows-only audio utility that captures system audio via WASAPI loopback and synchronously duplicates it to multiple HDMI audio devices. It's useful for multi-room audio or playing the same audio through multiple TVs/monitors.

## Build Commands

```bash
# Standard build
cargo build

# Release build
cargo build --release

# Build with Windows Service support
cargo build --release --features service

# Build with System Tray support
cargo build --release --features tray

# Build all binaries (CLI, service, tray)
cargo build --release --features service,tray

# Build only the service binary
cargo build --release --features service --bin wemux-service

# Build only the tray binary
cargo build --release --features tray --bin wemux-tray

# Format code
cargo fmt

# Lint
cargo clippy
```

## Running

```bash
# List audio devices
cargo run -- list
cargo run -- list --hdmi-only --show-ids

# Start audio sync (auto-detect HDMI devices)
cargo run -- start

# Start with specific devices/options
cargo run -- start -d "NVIDIA,Intel" -b 100 -v

# Show device info
cargo run -- info "NVIDIA"

# Service management (requires admin privileges)
cargo run -- service install
cargo run -- service status
cargo run -- service uninstall
```

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

- **`src/service/`** - Windows Service support (feature-gated)
  - `runner.rs` - Service main loop
  - `config.rs` - TOML-based service configuration

- **`src/tray/`** - System tray application (feature-gated)
  - `app.rs` - Main tray application loop with Windows message pump
  - `controller.rs` - Engine controller thread managing AudioEngine lifecycle
  - `icon.rs` - Icon management and state-based icon updates
  - `menu.rs` - Dynamic context menu with device toggles

- **`src/config/`** - CLI argument parsing (clap)

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
- `clap` for CLI parsing
- `crossbeam-channel` for inter-thread communication
- `parking_lot` for mutexes
- `windows-service` (optional) for Windows Service support
- `tray-icon` and `muda` (optional) for system tray support
