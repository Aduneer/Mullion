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
    /// The running apps' icons, in the middle of the bar, as a taskbar shows them. Unset until
    /// it's switched: then on, unless Mullion runs Apron, whose dock shows them (`apps_default`).
    /// What's shown to the page and its menus is always set (`shown`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apps: Option<bool>,
    /// `apps` was switched by hand. Lintel used to save `"apps": true` whether or not it was, so
    /// a file without this has it unset (`load`), and only a `false` is kept.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub apps_chosen: bool,
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
            apps: None,
            apps_chosen: false,
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

/// The open apps are on, unless Mullion runs Apron too: its dock has them, and two rows of the
/// same icons would be one too many.
pub fn apps_default() -> bool {
    !mullion_core::suite::runs("Apron")
}

pub fn path() -> PathBuf {
    mullion_core::settings::dir("Lintel").join("settings.json")
}

impl Settings {
    pub fn load() -> Self {
        read_json::<Settings>(&path()).unwrap_or_default().settled()
    }

    /// A `"apps": true` that was never switched is what Lintel used to save for everyone: unset.
    fn settled(mut self) -> Self {
        if !self.apps_chosen && self.apps == Some(true) {
            self.apps = None;
        }
        self
    }

    pub fn save(&self) -> std::io::Result<()> {
        write_json(&path(), self, true)
    }

    /// What the bar and its menus show: these settings, with Mullion's look for every app over
    /// Lintel's own while there is one (`mullion_core::look`).
    pub fn shown(&self) -> Self {
        let mut s = self.clone();
        mullion_core::look::overlay(&mut s.theme, &mut s.palette, Some(&mut s.glass));
        s.apps = Some(self.apps.unwrap_or_else(apps_default));
        s
    }

    /// A save from the page sends every setting as it was shown; the open apps stay unset unless
    /// they were switched, so they keep following Apron.
    pub fn keep_defaults(mut self, own: &Settings) -> Self {
        if own.apps.is_none() && self.apps == Some(apps_default()) {
            self.apps = None;
        }
        self.apps_chosen = self.apps.is_some();
        self
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
    fn open_apps_follow_apron_until_switched() {
        // Run on its own (as tests are), the default is on.
        let s = Settings::default();
        assert_eq!((s.apps, s.shown().apps), (None, Some(true)));
        // A save of what was shown keeps it unset; a switch sets it, either way.
        assert_eq!(s.shown().keep_defaults(&s).apps, None);
        let off = Settings { apps: Some(false), ..s.shown() };
        assert_eq!(off.keep_defaults(&s).apps, Some(false));
        let on = Settings { apps: Some(true), ..Default::default() };
        assert_eq!(on.clone().keep_defaults(&on).apps, Some(true));
        // A switch is saved as one.
        let saved = Settings { apps: Some(true), ..Default::default() }.keep_defaults(&on);
        assert!(saved.apps_chosen);
        let back: Settings = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
        assert_eq!(back.settled().apps, Some(true));
        // Files from before it could be unset have it on for everyone: that's unset. Off stays off.
        let old: Settings = serde_json::from_str(r#"{"apps":true}"#).unwrap();
        assert_eq!(old.settled().apps, None);
        let old: Settings = serde_json::from_str(r#"{"apps":false}"#).unwrap();
        assert_eq!(old.settled().apps, Some(false));
        let plain = serde_json::to_string(&Settings::default()).unwrap();
        assert!(!plain.contains("\"apps\"") && !plain.contains("apps_chosen"));
    }

    #[test]
    fn side_bars_are_wider() {
        let s = Settings { edge: "left".into(), ..Default::default() };
        assert_eq!((s.thickness(), Settings::default().thickness()), (46, 30));
    }
}
