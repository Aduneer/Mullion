//! Wi-Fi and Bluetooth for the Control Centre: whether each radio is on, turning it on or off, and
//! the paired Bluetooth devices. Windows' own radio API, the one its quick settings use; it asks
//! nothing of the user for a desktop app. The calls wait on Windows, so they're made from async
//! commands, off the main thread.

use serde::Serialize;
use windows::Devices::Bluetooth::{BluetoothConnectionStatus, BluetoothDevice, BluetoothLEDevice};
use windows::Devices::Enumeration::DeviceInformation;
use windows::Devices::Radios::{Radio, RadioAccessStatus, RadioKind, RadioState};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Radios {
    /// None when there's no such radio (a desktop without Wi-Fi, say).
    pub wifi: Option<bool>,
    pub bluetooth: Option<bool>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Device {
    pub name: String,
    pub connected: bool,
}

fn init() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

fn all() -> Vec<Radio> {
    Radio::GetRadiosAsync().and_then(|op| op.join()).map(|v| v.into_iter().collect()).unwrap_or_default()
}

fn kind_of(name: &str) -> Option<RadioKind> {
    match name {
        "wifi" => Some(RadioKind::WiFi),
        "bluetooth" => Some(RadioKind::Bluetooth),
        _ => None,
    }
}

pub fn state() -> Radios {
    init();
    let radios = all();
    let on = |kind: RadioKind| {
        let mut of_kind = radios.iter().filter(|r| r.Kind().ok() == Some(kind)).peekable();
        of_kind.peek()?;
        Some(of_kind.any(|r| r.State().ok() == Some(RadioState::On)))
    };
    Radios { wifi: on(RadioKind::WiFi), bluetooth: on(RadioKind::Bluetooth) }
}

/// Turn the Wi-Fi or Bluetooth radio on or off.
pub fn set(kind: &str, on: bool) -> Result<(), String> {
    init();
    let kind = kind_of(kind).ok_or("no such radio")?;
    let access = Radio::RequestAccessAsync().and_then(|op| op.join()).map_err(|e| e.message())?;
    if access != RadioAccessStatus::Allowed {
        return Err("Windows didn't allow it".into());
    }
    let radios: Vec<Radio> = all().into_iter().filter(|r| r.Kind().ok() == Some(kind)).collect();
    if radios.is_empty() {
        return Err("there's no such radio".into());
    }
    let state = if on { RadioState::On } else { RadioState::Off };
    for r in radios {
        let status = r.SetStateAsync(state).and_then(|op| op.join()).map_err(|e| e.message())?;
        if status != RadioAccessStatus::Allowed {
            return Err("Windows didn't allow it".into());
        }
    }
    Ok(())
}

/// The paired Bluetooth devices, classic and Low Energy, connected ones first.
pub fn devices() -> Vec<Device> {
    init();
    let mut out = Vec::new();
    let find = |selector: windows::core::Result<windows::core::HSTRING>| {
        selector
            .and_then(|s| DeviceInformation::FindAllAsyncAqsFilter(&s))
            .and_then(|op| op.join())
            .map(|c| c.into_iter().collect::<Vec<_>>())
            .unwrap_or_default()
    };
    for info in find(BluetoothDevice::GetDeviceSelectorFromPairingState(true)) {
        let connected = info.Id().ok().and_then(|id| BluetoothDevice::FromIdAsync(&id).ok()?.join().ok()).and_then(|d| d.ConnectionStatus().ok());
        push(&mut out, &info, connected);
    }
    for info in find(BluetoothLEDevice::GetDeviceSelectorFromPairingState(true)) {
        let connected = info.Id().ok().and_then(|id| BluetoothLEDevice::FromIdAsync(&id).ok()?.join().ok()).and_then(|d| d.ConnectionStatus().ok());
        push(&mut out, &info, connected);
    }
    out.sort_by_key(|d| (!d.connected, d.name.to_lowercase()));
    out
}

/// One device, once: a device that pairs over both kinds of Bluetooth is listed by both.
fn push(out: &mut Vec<Device>, info: &DeviceInformation, status: Option<BluetoothConnectionStatus>) {
    let name = info.Name().map(|n| n.to_string()).unwrap_or_default();
    if name.is_empty() {
        return;
    }
    let connected = status == Some(BluetoothConnectionStatus::Connected);
    match out.iter_mut().find(|d| d.name == name) {
        Some(d) => d.connected |= connected,
        None => out.push(Device { name, connected }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_this_pcs_radios() {
        // Whatever this PC has; it mustn't fail.
        let _ = (state(), devices());
        assert!(kind_of("wifi").is_some() && kind_of("nfc").is_none());
    }
}
