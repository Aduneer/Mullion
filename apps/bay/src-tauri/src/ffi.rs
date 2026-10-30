//! Hand-written bindings for the flat Win32 calls only Bay uses: finding the desktop's window,
//! following a drag, moving a window to another screen, and putting a picked window on top with
//! the pointer in its middle. The ones the apps share (DWM's thumbnails among them) come from
//! `mullion_core::ffi`.
#![allow(non_snake_case, clippy::upper_case_acronyms)]

pub use mullion_core::ffi::*;

#[link(name = "user32", kind = "raw-dylib")]
extern "system" {
    pub fn FindWindowW(class: *const u16, name: *const u16) -> isize;
    pub fn BringWindowToTop(hwnd: isize) -> i32;
    pub fn GetCursorPos(point: *mut POINT) -> i32;
    pub fn SetCursorPos(x: i32, y: i32) -> i32;
}

pub const HWND_TOP: isize = 0;
pub const SW_MAXIMIZE: i32 = 3;
