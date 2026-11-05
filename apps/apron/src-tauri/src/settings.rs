//! User settings, stored as JSON in %APPDATA%\Apron\settings.json (beside the other apps' folders).

use mullion_core::settings::{read_json, write_json, PALETTES};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The dock's sizes: the icons' size at 100% scaling, in pixels.
pub const SIZES: &[(&str, u32)] = &[("small", 32), ("medium", 40), ("large", 52)];
pub const EDGES: &[&str] = &["bottom", "left", "right"];

/// An app kept in the dock, running or not.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Pin {
    /// The program's path, lowercased, as the running apps are known (`mullion_core::apps::App`).
    pub key: String,
    pub name: String,
    /// What to open: the program, a shortcut, or a Store app (`shell:AppsFolder\…`).
    pub launch: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// "auto" (follow Windows), "dark" or "light".
    pub theme: String,
    /// One of the suite's palettes.
    pub palette: String,
    /// See-through: the wallpaper, blurred, behind the dock.
    pub glass: bool,
    /// The screen it's on, as Windows names it ("\\.\DISPLAY2"); empty for the main screen, which
    /// it's on too while the one named isn't plugged in.
    pub screen: String,
    /// The screen edge it sits on: "bottom", "left" or "right".
    pub edge: String,
    /// "small", "medium" or "large" (`SIZES`).
    pub size: String,
    /// Float and auto-hide: keep Windows' taskbar, and float the dock above it while the pointer
    /// is at the edge. Off, the dock takes the taskbar's place and the taskbar is hidden.
    pub float: bool,
    /// Keep the notification area's icons in the dock while Lintel shows them on its bar too.
    /// Off, they're only on Lintel's bar then, so nothing shows twice.
    pub tray_too: bool,
    /// The apps kept in the dock, in order.
    pub pins: Vec<Pin>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "auto".into(),
            palette: "oriel".into(),
            glass: false,
            screen: String::new(),
            edge: "bottom".into(),
            size: "medium".into(),
            float: false,
            tray_too: false,
            pins: Vec::new(),
        }
    }
}

pub fn dir() -> PathBuf {
    mullion_core::settings::dir("Apron")
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

    /// What the dock and its settings show: these settings, with Mullion's look for every app
    /// over Apron's own while there is one (`mullion_core::look`).
    pub fn shown(&self) -> Self {
        let mut s = self.clone();
        mullion_core::look::overlay(&mut s.theme, &mut s.palette, Some(&mut s.glass));
        s
    }

    /// While Mullion's look stands in for Apron's own, a save keeps `own`'s, so it comes back when
    /// that's turned off.
    pub fn keep_look(mut self, own: &Settings) -> Self {
        if mullion_core::look::shared().is_some() {
            self.theme = own.theme.clone();
            self.palette = own.palette.clone();
            self.glass = own.glass;
        }
        self
    }

    /// Anything a hand-edited file got wrong goes back to its default; a pin without a program
    /// is dropped, and so is a second pin of the same app.
    pub fn clamp(mut self) -> Self {
        let d = Settings::default();
        if !["auto", "dark", "light"].contains(&self.theme.as_str()) {
            self.theme = d.theme;
        }
        if !PALETTES.contains(&self.palette.as_str()) {
            self.palette = d.palette;
        }
        if !EDGES.contains(&self.edge.as_str()) {
            self.edge = d.edge;
        }
        if !SIZES.iter().any(|(s, _)| *s == self.size) {
            self.size = d.size;
        }
        let mut seen = Vec::new();
        self.pins.retain(|p| !p.key.is_empty() && !p.launch.is_empty() && !seen.contains(&p.key) && {
            seen.push(p.key.clone());
            true
        });
        self
    }

    /// The icons' size at 100% scaling.
    pub fn icon_px(&self) -> u32 {
        SIZES.iter().find(|(s, _)| *s == self.size).map_or(40, |(_, px)| *px)
    }

    /// How far the dock reaches into the screen at 100% scaling: the icon, the shelf's padding
    /// around it, room for the dots, and a little air between the shelf and the edge.
    pub fn thickness(&self) -> u32 {
        self.icon_px() + 26
    }

    pub fn is_pinned(&self, key: &str) -> bool {
        self.pins.iter().any(|p| p.key == key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_wrong_values_fall_back() {
        let s: Settings = serde_json::from_str(r#"{"palette":"nord","theme":"neon","edge":"top","size":"huge","float":true}"#).unwrap();
        let s = s.clamp();
        assert_eq!((s.palette.as_str(), s.theme.as_str(), s.edge.as_str(), s.size.as_str(), s.float), ("nord", "auto", "bottom", "medium", true));
        let s: Settings = serde_json::from_str(r#"{"palette":"<script>"}"#).unwrap();
        assert_eq!(s.clamp(), Settings::default());
    }

    #[test]
    fn pins_are_kept_tidy() {
        let pin = |key: &str, launch: &str| Pin { key: key.into(), name: "x".into(), launch: launch.into() };
        let s = Settings { pins: vec![pin("a", "a.exe"), pin("", "b.exe"), pin("a", "a2.exe"), pin("c", "")], ..Default::default() };
        let s = s.clamp();
        assert_eq!(s.pins, vec![pin("a", "a.exe")]);
        assert!(s.is_pinned("a") && !s.is_pinned("c"));
    }

    #[test]
    fn sizes_and_thickness() {
        let s = Settings::default();
        assert_eq!((s.icon_px(), s.thickness()), (40, 66));
        let large = Settings { size: "large".into(), ..Default::default() };
        assert_eq!(large.icon_px(), 52);
    }
}
