//! The open windows, to switch to by title or by app: the ones Alt+Tab lists (from Lintel).

use crate::ffi::*;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Clone, Debug, Default)]
pub struct Win {
    pub hwnd: isize,
    pub pid: u32,
    pub title: String,
    /// The app's name, from its program file ("Google Chrome").
    pub app: String,
    /// The program's path, for its icon.
    pub path: String,
    pub minimized: bool,
}

/// Display names by program path, looked up once per program.
static NAMES: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const WS_EX_APPWINDOW: u32 = 0x0004_0000;
const WS_EX_NOACTIVATE: u32 = 0x0800_0000;

/// Shell windows that aren't apps to switch to: the taskbar, Start, Search, Alt+Tab, the desktop.
const SHELL_CLASSES: &[&str] = &[
    "Shell_TrayWnd", "Shell_SecondaryTrayWnd", "NotifyIconOverflowWindow", "TopLevelWindowForOverflowXamlIsland",
    "XamlExplorerHostIslandWindow", "ForegroundStaging", "MultitaskingViewFrame", "TaskSwitcherWnd", "#32768", "Progman", "WorkerW",
];

/// What Windows says about one top-level window.
#[derive(Clone, Debug, Default)]
struct Raw {
    hwnd: isize,
    pid: u32,
    owned: bool,
    ex_style: u32,
    cloaked: bool,
    minimized: bool,
    class: String,
    title: String,
}

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

/// The same test Alt+Tab applies, as far as it's documented.
fn listed(w: &Raw, own_pid: u32) -> bool {
    let app = w.ex_style & WS_EX_APPWINDOW != 0;
    !w.title.is_empty()
        && w.pid != own_pid
        && !w.cloaked
        && (app || (w.ex_style & (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) == 0 && !w.owned))
        && !SHELL_CLASSES.contains(&w.class.as_str())
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

/// The open windows, top-most first.
pub fn list() -> Vec<Win> {
    let own = std::process::id();
    let mut names = NAMES.lock().unwrap();
    let names = names.get_or_insert_with(HashMap::new);
    enumerate()
        .into_iter()
        .filter(|w| listed(w, own))
        .map(|w| {
            let path = image_path(w.pid).unwrap_or_default();
            let app = names.entry(path.clone()).or_insert_with(|| app_name(&path)).clone();
            Win { hwnd: w.hwnd, pid: w.pid, title: w.title, app, path, minimized: w.minimized }
        })
        .collect()
}

fn app_name(path: &str) -> String {
    let exe = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    match exe.to_lowercase().as_str() {
        "explorer" => return "File Explorer".into(),
        "windowsterminal" => return "Terminal".into(),
        "systemsettings" => return "Settings".into(),
        "applicationframehost" => return "Windows app".into(),
        _ => {}
    }
    match file_description(path) {
        // Runtimes that name themselves rather than the app built on them.
        Some(d) if !["electron", "java(tm) platform se binary", "python", "node.js"].contains(&d.to_lowercase().as_str()) => {
            d.split(" - ").next().unwrap_or(&d).trim().to_string()
        }
        _ => exe,
    }
}

/// Bring a window to the front, restoring it first if it's minimized. Only one the list showed,
/// still the same program's: Windows reuses handles.
pub fn focus(hwnd: isize, pid: u32) -> Result<(), String> {
    match read(hwnd) {
        Some(w) if w.pid == pid && listed(&w, std::process::id()) => {}
        _ => return Err("that window has gone".into()),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const ME: u32 = 100;

    fn win(pid: u32, class: &str, title: &str) -> Raw {
        Raw { hwnd: 1, pid, class: class.into(), title: title.into(), ..Default::default() }
    }

    #[test]
    fn listing_follows_alt_tab() {
        assert!(listed(&win(7, "X", "Notes"), ME));
        assert!(!listed(&win(7, "X", ""), ME));
        assert!(!listed(&win(ME, "X", "Fanlight"), ME));
        assert!(!listed(&Raw { cloaked: true, ..win(7, "X", "Calculator") }, ME));
        assert!(!listed(&Raw { ex_style: WS_EX_TOOLWINDOW, ..win(7, "X", "Palette") }, ME));
        assert!(!listed(&Raw { owned: true, ..win(7, "X", "Save as") }, ME));
        assert!(!listed(&win(7, "Progman", "Program Manager"), ME));
    }

    #[test]
    fn refuses_handles_that_are_not_windows() {
        assert!(focus(0, 1).is_err());
        assert!(focus(0x7fff_fff0, 1).is_err());
    }
}
