//! Makes each bar a Windows "app bar", like the taskbar: it claims a strip along one edge of its
//! screen, so maximized windows stop beside it instead of sliding underneath, and Windows tells it
//! when a game or video goes fullscreen so it can get out of the way.
//!
//! Everything that changes a bar runs on the main thread (setup, sync commands and the window
//! procedure), and the shell sends us messages while we're inside SHAppBarMessage. So the list of
//! bars is only ever locked for a moment, to copy a bar out or write one back, never while
//! calling into Windows.

use crate::ffi::*;
use std::sync::{Mutex, OnceLock};

/// The message the shell sends us its notifications with.
const CALLBACK: u32 = WM_APP + 1;
/// Ours, to place the bar again once the window has finished handling a DPI change.
const REPLACE: u32 = WM_APP + 2;
const SUBCLASS_ID: usize = 0x4c4e_544c; // "LNTL"

#[derive(Clone, Debug)]
struct Bar {
    hwnd: isize,
    /// The screen's device name (`\\.\DISPLAY2`); a screen that's gone means the main one.
    screen: String,
    edge: u32,
    thickness: u32,
    fullscreen: bool,
}

static BARS: Mutex<Vec<Bar>> = Mutex::new(Vec::new());
static ON_FULLSCREEN: OnceLock<Box<dyn Fn(bool) + Send + Sync>> = OnceLock::new();
static ON_BACKDROP: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();
static ON_DISPLAYS: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// A screen, as Windows lists them.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Screen {
    /// Its device name, which stays put while the screens are rearranged.
    pub id: String,
    pub primary: bool,
    pub rect: [i32; 4],
    #[serde(skip)]
    area: RECT,
}

/// The screens, the main one first, then from left to right.
pub fn screens() -> Vec<Screen> {
    unsafe extern "system" fn each(monitor: isize, _dc: isize, _rect: *mut RECT, out: isize) -> i32 {
        let out = &mut *(out as *mut Vec<Screen>);
        let mut mi = MONITORINFOEXW { info: MONITORINFO { cbSize: std::mem::size_of::<MONITORINFOEXW>() as u32, ..Default::default() }, szDevice: [0; 32] };
        if GetMonitorInfoW(monitor, (&mut mi as *mut MONITORINFOEXW).cast()) != 0 {
            let r = mi.info.rcMonitor;
            out.push(Screen {
                id: from_wide(&mi.szDevice),
                primary: mi.info.dwFlags & MONITORINFOF_PRIMARY != 0,
                rect: [r.left, r.top, r.right - r.left, r.bottom - r.top],
                area: r,
            });
        }
        1
    }
    let mut all: Vec<Screen> = Vec::new();
    unsafe { EnumDisplayMonitors(0, std::ptr::null(), each, (&mut all as *mut Vec<Screen>) as isize) };
    all.sort_by_key(|s| (!s.primary, s.rect[0], s.rect[1]));
    all
}

/// The shell's number for an edge setting.
pub fn edge(name: &str) -> u32 {
    match name {
        "bottom" => ABE_BOTTOM,
        "left" => ABE_LEFT,
        "right" => ABE_RIGHT,
        _ => ABE_TOP,
    }
}

/// What the app hears about: a fullscreen app came or went (`on_fullscreen`, after the bar has
/// hidden or shown itself), what's behind a bar may have changed (`on_backdrop`: it moved, or the
/// wallpaper did), and screens were added, removed or rearranged (`on_displays`).
pub fn on_events(
    on_fullscreen: impl Fn(bool) + Send + Sync + 'static,
    on_backdrop: impl Fn() + Send + Sync + 'static,
    on_displays: impl Fn() + Send + Sync + 'static,
) {
    let _ = ON_FULLSCREEN.set(Box::new(on_fullscreen));
    let _ = ON_BACKDROP.set(Box::new(on_backdrop));
    let _ = ON_DISPLAYS.set(Box::new(on_displays));
}

/// Claim `edge` of `screen` for `hwnd`, `thickness` pixels deep at 100% scaling.
pub fn register(hwnd: isize, screen: &str, edge: u32, thickness: u32) {
    BARS.lock().unwrap().push(Bar { hwnd, screen: screen.to_string(), edge, thickness, fullscreen: false });
    unsafe {
        SetWindowSubclass(hwnd, proc_, SUBCLASS_ID, 0);
        let mut abd = data(hwnd, edge);
        abd.uCallbackMessage = CALLBACK;
        SHAppBarMessage(ABM_NEW, &mut abd);
    }
    place(hwnd);
}

/// Give a bar's strip back. Windows also does this when the window goes away, but only eventually.
pub fn unregister(hwnd: isize) {
    let removed = {
        let mut bars = BARS.lock().unwrap();
        let before = bars.len();
        bars.retain(|b| b.hwnd != hwnd);
        bars.len() != before
    };
    if removed {
        unsafe {
            let mut abd = data(hwnd, ABE_TOP);
            SHAppBarMessage(ABM_REMOVE, &mut abd);
            RemoveWindowSubclass(hwnd, proc_, SUBCLASS_ID);
        }
    }
}

pub fn unregister_all() {
    let all: Vec<isize> = BARS.lock().unwrap().iter().map(|b| b.hwnd).collect();
    for hwnd in all {
        unregister(hwnd);
    }
}

/// Move every bar to another edge or change their depth.
pub fn set_layout(edge: u32, thickness: u32) {
    let changed: Vec<isize> = {
        let mut bars = BARS.lock().unwrap();
        bars.iter_mut()
            .filter(|b| (b.edge, b.thickness) != (edge, thickness))
            .map(|b| {
                b.edge = edge;
                b.thickness = thickness;
                b.hwnd
            })
            .collect()
    };
    for hwnd in changed {
        place(hwnd);
    }
}

/// Whether every bar is hidden for a fullscreen app (so nothing needs drawing).
pub fn fullscreen() -> bool {
    let bars = BARS.lock().unwrap();
    !bars.is_empty() && bars.iter().all(|b| b.fullscreen)
}

/// A window's rectangle on screen, in physical pixels.
pub fn rect(hwnd: isize) -> Option<RECT> {
    let mut rc = RECT::default();
    (hwnd != 0 && unsafe { GetWindowRect(hwnd, &mut rc) } != 0).then_some(rc)
}

fn bar(hwnd: isize) -> Option<Bar> {
    BARS.lock().unwrap().iter().find(|b| b.hwnd == hwnd).cloned()
}

fn data(hwnd: isize, edge: u32) -> APPBARDATA {
    APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: hwnd,
        uCallbackMessage: 0,
        uEdge: edge,
        rc: RECT::default(),
        lParam: 0,
    }
}

/// Ask the shell for the bar's strip and move the bar into it.
fn place(hwnd: isize) {
    let Some(b) = bar(hwnd) else { return };
    let screens = screens();
    let Some(screen) = screens.iter().find(|s| s.id == b.screen).or_else(|| screens.first()) else { return };
    unsafe {
        let dpi = GetDpiForWindow(hwnd).max(96);
        let depth = (b.thickness * dpi).div_ceil(96) as i32;
        let mut abd = data(hwnd, b.edge);
        abd.rc = strip(screen.area, b.edge, depth);
        // Other app bars on the same edge (the taskbar, say) may push us inwards; the shell adjusts rc.
        SHAppBarMessage(ABM_QUERYPOS, &mut abd);
        abd.rc = strip(abd.rc, b.edge, depth);
        SHAppBarMessage(ABM_SETPOS, &mut abd);
        let rc = abd.rc;
        SetWindowPos(hwnd, HWND_TOPMOST, rc.left, rc.top, rc.right - rc.left, rc.bottom - rc.top, SWP_NOACTIVATE);
    }
    backdrop_changed();
}

fn backdrop_changed() {
    if let Some(f) = ON_BACKDROP.get() {
        f();
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
            ABN_POSCHANGED => place(hwnd),
            ABN_FULLSCREENAPP => {
                let full = lparam != 0;
                let was = {
                    let mut bars = BARS.lock().unwrap();
                    bars.iter_mut().find(|b| b.hwnd == hwnd).map(|b| std::mem::replace(&mut b.fullscreen, full))
                };
                if was == Some(!full) {
                    if full {
                        ShowWindow(hwnd, SW_HIDE);
                    } else {
                        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                        place(hwnd);
                    }
                    if let Some(f) = ON_FULLSCREEN.get() {
                        f(full);
                    }
                }
            }
            _ => {}
        },
        REPLACE => place(hwnd),
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            let r = DefSubclassProc(hwnd, msg, wparam, lparam);
            PostMessageW(hwnd, REPLACE, 0, 0);
            if msg == WM_DISPLAYCHANGE {
                if let Some(f) = ON_DISPLAYS.get() {
                    f();
                }
            }
            return r;
        }
        WM_SETTINGCHANGE if wparam == SPI_SETDESKWALLPAPER => backdrop_changed(),
        // The shell asks app bars to pass these on, so it can keep their z-order right.
        WM_ACTIVATE => {
            let mut abd = data(hwnd, ABE_TOP);
            SHAppBarMessage(ABM_ACTIVATE, &mut abd);
        }
        WM_WINDOWPOSCHANGED => {
            let mut abd = data(hwnd, ABE_TOP);
            SHAppBarMessage(ABM_WINDOWPOSCHANGED, &mut abd);
        }
        WM_NCDESTROY => unregister(hwnd),
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

    #[test]
    fn lists_this_pcs_screens_main_first() {
        let all = screens();
        assert!(!all.is_empty() && all[0].primary);
        assert!(all.iter().all(|s| s.id.starts_with(r"\\.\") && s.rect[2] > 0));
    }
}
