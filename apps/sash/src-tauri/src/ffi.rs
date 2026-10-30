//! Hand-written bindings for the flat Win32 calls only Sash uses: telling a hung window, why
//! Windows refused a move, and each screen's scaling. The ones the apps share (moving windows and
//! listing screens among them) come from `mullion_core::ffi`.
#![allow(non_snake_case, clippy::upper_case_acronyms)]

pub use mullion_core::ffi::*;

#[link(name = "user32", kind = "raw-dylib")]
extern "system" {
    pub fn IsHungAppWindow(hwnd: isize) -> i32;
    pub fn MonitorFromRect(rect: *const RECT, flags: u32) -> isize;
}

#[link(name = "kernel32", kind = "raw-dylib")]
extern "system" {
    pub fn GetLastError() -> u32;
}

#[link(name = "shcore", kind = "raw-dylib")]
extern "system" {
    pub fn GetDpiForMonitor(monitor: isize, kind: u32, x: *mut u32, y: *mut u32) -> i32;
}

pub const MDT_EFFECTIVE_DPI: u32 = 0;
pub const SWP_FRAMECHANGED: u32 = 0x0020;
pub const SWP_NOOWNERZORDER: u32 = 0x0200;
pub const HWND_TOPMOST: isize = -1;
pub const SW_MAXIMIZE: i32 = 3;
pub const SW_SHOWNOACTIVATE: i32 = 4;
pub const ERROR_ACCESS_DENIED: u32 = 5;
