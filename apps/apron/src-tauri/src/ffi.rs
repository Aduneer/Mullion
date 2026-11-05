//! Hand-written bindings for the flat Win32 calls only Apron uses: the tray host's windows and
//! messages, the app bar and the taskbar, the work area, drawing other programs' icons, and the
//! dock's menus. The ones the apps share come from `mullion_core::ffi`.
#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms, dead_code)]

pub use mullion_core::ffi::*;

use std::ffi::c_void;

// --- windows -------------------------------------------------------------------

pub type WNDPROC = unsafe extern "system" fn(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
pub type SUBCLASSPROC = unsafe extern "system" fn(hwnd: isize, msg: u32, wparam: usize, lparam: isize, id: usize, data: usize) -> isize;

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
pub struct COPYDATASTRUCT {
    pub dwData: usize,
    pub cbData: u32,
    pub lpData: *mut c_void,
}

#[repr(C)]
pub struct MONITORINFOEXW {
    pub info: MONITORINFO,
    pub szDevice: [u16; 32],
}

pub const WS_POPUP: u32 = 0x8000_0000;
pub const WS_CHILD: u32 = 0x4000_0000;
pub const WS_CLIPCHILDREN: u32 = 0x0200_0000;
pub const WS_CLIPSIBLINGS: u32 = 0x0400_0000;
pub const WS_CAPTION: u32 = 0x00C0_0000;
pub const WS_EX_TOPMOST: u32 = 0x0000_0008;
pub const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
pub const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
pub const GWL_STYLE: i32 = -16;

pub const SW_HIDE: i32 = 0;
pub const SW_SHOWNOACTIVATE: i32 = 4;
pub const SW_SHOWNA: i32 = 8;
pub const HWND_TOPMOST: isize = -1;
pub const HWND_BROADCAST: isize = 0xffff;
pub const SWP_SHOWWINDOW: u32 = 0x0040;

pub const WM_DESTROY: u32 = 0x0002;
pub const WM_ACTIVATE: u32 = 0x0006;
pub const WM_COPYDATA: u32 = 0x004A;
pub const WM_WINDOWPOSCHANGED: u32 = 0x0047;
pub const WM_DISPLAYCHANGE: u32 = 0x007E;
pub const WM_DPICHANGED: u32 = 0x02E0;
pub const WM_NCDESTROY: u32 = 0x0082;
pub const WM_NULL: u32 = 0x0000;
pub const WM_USER: u32 = 0x0400;
pub const WM_CONTEXTMENU: u32 = 0x007B;
pub const WM_LBUTTONDOWN: u32 = 0x0201;
pub const WM_LBUTTONUP: u32 = 0x0202;
pub const WM_LBUTTONDBLCLK: u32 = 0x0203;
pub const WM_RBUTTONDOWN: u32 = 0x0204;
pub const WM_RBUTTONUP: u32 = 0x0205;

pub const SMTO_ABORTIFHUNG: u32 = 0x0002;
pub const MONITORINFOF_PRIMARY: u32 = 1;
pub const SPI_GETWORKAREA: u32 = 0x0030;
pub const SPI_SETWORKAREA: u32 = 0x002F;
pub const SPIF_SENDCHANGE: u32 = 0x0002;

#[link(name = "user32", kind = "raw-dylib")]
extern "system" {
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
        param: *const c_void,
    ) -> isize;
    pub fn DefWindowProcW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
    pub fn DestroyWindow(hwnd: isize) -> i32;
    pub fn PostQuitMessage(code: i32);
    pub fn RegisterWindowMessageW(name: *const u16) -> u32;
    pub fn FindWindowW(class: *const u16, title: *const u16) -> isize;
    pub fn SendMessageTimeoutW(hwnd: isize, msg: u32, wparam: usize, lparam: isize, flags: u32, ms: u32, result: *mut usize) -> isize;
    pub fn SendNotifyMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> i32;
    pub fn AllowSetForegroundWindow(pid: u32) -> i32;
    pub fn GetCursorPos(pt: *mut POINT) -> i32;
    pub fn ShowWindowAsync(hwnd: isize, cmd: i32) -> i32;
    pub fn GetDpiForWindow(hwnd: isize) -> u32;
    pub fn SystemParametersInfoW(action: u32, param: u32, value: *mut c_void, flags: u32) -> i32;
    pub fn DrawIconEx(dc: isize, x: i32, y: i32, icon: isize, w: i32, h: i32, step: u32, brush: isize, flags: u32) -> i32;
    pub fn CreatePopupMenu() -> isize;
    pub fn AppendMenuW(menu: isize, flags: u32, id: usize, text: *const u16) -> i32;
    pub fn TrackPopupMenuEx(menu: isize, flags: u32, x: i32, y: i32, hwnd: isize, params: *const c_void) -> i32;
    pub fn DestroyMenu(menu: isize) -> i32;
}

pub const MF_STRING: u32 = 0x0000;
pub const MF_GRAYED: u32 = 0x0001;
pub const MF_CHECKED: u32 = 0x0008;
pub const MF_SEPARATOR: u32 = 0x0800;
pub const TPM_RIGHTBUTTON: u32 = 0x0002;
pub const TPM_CENTERALIGN: u32 = 0x0004;
pub const TPM_RIGHTALIGN: u32 = 0x0008;
pub const TPM_VCENTERALIGN: u32 = 0x0010;
pub const TPM_BOTTOMALIGN: u32 = 0x0020;
pub const TPM_RETURNCMD: u32 = 0x0100;
pub const DI_MASK: u32 = 0x0001;
pub const DI_NORMAL: u32 = 0x0003;

#[link(name = "comctl32", kind = "raw-dylib")]
extern "system" {
    pub fn SetWindowSubclass(hwnd: isize, proc_: SUBCLASSPROC, id: usize, data: usize) -> i32;
    pub fn RemoveWindowSubclass(hwnd: isize, proc_: SUBCLASSPROC, id: usize) -> i32;
    pub fn DefSubclassProc(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
}

#[link(name = "kernel32", kind = "raw-dylib")]
extern "system" {
    pub fn GetModuleHandleW(name: *const u16) -> isize;
}

// --- drawing -------------------------------------------------------------------

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

#[link(name = "gdi32", kind = "raw-dylib")]
extern "system" {
    pub fn CreateCompatibleDC(dc: isize) -> isize;
    pub fn CreateDIBSection(dc: isize, info: *const BITMAPINFOHEADER, usage: u32, bits: *mut *mut c_void, section: isize, offset: u32) -> isize;
    pub fn SelectObject(dc: isize, object: isize) -> isize;
    pub fn DeleteObject(object: isize) -> i32;
    pub fn DeleteDC(dc: isize) -> i32;
    pub fn GdiFlush() -> i32;
}

// --- shell ---------------------------------------------------------------------

pub const ABM_NEW: u32 = 0x00;
pub const ABM_REMOVE: u32 = 0x01;
pub const ABM_QUERYPOS: u32 = 0x02;
pub const ABM_SETPOS: u32 = 0x03;
pub const ABM_GETSTATE: u32 = 0x04;
pub const ABM_ACTIVATE: u32 = 0x06;
pub const ABM_WINDOWPOSCHANGED: u32 = 0x09;
pub const ABM_SETSTATE: u32 = 0x0a;
pub const ABE_LEFT: u32 = 0;
pub const ABE_RIGHT: u32 = 2;
pub const ABE_BOTTOM: u32 = 3;
pub const ABN_POSCHANGED: usize = 0x01;
pub const ABS_AUTOHIDE: isize = 0x01;

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

// --- registry --------------------------------------------------------------------

pub const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as i32 as isize;
pub const KEY_READ: u32 = 0x20019;
pub const RRF_RT_REG_DWORD: u32 = 0x0000_0010;
pub const RRF_RT_REG_SZ: u32 = 0x0000_0002;

#[link(name = "advapi32", kind = "raw-dylib")]
extern "system" {
    pub fn RegOpenKeyExW(key: isize, subkey: *const u16, options: u32, access: u32, out: *mut isize) -> i32;
    pub fn RegEnumKeyExW(key: isize, index: u32, name: *mut u16, len: *mut u32, reserved: *mut u32, class: *mut u16, class_len: *mut u32, time: *mut u64) -> i32;
    pub fn RegGetValueW(key: isize, subkey: *const u16, value: *const u16, flags: u32, ty: *mut u32, data: *mut c_void, len: *mut u32) -> i32;
    pub fn RegCloseKey(key: isize) -> i32;
}

// --- menus' theme ----------------------------------------------------------------

// Undocumented, but how every Win32 app gets dark menus: 135 sets the app's preference, 136 has
// the menus already themed draw again. Present since Windows 10 1903.
#[link(name = "uxtheme", kind = "raw-dylib")]
extern "system" {
    #[link_ordinal(135)]
    pub fn SetPreferredAppMode(mode: i32) -> i32;
    #[link_ordinal(136)]
    pub fn FlushMenuThemes();
}
pub const APPMODE_ALLOW_DARK: i32 = 1;
pub const APPMODE_FORCE_DARK: i32 = 2;
pub const APPMODE_FORCE_LIGHT: i32 = 3;

// --- helpers -------------------------------------------------------------------

/// A window's class name.
pub fn class_of(hwnd: isize) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) }.max(0) as usize;
    String::from_utf16_lossy(&buf[..n])
}

/// The top-level windows of a class, top of the z-order first.
pub fn windows_of_class(class: &str) -> Vec<isize> {
    unsafe extern "system" fn each(hwnd: isize, out: isize) -> i32 {
        let out = &mut *(out as *mut (String, Vec<isize>));
        if class_of(hwnd) == out.0 {
            out.1.push(hwnd);
        }
        1
    }
    let mut found = (class.to_string(), Vec::new());
    unsafe { EnumWindows(each, (&mut found as *mut (String, Vec<isize>)) as isize) };
    found.1
}

/// The process behind a window.
pub fn pid_of(hwnd: isize) -> u32 {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    pid
}

/// The main screen's rectangle and work area (the part windows maximize into).
pub fn primary() -> (RECT, RECT) {
    monitor_rects(unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, 1) }) // MONITOR_DEFAULTTOPRIMARY
}

/// The rectangle and work area of the screen at `pt`, or the nearest one.
pub fn screen_at(pt: POINT) -> (RECT, RECT) {
    monitor_rects(unsafe { MonitorFromPoint(pt, 2) }) // MONITOR_DEFAULTTONEAREST
}

fn monitor_rects(mon: isize) -> (RECT, RECT) {
    let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    unsafe { GetMonitorInfoW(mon, &mut mi) };
    (mi.rcMonitor, mi.rcWork)
}

/// A screen, as Windows names it ("\\.\DISPLAY2").
pub struct Screen {
    pub device: String,
    pub main: bool,
    pub rect: RECT,
    pub work: RECT,
}

/// Every screen, in Windows' order.
pub fn screens() -> Vec<Screen> {
    unsafe extern "system" fn each(mon: isize, _dc: isize, _rect: *mut RECT, out: isize) -> i32 {
        let out = &mut *(out as *mut Vec<Screen>);
        let mut mi = MONITORINFOEXW {
            info: MONITORINFO { cbSize: std::mem::size_of::<MONITORINFOEXW>() as u32, ..Default::default() },
            szDevice: [0; 32],
        };
        if GetMonitorInfoW(mon, (&mut mi as *mut MONITORINFOEXW).cast()) != 0 {
            let main = mi.info.dwFlags & MONITORINFOF_PRIMARY != 0;
            out.push(Screen { device: from_wide(&mi.szDevice), main, rect: mi.info.rcMonitor, work: mi.info.rcWork });
        }
        1
    }
    let mut all: Vec<Screen> = Vec::new();
    unsafe { EnumDisplayMonitors(0, std::ptr::null(), each, (&mut all as *mut Vec<Screen>) as isize) };
    all
}

/// The screen the settings put the dock on, by its device name; empty for the main screen.
static SCREEN: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

pub fn set_screen(device: &str) {
    *SCREEN.lock().unwrap() = device.to_string();
}

/// The dock's screen: the one the settings name, or the main screen when that's it or it isn't
/// plugged in. Its rectangle and work area.
pub fn screen() -> (RECT, RECT) {
    let want = SCREEN.lock().unwrap().clone();
    if !want.is_empty() {
        if let Some(s) = screens().into_iter().find(|s| s.device == want) {
            return (s.rect, s.work);
        }
    }
    primary()
}
