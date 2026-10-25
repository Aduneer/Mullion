//! The open windows, to switch to by title or by app: the ones Alt+Tab lists (`mullion_core::wins`).

use crate::ffi::*;
use mullion_core::wins::{enumerate, listed};
use std::collections::HashMap;
use std::sync::Mutex;

pub use mullion_core::wins::focus;

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
