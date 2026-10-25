//! Hand-written bindings for the flat Win32 calls only Fanlight uses: the clipboard, the shortcut
//! and its message-only window. The ones the apps share come from `mullion_core::ffi`.
//! COM (the shell's app list, icons, known folders) goes through the `windows` crate instead.
#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms, dead_code)]

pub use mullion_core::ffi::*;

use std::ffi::c_void;

// --- kernel32 ------------------------------------------------------------------

pub const GMEM_MOVEABLE: u32 = 0x0002;

#[link(name = "kernel32", kind = "raw-dylib")]
extern "system" {
    pub fn GetCurrentThreadId() -> u32;
    pub fn GlobalAlloc(flags: u32, bytes: usize) -> HANDLE;
    pub fn GlobalLock(mem: HANDLE) -> *mut c_void;
    pub fn GlobalUnlock(mem: HANDLE) -> i32;
    pub fn GlobalSize(mem: HANDLE) -> usize;
    pub fn GlobalFree(mem: HANDLE) -> HANDLE;
    pub fn GetModuleHandleW(name: *const u16) -> isize;
}

// --- windows -------------------------------------------------------------------

pub const WM_HOTKEY: u32 = 0x0312;
pub const WM_CLIPBOARDUPDATE: u32 = 0x031D;
pub const HWND_MESSAGE: isize = -3;

pub const MOD_ALT: u32 = 0x0001;
pub const MOD_CONTROL: u32 = 0x0002;
pub const MOD_SHIFT: u32 = 0x0004;
pub const MOD_WIN: u32 = 0x0008;
pub const MOD_NOREPEAT: u32 = 0x4000;

pub const VK_CONTROL: u16 = 0x11;
pub const VK_SHIFT: u16 = 0x10;
pub const VK_MENU: u16 = 0x12;
pub const VK_SPACE: u16 = 0x20;

pub const CF_UNICODETEXT: u32 = 13;

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

#[link(name = "user32", kind = "raw-dylib")]
extern "system" {
    pub fn GetCursorPos(pt: *mut POINT) -> i32;
    pub fn RegisterHotKey(hwnd: isize, id: i32, modifiers: u32, vk: u32) -> i32;
    pub fn UnregisterHotKey(hwnd: isize, id: i32) -> i32;
    pub fn PeekMessageW(msg: *mut MSG, hwnd: isize, min: u32, max: u32, remove: u32) -> i32;
    pub fn PostThreadMessageW(thread: u32, msg: u32, wparam: usize, lparam: isize) -> i32;
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
    pub fn AddClipboardFormatListener(hwnd: isize) -> i32;
    pub fn OpenClipboard(owner: isize) -> i32;
    pub fn CloseClipboard() -> i32;
    pub fn EmptyClipboard() -> i32;
    pub fn GetClipboardData(format: u32) -> HANDLE;
    pub fn SetClipboardData(format: u32, mem: HANDLE) -> HANDLE;
    pub fn IsClipboardFormatAvailable(format: u32) -> i32;
    pub fn RegisterClipboardFormatW(name: *const u16) -> u32;
    pub fn GetClipboardSequenceNumber() -> u32;
    pub fn GetClipboardOwner() -> isize;
    pub fn GetAsyncKeyState(vk: i32) -> i16;
}
