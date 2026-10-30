//! User settings, stored as JSON in %APPDATA%\Mitre\settings.json (beside the other apps' folders).

use crate::actions::{self, NOTHING};
use crate::corners::Corner;
use mullion_core::settings::PALETTES;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub use mullion_core::settings::{read_json, write_json};
/// How long the pointer waits in a corner before it fires, in ms.
pub const DWELLS: &[u32] = &[100, 150, 250, 400, 600];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// "auto" (follow Windows), "dark" or "light".
    pub theme: String,
    /// One of the suite's palettes; the glow is its accent.
    pub palette: String,
    /// One of `DWELLS`.
    pub dwell: u32,
    /// A glow grows in the corner while the pointer waits there.
    pub glow: bool,
    /// Each screen's corners, by the screen's lasting name (`corners::Screen::key`): an action's
    /// id for top-left, top-right, bottom-left and bottom-right. A screen not here does nothing
    /// yet; one that's unplugged keeps its corners for when it's back.
    pub corners: BTreeMap<String, [String; 4]>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { theme: "auto".into(), palette: "oriel".into(), dwell: 150, glow: true, corners: BTreeMap::new() }
    }
}

pub fn dir() -> PathBuf {
    mullion_core::settings::dir("Mitre")
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

    /// What the page and the glow show: these settings, with Mullion's look for every app over
    /// Mitre's own while there is one (`mullion_core::look`).
    pub fn shown(&self) -> Self {
        let mut s = self.clone();
        mullion_core::look::overlay(&mut s.theme, &mut s.palette, None);
        s
    }

    /// While Mullion's look stands in for Mitre's own, a save keeps `own`'s, so it comes back
    /// when that's turned off.
    pub fn keep_look(mut self, own: &Settings) -> Self {
        if mullion_core::look::shared().is_some() {
            self.theme = own.theme.clone();
            self.palette = own.palette.clone();
        }
        self
    }

    /// Anything a hand-edited file got wrong goes back to its default; an action Mitre doesn't
    /// know is nothing, and a screen with nothing in any corner is dropped.
    pub fn clamp(mut self) -> Self {
        let d = Settings::default();
        if !["auto", "dark", "light"].contains(&self.theme.as_str()) {
            self.theme = d.theme;
        }
        if !PALETTES.contains(&self.palette.as_str()) {
            self.palette = d.palette;
        }
        if !DWELLS.contains(&self.dwell) {
            self.dwell = d.dwell;
        }
        for corners in self.corners.values_mut() {
            for a in corners.iter_mut() {
                if actions::by_id(a).is_none() {
                    *a = NOTHING.into();
                }
            }
        }
        self.corners.retain(|key, c| !key.is_empty() && c.iter().any(|a| a != NOTHING));
        self
    }

    /// What a screen's corner does.
    pub fn action(&self, screen: &str, corner: Corner) -> &str {
        self.corners.get(screen).map_or(NOTHING, |c| c[corner.index()].as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_wrong_values_fall_back() {
        let s: Settings = serde_json::from_str(r#"{"palette":"nord","theme":"neon","dwell":250,"glow":false}"#).unwrap();
        let s = s.clamp();
        assert_eq!((s.palette.as_str(), s.theme.as_str(), s.dwell, s.glow), ("nord", "auto", 250, false));
        let s: Settings = serde_json::from_str(r#"{"palette":"<script>","dwell":7}"#).unwrap();
        assert_eq!(s.clamp(), Settings::default());
    }

    #[test]
    fn corners_are_kept_tidy() {
        let s: Settings = serde_json::from_str(
            r#"{"corners":{"a":["bay","nope","nothing","lock"],"b":["nothing","nothing","nothing","nothing"],"c":["launch","","nothing","nothing"]}}"#,
        )
        .unwrap();
        let s = s.clamp();
        assert_eq!(s.corners.len(), 1);
        assert_eq!(s.action("a", Corner::TopLeft), "bay");
        assert_eq!(s.action("a", Corner::TopRight), NOTHING);
        assert_eq!(s.action("a", Corner::BottomRight), "lock");
        assert_eq!(s.action("elsewhere", Corner::BottomRight), NOTHING);
    }
}
