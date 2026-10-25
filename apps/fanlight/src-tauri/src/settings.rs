//! User settings, stored as JSON in %APPDATA%\Fanlight\settings.json (beside Oriel's and Lintel's own folders).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const PALETTES: &[&str] = &["oriel", "catppuccin", "nord", "gruvbox", "rose-pine", "solarized"];
pub const HOTKEYS: &[&str] = &["alt+space", "ctrl+space", "ctrl+alt+space", "alt+shift+space", "win+alt+space", "ctrl+shift+space"];
pub const ENGINES: &[&str] = &["google", "bing", "duckduckgo", "brave", "ecosia", "startpage", "kagi"];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// "auto" (follow Windows), "dark" or "light".
    pub theme: String,
    /// One of Oriel's palettes, so the suite looks alike.
    pub palette: String,
    /// A see-through panel that blurs what's behind it (Windows' acrylic), rather than a solid one.
    pub glass: bool,
    /// The shortcut that opens the panel, one of `HOTKEYS`.
    pub hotkey: String,
    /// Where "Search the web" goes, one of `ENGINES`.
    pub engine: String,
    /// Files and folders in the results.
    pub files: bool,
    /// Folders searched for files besides the usual ones (Desktop, Documents, Downloads, Pictures,
    /// Music, Videos).
    pub folders: Vec<String>,
    /// Open windows in the results, to switch to.
    pub windows: bool,
    /// How the Apps category shows apps: "grid" (icons, as Launchpad does) or "list".
    pub apps_view: String,
    /// Keep a history of what's copied, to search and paste again.
    pub clipboard: bool,
    /// How many copied texts the history keeps.
    pub clipboard_size: usize,
    /// Keep the clipboard history when Fanlight quits (in %APPDATA%\Fanlight\clipboard.json),
    /// rather than in memory only.
    pub keep_clipboard: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "auto".into(),
            palette: "oriel".into(),
            glass: false,
            hotkey: "alt+space".into(),
            engine: "google".into(),
            files: true,
            folders: Vec::new(),
            windows: true,
            apps_view: "grid".into(),
            clipboard: true,
            clipboard_size: 100,
            keep_clipboard: false,
        }
    }
}

pub fn dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("Fanlight")
}

pub fn path() -> PathBuf {
    dir().join("settings.json")
}

/// Read a JSON file of ours, or None if it's missing or broken.
pub fn read_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Option<T> {
    std::fs::read_to_string(path)
        .ok()
        // Notepad and PowerShell can save with a byte-order mark, which serde_json rejects.
        .and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok())
}

/// Write a JSON file of ours: to a temporary file first, then over the old one, so a crash
/// halfway never leaves half a file.
pub fn write_json<T: Serialize>(path: &std::path::Path, value: &T, pretty: bool) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = if pretty { serde_json::to_string_pretty(value)? } else { serde_json::to_string(value)? };
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, path)
}

impl Settings {
    pub fn load() -> Self {
        read_json(&path()).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        write_json(&path(), self, true)
    }

    /// Anything a hand-edited file got wrong goes back to its default.
    pub fn clamp(mut self) -> Self {
        let d = Settings::default();
        if !["auto", "dark", "light"].contains(&self.theme.as_str()) {
            self.theme = d.theme;
        }
        if !PALETTES.contains(&self.palette.as_str()) {
            self.palette = d.palette;
        }
        self.hotkey = self.hotkey.to_lowercase();
        if !HOTKEYS.contains(&self.hotkey.as_str()) {
            self.hotkey = d.hotkey;
        }
        if !ENGINES.contains(&self.engine.as_str()) {
            self.engine = d.engine;
        }
        if !["grid", "list"].contains(&self.apps_view.as_str()) {
            self.apps_view = d.apps_view;
        }
        let mut seen = Vec::new();
        self.folders.retain(|f| {
            let f = f.trim().trim_end_matches(['\\', '/']).to_lowercase();
            !f.is_empty() && !seen.contains(&f) && {
                seen.push(f);
                true
            }
        });
        self.folders.truncate(32);
        self.clipboard_size = self.clipboard_size.clamp(10, 1000);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_wrong_values_fall_back() {
        let s: Settings = serde_json::from_str(r#"{"palette":"nord","clipboard_size":5000,"theme":"neon","hotkey":"Ctrl+Space"}"#).unwrap();
        let s = s.clamp();
        assert_eq!((s.palette.as_str(), s.clipboard_size, s.theme.as_str(), s.hotkey.as_str()), ("nord", 1000, "auto", "ctrl+space"));
        let s: Settings = serde_json::from_str(r#"{"palette":"<script>","engine":"evil.example","hotkey":"f13","apps_view":"tiles"}"#).unwrap();
        assert_eq!(s.clamp(), Settings::default());
    }

    #[test]
    fn folders_are_tidied() {
        let s = Settings { folders: vec![r"D:\Work\".into(), r"d:\work".into(), "  ".into(), r"E:\Photos".into()], ..Default::default() }.clamp();
        assert_eq!(s.folders, [r"D:\Work\", r"E:\Photos"]);
    }
}
