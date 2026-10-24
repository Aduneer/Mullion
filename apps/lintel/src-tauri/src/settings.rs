//! User settings, stored as JSON in %APPDATA%\Lintel\settings.json (beside Oriel's own folder).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const PALETTES: &[&str] = &["oriel", "catppuccin", "nord", "gruvbox", "rose-pine", "solarized"];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// "auto" (follow Windows), "dark" or "light".
    pub theme: String,
    /// One of Oriel's palettes, so the two look alike.
    pub palette: String,
    /// Translucent bar and menus that blur what's behind them (acrylic), rather than solid ones.
    pub glass: bool,
    /// The screen edge the bar sits on: "top", "bottom", "left" or "right".
    pub edge: String,
    /// The bar's height at 100% scaling, in pixels; a bar on the left or right is a little wider
    /// than that (see `thickness`).
    pub height: u32,
    /// "auto" (as Windows is set), "12" or "24".
    pub clock: String,
    pub seconds: bool,
    /// The date beside the time.
    pub date: bool,
    /// Which items the bar shows, right to left after the clock.
    pub media: bool,
    pub stats: bool,
    pub volume: bool,
    pub network: bool,
    pub battery: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "auto".into(),
            palette: "oriel".into(),
            glass: false,
            edge: "top".into(),
            height: 30,
            clock: "auto".into(),
            seconds: false,
            date: true,
            media: true,
            stats: true,
            volume: true,
            network: true,
            battery: true,
        }
    }
}

pub fn path() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("Lintel").join("settings.json")
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .ok()
            // Notepad and PowerShell can save with a byte-order mark, which serde_json rejects.
            .and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let p = path();
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(p, serde_json::to_string_pretty(self)?)
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
        if !["auto", "12", "24"].contains(&self.clock.as_str()) {
            self.clock = d.clock;
        }
        if !["top", "bottom", "left", "right"].contains(&self.edge.as_str()) {
            self.edge = d.edge;
        }
        self.height = self.height.clamp(24, 44);
        self
    }

    pub fn vertical(&self) -> bool {
        matches!(self.edge.as_str(), "left" | "right")
    }

    /// How far the bar reaches into the screen, at 100% scaling. Standing on its side, it needs
    /// room for "17:42" and "100%" across.
    pub fn thickness(&self) -> u32 {
        if self.vertical() {
            self.height + 16
        } else {
            self.height
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_wrong_values_fall_back() {
        let s: Settings = serde_json::from_str(r#"{"palette":"nord","height":500,"theme":"neon"}"#).unwrap();
        let s = s.clamp();
        assert_eq!((s.palette.as_str(), s.height, s.theme.as_str(), s.media), ("nord", 44, "auto", true));
        let s: Settings = serde_json::from_str(r#"{"palette":"<script>","clock":"25","edge":"middle"}"#).unwrap();
        assert_eq!(s.clamp(), Settings::default());
    }

    #[test]
    fn side_bars_are_wider() {
        let s = Settings { edge: "left".into(), ..Default::default() };
        assert_eq!((s.thickness(), Settings::default().thickness()), (46, 30));
    }
}
