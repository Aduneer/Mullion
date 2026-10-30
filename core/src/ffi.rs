//! Hand-written bindings for the flat Win32 calls more than one of Mullion's apps uses, and the
//! helpers around them. Each app's own `ffi` module adds the ones only it needs.
//! COM goes through the `windows` crate instead.
#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms, dead_code)]

use std::ffi::c_void;
use std::ptr::null_mut;

pub type HANDLE = *mut c_void;

// --- Performance Data Helper ---------------------------------------------------

pub const PDH_FMT_DOUBLE: u32 = 0x0000_0200;
pub const PDH_FMT_NOCAP100: u32 = 0x0000_8000;
pub const PDH_MORE_DATA: u32 = 0x8000_07D2;
pub const PDH_CSTATUS_VALID_DATA: u32 = 0;
pub const PDH_CSTATUS_NEW_DATA: u32 = 1;

#[repr(C)]
pub struct PDH_FMT_COUNTERVALUE {
    pub CStatus: u32,
    pub doubleValue: f64,
}

#[repr(C)]
pub struct PDH_FMT_COUNTERVALUE_ITEM_W {
    pub szName: *const u16,
    pub FmtValue: PDH_FMT_COUNTERVALUE,
}

#[link(name = "pdh", kind = "raw-dylib")]
extern "system" {
    pub fn PdhOpenQueryW(source: *const u16, user: usize, query: *mut HANDLE) -> u32;
    pub fn PdhAddEnglishCounterW(query: HANDLE, path: *const u16, user: usize, counter: *mut HANDLE) -> u32;
    pub fn PdhCollectQueryData(query: HANDLE) -> u32;
    pub fn PdhGetFormattedCounterArrayW(
        counter: HANDLE,
        format: u32,
        size: *mut u32,
        count: *mut u32,
        items: *mut PDH_FMT_COUNTERVALUE_ITEM_W,
    ) -> u32;
    pub fn PdhCloseQuery(query: HANDLE) -> u32;
}

// --- kernel32 / advapi32 -------------------------------------------------------

#[repr(C)]
#[derive(Default)]
pub struct MEMORYSTATUSEX {
    pub dwLength: u32,
    pub dwMemoryLoad: u32,
    pub ullTotalPhys: u64,
    pub ullAvailPhys: u64,
    pub ullTotalPageFile: u64,
    pub ullAvailPageFile: u64,
    pub ullTotalVirtual: u64,
    pub ullAvailVirtual: u64,
    pub ullAvailExtendedVirtual: u64,
}

#[repr(C)]
#[derive(Default)]
pub struct SYSTEM_POWER_STATUS {
    pub ACLineStatus: u8,
    pub BatteryFlag: u8,
    pub BatteryLifePercent: u8,
    pub SystemStatusFlag: u8,
    pub BatteryLifeTime: u32,
    pub BatteryFullLifeTime: u32,
}

pub const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

#[link(name = "kernel32", kind = "raw-dylib")]
extern "system" {
    pub fn OpenProcess(access: u32, inherit: i32, pid: u32) -> HANDLE;
    pub fn QueryFullProcessImageNameW(process: HANDLE, flags: u32, name: *mut u16, len: *mut u32) -> i32;
    pub fn CloseHandle(handle: HANDLE) -> i32;
    pub fn GetCurrentProcess() -> HANDLE;
    pub fn GlobalMemoryStatusEx(buf: *mut MEMORYSTATUSEX) -> i32;
    pub fn GetSystemPowerStatus(status: *mut SYSTEM_POWER_STATUS) -> i32;
    pub fn GetSystemDirectoryW(buf: *mut u16, len: u32) -> u32;
    pub fn K32EnumProcesses(pids: *mut u32, size: u32, needed: *mut u32) -> i32;
    pub fn GetCurrentThreadId() -> u32;
}

pub const TOKEN_ADJUST_PRIVILEGES: u32 = 0x0020;
pub const TOKEN_QUERY: u32 = 0x0008;
pub const SE_PRIVILEGE_ENABLED: u32 = 0x0000_0002;

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct LUID {
    pub LowPart: u32,
    pub HighPart: i32,
}

/// TOKEN_PRIVILEGES with room for exactly one privilege, which is all we ever ask for.
#[repr(C)]
pub struct TOKEN_PRIVILEGES {
    pub PrivilegeCount: u32,
    pub Luid: LUID,
    pub Attributes: u32,
}

/// HKEY_CURRENT_USER is a sign-extended 32-bit constant.
const HKEY_CURRENT_USER: isize = 0x8000_0001_u32 as i32 as isize;
const RRF_RT_REG_SZ: u32 = 0x0000_0002;

#[link(name = "advapi32", kind = "raw-dylib")]
extern "system" {
    fn RegGetValueW(hkey: isize, subkey: *const u16, value: *const u16, flags: u32, ty: *mut u32, data: *mut c_void, len: *mut u32) -> i32;
    pub fn OpenProcessToken(process: HANDLE, access: u32, token: *mut HANDLE) -> i32;
    pub fn LookupPrivilegeValueW(system: *const u16, name: *const u16, luid: *mut LUID) -> i32;
    pub fn AdjustTokenPrivileges(token: HANDLE, disable_all: i32, new: *const TOKEN_PRIVILEGES, len: u32, prev: *mut c_void, ret_len: *mut u32) -> i32;
}

// --- file version info (the API set, so no version.dll is looked for beside the exe) ----

pub const FILE_VER_GET_LOCALISED: u32 = 0x01;

#[link(name = "api-ms-win-core-version-l1-1-0", kind = "raw-dylib")]
extern "system" {
    pub fn GetFileVersionInfoSizeExW(flags: u32, file: *const u16, handle: *mut u32) -> u32;
    pub fn GetFileVersionInfoExW(flags: u32, file: *const u16, handle: u32, len: u32, data: *mut c_void) -> i32;
    pub fn VerQueryValueW(block: *const c_void, sub: *const u16, buf: *mut *mut c_void, len: *mut u32) -> i32;
}

// --- shell ---------------------------------------------------------------------

pub const SW_SHOWNORMAL: i32 = 1;

#[link(name = "shell32", kind = "raw-dylib")]
extern "system" {
    /// Returns a value > 32 on success.
    pub fn ShellExecuteW(hwnd: isize, op: *const u16, file: *const u16, params: *const u16, dir: *const u16, show: i32) -> isize;
}

// --- COM -----------------------------------------------------------------------

pub const COINIT_MULTITHREADED: u32 = 0;
pub const COINIT_APARTMENTTHREADED: u32 = 2;

#[link(name = "ole32", kind = "raw-dylib")]
extern "system" {
    pub fn CoInitializeEx(reserved: *mut c_void, coinit: u32) -> i32;
}

// --- windows -------------------------------------------------------------------

pub const GWL_EXSTYLE: i32 = -20;
pub const GW_OWNER: u32 = 4;
pub const SW_RESTORE: i32 = 9;
pub const SW_MINIMIZE: i32 = 6;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_APP: u32 = 0x8000;
pub const DWMWA_CLOAKED: u32 = 14;
pub const MONITOR_DEFAULTTONEAREST: u32 = 2;


pub const EWX_LOGOFF: u32 = 0x0000_0000;
pub const EWX_SHUTDOWN: u32 = 0x0000_0001;
pub const EWX_REBOOT: u32 = 0x0000_0002;
pub const EWX_POWEROFF: u32 = 0x0000_0008;
pub const EWX_HYBRID_SHUTDOWN: u32 = 0x0040_0000;
/// "Operating system: planned", the reason the Start menu gives.
pub const SHTDN_REASON_PLANNED: u32 = 0x8000_0000 | 0x0002_0000;

pub const INPUT_KEYBOARD: u32 = 1;
pub const KEYEVENTF_KEYUP: u32 = 0x0002;
pub const VK_LWIN: u16 = 0x5B;

pub const WM_HOTKEY: u32 = 0x0312;
pub const MOD_ALT: u32 = 0x0001;
pub const MOD_CONTROL: u32 = 0x0002;
pub const MOD_SHIFT: u32 = 0x0004;
pub const MOD_WIN: u32 = 0x0008;
pub const MOD_NOREPEAT: u32 = 0x4000;
pub const VK_BACK: u16 = 0x08;
pub const VK_TAB: u16 = 0x09;
pub const VK_RETURN: u16 = 0x0D;
pub const VK_SPACE: u16 = 0x20;
pub const VK_PRIOR: u16 = 0x21;
pub const VK_NEXT: u16 = 0x22;
pub const VK_END: u16 = 0x23;
pub const VK_HOME: u16 = 0x24;
pub const VK_INSERT: u16 = 0x2D;
pub const VK_DELETE: u16 = 0x2E;
pub const VK_LEFT: u16 = 0x25;
pub const VK_UP: u16 = 0x26;
pub const VK_RIGHT: u16 = 0x27;
pub const VK_DOWN: u16 = 0x28;
pub const VK_F1: u16 = 0x70;


#[repr(C)]
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct WINDOWPLACEMENT {
    pub length: u32,
    pub flags: u32,
    pub showCmd: u32,
    pub ptMinPosition: POINT,
    pub ptMaxPosition: POINT,
    pub rcNormalPosition: RECT,
    pub rcDevice: RECT,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct MONITORINFO {
    pub cbSize: u32,
    pub rcMonitor: RECT,
    pub rcWork: RECT,
    pub dwFlags: u32,
}

#[repr(C)]
#[derive(Default)]
pub struct MSG {
    pub hwnd: isize,
    pub message: u32,
    pub wParam: usize,
    pub lParam: isize,
    pub time: u32,
    pub pt_x: i32,
    pub pt_y: i32,
    pub lPrivate: u32,
}



#[repr(C)]
#[derive(Clone, Copy)]
pub struct KEYBDINPUT {
    pub wVk: u16,
    pub wScan: u16,
    pub dwFlags: u32,
    pub time: u32,
    pub dwExtraInfo: usize,
}

/// INPUT for keyboard input only; padded to the size of the union's largest member (MOUSEINPUT).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct INPUT {
    pub kind: u32,
    pub ki: KEYBDINPUT,
    pub _pad: [u8; 8],
}

pub type WNDENUMPROC = unsafe extern "system" fn(hwnd: isize, lparam: isize) -> i32;
pub type MONITORENUMPROC = unsafe extern "system" fn(monitor: isize, dc: isize, rect: *mut RECT, lparam: isize) -> i32;

pub const SWP_NOSIZE: u32 = 0x0001;
pub const SWP_NOMOVE: u32 = 0x0002;
pub const SWP_NOZORDER: u32 = 0x0004;
pub const SWP_NOACTIVATE: u32 = 0x0010;

#[link(name = "user32", kind = "raw-dylib")]
extern "system" {
    pub fn EnumWindows(callback: WNDENUMPROC, lparam: isize) -> i32;
    pub fn EnumChildWindows(parent: isize, callback: WNDENUMPROC, lparam: isize) -> i32;
    pub fn IsWindow(hwnd: isize) -> i32;
    pub fn IsWindowVisible(hwnd: isize) -> i32;
    pub fn IsIconic(hwnd: isize) -> i32;
    pub fn GetWindow(hwnd: isize, cmd: u32) -> isize;
    pub fn GetWindowLongW(hwnd: isize, index: i32) -> i32;
    pub fn GetWindowTextW(hwnd: isize, buf: *mut u16, max: i32) -> i32;
    pub fn GetClassNameW(hwnd: isize, buf: *mut u16, max: i32) -> i32;
    pub fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
    pub fn GetForegroundWindow() -> isize;
    pub fn SetForegroundWindow(hwnd: isize) -> i32;
    pub fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
    pub fn PostMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> i32;
    pub fn MonitorFromPoint(pt: POINT, flags: u32) -> isize;
    pub fn MonitorFromWindow(hwnd: isize, flags: u32) -> isize;
    pub fn GetMonitorInfoW(monitor: isize, info: *mut MONITORINFO) -> i32;
    pub fn GetWindowRect(hwnd: isize, rect: *mut RECT) -> i32;
    pub fn SetWindowPos(hwnd: isize, after: isize, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    pub fn IsZoomed(hwnd: isize) -> i32;
    pub fn EnumDisplayMonitors(dc: isize, clip: *const RECT, proc_: MONITORENUMPROC, lparam: isize) -> i32;
    pub fn GetMessageW(msg: *mut MSG, hwnd: isize, min: u32, max: u32) -> i32;
    pub fn DispatchMessageW(msg: *const MSG) -> isize;
    pub fn LockWorkStation() -> i32;
    pub fn ExitWindowsEx(flags: u32, reason: u32) -> i32;
    pub fn SendInput(count: u32, inputs: *const INPUT, size: i32) -> u32;
    pub fn GetWindowPlacement(hwnd: isize, placement: *mut WINDOWPLACEMENT) -> i32;
    pub fn RegisterHotKey(hwnd: isize, id: i32, modifiers: u32, vk: u32) -> i32;
    pub fn UnregisterHotKey(hwnd: isize, id: i32) -> i32;
    pub fn PeekMessageW(msg: *mut MSG, hwnd: isize, min: u32, max: u32, remove: u32) -> i32;
    pub fn PostThreadMessageW(thread: u32, msg: u32, wparam: usize, lparam: isize) -> i32;
}

#[link(name = "dwmapi", kind = "raw-dylib")]
extern "system" {
    pub fn DwmGetWindowAttribute(hwnd: isize, attr: u32, value: *mut c_void, size: u32) -> i32;
    pub fn DwmSetWindowAttribute(hwnd: isize, attr: u32, value: *const c_void, size: u32) -> i32;
    pub fn DwmRegisterThumbnail(dest: isize, source: isize, thumb: *mut isize) -> i32;
    pub fn DwmUnregisterThumbnail(thumb: isize) -> i32;
    pub fn DwmUpdateThumbnailProperties(thumb: isize, props: *const DWM_THUMBNAIL_PROPERTIES) -> i32;
    /// Returns once DWM has composed its next frame.
    pub fn DwmFlush() -> i32;
}

pub const DWMWA_EXTENDED_FRAME_BOUNDS: u32 = 9;
pub const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
pub const DWMWCP_ROUND: u32 = 2;
pub const DWM_TNP_RECTDESTINATION: u32 = 0x01;
pub const DWM_TNP_RECTSOURCE: u32 = 0x02;
pub const DWM_TNP_OPACITY: u32 = 0x04;
pub const DWM_TNP_VISIBLE: u32 = 0x08;

/// dwmapi.h packs its structs to one byte (`pshpack1.h`), so this is 45 bytes, not 48.
#[repr(C, packed)]
#[derive(Default, Clone, Copy)]
pub struct DWM_THUMBNAIL_PROPERTIES {
    pub dwFlags: u32,
    pub rcDestination: RECT,
    pub rcSource: RECT,
    pub opacity: u8,
    pub fVisible: i32,
    pub fSourceClientAreaOnly: i32,
}

// --- power ---------------------------------------------------------------------

#[link(name = "powrprof", kind = "raw-dylib")]
extern "system" {
    pub fn SetSuspendState(hibernate: u8, force: u8, wake_events_off: u8) -> u8;
}

// --- helpers -------------------------------------------------------------------

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// C:\Windows\System32, asked of Windows rather than built from environment variables.
pub fn system_dir() -> Option<std::path::PathBuf> {
    let mut buf = [0u16; 260];
    let n = unsafe { GetSystemDirectoryW(buf.as_mut_ptr(), buf.len() as u32) } as usize;
    (n > 0 && n < buf.len()).then(|| String::from_utf16_lossy(&buf[..n]).into())
}

/// # Safety
/// `p` must be null or point to a NUL-terminated UTF-16 string.
pub unsafe fn from_wide_ptr(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut n = 0;
    while *p.add(n) != 0 {
        n += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
}

pub fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// A string value under HKEY_CURRENT_USER.
pub fn reg_user_string(path: &str, value: &str) -> Option<String> {
    let (p, v) = (wide(path), wide(value));
    let mut len = 0u32;
    unsafe {
        if RegGetValueW(HKEY_CURRENT_USER, p.as_ptr(), v.as_ptr(), RRF_RT_REG_SZ, null_mut(), null_mut(), &mut len) != 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize / 2 + 1];
        if RegGetValueW(HKEY_CURRENT_USER, p.as_ptr(), v.as_ptr(), RRF_RT_REG_SZ, null_mut(), buf.as_mut_ptr().cast(), &mut len) != 0 {
            return None;
        }
        Some(from_wide(&buf).trim().to_string())
    }
}

/// A program's full path, or None when Windows won't say (an elevated or another user's process).
pub fn image_path(pid: u32) -> Option<String> {
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len) != 0;
        CloseHandle(h);
        ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// A file's version info, as one block for `VerQueryValueW`. None when it has none.
fn version_info(path: &str) -> Option<Vec<u8>> {
    let p = wide(path);
    unsafe {
        let mut handle = 0;
        let size = GetFileVersionInfoSizeExW(FILE_VER_GET_LOCALISED, p.as_ptr(), &mut handle);
        if size == 0 {
            return None;
        }
        let mut block = vec![0u8; size as usize];
        (GetFileVersionInfoExW(FILE_VER_GET_LOCALISED, p.as_ptr(), 0, size, block.as_mut_ptr().cast()) != 0).then_some(block)
    }
}

/// A program's version, as its file's version info has it ("0.4.1"): major, minor and patch.
/// None when it has none.
pub fn file_version(path: &str) -> Option<String> {
    let block = version_info(path)?;
    unsafe {
        let (mut ptr, mut len) = (null_mut(), 0u32);
        // VS_FIXEDFILEINFO: a signature, the structure's version, then the file's version.
        if VerQueryValueW(block.as_ptr().cast(), wide("\\").as_ptr(), &mut ptr, &mut len) == 0 || ptr.is_null() || len < 16 {
            return None;
        }
        let fixed = std::slice::from_raw_parts(ptr as *const u32, 4);
        if fixed[0] != 0xFEEF_04BD {
            return None;
        }
        let (ms, ls) = (fixed[2], fixed[3]);
        Some(format!("{}.{}.{}", ms >> 16, ms & 0xFFFF, ls >> 16))
    }
}

/// The name a program gives itself in its file's version info ("Google Chrome", "Visual Studio
/// Code"). None when it has none.
pub fn file_description(path: &str) -> Option<String> {
    let block = version_info(path)?;
    unsafe {
        let query = |sub: &str| -> Option<(*mut c_void, u32)> {
            let s = wide(sub);
            let (mut ptr, mut len) = (null_mut(), 0u32);
            (VerQueryValueW(block.as_ptr().cast(), s.as_ptr(), &mut ptr, &mut len) != 0 && !ptr.is_null() && len > 0).then_some((ptr, len))
        };
        // The file's own languages first, then the two English tables most files have.
        let mut tables = Vec::new();
        if let Some((ptr, len)) = query(r"\VarFileInfo\Translation") {
            let pairs = std::slice::from_raw_parts(ptr as *const u16, (len / 2) as usize);
            for [lang, codepage] in pairs.as_chunks::<2>().0 {
                tables.push(format!("{lang:04x}{codepage:04x}"));
            }
        }
        tables.extend(["040904b0".into(), "040904e4".into()]);
        for t in tables {
            if let Some((ptr, len)) = query(&format!(r"\StringFileInfo\{t}\FileDescription")) {
                let s = from_wide(std::slice::from_raw_parts(ptr as *const u16, len as usize));
                let s = s.trim();
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
        None
    }
}

/// Open a file, folder, program, `shell:` path or URL as a double click (or Win+R) would.
pub fn shell_open(target: &str, params: Option<&str>, verb: &str) -> Result<(), String> {
    let (op, file) = (wide(verb), wide(target));
    let params = params.map(wide);
    let r = unsafe {
        ShellExecuteW(0, op.as_ptr(), file.as_ptr(), params.as_ref().map_or(std::ptr::null(), |p| p.as_ptr()), std::ptr::null(), SW_SHOWNORMAL)
    };
    if r > 32 {
        Ok(())
    } else {
        Err(format!("Windows couldn't open it (error {r})"))
    }
}

/// Press and release keys, as if typed: each of `keys` down in order, then up in reverse.
pub fn send_keys(keys: &[u16]) {
    let input = |vk: u16, up: bool| INPUT {
        kind: INPUT_KEYBOARD,
        ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, dwExtraInfo: 0 },
        _pad: [0; 8],
    };
    let all: Vec<INPUT> = keys.iter().map(|&k| input(k, false)).chain(keys.iter().rev().map(|&k| input(k, true))).collect();
    unsafe { SendInput(all.len() as u32, all.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_have_versions() {
        let notepad = format!(r"{}\System32\notepad.exe", std::env::var("SystemRoot").unwrap());
        let v = file_version(&notepad).expect("Notepad has a version");
        assert_eq!(v.split('.').count(), 3, "{v}");
        assert!(v.split('.').all(|n| n.parse::<u32>().is_ok()), "{v}");
        assert_eq!(file_version(r"C:\no such folder\nothing.exe"), None);
    }
}
