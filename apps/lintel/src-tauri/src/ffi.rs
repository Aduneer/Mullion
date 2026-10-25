//! Hand-written bindings for the flat Win32 calls only Lintel uses: the app bar, the WinEvent
//! hooks, monitors and Wi-Fi. The ones the apps share come from `mullion_core::ffi`.
//! COM and WinRT (Core Audio, media controls, network list) go through the `windows` crate instead.
#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms, dead_code)]

pub use mullion_core::ffi::*;

use std::ffi::c_void;

// --- shell ---------------------------------------------------------------------

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
    pub fn SHAppBarMessage(msg: u32, data: *mut APPBARDATA) -> usize;
}

// --- windows -------------------------------------------------------------------

pub const SW_HIDE: i32 = 0;
pub const SW_SHOWNOACTIVATE: i32 = 4;
pub const WM_ACTIVATE: u32 = 0x0006;
pub const WM_WINDOWPOSCHANGED: u32 = 0x0047;
pub const WM_DISPLAYCHANGE: u32 = 0x007E;
pub const WM_DPICHANGED: u32 = 0x02E0;
pub const WM_SETTINGCHANGE: u32 = 0x001A;
pub const SPI_SETDESKWALLPAPER: usize = 0x0014;
pub const WM_NCDESTROY: u32 = 0x0082;
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
pub const WINEVENT_OUTOFCONTEXT: u32 = 0x0000;
pub const WINEVENT_SKIPOWNPROCESS: u32 = 0x0002;

#[repr(C)]
pub struct MONITORINFOEXW {
    pub info: MONITORINFO,
    pub szDevice: [u16; 32],
}
pub const MONITORINFOF_PRIMARY: u32 = 1;
pub type MONITORENUMPROC = unsafe extern "system" fn(monitor: isize, dc: isize, rect: *mut RECT, lparam: isize) -> i32;

pub type TIMERPROC = unsafe extern "system" fn(hwnd: isize, msg: u32, id: usize, time: u32);
pub type WINEVENTPROC = unsafe extern "system" fn(hook: isize, event: u32, hwnd: isize, object: i32, child: i32, thread: u32, time: u32);
pub type SUBCLASSPROC = unsafe extern "system" fn(hwnd: isize, msg: u32, wparam: usize, lparam: isize, id: usize, data: usize) -> isize;

#[link(name = "user32", kind = "raw-dylib")]
extern "system" {
    pub fn FindWindowW(class: *const u16, title: *const u16) -> isize;
    pub fn GetAncestor(hwnd: isize, flags: u32) -> isize;
    pub fn SetWindowPos(hwnd: isize, after: isize, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    pub fn EnumDisplayMonitors(dc: isize, clip: *const RECT, proc_: MONITORENUMPROC, lparam: isize) -> i32;
    pub fn GetDpiForWindow(hwnd: isize) -> u32;
    pub fn SetTimer(hwnd: isize, id: usize, ms: u32, proc_: TIMERPROC) -> usize;
    pub fn KillTimer(hwnd: isize, id: usize) -> i32;
    pub fn SetWinEventHook(min: u32, max: u32, module: isize, proc_: WINEVENTPROC, pid: u32, thread: u32, flags: u32) -> isize;
}

#[link(name = "comctl32", kind = "raw-dylib")]
extern "system" {
    pub fn SetWindowSubclass(hwnd: isize, proc_: SUBCLASSPROC, id: usize, data: usize) -> i32;
    pub fn RemoveWindowSubclass(hwnd: isize, proc_: SUBCLASSPROC, id: usize) -> i32;
    pub fn DefSubclassProc(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
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
