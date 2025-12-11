# Privacy Policy for wemux

**Last Updated:** December 11, 2025

## Overview

wemux is a local audio processing application that duplicates system audio to multiple HDMI devices. This privacy policy explains how wemux handles data.

## Data Collection

**wemux does NOT collect, store, or transmit any personal data.**

Specifically:
- ✅ No personal information is collected
- ✅ No usage analytics or telemetry is collected
- ✅ No data is sent to external servers or third parties
- ✅ No user tracking or profiling occurs
- ✅ No crash reports are automatically sent

## Data Storage (Local Only)

wemux stores the following data **locally on your computer only**:

### Device Preferences
- **What:** Device enable/disable settings for HDMI audio devices
- **Where:**
  - MSIX Package: `%LOCALAPPDATA%\wemux\wemux.toml`
  - Standalone Executable: `wemux.toml` (in same folder as executable)
- **Format:** Plain text TOML configuration file
- **Purpose:** Remember your preferred HDMI device settings between sessions
- **Access:** Only you and wemux can access this file

Example of stored data:
```toml
[devices."device-id-12345"]
name = "LG TV HDMI"
enabled = true
```

This file contains:
- Device IDs (system-generated identifiers)
- Device names (as reported by Windows)
- Enabled/disabled state for each device

**No audio data is ever stored to disk.**

## Audio Processing

### How wemux Accesses Audio
- wemux uses Windows Audio Session API (WASAPI) to access system audio
- Audio is captured in real-time from your default audio output device
- Audio is immediately duplicated to selected HDMI devices
- **All audio processing happens in memory only**

### What Happens to Audio Data
- ✅ Audio is processed locally on your device
- ✅ Audio stays in your computer's memory (RAM)
- ✅ Audio is never written to disk
- ✅ Audio is never sent over the network
- ✅ Audio is never transmitted to external servers
- ✅ Audio is immediately discarded after playback

## Permissions Required

wemux requires the following Windows permissions:

| Permission | Purpose | Required? |
|------------|---------|-----------|
| **Audio Device Access** | To capture and playback audio | ✅ Yes |
| **Full Trust (MSIX)** | To use Windows Audio APIs | ✅ Yes (MSIX only) |

### Why Full Trust?
MSIX packages require "Full Trust" capability to:
- Access Windows Audio Session API (WASAPI) for loopback capture
- Enumerate audio devices
- Create system tray icons
- Access audio endpoints

This is a Windows requirement for all audio processing applications.

## Network Access

**wemux does NOT use network access.**

- No internet connection is required
- No network communication occurs
- No data is sent or received over the network
- The app works completely offline

## Third-Party Services

**wemux does NOT use any third-party services, analytics, or advertising.**

- No third-party SDKs are included
- No advertising networks
- No analytics platforms
- No cloud services

## Data Retention

Since wemux does not collect any personal data:
- There is no data retention policy needed
- Device preferences are stored until you uninstall the app or manually delete the settings file
- You can delete your settings file at any time without affecting app functionality (settings will be recreated with defaults)

## User Rights

### Your Data Control
You have complete control over your data:

1. **View Your Data:**
   - Open `%LOCALAPPDATA%\wemux\wemux.toml` (MSIX) or `wemux.toml` (standalone)
   - The file is in plain text TOML format

2. **Delete Your Data:**
   - Delete the `wemux.toml` file
   - Uninstall wemux
   - Settings will be recreated with defaults if you continue using wemux

3. **Export Your Data:**
   - Copy the `wemux.toml` file to another location
   - The file is portable and human-readable

### No Account Required
- wemux does not require an account
- No registration or login needed
- No email or personal information collected

## Children's Privacy

wemux does not collect any personal information from anyone, including children under 13. The app is rated for general audiences and is safe for all ages.

## Changes to This Policy

We may update this privacy policy from time to time. Changes will be posted to this page with an updated "Last Updated" date.

- Check this page periodically for updates
- Continued use of wemux after policy changes constitutes acceptance
- Major changes will be noted in release notes

## Compliance

wemux complies with:
- Microsoft Store Privacy Requirements
- Windows App Certification Requirements
- General Data Protection Regulation (GDPR) principles
- California Consumer Privacy Act (CCPA) principles

## Open Source

wemux is open source software. You can:
- Review the source code at: https://github.com/superyngo/wemux
- Verify that no data collection occurs
- Build the app yourself from source
- Contribute to development

## Contact

If you have questions about this privacy policy or wemux's data practices:

- **GitHub Issues:** https://github.com/superyngo/wemux/issues
- **Email:** superyngo@hotmail.com

We will respond to privacy inquiries within 7 business days.

---

## Summary

**In simple terms:**

✅ wemux runs entirely on your computer
✅ No data leaves your computer
✅ No personal information is collected
✅ No internet connection needed
✅ Audio is processed in memory only
✅ Only device preferences are saved locally
✅ You can delete all data by uninstalling or deleting settings file

**wemux is a privacy-first application that respects your data.**
