//! Makes the bar a Windows "app bar", like the taskbar: it claims a strip along one edge of the
//! primary screen, so maximized windows stop beside it instead of sliding underneath, and Windows
//! tells it when a game or video goes fullscreen so it can get out of the way.
//!
//! Everything here runs on the main thread (setup, sync commands and the window procedure), and
//! the shell sends us messages while we're inside SHAppBarMessage, so state is kept in atomics
//! rather than behind a lock that could be taken twice.

use crate::ffi::*;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::OnceLock;

/// The message the shell sends us its notifications with.
const CALLBACK: u32 = WM_APP + 1;
/// Ours, to place the bar again once the window has finished handling a DPI change.
const REPLACE: u32 = WM_APP + 2;
const SUBCLASS_ID: usize = 0x4c4e_544c; // "LNTL"

static HWND: AtomicIsize = AtomicIsize::new(0);
static EDGE: AtomicU32 = AtomicU32::new(ABE_TOP);
static THICKNESS: AtomicU32 = AtomicU32::new(30);
static FULLSCREEN: AtomicBool = AtomicBool::new(false);
static ON_FULLSCREEN: OnceLock<Box<dyn Fn(bool) + Send + Sync>> = OnceLock::new();

/// The shell's number for an edge setting.
pub fn edge(name: &str) -> u32 {
    match name {
        "bottom" => ABE_BOTTOM,
        "left" => ABE_LEFT,
        "right" => ABE_RIGHT,
        _ => ABE_TOP,
    }
}

/// Claim `edge` of the primary screen for `hwnd`, `thickness` pixels deep at 100% scaling.
/// `on_fullscreen` is told when a fullscreen app comes and goes; the bar has hidden itself by then.
pub fn register(hwnd: isize, edge: u32, thickness: u32, on_fullscreen: impl Fn(bool) + Send + Sync + 'static) {
    let _ = ON_FULLSCREEN.set(Box::new(on_fullscreen));
    EDGE.store(edge, Ordering::Relaxed);
    THICKNESS.store(thickness, Ordering::Relaxed);
    HWND.store(hwnd, Ordering::Relaxed);
    unsafe {
        SetWindowSubclass(hwnd, proc_, SUBCLASS_ID, 0);
        let mut abd = data(hwnd);
        abd.uCallbackMessage = CALLBACK;
        SHAppBarMessage(ABM_NEW, &mut abd);
    }
    place();
}

/// Give the strip back. Windows also does this when the window goes away, but only eventually.
pub fn unregister() {
    let hwnd = HWND.swap(0, Ordering::Relaxed);
    if hwnd != 0 {
        unsafe {
            let mut abd = data(hwnd);
            SHAppBarMessage(ABM_REMOVE, &mut abd);
            RemoveWindowSubclass(hwnd, proc_, SUBCLASS_ID);
        }
    }
}

/// Move to another edge or change the depth.
pub fn set_layout(edge: u32, thickness: u32) {
    let moved = EDGE.swap(edge, Ordering::Relaxed) != edge;
    if THICKNESS.swap(thickness, Ordering::Relaxed) != thickness || moved {
        place();
    }
}

/// Whether a fullscreen app has the bar hidden.
pub fn fullscreen() -> bool {
    FULLSCREEN.load(Ordering::Relaxed)
}

fn data(hwnd: isize) -> APPBARDATA {
    APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: hwnd,
        uCallbackMessage: 0,
        uEdge: EDGE.load(Ordering::Relaxed),
        rc: RECT::default(),
        lParam: 0,
    }
}

/// Ask the shell for the strip and move the bar into it.
fn place() {
    let hwnd = HWND.load(Ordering::Relaxed);
    if hwnd == 0 {
        return;
    }
    unsafe {
        // The primary screen is the one with (0, 0) in it.
        let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(monitor, &mut mi) == 0 {
            return;
        }
        let dpi = GetDpiForWindow(hwnd).max(96);
        let depth = (THICKNESS.load(Ordering::Relaxed) * dpi).div_ceil(96) as i32;
        let mut abd = data(hwnd);
        abd.rc = strip(mi.rcMonitor, abd.uEdge, depth);
        // Other app bars on the same edge (the taskbar, say) may push us inwards; the shell adjusts rc.
        SHAppBarMessage(ABM_QUERYPOS, &mut abd);
        abd.rc = strip(abd.rc, abd.uEdge, depth);
        SHAppBarMessage(ABM_SETPOS, &mut abd);
        let rc = abd.rc;
        SetWindowPos(hwnd, HWND_TOPMOST, rc.left, rc.top, rc.right - rc.left, rc.bottom - rc.top, SWP_NOACTIVATE);
    }
}

/// The `depth` pixels of `area` along `edge`.
fn strip(area: RECT, edge: u32, depth: i32) -> RECT {
    match edge {
        ABE_BOTTOM => RECT { top: area.bottom - depth, ..area },
        ABE_LEFT => RECT { right: area.left + depth, ..area },
        ABE_RIGHT => RECT { left: area.right - depth, ..area },
        _ => RECT { bottom: area.top + depth, ..area },
    }
}

unsafe extern "system" fn proc_(hwnd: isize, msg: u32, wparam: usize, lparam: isize, _id: usize, _data: usize) -> isize {
    match msg {
        CALLBACK => match wparam {
            ABN_POSCHANGED => place(),
            ABN_FULLSCREENAPP => {
                let full = lparam != 0;
                if FULLSCREEN.swap(full, Ordering::Relaxed) != full {
                    if full {
                        ShowWindow(hwnd, SW_HIDE);
                    } else {
                        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                        place();
                    }
                    if let Some(f) = ON_FULLSCREEN.get() {
                        f(full);
                    }
                }
            }
            _ => {}
        },
        REPLACE => place(),
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            let r = DefSubclassProc(hwnd, msg, wparam, lparam);
            PostMessageW(hwnd, REPLACE, 0, 0);
            return r;
        }
        // The shell asks app bars to pass these on, so it can keep their z-order right.
        WM_ACTIVATE => {
            let mut abd = data(hwnd);
            SHAppBarMessage(ABM_ACTIVATE, &mut abd);
        }
        WM_WINDOWPOSCHANGED => {
            let mut abd = data(hwnd);
            SHAppBarMessage(ABM_WINDOWPOSCHANGED, &mut abd);
        }
        WM_NCDESTROY => unregister(),
        _ => {}
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_strip_is_along_the_edge() {
        let screen = RECT { left: -1920, top: 0, right: 0, bottom: 1080 };
        assert_eq!(strip(screen, ABE_TOP, 30), RECT { left: -1920, top: 0, right: 0, bottom: 30 });
        assert_eq!(strip(screen, ABE_BOTTOM, 30), RECT { left: -1920, top: 1050, right: 0, bottom: 1080 });
        assert_eq!(strip(screen, ABE_LEFT, 46), RECT { left: -1920, top: 0, right: -1874, bottom: 1080 });
        assert_eq!(strip(screen, ABE_RIGHT, 46), RECT { left: -46, top: 0, right: 0, bottom: 1080 });
        // Pushed inwards by the taskbar on the same edge.
        assert_eq!(strip(RECT { top: 48, ..screen }, ABE_TOP, 30), RECT { left: -1920, top: 48, right: 0, bottom: 78 });
        assert_eq!(strip(RECT { bottom: 1032, ..screen }, ABE_BOTTOM, 30), RECT { left: -1920, top: 1002, right: 0, bottom: 1032 });
    }

    #[test]
    fn edges_by_name() {
        assert_eq!([edge("top"), edge("bottom"), edge("left"), edge("right"), edge("??")], [ABE_TOP, ABE_BOTTOM, ABE_LEFT, ABE_RIGHT, ABE_TOP]);
    }
}
