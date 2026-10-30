//! What every app's windows do to look the part: follow the theme, and have Windows 11's corners.
//!
//! And the one look Mullion can give them all: with **Same look for every app** on in Mullion's
//! settings, `%APPDATA%\Mullion\look.json` has the theme, palette and glass, and every app Mullion
//! runs shows those instead of its own (`shared`). Its own stay in its settings file, untouched,
//! for when the switch goes off. Mullion asks running apps to look again with `--look`.

use crate::ffi::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND};
use crate::settings::{dir, read_json, write_json, PALETTES};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Theme;

pub const THEMES: &[&str] = &["auto", "dark", "light"];

/// Mullion's look for every app, as it keeps it in look.json.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shared {
    /// Same look for every app: off, each app has its own.
    pub on: bool,
    /// "auto" (Windows' own), "dark" or "light".
    pub theme: String,
    /// One of `PALETTES`.
    pub palette: String,
    /// See-through where an app can be (Lintel's bar, Fanlight's panel).
    pub glass: bool,
}

impl Default for Shared {
    fn default() -> Self {
        Shared { on: false, theme: "auto".into(), palette: "oriel".into(), glass: false }
    }
}

impl Shared {
    pub fn path() -> PathBuf {
        dir("Mullion").join("look.json")
    }

    /// What's in look.json, with anything unknown put back to the default.
    pub fn load() -> Self {
        read_json::<Shared>(&Self::path()).unwrap_or_default().clamp()
    }

    pub fn save(&self) -> std::io::Result<()> {
        write_json(&Self::path(), self, true)
    }

    pub fn clamp(mut self) -> Self {
        let d = Shared::default();
        if !THEMES.contains(&self.theme.as_str()) {
            self.theme = d.theme;
        }
        if !PALETTES.contains(&self.palette.as_str()) {
            self.palette = d.palette;
        }
        self
    }
}

/// Mullion's look, if this app should show it: Mullion runs it, and the switch is on. An app on
/// its own keeps its own look, whatever look.json says.
pub fn shared() -> Option<Shared> {
    if !crate::suite::is_run_by_mullion() {
        return None;
    }
    Some(Shared::load()).filter(|s| s.on)
}

/// Put Mullion's look over an app's own `theme`, `palette` and (if it has it) `glass`, while
/// there's one. True if there was.
pub fn overlay(theme: &mut String, palette: &mut String, glass: Option<&mut bool>) -> bool {
    let Some(s) = shared() else { return false };
    *theme = s.theme;
    *palette = s.palette;
    if let Some(g) = glass {
        *g = s.glass;
    }
    true
}

/// The WebView2 flags Lintel's and Fanlight's windows have: Tauri's defaults (which setting our own
/// would otherwise drop), plus no GPU process, which pages of text and switches don't need and
/// is the biggest part of a WebView's memory. Windows with the same flags and data folder could
/// one day share one browser process.
pub const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-gpu";

/// The native theme for a `theme` setting: "dark", "light", or anything else to follow Windows.
pub fn native_theme(theme: &str) -> Option<Theme> {
    match theme {
        "dark" => Some(Theme::Dark),
        "light" => Some(Theme::Light),
        _ => None,
    }
}

/// Windows 11's rounded corners, which a window without a title bar doesn't get by itself.
/// Windows 10 has none, and ignores this.
pub fn round_corners(hwnd: isize) {
    unsafe { DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, (&DWMWCP_ROUND as *const u32).cast(), 4) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shared_look_is_off_and_sane_by_default() {
        let s = Shared::default();
        assert!(!s.on && s.theme == "auto" && s.palette == "oriel" && !s.glass);
        let odd: Shared = serde_json::from_str(r#"{"on":true,"theme":"neon","palette":"<script>"}"#).unwrap();
        let odd = odd.clamp();
        assert!(odd.on);
        assert_eq!((odd.theme.as_str(), odd.palette.as_str()), ("auto", "oriel"));
    }

    #[test]
    fn an_app_on_its_own_keeps_its_look() {
        // These tests never come from Mullion, so whatever look.json says, nothing is laid over.
        let (mut theme, mut palette, mut glass) = ("dark".to_string(), "nord".to_string(), true);
        assert!(!overlay(&mut theme, &mut palette, Some(&mut glass)));
        assert_eq!((theme.as_str(), palette.as_str(), glass), ("dark", "nord", true));
    }

    #[test]
    fn themes_follow_the_setting() {
        assert_eq!(native_theme("dark"), Some(Theme::Dark));
        assert_eq!(native_theme("light"), Some(Theme::Light));
        assert_eq!(native_theme("auto"), None);
    }
}
