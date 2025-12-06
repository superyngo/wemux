# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/superyngo/wemux/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/superyngo/wemux/releases/tag/v0.1.0
