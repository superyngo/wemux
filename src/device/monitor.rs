//! Device hotplug monitoring using IMMNotificationClient

use crate::error::Result;
use crossbeam_channel::Sender;
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::{debug, info, warn};
use windows::{
    core::{implement, PCWSTR},
    Win32::{
        Media::Audio::{
            EDataFlow, ERole, IMMDeviceEnumerator, IMMNotificationClient,
            IMMNotificationClient_Impl, MMDeviceEnumerator, DEVICE_STATE,
        },
        System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED},
    },
};

/// Events from device monitoring
#[derive(Debug, Clone)]
pub enum DeviceEvent {
    /// A new device was added
    Added(String),
    /// A device was removed
    Removed(String),
    /// The default device changed
    DefaultChanged {
        data_flow: i32,
        role: i32,
        device_id: String,
    },
    /// Device state changed
    StateChanged { device_id: String, new_state: u32 },
    /// Device property changed
    PropertyChanged { device_id: String },
}

/// Device monitor for hot-plug detection
pub struct DeviceMonitor {
    _enumerator: IMMDeviceEnumerator,
    _callback: IMMNotificationClient,
}

impl DeviceMonitor {
    /// Create and start a new device monitor
    ///
    /// Events will be sent through the provided channel
    pub fn new(event_sender: Sender<DeviceEvent>) -> Result<Self> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

            let callback_impl = NotificationCallback {
                sender: Arc::new(Mutex::new(event_sender)),
            };
            let callback: IMMNotificationClient = callback_impl.into();

            enumerator.RegisterEndpointNotificationCallback(&callback)?;

            info!("Device monitor started");

            Ok(Self {
                _enumerator: enumerator,
                _callback: callback,
            })
        }
    }
}

/// Internal notification callback implementation
#[implement(IMMNotificationClient)]
struct NotificationCallback {
    sender: Arc<Mutex<Sender<DeviceEvent>>>,
}

impl IMMNotificationClient_Impl for NotificationCallback_Impl {
    fn OnDeviceStateChanged(
        &self,
        pwstrdeviceid: &PCWSTR,
        dwnewstate: DEVICE_STATE,
    ) -> windows::core::Result<()> {
        if let Ok(device_id) = unsafe { pwstrdeviceid.to_string() } {
            debug!("Device state changed: {} -> {}", device_id, dwnewstate.0);
            let event = DeviceEvent::StateChanged {
                device_id,
                new_state: dwnewstate.0,
            };
            self.send_event(event);
        }
        Ok(())
    }

    fn OnDeviceAdded(&self, pwstrdeviceid: &PCWSTR) -> windows::core::Result<()> {
        if let Ok(device_id) = unsafe { pwstrdeviceid.to_string() } {
            info!("Device added: {}", device_id);
            self.send_event(DeviceEvent::Added(device_id));
        }
        Ok(())
    }

    fn OnDeviceRemoved(&self, pwstrdeviceid: &PCWSTR) -> windows::core::Result<()> {
        if let Ok(device_id) = unsafe { pwstrdeviceid.to_string() } {
            info!("Device removed: {}", device_id);
            self.send_event(DeviceEvent::Removed(device_id));
        }
        Ok(())
    }

    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        role: ERole,
        pwstrdefaultdeviceid: &PCWSTR,
    ) -> windows::core::Result<()> {
        if let Ok(device_id) = unsafe { pwstrdefaultdeviceid.to_string() } {
            info!(
                "Default device changed: {} (flow={:?}, role={:?})",
                device_id, flow, role
            );
            let event = DeviceEvent::DefaultChanged {
                data_flow: flow.0,
                role: role.0,
                device_id,
            };
            self.send_event(event);
        }
        Ok(())
    }

    fn OnPropertyValueChanged(
        &self,
        pwstrdeviceid: &PCWSTR,
        _key: &windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY,
    ) -> windows::core::Result<()> {
        if let Ok(device_id) = unsafe { pwstrdeviceid.to_string() } {
            debug!("Device property changed: {}", device_id);
            self.send_event(DeviceEvent::PropertyChanged { device_id });
        }
        Ok(())
    }
}

impl NotificationCallback_Impl {
    fn send_event(&self, event: DeviceEvent) {
        let sender = self.sender.lock();
        if sender.send(event).is_err() {
            warn!("Failed to send device event - receiver dropped");
        }
    }
}
