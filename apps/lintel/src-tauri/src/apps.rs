//! The app in front, for the bar's app name and the app menu, and acting on that app's windows.
//!
//! Windows says when the foreground window changes (a WinEvent hook), so nothing is polled: the
//! hook runs on a thread of its own that just waits for messages. The bar and its menus are left
//! out, so opening a menu doesn't change the name next to the logo.

use crate::ffi::*;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Mutex;

/// The app in front, as the bar shows it.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Front {
    /// The window that's in front, which a menu hands focus back to when it closes.
    pub hwnd: u64,
    pub pid: u32,
    /// The program's file name without ".exe".
    pub exe: String,
    /// What to call it: its own name from its file ("Google Chrome"), else `exe`.
    pub name: String,
    /// The desktop has the focus (clicked, or Win+D).
    pub desktop: bool,
}

/// One of the front app's windows, for the app menu.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Win {
    hwnd: u64,
    pid: u32,
    title: String,
    minimized: bool,
    front: bool,
}

static LATEST: Mutex<Option<Front>> = Mutex::new(None);

thread_local! {
    static WATCH: RefCell<Option<Watch>> = const { RefCell::new(None) };
}

struct Watch {
    on_change: Box<dyn Fn(&Front)>,
    names: HashMap<String, String>,
}

/// Start following the foreground window. `on_change` runs on the watcher's thread.
pub fn start(on_change: impl Fn(&Front) + Send + 'static) {
    let _ = std::thread::Builder::new().name("front".into()).spawn(move || unsafe {
        WATCH.with(|w| *w.borrow_mut() = Some(Watch { on_change: Box::new(on_change), names: HashMap::new() }));
        // What's in front already, before anything changes.
        changed(GetForegroundWindow());
        let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
        SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, 0, hook, 0, 0, flags);
        // Restoring a minimized window doesn't always count as a foreground change.
        SetWinEventHook(EVENT_SYSTEM_MINIMIZEEND, EVENT_SYSTEM_MINIMIZEEND, 0, hook, 0, 0, flags);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            DispatchMessageW(&msg);
        }
    });
}

pub fn latest() -> Option<Front> {
    LATEST.lock().unwrap().clone()
}

unsafe extern "system" fn hook(_hook: isize, _event: u32, hwnd: isize, object: i32, _child: i32, _thread: u32, _time: u32) {
    // OBJID_WINDOW: the window itself, not a part of it.
    if object == 0 {
        changed(hwnd);
    }
}

fn changed(hwnd: isize) {
    let Some(raw) = read(root(hwnd)) else { return };
    WATCH.with(|w| {
        let mut w = w.borrow_mut();
        let Some(w) = w.as_mut() else { return };
        let Some(front) = front(&raw, std::process::id(), |pid| name_of(&mut w.names, pid)) else { return };
        let mut latest = LATEST.lock().unwrap();
        if latest.as_ref() != Some(&front) {
            *latest = Some(front.clone());
            drop(latest);
            (w.on_change)(&front);
        }
    });
}

/// The program's file name and the name to show, looked up once per program.
fn name_of(cache: &mut HashMap<String, String>, pid: u32) -> (String, String) {
    let Some(path) = image_path(pid) else { return (String::new(), String::new()) };
    let exe = std::path::Path::new(&path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name = cache.entry(path.clone()).or_insert_with(|| display_name(&exe, file_description(&path).as_deref())).clone();
    (exe, name)
}

/// The name macOS would show for a program: its own, with a few that Windows names oddly fixed.
fn display_name(exe: &str, description: Option<&str>) -> String {
    match exe.to_lowercase().as_str() {
        "explorer" => return "File Explorer".into(),
        "applicationframehost" => return "Windows app".into(),
        "windowsterminal" => return "Terminal".into(),
        "systemsettings" => return "Settings".into(),
        _ => {}
    }
    match description {
        // Runtimes that name themselves rather than the app built on them.
        Some(d) if !d.is_empty() && !["electron", "java(tm) platform se binary", "python", "node.js"].contains(&d.to_lowercase().as_str()) => {
            // Some put the vendor's tagline after a dash or a version number after the name.
            d.split(" - ").next().unwrap_or(d).trim().to_string()
        }
        _ => exe.to_string(),
    }
}

/// Shell windows that take the focus without being an app the bar should name: the taskbar,
/// Start, Search, the notification and quick settings panels, Alt+Tab.
const SHELL_CLASSES: &[&str] = &[
    "Shell_TrayWnd", "Shell_SecondaryTrayWnd", "NotifyIconOverflowWindow", "TopLevelWindowForOverflowXamlIsland",
    "XamlExplorerHostIslandWindow", "ForegroundStaging", "MultitaskingViewFrame", "TaskSwitcherWnd", "#32768",
];
const SHELL_EXES: &[&str] = &["startmenuexperiencehost", "searchhost", "searchapp", "shellexperiencehost", "shellhost", "lockapp"];
const DESKTOP_CLASSES: &[&str] = &["Progman", "WorkerW"];

/// What the bar shows for a window that came to the front, or None to keep what it shows.
fn front(w: &Raw, own_pid: u32, mut names: impl FnMut(u32) -> (String, String)) -> Option<Front> {
    if w.pid == own_pid || w.pid == 0 || SHELL_CLASSES.contains(&w.class.as_str()) {
        return None;
    }
    if DESKTOP_CLASSES.contains(&w.class.as_str()) {
        return Some(Front { hwnd: w.hwnd as u64, pid: w.pid, exe: "explorer".into(), name: "Desktop".into(), desktop: true });
    }
    let (exe, name) = names(w.pid);
    if SHELL_EXES.contains(&exe.to_lowercase().as_str()) {
        return None;
    }
    Some(Front { hwnd: w.hwnd as u64, pid: w.pid, name: if name.is_empty() { "Window".into() } else { name }, exe, desktop: false })
}

// --- windows -------------------------------------------------------------------------

const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const WS_EX_APPWINDOW: u32 = 0x0004_0000;
const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
const GA_ROOT: u32 = 2;

/// What Windows says about one top-level window.
#[derive(Clone, Debug, Default)]
struct Raw {
    hwnd: isize,
    /// For a Store app's frame, the app's process rather than the frame host's.
    pid: u32,
    owned: bool,
    ex_style: u32,
    cloaked: bool,
    minimized: bool,
    class: String,
    title: String,
}

fn root(hwnd: isize) -> isize {
    match unsafe { GetAncestor(hwnd, GA_ROOT) } {
        0 => hwnd,
        r => r,
    }
}

/// Ask Windows about one window. None if it's gone.
fn read(hwnd: isize) -> Option<Raw> {
    unsafe {
        if hwnd == 0 || IsWindow(hwnd) == 0 {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let mut cloaked = 0u32;
        DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, (&mut cloaked as *mut u32).cast(), 4);
        let (mut title, mut class) = ([0u16; 512], [0u16; 256]);
        let t = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32).max(0) as usize;
        let c = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32).max(0) as usize;
        let class = String::from_utf16_lossy(&class[..c]);
        // Store apps live in a frame window that belongs to ApplicationFrameHost; the app is the
        // process behind the frame's content.
        if class == "ApplicationFrameWindow" {
            pid = store_app_pid(hwnd, pid).unwrap_or(pid);
        }
        Some(Raw {
            hwnd,
            pid,
            owned: GetWindow(hwnd, GW_OWNER) != 0,
            ex_style: GetWindowLongW(hwnd, GWL_EXSTYLE) as u32,
            cloaked: cloaked != 0,
            minimized: IsIconic(hwnd) != 0,
            class,
            title: String::from_utf16_lossy(&title[..t]),
        })
    }
}

fn store_app_pid(frame: isize, frame_pid: u32) -> Option<u32> {
    unsafe extern "system" fn child(hwnd: isize, out: isize) -> i32 {
        let out = &mut *(out as *mut (u32, Option<u32>));
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid != out.0 {
            out.1 = Some(pid);
            return 0;
        }
        1
    }
    let mut found = (frame_pid, None);
    unsafe { EnumChildWindows(frame, child, (&mut found as *mut (u32, Option<u32>)) as isize) };
    found.1
}

/// The same test Alt+Tab applies, as far as it's documented (from Oriel's Open windows widget).
fn listed(w: &Raw, own_pid: u32) -> bool {
    let app = w.ex_style & WS_EX_APPWINDOW != 0;
    !w.title.is_empty()
        && w.pid != own_pid
        && !w.cloaked
        && (app || (w.ex_style & (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) == 0 && !w.owned))
        && !SHELL_CLASSES.contains(&w.class.as_str())
        && !DESKTOP_CLASSES.contains(&w.class.as_str())
}

fn enumerate() -> Vec<Raw> {
    unsafe extern "system" fn collect(hwnd: isize, out: isize) -> i32 {
        if IsWindowVisible(hwnd) != 0 {
            if let Some(w) = read(hwnd) {
                (*(out as *mut Vec<Raw>)).push(w);
            }
        }
        1
    }
    let mut all: Vec<Raw> = Vec::new();
    unsafe { EnumWindows(collect, (&mut all as *mut Vec<Raw>) as isize) };
    all
}

/// The windows of the program `pid` runs, top-most first. Matched by program file, so every
/// window of an app counts even when it runs several processes.
pub fn windows_of(pid: u32) -> Vec<Win> {
    let Some(path) = image_path(pid) else { return Vec::new() };
    let own = std::process::id();
    let front = latest().map(|f| f.hwnd as isize).unwrap_or(0);
    let mut paths: HashMap<u32, Option<String>> = HashMap::new();
    enumerate()
        .into_iter()
        .filter(|w| listed(w, own))
        .filter(|w| paths.entry(w.pid).or_insert_with(|| image_path(w.pid)).as_deref() == Some(path.as_str()))
        .map(|w| Win { hwnd: w.hwnd as u64, pid: w.pid, title: w.title, minimized: w.minimized, front: w.hwnd == front })
        .collect()
}

/// The window the menu showed, if it's still there and still the same program's. Windows reuses
/// handles, so a handle alone isn't enough.
fn target(hwnd: u64, pid: u32) -> Result<isize, String> {
    let hwnd = isize::try_from(hwnd).map_err(|_| "not a window".to_string())?;
    match read(hwnd) {
        Some(w) if w.pid == pid && listed(&w, std::process::id()) => Ok(hwnd),
        _ => Err("that window has gone".into()),
    }
}

/// Bring a window to the front, restoring it first if it's minimized.
pub fn focus(hwnd: u64, pid: u32) -> Result<(), String> {
    let hwnd = target(hwnd, pid)?;
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        if SetForegroundWindow(hwnd) == 0 {
            return Err("Windows wouldn't bring that window to the front".into());
        }
    }
    Ok(())
}

pub fn minimize(hwnd: u64, pid: u32) -> Result<(), String> {
    let hwnd = target(hwnd, pid)?;
    unsafe { ShowWindow(hwnd, SW_MINIMIZE) };
    Ok(())
}

/// Ask a window to close, as its × button does: the program can still offer to save.
pub fn close(hwnd: u64, pid: u32) -> Result<(), String> {
    let hwnd = target(hwnd, pid)?;
    if unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) } == 0 {
        return Err("Windows wouldn't deliver the request (an elevated program?)".into());
    }
    Ok(())
}

/// "Quit": ask every window of the app to close. Never ends a process, so nothing unsaved is lost
/// without the app asking; apps that live on in the tray stay there, as they do on Windows.
pub fn quit(pid: u32) -> Result<usize, String> {
    let wins = windows_of(pid);
    if wins.is_empty() {
        return Err("it has no windows open".into());
    }
    let mut n = 0;
    for w in &wins {
        if close(w.hwnd, w.pid).is_ok() {
            n += 1;
        }
    }
    Ok(n)
}

/// Whether the window in front is one of ours (the bar took the focus when it was clicked).
pub fn foreground_is_ours() -> bool {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(GetForegroundWindow(), &mut pid) };
    pid == std::process::id()
}

/// Give the focus back to the window that had it before a menu opened.
pub fn restore_focus() {
    if let Some(f) = latest() {
        unsafe {
            let hwnd = f.hwnd as isize;
            if IsWindow(hwnd) != 0 && IsIconic(hwnd) == 0 {
                SetForegroundWindow(hwnd);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: u32 = 100;

    fn win(hwnd: isize, pid: u32, class: &str, title: &str) -> Raw {
        Raw { hwnd, pid, class: class.into(), title: title.into(), ..Default::default() }
    }

    fn names(pid: u32) -> (String, String) {
        match pid {
            7 => ("chrome".into(), "Google Chrome".into()),
            8 => ("SearchHost".into(), "Search".into()),
            _ => (String::new(), String::new()),
        }
    }

    #[test]
    fn apps_are_named_and_the_shell_is_skipped() {
        let f = front(&win(1, 7, "Chrome_WidgetWin_1", "GitHub"), ME, names).unwrap();
        assert_eq!((f.name.as_str(), f.exe.as_str(), f.desktop), ("Google Chrome", "chrome", false));
        // Our own bar and menus, the taskbar, Start and Search keep the name that was there.
        assert_eq!(front(&win(1, ME, "Tauri", "Lintel"), ME, names), None);
        assert_eq!(front(&win(1, 4, "Shell_TrayWnd", ""), ME, names), None);
        assert_eq!(front(&win(1, 8, "Windows.UI.Core.CoreWindow", "Search"), ME, names), None);
        // The desktop is named as such.
        assert!(front(&win(1, 4, "Progman", "Program Manager"), ME, names).unwrap().desktop);
        // A program Windows won't tell us about still gets a name.
        assert_eq!(front(&win(1, 9, "X", "Admin tool"), ME, names).unwrap().name, "Window");
    }

    #[test]
    fn display_names() {
        assert_eq!(display_name("chrome", Some("Google Chrome")), "Google Chrome");
        assert_eq!(display_name("explorer", Some("Windows Explorer")), "File Explorer");
        assert_eq!(display_name("Discord", Some("Electron")), "Discord");
        assert_eq!(display_name("foo", Some("Foo - the best foo")), "Foo");
        assert_eq!(display_name("foo", None), "foo");
        assert_eq!(display_name("foo", Some("")), "foo");
    }

    #[test]
    fn listing_follows_alt_tab() {
        assert!(listed(&win(1, 7, "X", "Notes"), ME));
        assert!(!listed(&win(1, 7, "X", ""), ME));
        assert!(!listed(&win(1, ME, "X", "Lintel"), ME));
        assert!(!listed(&Raw { cloaked: true, ..win(1, 7, "X", "Calculator") }, ME));
        assert!(!listed(&Raw { ex_style: WS_EX_TOOLWINDOW, ..win(1, 7, "X", "Palette") }, ME));
        assert!(!listed(&Raw { owned: true, ..win(1, 7, "X", "Save as") }, ME));
        assert!(!listed(&win(1, 7, "Progman", "Program Manager"), ME));
    }

    #[test]
    fn refuses_handles_that_are_not_windows() {
        assert!(focus(0, 1).is_err());
        assert!(close(u64::MAX, 1).is_err());
        assert!(minimize(0x7fff_fff0, 1).is_err());
    }
}
