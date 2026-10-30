//! User settings, stored as JSON in %APPDATA%\Bay\settings.json (beside the other apps' folders).

use mullion_core::settings::PALETTES;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use mullion_core::settings::{read_json, write_json};
/// The shortcuts Bay offers. Win+Ctrl+Up sits beside Windows' Win+Ctrl+Left and Right, which
/// switch desktops; none of these is Windows' own or Fanlight's.
pub const HOTKEYS: &[&str] = &["win+ctrl+up", "win+shift+tab", "ctrl+alt+up", "ctrl+up"];
pub const BACKDROPS: &[&str] = &["wallpaper", "plain"];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// "auto" (follow Windows), "dark" or "light".
    pub theme: String,
    /// One of the suite's palettes, for the ring around the window you're on.
    pub palette: String,
    /// The shortcut that opens the overview, one of `HOTKEYS`.
    pub hotkey: String,
    /// Behind the windows: "wallpaper" (yours, blurred and dimmed) or "plain" (the palette's).
    pub backdrop: String,
    /// Minimized windows in the overview, as cards.
    pub minimized: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { theme: "auto".into(), palette: "oriel".into(), hotkey: "win+ctrl+up".into(), backdrop: "wallpaper".into(), minimized: true }
    }
}

pub fn dir() -> PathBuf {
    mullion_core::settings::dir("Bay")
}

pub fn path() -> PathBuf {
    dir().join("settings.json")
}

impl Settings {
    pub fn load() -> Self {
        read_json(&path()).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        write_json(&path(), self, true)
    }

    /// What the pages show: these settings, with Mullion's look for every app over Bay's own
    /// while there is one (`mullion_core::look`).
    pub fn shown(&self) -> Self {
        let mut s = self.clone();
        mullion_core::look::overlay(&mut s.theme, &mut s.palette, None);
        s
    }

    /// While Mullion's look stands in for Bay's own, a save keeps `own`'s, so it comes back
    /// when that's turned off.
    pub fn keep_look(mut self, own: &Settings) -> Self {
        if mullion_core::look::shared().is_some() {
            self.theme = own.theme.clone();
            self.palette = own.palette.clone();
        }
        self
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
        if !BACKDROPS.contains(&self.backdrop.as_str()) {
            self.backdrop = d.backdrop;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_wrong_values_fall_back() {
        let s: Settings = serde_json::from_str(r#"{"palette":"nord","theme":"neon","hotkey":"Win+Shift+Tab","minimized":false}"#).unwrap();
        let s = s.clamp();
        assert_eq!((s.palette.as_str(), s.theme.as_str(), s.hotkey.as_str(), s.minimized), ("nord", "auto", "win+shift+tab", false));
        let s: Settings = serde_json::from_str(r#"{"palette":"<script>","hotkey":"alt+space","backdrop":"video"}"#).unwrap();
        assert_eq!(s.clamp(), Settings::default());
    }

    #[test]
    fn every_shortcut_offered_is_one_windows_takes() {
        for h in HOTKEYS {
            assert!(mullion_core::hotkey::parse(h).is_some(), "{h}");
        }
    }
}
