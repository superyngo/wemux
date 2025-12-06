# wemux

Windows Multi-HDMI Audio Sync - Duplicate system audio to multiple HDMI audio devices simultaneously.

## Overview

wemux captures audio from the Windows system default output device using WASAPI loopback and synchronously plays it to multiple HDMI audio devices. This is useful for scenarios like:

- Playing the same audio through multiple TVs/monitors
- Multi-room audio distribution via HDMI
- Audio mirroring for presentations

## Features

- **WASAPI Loopback Capture**: Captures mixed system audio from the default output
- **Multi-HDMI Output**: Simultaneously outputs to all detected HDMI audio devices
- **Master-Slave Sync**: Clock synchronization to keep all outputs in sync
- **Auto-Detection**: Automatically finds all HDMI audio devices
- **Hot-Plug Support**: Handles device connection/disconnection gracefully
- **Low Latency**: Configurable buffer size for latency tuning

## Requirements

- Windows 10 or later
- Rust 1.70+ (for building)
- Multiple HDMI audio outputs

## Installation

### From Source

```bash
git clone https://github.com/superyngo/wemux.git
cd wemux
cargo build --release
```

The binary will be at `target/release/wemux.exe`

## Usage

### List Audio Devices

```bash
# List all audio devices
wemux list

# List only HDMI devices with IDs
wemux list --hdmi-only --show-ids
```

### Start Audio Sync

```bash
# Auto-detect and sync all HDMI devices
wemux start

# Specify devices by ID or name
wemux start -d "NVIDIA,Intel"

# Exclude specific devices
wemux start -e "HDMI 3"

# Custom buffer size (default: 50ms)
wemux start -b 100

# Verbose output
wemux start -v
wemux start -vv  # More verbose
```

### Show Device Info

```bash
wemux info "NVIDIA"
```

## CLI Reference

```
wemux - Windows Multi-HDMI Audio Sync

USAGE:
    wemux [OPTIONS] [COMMAND]

COMMANDS:
    list        List all available audio devices
    start       Start audio synchronization
    info        Show detailed device information
    help        Print help information

OPTIONS:
    -v, --verbose       Verbose output (can be repeated)
    -q, --quiet         Quiet mode - only show errors
    --log <file>        Log output to file
    -h, --help          Print help
    -V, --version       Print version
```

## Architecture

```
┌─────────────────────┐
│  System Default     │
│  Audio Output       │
│  (WASAPI Loopback)  │
└──────────┬──────────┘
           │ capture PCM
           ▼
┌─────────────────────┐
│     AudioEngine     │
│  ┌───────────────┐  │
│  │ Ring Buffer   │  │
│  │ (lock-free)   │  │
│  └───────┬───────┘  │
│          │          │
│    ┌─────┴─────┐    │
│    ▼           ▼    │
│ Renderer 1  Renderer N  (Master-Slave Sync)
└─────────────────────┘
           │
     ┌─────┼─────┐
     ▼     ▼     ▼
  HDMI 1 HDMI 2 HDMI N
```

## Limitations

- **PCM Only**: DRM-protected or bitstream content (e.g., Dolby TrueHD passthrough) cannot be captured
- **User Mode**: Runs in user mode, not as a virtual audio driver
- **Windows Only**: Uses Windows-specific WASAPI APIs

## Future Plans

- [ ] Windows Service mode
- [ ] System tray application
- [ ] Configuration file support
- [ ] Per-device volume control
- [ ] Format conversion between devices

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.
