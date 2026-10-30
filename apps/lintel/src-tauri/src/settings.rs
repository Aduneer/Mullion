//! User settings, stored as JSON in %APPDATA%\Lintel\settings.json (beside Oriel's own folder).

use mullion_core::settings::{read_json, write_json, PALETTES};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// "auto" (follow Windows), "dark" or "light".
    pub theme: String,
    /// One of Oriel's palettes, so the two look alike.
    pub palette: String,
    /// Translucent bar and menus that blur what's behind them (acrylic), rather than solid ones.
    pub glass: bool,
    /// Which screens get a bar: "main", "all", or one screen by its device name (`\\.\DISPLAY2`).
    pub screens: String,
    /// With bars on several screens, each shows only the apps with a window on its own screen.
    pub apps_per_screen: bool,
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
    /// The running apps' icons, in the middle of the bar, as a taskbar shows them.
    pub apps: bool,
    /// Which items the bar shows, right to left after the clock.
    pub media: bool,
    pub stats: bool,
    pub volume: bool,
    pub network: bool,
    pub bluetooth: bool,
    /// The Control Centre: Wi-Fi, Bluetooth, brightness, sound and shortcuts in one menu.
    pub control: bool,
    pub battery: bool,
    /// Set Windows' taskbar to auto-hide while Lintel runs (see `taskbar`).
    pub hide_taskbar: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "auto".into(),
            palette: "oriel".into(),
            glass: false,
            screens: "main".into(),
            apps_per_screen: true,
            edge: "top".into(),
            height: 30,
            clock: "auto".into(),
            seconds: false,
            date: true,
            apps: true,
            media: true,
            stats: true,
            volume: true,
            network: true,
            bluetooth: true,
            control: true,
            battery: true,
            hide_taskbar: false,
        }
    }
}

pub fn path() -> PathBuf {
    mullion_core::settings::dir("Lintel").join("settings.json")
}

impl Settings {
    pub fn load() -> Self {
        read_json(&path()).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        write_json(&path(), self, true)
    }

    /// What the bar and its menus show: these settings, with Mullion's look for every app over
    /// Lintel's own while there is one (`mullion_core::look`).
    pub fn shown(&self) -> Self {
        let mut s = self.clone();
        mullion_core::look::overlay(&mut s.theme, &mut s.palette, Some(&mut s.glass));
        s
    }

    /// While Mullion's look stands in for Lintel's own, a save keeps `own`'s, so it comes back
    /// when that's turned off.
    pub fn keep_look(mut self, own: &Settings) -> Self {
        if mullion_core::look::shared().is_some() {
            self.theme = own.theme.clone();
            self.palette = own.palette.clone();
            self.glass = own.glass;
        }
        self
    }

    /// Anything a hand-edited file got wrong goes back to its default.
    pub fn clamp(mut self) -> Self {
        let d = Settings::default();
        if !(["main", "all"].contains(&self.screens.as_str()) || self.screens.starts_with(r"\\.\")) {
            self.screens = d.screens.clone();
        }
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
