//! Hand-written bindings for the flat Win32 calls only Bay uses: finding the desktop's window,
//! following a drag, moving a window to another screen, and putting a picked window on top with
//! the pointer in its middle; and whether a program runs as administrator. The ones the apps share
//! (DWM's thumbnails among them) come from `mullion_core::ffi`.
#![allow(non_snake_case, clippy::upper_case_acronyms)]

pub use mullion_core::ffi::*;

#[link(name = "user32", kind = "raw-dylib")]
extern "system" {
    pub fn FindWindowW(class: *const u16, name: *const u16) -> isize;
    pub fn SetWindowPos(hwnd: isize, after: isize, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    pub fn BringWindowToTop(hwnd: isize) -> i32;
    pub fn GetCursorPos(point: *mut POINT) -> i32;
    pub fn SetCursorPos(x: i32, y: i32) -> i32;
    pub fn GetAsyncKeyState(vk: i32) -> i16;
    pub fn GetSystemMetrics(index: i32) -> i32;
    pub fn IsZoomed(hwnd: isize) -> i32;
}

#[link(name = "advapi32", kind = "raw-dylib")]
extern "system" {
    pub fn GetTokenInformation(token: HANDLE, class: u32, info: *mut std::ffi::c_void, len: u32, ret_len: *mut u32) -> i32;
}

pub const TOKEN_ELEVATION: u32 = 20;

/// Whether a process runs as administrator. Windows won't let a program that isn't move such a
/// program's windows, so Bay asks before a drag rather than finding out at the drop.
pub fn elevated(pid: u32) -> bool {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return false;
        }
        let mut token = std::ptr::null_mut();
        let mut on = 0u32;
        let mut len = 0;
        if OpenProcessToken(process, TOKEN_QUERY, &mut token) != 0 {
            GetTokenInformation(token, TOKEN_ELEVATION, (&mut on as *mut u32).cast(), 4, &mut len);
            CloseHandle(token);
        }
        CloseHandle(process);
        on != 0
    }
}

pub const HWND_TOP: isize = 0;
pub const SWP_NOSIZE: u32 = 0x0001;
pub const SWP_NOMOVE: u32 = 0x0002;
pub const SWP_NOZORDER: u32 = 0x0004;
pub const SWP_NOACTIVATE: u32 = 0x0010;
pub const SW_MAXIMIZE: i32 = 3;
pub const SM_SWAPBUTTON: i32 = 23;
pub const VK_LBUTTON: i32 = 0x01;
pub const VK_RBUTTON: i32 = 0x02;
