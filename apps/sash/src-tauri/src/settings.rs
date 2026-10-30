//! User settings, stored as JSON in %APPDATA%\Sash\settings.json (beside the other apps' folders).

use crate::actions::ACTIONS;
use mullion_core::settings::PALETTES;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub use mullion_core::settings::{read_json, write_json};
/// The space Sash can leave between windows and around the screen's edges, in pixels at 100%.
pub const GAPS: &[u32] = &[0, 4, 8, 16];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// "auto" (follow Windows), "dark" or "light".
    pub theme: String,
    /// One of the suite's palettes.
    pub palette: String,
    /// Each action's shortcut, by its id in `ACTIONS`; empty for none.
    pub shortcuts: BTreeMap<String, String>,
    /// Pressing a half's arrow again makes it two-thirds, then a third.
    pub cycle: bool,
    /// One of `GAPS`.
    pub gap: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "auto".into(),
            palette: "oriel".into(),
            shortcuts: ACTIONS.iter().map(|a| (a.id.to_string(), a.key.to_string())).collect(),
            cycle: true,
            gap: 0,
        }
    }
}

pub fn dir() -> PathBuf {
    mullion_core::settings::dir("Sash")
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

    /// Anything a hand-edited file got wrong goes back to its default: an action it doesn't know
    /// is dropped, one it leaves out gets its own shortcut, and a shortcut Sash can't read, or one
    /// already given to an action above it, is none.
    pub fn clamp(mut self) -> Self {
        let d = Settings::default();
        if !["auto", "dark", "light"].contains(&self.theme.as_str()) {
            self.theme = d.theme;
        }
        if !PALETTES.contains(&self.palette.as_str()) {
            self.palette = d.palette;
        }
        if !GAPS.contains(&self.gap) {
            self.gap = d.gap;
        }
        let mut taken = Vec::new();
        self.shortcuts = ACTIONS
            .iter()
            .map(|a| {
                let mut spec = self.shortcuts.get(a.id).map_or(a.key.to_string(), |s| normal(s));
                if !spec.is_empty() && (mullion_core::hotkey::parse(&spec).is_none() || taken.contains(&spec)) {
                    spec.clear();
                }
                if !spec.is_empty() {
                    taken.push(spec.clone());
                }
                (a.id.to_string(), spec)
            })
            .collect();
        self
    }

    pub fn shortcut(&self, id: &str) -> &str {
        self.shortcuts.get(id).map_or("", |s| s.as_str())
    }
}

/// A shortcut as Sash keeps it: lower case, modifiers in one order ("ctrl+alt+shift+win+left"),
/// so the same keys always read the same.
pub fn normal(spec: &str) -> String {
    let parts: Vec<String> = spec.to_lowercase().split('+').map(|p| p.trim().replace("control", "ctrl")).filter(|p| !p.is_empty()).collect();
    let mods = ["ctrl", "alt", "shift", "win"];
    let mut out: Vec<String> = mods.iter().filter(|m| parts.iter().any(|p| p == *m)).map(|m| m.to_string()).collect();
    out.extend(parts.into_iter().filter(|p| !mods.contains(&p.as_str())));
    out.join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_wrong_values_fall_back() {
        let s: Settings = serde_json::from_str(r#"{"palette":"nord","theme":"neon","gap":8,"cycle":false}"#).unwrap();
        let s = s.clamp();
        assert_eq!((s.palette.as_str(), s.theme.as_str(), s.gap, s.cycle), ("nord", "auto", 8, false));
        assert_eq!(s.shortcut("left"), "ctrl+alt+left");
        let s: Settings = serde_json::from_str(r#"{"palette":"<script>","gap":7}"#).unwrap();
        assert_eq!(s.clamp(), Settings::default());
    }

    #[test]
    fn shortcuts_are_kept_tidy() {
        let s: Settings = serde_json::from_str(
            r#"{"shortcuts":{"left":"Alt+Ctrl+Left","right":"","top":"ctrl+alt+left","bottom":"ctrl+alt+nothing","nope":"ctrl+alt+x","top-left":"Shift+Win+Alt+Ctrl+U"}}"#,
        )
        .unwrap();
        let s = s.clamp();
        assert_eq!(s.shortcut("left"), "ctrl+alt+left");
        // Cleared stays cleared; a second use of the same keys, or keys Sash can't read, is none.
        assert_eq!(s.shortcut("right"), "");
        assert_eq!(s.shortcut("top"), "");
        assert_eq!(s.shortcut("bottom"), "");
        assert_eq!(s.shortcut("top-left"), "ctrl+alt+shift+win+u");
        // Left out: its own default.
        assert_eq!(s.shortcut("maximize"), "ctrl+alt+enter");
        assert!(!s.shortcuts.contains_key("nope"));
        assert_eq!(s.shortcuts.len(), ACTIONS.len());
    }
}
