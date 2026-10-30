//! What the dock shows: the pinned apps, in their order, then the other running apps in the order
//! they started; each with its windows (a dot apiece). The first time, the pins are the taskbar's.

use crate::settings::Pin;
use mullion_core::apps::{App, AppWin};
use mullion_core::icons;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use windows::core::{Interface, HSTRING};
use windows::Win32::System::Com::{CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER, STGM_READ};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

/// One app in the dock.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Item {
    pub key: String,
    pub name: String,
    pub icon: Option<String>,
    pub pinned: bool,
    /// Its windows, top-most first; none if it isn't running.
    pub windows: Vec<AppWin>,
}

/// The dock's apps: `pins` (with `pin_icons` for those not running), then the rest of `running`.
pub fn items(pins: &[Pin], running: &[App], pin_icons: &HashMap<String, Option<String>>) -> Vec<Item> {
    let mut out: Vec<Item> = pins
        .iter()
        .map(|p| match running.iter().find(|a| a.key == p.key) {
            Some(a) => Item { key: p.key.clone(), name: p.name.clone(), icon: a.icon.clone(), pinned: true, windows: a.windows.clone() },
            None => Item { key: p.key.clone(), name: p.name.clone(), icon: pin_icons.get(&p.key).cloned().flatten(), pinned: true, windows: Vec::new() },
        })
        .collect();
    out.extend(
        running
            .iter()
            .filter(|a| !pins.iter().any(|p| p.key == a.key))
            .map(|a| Item { key: a.key.clone(), name: a.name.clone(), icon: a.icon.clone(), pinned: false, windows: a.windows.clone() }),
    );
    out
}

/// A pin's icon, as the shell draws it. The calling thread must have COM initialized.
pub fn icon(pin: &Pin) -> Option<String> {
    let exe = pin.key.ends_with(".exe") && std::path::Path::new(&pin.key).is_file();
    if pin.launch.starts_with("shell:") || !exe {
        icons::icon(&pin.launch, icons::SIZE_PX)
    } else {
        icons::icon(&pin.key, icons::SIZE_PX).or_else(|| icons::icon(&pin.launch, icons::SIZE_PX))
    }
}

/// The apps pinned to Windows' taskbar, from their shortcuts, so the dock starts with them. The
/// calling thread must have COM initialized.
pub fn from_taskbar() -> Vec<Pin> {
    let Some(appdata) = std::env::var_os("APPDATA") else { return Vec::new() };
    let folder = std::path::PathBuf::from(appdata).join(r"Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar");
    let Ok(dir) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut links: Vec<std::path::PathBuf> = dir.flatten().map(|e| e.path()).filter(|p| is_link(p)).collect();
    links.sort();
    let mut pins: Vec<Pin> = Vec::new();
    for link in links {
        let p = from_link(&link);
        if !pins.iter().any(|q| q.key == p.key) {
            pins.push(p);
        }
    }
    pins
}

fn is_link(p: &Path) -> bool {
    p.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk"))
}

/// A shortcut, pinned. One to a program is known by the program, as its windows are; one to a
/// Store app or a folder by itself.
fn from_link(link: &Path) -> Pin {
    let launch = link.display().to_string();
    let name = link.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let key = target(&launch).filter(|t| t.to_lowercase().ends_with(".exe")).unwrap_or_else(|| launch.clone()).to_lowercase();
    Pin { key, name, launch }
}

/// A file dropped on the dock or picked in the settings, pinned: a shortcut, a program, or
/// anything else Windows opens (a folder, a document). The calling thread must have COM initialized.
pub fn from_path(path: &Path) -> Option<Pin> {
    if !path.exists() {
        return None;
    }
    if is_link(path) {
        return Some(from_link(path));
    }
    let launch = path.display().to_string();
    let name = if launch.to_lowercase().ends_with(".exe") {
        mullion_core::apps::program_name(&launch)
    } else {
        path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| launch.clone())
    };
    Some(Pin { key: launch.to_lowercase(), name, launch })
}

/// The Start menu's apps (its shortcuts, yours and everyone's), by name, for adding to the dock;
/// not their uninstallers or help files. The calling thread must have COM initialized.
pub fn start_menu() -> Vec<Pin> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if is_link(&p) {
                out.push(p);
            }
        }
    }
    let mut links = Vec::new();
    for root in ["APPDATA", "PROGRAMDATA"] {
        if let Some(base) = std::env::var_os(root) {
            walk(&PathBuf::from(base).join(r"Microsoft\Windows\Start Menu\Programs"), &mut links);
        }
    }
    let mut pins: Vec<Pin> = Vec::new();
    for link in links {
        let p = from_link(&link);
        let lower = p.name.to_lowercase();
        if ["uninstall", "readme", "read me", "help", "website", "manual"].iter().any(|w| lower.contains(w)) || pins.iter().any(|q| q.key == p.key) {
            continue;
        }
        pins.push(p);
    }
    pins.sort_by_key(|p| p.name.to_lowercase());
    pins
}

/// Where a shortcut points, if it's a file.
fn target(link: &str) -> Option<String> {
    unsafe {
        let sl: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        sl.cast::<IPersistFile>().ok()?.Load(&HSTRING::from(link), STGM_READ).ok()?;
        let mut buf = [0u16; 1024];
        sl.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
        let path = mullion_core::ffi::from_wide(&buf);
        (!path.is_empty()).then_some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(key: &str, windows: usize) -> App {
        let w = AppWin { hwnd: 1, pid: 1, minimized: false, front: false, monitor: 0 };
        App { key: key.into(), name: key.to_uppercase(), exe: key.into(), pid: 1, windows: vec![w; windows], icon: None, launch: key.into() }
    }

    #[test]
    fn pins_first_then_the_rest_running() {
        let pins = vec![
            Pin { key: "b".into(), name: "Bee".into(), launch: "b.exe".into() },
            Pin { key: "z".into(), name: "Zed".into(), launch: "z.exe".into() },
        ];
        let icons = HashMap::from([("z".to_string(), Some("data:z".to_string()))]);
        let all = items(&pins, &[app("a", 1), app("b", 2)], &icons);
        let keys: Vec<(&str, bool, usize)> = all.iter().map(|i| (i.key.as_str(), i.pinned, i.windows.len())).collect();
        assert_eq!(keys, [("b", true, 2), ("z", true, 0), ("a", false, 1)]);
        // A pin keeps its own name; one not running has its own icon.
        assert_eq!((all[0].name.as_str(), all[1].icon.as_deref()), ("Bee", Some("data:z")));
    }

    #[test]
    fn reads_the_taskbars_pins() {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
        }
        // Whatever this PC has pinned: each with a name and something to open, none twice.
        let pins = from_taskbar();
        for (i, p) in pins.iter().enumerate() {
            assert!(!p.name.is_empty() && p.launch.to_lowercase().ends_with(".lnk"));
            assert!(pins[i + 1..].iter().all(|q| q.key != p.key));
        }
    }

    #[test]
    fn files_are_pinned_by_what_they_are() {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
        }
        let notepad = std::env::var("WINDIR").unwrap() + r"\System32\notepad.exe";
        let p = from_path(Path::new(&notepad)).expect("notepad");
        assert_eq!((p.key, p.launch), (notepad.to_lowercase(), notepad));
        let dir = std::env::temp_dir();
        assert!(from_path(&dir).is_some_and(|p| p.key == dir.display().to_string().to_lowercase()));
        assert!(from_path(Path::new(r"C:\nowhere\at-all.exe")).is_none());
        // The Start menu's apps, sorted by name, none twice.
        let all = start_menu();
        assert!(all.windows(2).all(|w| w[0].name.to_lowercase() <= w[1].name.to_lowercase()));
    }
}
