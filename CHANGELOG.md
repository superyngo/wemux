# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2025-12-11

### Added

- Hardware capability detection module for automatic buffer size optimization
- Tray icon assets (active, idle, error states)
- Auto-calculation of optimal buffer sizes based on device latency class
- Config directory detection with `dirs` crate

### Improved

- Enhanced tray icon management with better state handling
- Better error handling in audio capture and renderer
- Optimized clock synchronization for lower latency
- Improved buffer management and synchronization
- Enhanced tray app initialization

### Fixed

- Various stability improvements in tray application

## [0.2.0] - 2025-12-09

### Added

- System tray application (wemux-tray) for easy control
  - Start/stop audio engine from tray icon
  - Toggle individual HDMI devices on/off
  - Visual status indication via icon states
  - Dynamic device menu updates on hot-plug events
- Build workflow now compiles all three binaries (CLI, service, tray)
- Comprehensive project documentation in CLAUDE.md

### Improved

- Fixed all clippy warnings
- Fixed code formatting issues
- Enhanced AudioEngine with public control API for tray integration

## [0.1.1] - 2025-12-08

### Added

- Send trait implementations for LoopbackCapture and HdmiRenderer
- FromUtf16Error to WemuxError conversion

### Fixed

- Windows API import errors
  - WAIT_OBJECT_0 moved to correct namespace (Win32::Foundation)
  - STGM_READ moved to correct namespace (Win32::System::Com)
  - Fixed monitor.rs namespace prefix
- PROPVARIANT access using repr(C) struct for compatibility
- DEVICE_STATE type in OnDeviceStateChanged callback
- All clippy warnings

### Changed

- Add windows-core 0.58 dependency for PROPVARIANT support
- Remove hardcoded release profile config (delegated to workflow env vars)

## [0.1.0] - 2025-12-07

### Added

- Initial release
- WASAPI loopback capture from system default audio output
- Multi-HDMI audio output support
- Master-slave clock synchronization
- Lock-free ring buffer for audio data
- Device enumeration and HDMI detection
- Device hot-plug monitoring via IMMNotificationClient
- CLI interface with `list`, `start`, and `info` commands
- Configurable buffer size
- Device filtering by ID or name
- Verbose logging with tracing
- Graceful Ctrl+C handling

### Architecture

- `audio/capture.rs` - WASAPI loopback capture
- `audio/renderer.rs` - WASAPI render client for HDMI output
- `audio/engine.rs` - Main controller coordinating capture and renderers
- `audio/buffer.rs` - Lock-free SPSC ring buffer
- `device/enumerator.rs` - Windows audio device enumeration
- `device/filter.rs` - HDMI device detection logic
- `device/monitor.rs` - Device change notifications
- `sync/clock.rs` - Master-slave clock synchronization
- `config/args.rs` - CLI argument parsing with clap

[Unreleased]: https://github.com/superyngo/wemux/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/superyngo/wemux/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/superyngo/wemux/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/superyngo/wemux/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/superyngo/wemux/releases/tag/v0.1.0
