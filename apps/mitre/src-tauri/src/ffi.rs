//! Hand-written bindings for the flat Win32 calls only Mitre uses: Raw Input for the mouse, the
//! hidden window and timers that hear it, each screen's lasting name and scaling, and the glow's
//! layered window. The ones the apps share come from `mullion_core::ffi`.
#![allow(non_snake_case, clippy::upper_case_acronyms)]

pub use mullion_core::ffi::*;
use std::ffi::c_void;

pub type WNDPROC = unsafe extern "system" fn(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;

#[repr(C)]
pub struct WNDCLASSW {
    pub style: u32,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: isize,
    pub hIcon: isize,
    pub hCursor: isize,
    pub hbrBackground: isize,
    pub lpszMenuName: *const u16,
    pub lpszClassName: *const u16,
}

#[repr(C)]
pub struct RAWINPUTDEVICE {
    pub usUsagePage: u16,
    pub usUsage: u16,
    pub dwFlags: u32,
    pub hwndTarget: isize,
}

#[repr(C)]
pub struct MONITORINFOEXW {
    pub info: MONITORINFO,
    pub szDevice: [u16; 32],
}

#[repr(C)]
pub struct DISPLAY_DEVICEW {
    pub cb: u32,
    pub DeviceName: [u16; 32],
    pub DeviceString: [u16; 128],
    pub StateFlags: u32,
    pub DeviceID: [u16; 128],
    pub DeviceKey: [u16; 128],
}

#[repr(C)]
pub struct BITMAPINFOHEADER {
    pub biSize: u32,
    pub biWidth: i32,
    pub biHeight: i32,
    pub biPlanes: u16,
    pub biBitCount: u16,
    pub biCompression: u32,
    pub biSizeImage: u32,
    pub biXPelsPerMeter: i32,
    pub biYPelsPerMeter: i32,
    pub biClrUsed: u32,
    pub biClrImportant: u32,
}

#[repr(C)]
pub struct BITMAPINFO {
    pub bmiHeader: BITMAPINFOHEADER,
    pub bmiColors: [u32; 1],
}

#[repr(C)]
pub struct SIZE {
    pub cx: i32,
    pub cy: i32,
}

#[repr(C)]
pub struct BLENDFUNCTION {
    pub BlendOp: u8,
    pub BlendFlags: u8,
    pub SourceConstantAlpha: u8,
    pub AlphaFormat: u8,
}

#[link(name = "user32", kind = "raw-dylib")]
extern "system" {
    pub fn GetCursorPos(pt: *mut POINT) -> i32;
    pub fn RegisterClassW(class: *const WNDCLASSW) -> u16;
    pub fn CreateWindowExW(
        ex_style: u32,
        class: *const u16,
        name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: isize,
        menu: isize,
        instance: isize,
        param: *mut c_void,
    ) -> isize;
    pub fn DefWindowProcW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
    pub fn RegisterRawInputDevices(devices: *const RAWINPUTDEVICE, count: u32, size: u32) -> i32;
    pub fn SetTimer(hwnd: isize, id: usize, ms: u32, proc_: *const c_void) -> usize;
    pub fn KillTimer(hwnd: isize, id: usize) -> i32;
    pub fn GetAsyncKeyState(vk: i32) -> i16;
    pub fn GetShellWindow() -> isize;
    pub fn EnumDisplayDevicesW(device: *const u16, index: u32, info: *mut DISPLAY_DEVICEW, flags: u32) -> i32;
    pub fn UpdateLayeredWindow(
        hwnd: isize,
        dst: isize,
        pos: *const POINT,
        size: *const SIZE,
        src: isize,
        src_pos: *const POINT,
        key: u32,
        blend: *const BLENDFUNCTION,
        flags: u32,
    ) -> i32;
}

#[link(name = "gdi32", kind = "raw-dylib")]
extern "system" {
    pub fn CreateCompatibleDC(dc: isize) -> isize;
    pub fn DeleteDC(dc: isize) -> i32;
    pub fn CreateDIBSection(dc: isize, info: *const BITMAPINFO, usage: u32, bits: *mut *mut c_void, section: isize, offset: u32) -> isize;
    pub fn SelectObject(dc: isize, obj: isize) -> isize;
    pub fn DeleteObject(obj: isize) -> i32;
}

#[link(name = "kernel32", kind = "raw-dylib")]
extern "system" {
    pub fn GetModuleHandleW(name: *const u16) -> isize;
}

#[link(name = "shell32", kind = "raw-dylib")]
extern "system" {
    pub fn SHQueryUserNotificationState(state: *mut i32) -> i32;
}

#[link(name = "shcore", kind = "raw-dylib")]
extern "system" {
    pub fn GetDpiForMonitor(monitor: isize, kind: u32, x: *mut u32, y: *mut u32) -> i32;
}

pub const WM_DISPLAYCHANGE: u32 = 0x007E;
pub const WM_INPUT: u32 = 0x00FF;
pub const WM_TIMER: u32 = 0x0113;
pub const WM_SYSCOMMAND: u32 = 0x0112;
pub const WM_DPICHANGED: u32 = 0x02E0;
pub const SC_MONITORPOWER: usize = 0xF170;
pub const RIDEV_INPUTSINK: u32 = 0x0000_0100;
pub const WS_POPUP: u32 = 0x8000_0000;
pub const WS_CAPTION: u32 = 0x00C0_0000;
pub const GWL_STYLE: i32 = -16;
pub const WS_EX_TOPMOST: u32 = 0x0000_0008;
pub const WS_EX_TRANSPARENT: u32 = 0x0000_0020;
pub const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
pub const WS_EX_LAYERED: u32 = 0x0008_0000;
pub const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
pub const HWND_TOPMOST: isize = -1;
pub const SW_HIDE: i32 = 0;
pub const SW_SHOWNOACTIVATE: i32 = 4;
pub const MONITORINFOF_PRIMARY: u32 = 1;
pub const MDT_EFFECTIVE_DPI: u32 = 0;
pub const EDD_GET_DEVICE_INTERFACE_NAME: u32 = 1;
pub const VK_LBUTTON: i32 = 0x01;
pub const VK_RBUTTON: i32 = 0x02;
pub const VK_MBUTTON: i32 = 0x04;
pub const VK_XBUTTON1: i32 = 0x05;
pub const VK_XBUTTON2: i32 = 0x06;
pub const QUNS_BUSY: i32 = 2;
pub const QUNS_RUNNING_D3D_FULL_SCREEN: i32 = 3;
pub const QUNS_PRESENTATION_MODE: i32 = 4;
pub const DIB_RGB_COLORS: u32 = 0;
pub const AC_SRC_OVER: u8 = 0;
pub const AC_SRC_ALPHA: u8 = 1;
pub const ULW_ALPHA: u32 = 2;
