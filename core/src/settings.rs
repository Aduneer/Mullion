//! Where each app keeps its settings (%APPDATA%\<App>, side by side), and reading and writing its
//! JSON files there. Each app has its own `Settings`; the palettes are the suite's.

use serde::Serialize;
use std::path::{Path, PathBuf};

/// The palettes in ui-common/themes.css, so every app looks alike.
pub const PALETTES: &[&str] = &["oriel", "catppuccin", "nord", "gruvbox", "rose-pine", "solarized"];

/// An app's own folder: %APPDATA%\<app>, or under the temp folder if there's no %APPDATA%.
pub fn dir(app: &str) -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join(app)
}

/// Read a JSON file of ours, or None if it's missing or broken.
pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    std::fs::read_to_string(path)
        .ok()
        // Notepad and PowerShell can save with a byte-order mark, which serde_json rejects.
        .and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok())
}

/// Write a JSON file of ours: to a temporary file first, then over the old one, so a crash
/// halfway never leaves half a file.
pub fn write_json<T: Serialize>(path: &Path, value: &T, pretty: bool) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = if pretty { serde_json::to_string_pretty(value)? } else { serde_json::to_string(value)? };
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trips_and_survives_a_byte_order_mark() {
        let dir = std::env::temp_dir().join(format!("mullion-core-test-{}", std::process::id()));
        let path = dir.join("test.json");
        write_json(&path, &vec![1, 2, 3], true).unwrap();
        assert_eq!(read_json::<Vec<i32>>(&path), Some(vec![1, 2, 3]));
        assert!(!path.with_extension("tmp").exists());
        std::fs::write(&path, "\u{feff}[4]").unwrap();
        assert_eq!(read_json::<Vec<i32>>(&path), Some(vec![4]));
        std::fs::write(&path, "[4").unwrap();
        assert_eq!(read_json::<Vec<i32>>(&path), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn each_app_has_its_own_folder() {
        assert!(dir("Fanlight").ends_with("Fanlight"));
        assert_ne!(dir("Fanlight"), dir("Lintel"));
    }
}
