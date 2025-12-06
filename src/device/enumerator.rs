//! Audio device enumeration using Windows Core Audio API

use crate::device::filter::HdmiFilter;
use crate::error::{Result, WemuxError};
use std::fmt;
use tracing::{debug, info};
use windows::{
    core::PCWSTR,
    Win32::{
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
        Media::Audio::{
            eConsole, eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
            DEVICE_STATE_ACTIVE,
        },
        System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED},
    },
};

/// Information about an audio device
#[derive(Clone)]
pub struct DeviceInfo {
    /// Unique device ID
    pub id: String,
    /// Human-readable device name
    pub name: String,
    /// Whether this device is identified as HDMI
    pub is_hdmi: bool,
    /// Whether this is the default render device
    pub is_default: bool,
}

impl fmt::Display for DeviceInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hdmi_marker = if self.is_hdmi { " [HDMI]" } else { "" };
        let default_marker = if self.is_default { " (default)" } else { "" };
        write!(f, "{}{}{}", self.name, hdmi_marker, default_marker)
    }
}

impl fmt::Debug for DeviceInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceInfo")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("is_hdmi", &self.is_hdmi)
            .field("is_default", &self.is_default)
            .finish()
    }
}

/// Audio device enumerator wrapping Windows MMDevice API
pub struct DeviceEnumerator {
    enumerator: IMMDeviceEnumerator,
    default_device_id: Option<String>,
}

impl DeviceEnumerator {
    /// Create a new device enumerator
    ///
    /// # Safety
    /// This initializes COM if not already initialized
    pub fn new() -> Result<Self> {
        unsafe {
            // Initialize COM (ignore error if already initialized)
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

            // Get default device ID
            let default_device_id = Self::get_default_device_id_internal(&enumerator)?;

            info!("Device enumerator initialized");

            Ok(Self {
                enumerator,
                default_device_id,
            })
        }
    }

    /// Get the default render device ID
    fn get_default_device_id_internal(enumerator: &IMMDeviceEnumerator) -> Result<Option<String>> {
        unsafe {
            match enumerator.GetDefaultAudioEndpoint(eRender, eConsole) {
                Ok(device) => {
                    let id_ptr = device.GetId()?;
                    let id = PCWSTR(id_ptr.0).to_string()?;
                    windows::Win32::System::Com::CoTaskMemFree(Some(id_ptr.0 as *const _));
                    Ok(Some(id))
                }
                Err(_) => Ok(None),
            }
        }
    }

    /// Get the default audio render device
    pub fn get_default_render_device(&self) -> Result<IMMDevice> {
        unsafe {
            self.enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(|e| WemuxError::DeviceError {
                    device_id: "default".into(),
                    message: format!("Failed to get default render device: {}", e),
                })
        }
    }

    /// Get a device by its ID
    pub fn get_device_by_id(&self, device_id: &str) -> Result<IMMDevice> {
        unsafe {
            let id_wide: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
            self.enumerator
                .GetDevice(PCWSTR(id_wide.as_ptr()))
                .map_err(|_| WemuxError::DeviceNotFound(device_id.to_string()))
        }
    }

    /// Enumerate all active render devices
    pub fn enumerate_all_devices(&self) -> Result<Vec<DeviceInfo>> {
        unsafe {
            let collection = self
                .enumerator
                .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;

            let count = collection.GetCount()?;
            debug!("Found {} render devices", count);

            let mut devices = Vec::with_capacity(count as usize);

            for i in 0..count {
                if let Ok(device) = collection.Item(i) {
                    if let Ok(info) = self.get_device_info(&device) {
                        devices.push(info);
                    }
                }
            }

            Ok(devices)
        }
    }

    /// Enumerate only HDMI devices
    pub fn enumerate_hdmi_devices(&self) -> Result<Vec<DeviceInfo>> {
        let all_devices = self.enumerate_all_devices()?;
        let hdmi_devices: Vec<_> = all_devices.into_iter().filter(|d| d.is_hdmi).collect();

        info!("Found {} HDMI devices", hdmi_devices.len());

        if hdmi_devices.is_empty() {
            return Err(WemuxError::NoHdmiDevices);
        }

        Ok(hdmi_devices)
    }

    /// Get device information from an IMMDevice
    fn get_device_info(&self, device: &IMMDevice) -> Result<DeviceInfo> {
        unsafe {
            // Get device ID
            let id_ptr = device.GetId()?;
            let id = PCWSTR(id_ptr.0).to_string()?;
            windows::Win32::System::Com::CoTaskMemFree(Some(id_ptr.0 as *const _));

            // Get friendly name from property store
            let store = device.OpenPropertyStore(windows::Win32::System::Com::StructuredStorage::STGM_READ)?;
            let name_prop = store.GetValue(&PKEY_Device_FriendlyName)?;

            let name = prop_variant_to_string(&name_prop).unwrap_or_else(|| "Unknown Device".to_string());

            // Check if HDMI
            let is_hdmi = HdmiFilter::is_hdmi_device(&name) || HdmiFilter::is_hdmi_device_id(&id);

            // Check if default
            let is_default = self.default_device_id.as_ref().map_or(false, |default_id| default_id == &id);

            Ok(DeviceInfo {
                id,
                name,
                is_hdmi,
                is_default,
            })
        }
    }

    /// Refresh the default device ID
    pub fn refresh_default_device(&mut self) -> Result<()> {
        self.default_device_id = Self::get_default_device_id_internal(&self.enumerator)?;
        Ok(())
    }
}

/// Extract string from PROPVARIANT
fn prop_variant_to_string(prop: &windows::Win32::System::Com::StructuredStorage::PROPVARIANT) -> Option<String> {
    unsafe {
        // Check if it's a string type (VT_LPWSTR = 31)
        if prop.Anonymous.Anonymous.vt == windows::Win32::System::Variant::VT_LPWSTR {
            let pwsz = prop.Anonymous.Anonymous.Anonymous.pwszVal;
            if !pwsz.0.is_null() {
                return PCWSTR(pwsz.0).to_string().ok();
            }
        }
        None
    }
}
