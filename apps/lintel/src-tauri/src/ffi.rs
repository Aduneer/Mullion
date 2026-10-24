//! Hand-written bindings for the flat Win32 calls Lintel uses, in the style of Oriel's.
//! COM and WinRT (Core Audio, media controls, network list) go through the `windows` crate instead.
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

// --- COM -----------------------------------------------------------------------

pub const COINIT_MULTITHREADED: u32 = 0;

#[link(name = "ole32", kind = "raw-dylib")]
extern "system" {
    pub fn CoInitializeEx(reserved: *mut c_void, coinit: u32) -> i32;
}

// --- shell ---------------------------------------------------------------------

pub const SW_SHOWNORMAL: i32 = 1;

pub const ABM_NEW: u32 = 0x00;
pub const ABM_REMOVE: u32 = 0x01;
pub const ABM_QUERYPOS: u32 = 0x02;
pub const ABM_SETPOS: u32 = 0x03;
pub const ABM_ACTIVATE: u32 = 0x06;
pub const ABM_WINDOWPOSCHANGED: u32 = 0x09;
pub const ABE_LEFT: u32 = 0;
pub const ABE_TOP: u32 = 1;
pub const ABE_RIGHT: u32 = 2;
pub const ABE_BOTTOM: u32 = 3;
pub const ABN_POSCHANGED: usize = 0x01;
pub const ABN_FULLSCREENAPP: usize = 0x02;

#[repr(C)]
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
pub struct APPBARDATA {
    pub cbSize: u32,
    pub hWnd: isize,
    pub uCallbackMessage: u32,
    pub uEdge: u32,
    pub rc: RECT,
    pub lParam: isize,
}

#[link(name = "shell32", kind = "raw-dylib")]
extern "system" {
    /// Returns a value > 32 on success.
    pub fn ShellExecuteW(hwnd: isize, op: *const u16, file: *const u16, params: *const u16, dir: *const u16, show: i32) -> isize;
    pub fn SHAppBarMessage(msg: u32, data: *mut APPBARDATA) -> usize;
}

// --- windows -------------------------------------------------------------------

pub const GWL_EXSTYLE: i32 = -20;
pub const GW_OWNER: u32 = 4;
pub const SW_HIDE: i32 = 0;
pub const SW_MINIMIZE: i32 = 6;
pub const SW_SHOWNOACTIVATE: i32 = 4;
pub const SW_RESTORE: i32 = 9;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_ACTIVATE: u32 = 0x0006;
pub const WM_WINDOWPOSCHANGED: u32 = 0x0047;
pub const WM_DISPLAYCHANGE: u32 = 0x007E;
pub const WM_DPICHANGED: u32 = 0x02E0;
pub const WM_SETTINGCHANGE: u32 = 0x001A;
pub const SPI_SETDESKWALLPAPER: usize = 0x0014;
pub const WM_NCDESTROY: u32 = 0x0082;
pub const WM_APP: u32 = 0x8000;
pub const DWMWA_CLOAKED: u32 = 14;
pub const MONITOR_DEFAULTTOPRIMARY: u32 = 1;
pub const HWND_TOPMOST: isize = -1;
pub const SWP_NOACTIVATE: u32 = 0x0010;
pub const SWP_NOZORDER: u32 = 0x0004;

pub const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
pub const EVENT_SYSTEM_MINIMIZEEND: u32 = 0x0017;
pub const EVENT_SYSTEM_MOVESIZEEND: u32 = 0x000B;
pub const EVENT_SYSTEM_MINIMIZESTART: u32 = 0x0016;
pub const EVENT_OBJECT_DESTROY: u32 = 0x8001;
pub const EVENT_OBJECT_SHOW: u32 = 0x8002;
pub const EVENT_OBJECT_HIDE: u32 = 0x8003;
pub const EVENT_OBJECT_NAMECHANGE: u32 = 0x800C;
pub const EVENT_OBJECT_CLOAKED: u32 = 0x8017;
pub const EVENT_OBJECT_UNCLOAKED: u32 = 0x8018;
pub const MONITOR_DEFAULTTONEAREST: u32 = 2;
pub const WINEVENT_OUTOFCONTEXT: u32 = 0x0000;
pub const WINEVENT_SKIPOWNPROCESS: u32 = 0x0002;

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

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
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
pub struct MONITORINFOEXW {
    pub info: MONITORINFO,
    pub szDevice: [u16; 32],
}
pub const MONITORINFOF_PRIMARY: u32 = 1;
pub type MONITORENUMPROC = unsafe extern "system" fn(monitor: isize, dc: isize, rect: *mut RECT, lparam: isize) -> i32;

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
pub type TIMERPROC = unsafe extern "system" fn(hwnd: isize, msg: u32, id: usize, time: u32);
pub type WINEVENTPROC = unsafe extern "system" fn(hook: isize, event: u32, hwnd: isize, object: i32, child: i32, thread: u32, time: u32);
pub type SUBCLASSPROC = unsafe extern "system" fn(hwnd: isize, msg: u32, wparam: usize, lparam: isize, id: usize, data: usize) -> isize;

#[link(name = "user32", kind = "raw-dylib")]
extern "system" {
    pub fn GetWindowRect(hwnd: isize, rect: *mut RECT) -> i32;
    pub fn EnumWindows(callback: WNDENUMPROC, lparam: isize) -> i32;
    pub fn EnumChildWindows(parent: isize, callback: WNDENUMPROC, lparam: isize) -> i32;
    pub fn IsWindow(hwnd: isize) -> i32;
    pub fn IsWindowVisible(hwnd: isize) -> i32;
    pub fn IsIconic(hwnd: isize) -> i32;
    pub fn GetWindow(hwnd: isize, cmd: u32) -> isize;
    pub fn GetAncestor(hwnd: isize, flags: u32) -> isize;
    pub fn GetWindowLongW(hwnd: isize, index: i32) -> i32;
    pub fn GetWindowTextW(hwnd: isize, buf: *mut u16, max: i32) -> i32;
    pub fn GetClassNameW(hwnd: isize, buf: *mut u16, max: i32) -> i32;
    pub fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
    pub fn GetForegroundWindow() -> isize;
    pub fn SetForegroundWindow(hwnd: isize) -> i32;
    pub fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
    pub fn PostMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> i32;
    pub fn SetWindowPos(hwnd: isize, after: isize, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    pub fn MonitorFromWindow(hwnd: isize, flags: u32) -> isize;
    pub fn MonitorFromPoint(pt: POINT, flags: u32) -> isize;
    pub fn GetMonitorInfoW(monitor: isize, info: *mut MONITORINFO) -> i32;
    pub fn EnumDisplayMonitors(dc: isize, clip: *const RECT, proc_: MONITORENUMPROC, lparam: isize) -> i32;
    pub fn GetDpiForWindow(hwnd: isize) -> u32;
    pub fn SetTimer(hwnd: isize, id: usize, ms: u32, proc_: TIMERPROC) -> usize;
    pub fn KillTimer(hwnd: isize, id: usize) -> i32;
    pub fn SetWinEventHook(min: u32, max: u32, module: isize, proc_: WINEVENTPROC, pid: u32, thread: u32, flags: u32) -> isize;
    pub fn GetMessageW(msg: *mut MSG, hwnd: isize, min: u32, max: u32) -> i32;
    pub fn DispatchMessageW(msg: *const MSG) -> isize;
    pub fn LockWorkStation() -> i32;
    pub fn ExitWindowsEx(flags: u32, reason: u32) -> i32;
    pub fn SendInput(count: u32, inputs: *const INPUT, size: i32) -> u32;
}

#[link(name = "comctl32", kind = "raw-dylib")]
extern "system" {
    pub fn SetWindowSubclass(hwnd: isize, proc_: SUBCLASSPROC, id: usize, data: usize) -> i32;
    pub fn RemoveWindowSubclass(hwnd: isize, proc_: SUBCLASSPROC, id: usize) -> i32;
    pub fn DefSubclassProc(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
}

#[link(name = "dwmapi", kind = "raw-dylib")]
extern "system" {
    pub fn DwmGetWindowAttribute(hwnd: isize, attr: u32, value: *mut c_void, size: u32) -> i32;
}

// --- power ---------------------------------------------------------------------

#[link(name = "powrprof", kind = "raw-dylib")]
extern "system" {
    pub fn SetSuspendState(hibernate: u8, force: u8, wake_events_off: u8) -> u8;
}

// --- Wi-Fi ---------------------------------------------------------------------

pub const WLAN_INTF_OPCODE_CURRENT_CONNECTION: u32 = 7;
pub const WLAN_INTERFACE_STATE_CONNECTED: u32 = 1;

#[repr(C)]
pub struct WLAN_INTERFACE_INFO {
    pub InterfaceGuid: [u8; 16],
    pub strInterfaceDescription: [u16; 256],
    pub isState: u32,
}

#[repr(C)]
pub struct WLAN_INTERFACE_INFO_LIST {
    pub dwNumberOfItems: u32,
    pub dwIndex: u32,
    pub InterfaceInfo: [WLAN_INTERFACE_INFO; 1],
}

/// The leading part of WLAN_CONNECTION_ATTRIBUTES; we never read the security attributes after it.
#[repr(C)]
pub struct WLAN_CONNECTION_ATTRIBUTES {
    pub isState: u32,
    pub wlanConnectionMode: u32,
    pub strProfileName: [u16; 256],
    pub uSSIDLength: u32,
    pub ucSSID: [u8; 32],
    pub dot11BssType: u32,
    pub dot11Bssid: [u8; 6],
    pub dot11PhyType: u32,
    pub uDot11PhyIndex: u32,
    /// 0–100
    pub wlanSignalQuality: u32,
    pub ulRxRate: u32,
    pub ulTxRate: u32,
}

#[link(name = "wlanapi", kind = "raw-dylib")]
extern "system" {
    pub fn WlanOpenHandle(version: u32, reserved: *mut c_void, negotiated: *mut u32, handle: *mut HANDLE) -> u32;
    pub fn WlanEnumInterfaces(handle: HANDLE, reserved: *mut c_void, list: *mut *mut WLAN_INTERFACE_INFO_LIST) -> u32;
    pub fn WlanQueryInterface(
        handle: HANDLE,
        guid: *const [u8; 16],
        opcode: u32,
        reserved: *mut c_void,
        size: *mut u32,
        data: *mut *mut c_void,
        value_type: *mut u32,
    ) -> u32;
    pub fn WlanFreeMemory(memory: *mut c_void);
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

/// The name a program gives itself in its file's version info ("Google Chrome", "Visual Studio
/// Code"), which is what macOS would show. None when it has none.
pub fn file_description(path: &str) -> Option<String> {
    let p = wide(path);
    unsafe {
        let mut handle = 0;
        let size = GetFileVersionInfoSizeExW(FILE_VER_GET_LOCALISED, p.as_ptr(), &mut handle);
        if size == 0 {
            return None;
        }
        let mut block = vec![0u8; size as usize];
        if GetFileVersionInfoExW(FILE_VER_GET_LOCALISED, p.as_ptr(), 0, size, block.as_mut_ptr().cast()) == 0 {
            return None;
        }
        let query = |sub: &str| -> Option<(*mut c_void, u32)> {
            let s = wide(sub);
            let (mut ptr, mut len) = (null_mut(), 0u32);
            (VerQueryValueW(block.as_ptr().cast(), s.as_ptr(), &mut ptr, &mut len) != 0 && !ptr.is_null() && len > 0).then_some((ptr, len))
        };
        // The file's own languages first, then the two English tables most files have.
        let mut tables = Vec::new();
        if let Some((ptr, len)) = query(r"\VarFileInfo\Translation") {
            let pairs = std::slice::from_raw_parts(ptr as *const u16, (len / 2) as usize);
            for pair in pairs.chunks_exact(2) {
                tables.push(format!("{:04x}{:04x}", pair[0], pair[1]));
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
