//! Display brightness for the Control Centre. A laptop's own panel is set the way Windows' slider
//! sets it, through the active power plan's "display brightness"; an external monitor over
//! DDC/CI, where it supports it (most do, some have it off in their own menu). The DDC/CI calls
//! talk to the monitor and take a while, so they're made from async commands.

use serde::Serialize;
use windows::core::GUID;
use windows::Win32::Devices::Display::{
    DestroyPhysicalMonitors, DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, GetMonitorBrightness, GetNumberOfPhysicalMonitorsFromHMONITOR,
    GetPhysicalMonitorsFromHMONITOR, QueryDisplayConfig, SetMonitorBrightness, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
    DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED,
    DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EMBEDDED, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
    DISPLAYCONFIG_TARGET_DEVICE_NAME, PHYSICAL_MONITOR, QDC_ONLY_ACTIVE_PATHS,
};
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Graphics::Gdi::{MonitorFromPoint, HMONITOR, MONITOR_DEFAULTTONULL};
use windows::Win32::System::Power::{GetSystemPowerStatus, PowerGetActiveScheme, PowerReadACValueIndex, PowerReadDCValueIndex, PowerSetActiveScheme, PowerWriteACValueIndex, PowerWriteDCValueIndex, SYSTEM_POWER_STATUS};

/// The power plan's display settings, and its "display brightness" (0–100).
const VIDEO: GUID = GUID::from_u128(0x7516b95f_f776_4464_8c53_06167f40cc99);
const BRIGHTNESS: GUID = GUID::from_u128(0xaded5e82_b909_4619_9949_f5d71dac0bcb);

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Display {
    /// The screen's device name, as `appbar::screens` has it.
    pub id: String,
    /// "Built-in display", or the monitor's own name ("DELL U2720Q").
    pub name: String,
    pub builtin: bool,
    /// 0–100, or None when it can't be read or set.
    pub brightness: Option<u8>,
}

/// What Windows' display configuration says about each active screen: its name and whether it's
/// a built-in panel, by device name.
fn identities() -> Vec<(String, String, bool)> {
    unsafe {
        let (mut np, mut nm) = (0u32, 0u32);
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm).is_err() {
            return Vec::new();
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); nm as usize];
        if QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut np, paths.as_mut_ptr(), &mut nm, modes.as_mut_ptr(), None).is_err() {
            return Vec::new();
        }
        paths.truncate(np as usize);
        paths
            .iter()
            .filter_map(|p| {
                let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
                source.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
                source.header.size = std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
                source.header.adapterId = p.sourceInfo.adapterId;
                source.header.id = p.sourceInfo.id;
                let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
                target.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
                target.header.size = std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
                target.header.adapterId = p.targetInfo.adapterId;
                target.header.id = p.targetInfo.id;
                if DisplayConfigGetDeviceInfo(&mut source.header) != 0 || DisplayConfigGetDeviceInfo(&mut target.header) != 0 {
                    return None;
                }
                let tech = target.outputTechnology;
                let builtin = [DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EMBEDDED].contains(&tech);
                let gdi = crate::ffi::from_wide(&source.viewGdiDeviceName);
                let name = crate::ffi::from_wide(&target.monitorFriendlyDeviceName);
                Some((gdi, name, builtin))
            })
            .collect()
    }
}

/// The screens, each with what can be done about its brightness.
pub fn displays() -> Vec<Display> {
    let ids = identities();
    crate::appbar::screens()
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let (name, builtin) = ids.iter().find(|(g, _, _)| *g == s.id).map(|(_, n, b)| (n.clone(), *b)).unwrap_or_default();
            let name = if builtin {
                "Built-in display".to_string()
            } else if name.is_empty() {
                format!("Display {}", i + 1)
            } else {
                name
            };
            let brightness = if builtin { panel() } else { ddc(&s.rect, None) };
            Display { id: s.id, name, builtin, brightness }
        })
        .collect()
}

/// Set a screen's brightness, 0–100.
pub fn set(id: &str, value: u8) -> Result<(), String> {
    let value = value.min(100);
    let display = displays().into_iter().find(|d| d.id == id).ok_or("that screen has gone")?;
    let ok = if display.builtin {
        set_panel(value)
    } else {
        let rect = crate::appbar::screens().into_iter().find(|s| s.id == id).map(|s| s.rect).ok_or("that screen has gone")?;
        ddc(&rect, Some(value)).is_some()
    };
    if ok {
        Ok(())
    } else {
        Err("that screen's brightness can't be changed from here".into())
    }
}

/// The built-in panel's brightness in the active power plan, for the power source in use.
fn panel() -> Option<u8> {
    unsafe {
        let scheme = active_scheme()?;
        let mut value = 0u32;
        let ok = if on_battery() {
            PowerReadDCValueIndex(None, Some(&scheme), Some(&VIDEO), Some(&BRIGHTNESS), &mut value) == 0
        } else {
            PowerReadACValueIndex(None, Some(&scheme), Some(&VIDEO), Some(&BRIGHTNESS), &mut value).is_ok()
        };
        ok.then_some(value.min(100) as u8)
    }
}

/// Write the value for both power sources, as Windows' own slider does over time, and apply it.
fn set_panel(value: u8) -> bool {
    unsafe {
        let Some(scheme) = active_scheme() else { return false };
        let ac = PowerWriteACValueIndex(None, &scheme, Some(&VIDEO), Some(&BRIGHTNESS), value as u32).is_ok();
        let dc = PowerWriteDCValueIndex(None, &scheme, Some(&VIDEO), Some(&BRIGHTNESS), value as u32) == 0;
        (ac || dc) && PowerSetActiveScheme(None, Some(&scheme)).is_ok()
    }
}

fn active_scheme() -> Option<GUID> {
    unsafe {
        let mut p: *mut GUID = std::ptr::null_mut();
        if PowerGetActiveScheme(None, &mut p).is_err() || p.is_null() {
            return None;
        }
        let g = *p;
        let _ = LocalFree(Some(HLOCAL(p.cast())));
        Some(g)
    }
}

fn on_battery() -> bool {
    let mut s = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut s).is_ok() && s.ACLineStatus == 0 }
}

/// Read (and with `set`, first change) an external monitor's brightness over DDC/CI, as 0–100.
fn ddc(rect: &[i32; 4], set: Option<u8>) -> Option<u8> {
    unsafe {
        let center = windows::Win32::Foundation::POINT { x: rect[0] + rect[2] / 2, y: rect[1] + rect[3] / 2 };
        let monitor: HMONITOR = MonitorFromPoint(center, MONITOR_DEFAULTTONULL);
        if monitor.is_invalid() {
            return None;
        }
        let mut n = 0u32;
        GetNumberOfPhysicalMonitorsFromHMONITOR(monitor, &mut n).ok()?;
        if n == 0 {
            return None;
        }
        let mut physical = vec![PHYSICAL_MONITOR::default(); n as usize];
        GetPhysicalMonitorsFromHMONITOR(monitor, &mut physical).ok()?;
        let handle = physical[0].hPhysicalMonitor;
        let (mut min, mut cur, mut max) = (0u32, 0u32, 0u32);
        let mut result = None;
        if GetMonitorBrightness(handle, &mut min, &mut cur, &mut max) != 0 && max > min {
            if let Some(v) = set {
                let raw = min + (max - min) * v as u32 / 100;
                if SetMonitorBrightness(handle, raw) != 0 {
                    result = Some(v);
                }
            } else {
                result = Some(((cur.clamp(min, max) - min) * 100 / (max - min)) as u8);
            }
        }
        let _ = DestroyPhysicalMonitors(&physical);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_this_pcs_displays() {
        let all = displays();
        assert!(!all.is_empty());
        assert!(all.iter().all(|d| !d.name.is_empty() && d.brightness.is_none_or(|b| b <= 100)));
    }
}
