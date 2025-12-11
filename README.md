# wemux

Windows Multi-HDMI Audio Sync - System tray application for duplicating audio to multiple HDMI devices simultaneously.

## Overview

wemux is a Windows system tray application that captures audio from your default output device using WASAPI loopback and synchronously plays it to multiple HDMI audio devices. This is useful for scenarios like:

- Playing the same audio through multiple TVs/monitors
- Multi-room audio distribution via HDMI
- Audio mirroring for presentations

## Features

- **System Tray Interface**: Easy control via system tray icon
- **Device Toggle**: Enable/disable individual HDMI devices from the menu
- **WASAPI Loopback Capture**: Captures mixed system audio from the default output
- **Multi-HDMI Output**: Simultaneously outputs to all detected HDMI audio devices
- **Master-Slave Sync**: Clock synchronization to keep all outputs in sync
- **Auto-Detection**: Automatically finds all HDMI audio devices
- **Hot-Plug Support**: Handles device connection/disconnection gracefully
- **Settings Persistence**: Remembers your device preferences

## Requirements

- Windows 10 or later
- Multiple HDMI audio outputs

## Installation

### From Microsoft Store (Coming Soon)

wemux will be available on the Microsoft Store for easy installation and automatic updates.

### Manual Download

Download the latest release from [GitHub Releases](https://github.com/superyngo/wemux/releases).

### From Source

Requires Rust 1.70+

```bash
git clone https://github.com/superyngo/wemux.git
cd wemux
cargo build --release
```

The binary will be at `target/release/wemux.exe`

## Usage

Simply run `wemux.exe` - the application will appear in your system tray.

### System Tray Menu

Right-click the tray icon to access:

- **System Output**: Shows current Windows default audio device
- **Output Devices**: Toggle individual HDMI devices on/off
- **Start/Stop**: Control audio synchronization
- **Refresh Devices**: Rescan for HDMI devices
- **Exit**: Close the application

### Debug Mode

Run with `--debug` flag to show console output:

```bash
wemux.exe --debug
```

### Settings

Device preferences are saved to `wemux.toml` in the same directory as the executable. The file is automatically created and updated when you enable/disable devices.

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

- [x] System tray application (v0.2.0+)
- [x] Device toggle controls (v0.3.0+)
- [ ] Microsoft Store distribution
- [ ] Per-device volume control
- [ ] Format conversion between devices
- [ ] Auto-update mechanism

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.
