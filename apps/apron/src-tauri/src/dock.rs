//! Where the dock is, and when it shows. In the taskbar's place it's an app bar on the main
//! screen, as the taskbar is: it claims its strip along the edge, so maximized windows stop at it.
//! Floating, it claims nothing: it sits above the taskbar, hidden until the pointer reaches the
//! edge, and goes again when the pointer leaves it. Either way it gets out of the way of a
//! full-screen video or game.
//!
//! The app bar is set up and moved on the main thread, which owns the dock's window; the beat
//! (from the host's thread) and the pointer's watch only show or hide it, without waiting.

use crate::ffi::*;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// The message the shell sends the app bar its notifications with.
const CALLBACK: u32 = WM_APP + 1;
/// Ours, to place the dock again once the window has finished with a display or DPI change.
const REPLACE: u32 = WM_APP + 2;
const SETTLE: usize = 0x5349;
const SUBCLASS_ID: usize = 0x5349_4c4c; // "APRON"
/// Floating: how far above the taskbar (or in from a side) the shelf sits, at 100%.
const GAP: i32 = 6;
/// Floating: how long the pointer rests at the edge before the dock comes, and how long it's
/// away from the dock before it goes.
const REVEAL_AFTER: Duration = Duration::from_millis(90);
const CONCEAL_AFTER: Duration = Duration::from_millis(450);
/// The ends of the edge are Mitre's corners, which shouldn't bring the dock too.
const CORNER: i32 = 12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub edge: u32,
    /// How far it reaches into the screen at 100%.
    pub thickness: u32,
    pub float: bool,
}

static DOCK: AtomicIsize = AtomicIsize::new(0);
static LAYOUT: Mutex<Layout> = Mutex::new(Layout { edge: ABE_BOTTOM, thickness: 66, float: false });
static REGISTERED: AtomicBool = AtomicBool::new(false);
/// The shelf's size the page last gave, in physical pixels: floating, the window is just that.
static SHELF: Mutex<(i32, i32)> = Mutex::new((0, 0));
static FULL_SCREEN: AtomicBool = AtomicBool::new(false);
static REVEALED: AtomicBool = AtomicBool::new(false);
/// A menu of the dock's is open: it stays while it is.
pub static MENU_OPEN: AtomicBool = AtomicBool::new(false);
static ON_SLIDE: OnceLock<Box<dyn Fn(bool) + Send + Sync>> = OnceLock::new();

/// What to call when the floating dock comes (true) or goes (false), for the page to slide it.
pub fn on_slide(f: impl Fn(bool) + Send + Sync + 'static) {
    let _ = ON_SLIDE.set(Box::new(f));
}

pub fn edge(name: &str) -> u32 {
    match name {
        "left" => ABE_LEFT,
        "right" => ABE_RIGHT,
        _ => ABE_BOTTOM,
    }
}

pub fn hwnd() -> isize {
    DOCK.load(Ordering::Relaxed)
}

pub fn layout() -> Layout {
    *LAYOUT.lock().unwrap()
}

/// The dock's window, on the main thread, once.
pub fn init(hwnd: isize) {
    DOCK.store(hwnd, Ordering::Relaxed);
    unsafe { SetWindowSubclass(hwnd, proc_, SUBCLASS_ID, 0) };
}

/// Put the dock where `layout` says. Main thread.
pub fn apply(layout: Layout) {
    let was = std::mem::replace(&mut *LAYOUT.lock().unwrap(), layout);
    let hwnd = hwnd();
    if hwnd == 0 {
        return;
    }
    if layout.float {
        unregister();
        REVEALED.store(false, Ordering::Relaxed);
        place_floating();
        update();
    } else {
        if was.edge != layout.edge {
            unregister();
        }
        register();
        place();
        REVEALED.store(true, Ordering::Relaxed);
        update();
    }
}

/// Explorer started again and forgot every app bar: claim the strip again. Main thread.
pub fn register_again() {
    if !layout().float {
        unregister();
        register();
        place();
    }
}

fn data(hwnd: isize, edge: u32) -> APPBARDATA {
    APPBARDATA { cbSize: std::mem::size_of::<APPBARDATA>() as u32, hWnd: hwnd, uCallbackMessage: 0, uEdge: edge, rc: RECT::default(), lParam: 0 }
}

fn register() {
    if REGISTERED.swap(true, Ordering::Relaxed) {
        return;
    }
    let mut abd = data(hwnd(), layout().edge);
    abd.uCallbackMessage = CALLBACK;
    unsafe { SHAppBarMessage(ABM_NEW, &mut abd) };
}

/// Give the strip back. Main thread; also on quit.
pub fn unregister() {
    if REGISTERED.swap(false, Ordering::Relaxed) {
        unsafe { SHAppBarMessage(ABM_REMOVE, &mut data(hwnd(), layout().edge)) };
    }
}

fn depth(hwnd: isize, at_100: u32) -> i32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    (at_100 * dpi).div_ceil(96) as i32
}

/// The `depth` pixels of `area` along `edge`.
pub fn strip(area: RECT, edge: u32, depth: i32) -> RECT {
    match edge {
        ABE_LEFT => RECT { right: area.left + depth, ..area },
        ABE_RIGHT => RECT { left: area.right - depth, ..area },
        _ => RECT { top: area.bottom - depth, ..area },
    }
}

/// In the taskbar's place: ask the shell for the strip and move the dock into it.
fn place() {
    let hwnd = hwnd();
    let l = layout();
    if l.float || hwnd == 0 {
        return;
    }
    let (screen, _) = screen();
    let depth = depth(hwnd, l.thickness);
    let mut abd = data(hwnd, l.edge);
    abd.rc = strip(screen, l.edge, depth);
    unsafe {
        // Lintel on the same edge, say, may push us inwards; the shell adjusts rc.
        SHAppBarMessage(ABM_QUERYPOS, &mut abd);
        abd.rc = strip(abd.rc, l.edge, depth);
        SHAppBarMessage(ABM_SETPOS, &mut abd);
        let rc = abd.rc;
        SetWindowPos(hwnd, HWND_TOPMOST, rc.left, rc.top, rc.right - rc.left, rc.bottom - rc.top, SWP_NOACTIVATE);
    }
}

/// Floating: just the shelf, centred along the edge of the main screen's work area, which the
/// taskbar keeps out of.
fn place_floating() {
    let hwnd = hwnd();
    let l = layout();
    if !l.float || hwnd == 0 {
        return;
    }
    let (_, work) = screen();
    let (w, h) = *SHELF.lock().unwrap();
    let gap = depth(hwnd, GAP as u32);
    let (w, h) = (w.max(1), h.max(1));
    let (x, y) = floating_spot(work, l.edge, w, h, gap);
    unsafe { SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE | 0x4000) }; // SWP_ASYNCWINDOWPOS
}

/// Where a floating shelf of `w` by `h` goes: centred along `edge` of `work`, `gap` in from it.
fn floating_spot(work: RECT, edge: u32, w: i32, h: i32, gap: i32) -> (i32, i32) {
    let cx = (work.left + work.right - w) / 2;
    let cy = (work.top + work.bottom - h) / 2;
    match edge {
        ABE_LEFT => (work.left + gap, cy),
        ABE_RIGHT => (work.right - gap - w, cy),
        _ => (cx, work.bottom - gap - h),
    }
}

/// The page's shelf is this size now (physical pixels).
pub fn shelf(w: i32, h: i32) {
    let changed = std::mem::replace(&mut *SHELF.lock().unwrap(), (w, h)) != (w, h);
    if changed && layout().float {
        place_floating();
    }
}

/// The dock's rectangle on screen.
pub fn rect() -> Option<RECT> {
    let mut rc = RECT::default();
    (hwnd() != 0 && unsafe { GetWindowRect(hwnd(), &mut rc) } != 0).then_some(rc)
}

/// Show or hide the dock as things stand: not over a full-screen app, and, floating, only once
/// the pointer has called it. Never waits.
fn update() {
    let hwnd = hwnd();
    if hwnd == 0 {
        return;
    }
    let l = layout();
    let want = !FULL_SCREEN.load(Ordering::Relaxed) && (!l.float || REVEALED.load(Ordering::Relaxed));
    let shown = unsafe { IsWindowVisible(hwnd) } != 0;
    if want && !shown {
        unsafe {
            ShowWindowAsync(hwnd, SW_SHOWNOACTIVATE);
            SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | 0x4000);
        }
    } else if !want && shown {
        unsafe { ShowWindowAsync(hwnd, SW_HIDE) };
    }
}

/// The host's beat: a full-screen app or game on the main screen hides the dock until it's gone.
pub fn beat() {
    let full = full_screen();
    if FULL_SCREEN.swap(full, Ordering::Relaxed) != full {
        update();
    }
}

fn full_screen() -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg == 0 || fg == hwnd() {
            return false;
        }
        if image_path(pid_of(fg)).is_some_and(|p| ours(&p)) {
            return false;
        }
        let (screen, _) = screen();
        if MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST) != MonitorFromWindow(hwnd(), MONITOR_DEFAULTTONEAREST) {
            return false;
        }
        let style = GetWindowLongW(fg, GWL_STYLE) as u32;
        let mut wr = RECT::default();
        GetWindowRect(fg, &mut wr);
        // A window with a title bar isn't full screen even if it covers the screen: with the
        // taskbar hidden, a maximized one does.
        style & WS_CAPTION != WS_CAPTION && wr.left <= screen.left && wr.top <= screen.top && wr.right >= screen.right && wr.bottom >= screen.bottom
    }
}

/// The windows of these programs are never a full-screen app: Explorer's (the desktop, Start)
/// and the suite's own (Bay's overview), as Mitre has it.
fn ours(exe: &str) -> bool {
    let name = exe.rsplit(['\\', '/']).next().unwrap_or(exe).to_lowercase();
    let name = name.trim_end_matches(".exe");
    name == "explorer" || name == "mullion" || mullion_core::suite::APPS.iter().any(|a| a.exe == name)
}

/// Floating: watch the pointer, ten times a second, from a thread of its own; only while
/// floating, and it does nothing else.
pub fn watch_pointer() {
    let _ = std::thread::Builder::new().name("pointer".into()).spawn(|| {
        let mut at_edge: Option<Instant> = None;
        let mut away: Option<Instant> = None;
        loop {
            std::thread::sleep(Duration::from_millis(100));
            let l = layout();
            if !l.float || hwnd() == 0 {
                at_edge = None;
                away = None;
                continue;
            }
            let mut pt = POINT::default();
            if unsafe { GetCursorPos(&mut pt) } == 0 {
                continue;
            }
            let (screen, _) = screen();
            let revealed = REVEALED.load(Ordering::Relaxed);
            if !revealed {
                if FULL_SCREEN.load(Ordering::Relaxed) || !touches_edge(screen, l.edge, pt) {
                    at_edge = None;
                    continue;
                }
                let since = *at_edge.get_or_insert_with(Instant::now);
                if since.elapsed() >= REVEAL_AFTER {
                    at_edge = None;
                    slide(true);
                }
            } else {
                let near = rect().is_some_and(|r| near_dock(r, screen, l.edge, pt));
                if near || MENU_OPEN.load(Ordering::Relaxed) {
                    away = None;
                    continue;
                }
                let since = *away.get_or_insert_with(Instant::now);
                if since.elapsed() >= CONCEAL_AFTER {
                    away = None;
                    slide(false);
                }
            }
        }
    });
}

/// The pointer is pushed against the edge, away from its corners.
fn touches_edge(screen: RECT, edge: u32, pt: POINT) -> bool {
    let along_x = pt.x >= screen.left + CORNER && pt.x < screen.right - CORNER;
    let along_y = pt.y >= screen.top + CORNER && pt.y < screen.bottom - CORNER;
    match edge {
        ABE_LEFT => pt.x <= screen.left && along_y,
        ABE_RIGHT => pt.x >= screen.right - 1 && along_y,
        _ => pt.y >= screen.bottom - 1 && along_x,
    }
}

/// The pointer is on the dock, or between it and the edge (over the taskbar, on its way up).
fn near_dock(dock: RECT, screen: RECT, edge: u32, pt: POINT) -> bool {
    let m = 14;
    let r = match edge {
        ABE_LEFT => RECT { left: screen.left, top: dock.top - m, right: dock.right + m, bottom: dock.bottom + m },
        ABE_RIGHT => RECT { left: dock.left - m, top: dock.top - m, right: screen.right, bottom: dock.bottom + m },
        _ => RECT { left: dock.left - m, top: dock.top - m, right: dock.right + m, bottom: screen.bottom },
    };
    pt.x >= r.left && pt.x < r.right && pt.y >= r.top && pt.y < r.bottom
}

/// Floating: bring the dock (the page slides it in), or send it away (the page slides it out,
/// then the window hides).
fn slide(show: bool) {
    REVEALED.store(show, Ordering::Relaxed);
    if let Some(f) = ON_SLIDE.get() {
        f(show);
    }
    if show {
        place_floating();
        update();
    } else {
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(180));
            if !REVEALED.load(Ordering::Relaxed) {
                update();
            }
        });
    }
}

unsafe extern "system" fn settled(hwnd: isize, _msg: u32, id: usize, _time: u32) {
    KillTimer(hwnd, id);
    place();
}

unsafe extern "system" fn proc_(hwnd: isize, msg: u32, wparam: usize, lparam: isize, _id: usize, _data: usize) -> isize {
    match msg {
        CALLBACK if wparam == ABN_POSCHANGED => {
            place();
            // While the taskbar hides or shows, the shell can answer with where things were.
            SetTimer(hwnd, SETTLE, 400, settled);
        }
        REPLACE => {
            place();
            place_floating();
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            let r = DefSubclassProc(hwnd, msg, wparam, lparam);
            PostMessageW(hwnd, REPLACE, 0, 0);
            return r;
        }
        // The shell asks app bars to pass these on, so it can keep their z-order right.
        WM_ACTIVATE if REGISTERED.load(Ordering::Relaxed) => {
            SHAppBarMessage(ABM_ACTIVATE, &mut data(hwnd, layout().edge));
        }
        WM_WINDOWPOSCHANGED if REGISTERED.load(Ordering::Relaxed) => {
            SHAppBarMessage(ABM_WINDOWPOSCHANGED, &mut data(hwnd, layout().edge));
        }
        WM_NCDESTROY => {
            unregister();
            RemoveWindowSubclass(hwnd, proc_, SUBCLASS_ID);
        }
        _ => {}
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: RECT = RECT { left: 0, top: 0, right: 2560, bottom: 1080 };

    #[test]
    fn the_strip_is_along_the_edge() {
        assert_eq!(strip(SCREEN, ABE_BOTTOM, 66), RECT { top: 1014, ..SCREEN });
        assert_eq!(strip(SCREEN, ABE_LEFT, 66), RECT { right: 66, ..SCREEN });
        assert_eq!(strip(SCREEN, ABE_RIGHT, 66), RECT { left: 2494, ..SCREEN });
        assert_eq!([edge("bottom"), edge("left"), edge("right"), edge("top")], [ABE_BOTTOM, ABE_LEFT, ABE_RIGHT, ABE_BOTTOM]);
    }

    #[test]
    fn floating_sits_above_the_taskbar_centred() {
        let work = RECT { bottom: 1032, ..SCREEN };
        assert_eq!(floating_spot(work, ABE_BOTTOM, 600, 60, 6), (980, 966));
        assert_eq!(floating_spot(work, ABE_LEFT, 60, 400, 6), (6, 316));
        assert_eq!(floating_spot(work, ABE_RIGHT, 60, 400, 6), (2494, 316));
    }

    #[test]
    fn the_pointer_calls_it_from_the_edge_not_the_corners() {
        let p = |x, y| POINT { x, y };
        assert!(touches_edge(SCREEN, ABE_BOTTOM, p(1200, 1079)));
        assert!(!touches_edge(SCREEN, ABE_BOTTOM, p(1200, 1070)));
        assert!(!touches_edge(SCREEN, ABE_BOTTOM, p(3, 1079)));
        assert!(!touches_edge(SCREEN, ABE_BOTTOM, p(2555, 1079)));
        assert!(touches_edge(SCREEN, ABE_LEFT, p(0, 500)));
        assert!(touches_edge(SCREEN, ABE_RIGHT, p(2559, 500)));
        let dock = RECT { left: 980, top: 966, right: 1580, bottom: 1026 };
        assert!(near_dock(dock, SCREEN, ABE_BOTTOM, p(1000, 1060)));
        assert!(near_dock(dock, SCREEN, ABE_BOTTOM, p(1000, 960)));
        assert!(!near_dock(dock, SCREEN, ABE_BOTTOM, p(1000, 900)));
        assert!(!near_dock(dock, SCREEN, ABE_BOTTOM, p(500, 1060)));
    }

    #[test]
    fn explorer_and_the_suite_are_never_full_screen() {
        assert!(ours(r"C:\Windows\explorer.exe"));
        assert!(ours(r"C:\Users\a\AppData\Local\Mullion\bay.exe"));
        assert!(!ours(r"C:\Program Files\Game\game.exe"));
    }
}
