//! Mullion's own settings, in %APPDATA%\Mullion\settings.json. Each app keeps its own beside it.

use mullion_core::settings::{dir, read_json, write_json};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The apps Mullion doesn't start, by name. Every other app it finds, it starts.
    pub off: Vec<String>,
    /// The welcome has been through: until then Mullion starts no apps, and opens the welcome
    /// even at sign-in. A file from before it existed was made by someone who's used Mullion.
    #[serde(default = "yes")]
    pub welcomed: bool,
    /// Ask GitHub once a day whether there's a newer Mullion (`updates`). On unless turned off.
    #[serde(default = "yes")]
    pub check_updates: bool,
    /// Apps an update brought, left off and marked new until they're first turned on.
    pub new: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { off: Vec::new(), welcomed: false, check_updates: true, new: Vec::new() }
    }
}

/// Where Mullion's installer lists the apps an update brought (installer/mullion.nsi).
pub fn new_apps_path() -> PathBuf {
    dir("Mullion").join("new-apps.txt")
}

fn yes() -> bool {
    true
}

pub fn path() -> PathBuf {
    dir("Mullion").join("settings.json")
}

impl Settings {
    pub fn load() -> Self {
        read_json(&path()).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        write_json(&path(), self, true)
    }

    pub fn is_on(&self, app: &str) -> bool {
        !self.off.iter().any(|a| a == app)
    }

    /// The apps an update brought (`list`, one name per line): off, and new, so trying one is a
    /// choice (Apron takes the taskbar's place). Someone still to be welcomed picks them there.
    pub fn arrived(&mut self, list: &str) {
        if !self.welcomed {
            return;
        }
        for name in list.lines().map(str::trim) {
            let Some(a) = mullion_core::suite::APPS.iter().find(|a| a.name == name) else { continue };
            if !self.new.iter().any(|n| n == a.name) {
                self.set(a.name, false);
                self.new.push(a.name.to_string());
            }
        }
    }

    pub fn set(&mut self, app: &str, on: bool) {
        self.new.retain(|a| a != app || !on);
        self.off.retain(|a| a != app);
        if !on {
            self.off.push(app.to_string());
            self.off.sort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apps_are_on_until_turned_off() {
        let mut s = Settings::default();
        assert!(s.is_on("Lintel"));
        s.set("Lintel", false);
        s.set("Lintel", false);
        assert_eq!(s.off, ["Lintel"]);
        assert!(!s.is_on("Lintel") && s.is_on("Fanlight"));
        s.set("Lintel", true);
        assert!(s.is_on("Lintel") && s.off.is_empty());
        let old: Settings = serde_json::from_str(r#"{"off":["Oriel"],"gone":1}"#).unwrap();
        assert!(!old.is_on("Oriel"));
    }

    #[test]
    fn apps_an_update_brings_arrive_off_until_turned_on() {
        let mut s = Settings { welcomed: true, ..Default::default() };
        s.arrived("Apron
Not an app

");
        assert_eq!((s.off.clone(), s.new.clone()), (vec!["Apron".to_string()], vec!["Apron".to_string()]));
        // Listed again by a second update: still the once.
        s.arrived("Apron
");
        assert_eq!(s.new, ["Apron"]);
        s.set("Apron", true);
        assert!(s.is_on("Apron") && s.new.is_empty());
        // Before the welcome, it's for the welcome to say.
        let mut fresh = Settings::default();
        fresh.arrived("Apron");
        assert!(fresh.is_on("Apron") && fresh.new.is_empty());
    }

    #[test]
    fn only_someone_new_is_welcomed() {
        assert!(!Settings::default().welcomed);
        assert!(serde_json::from_str::<Settings>(r#"{"off":[]}"#).unwrap().welcomed);
        assert!(!serde_json::from_str::<Settings>(r#"{"welcomed":false}"#).unwrap().welcomed);
    }

    #[test]
    fn updates_are_checked_unless_turned_off() {
        assert!(Settings::default().check_updates);
        assert!(serde_json::from_str::<Settings>(r#"{"off":[]}"#).unwrap().check_updates);
        assert!(!serde_json::from_str::<Settings>(r#"{"check_updates":false}"#).unwrap().check_updates);
    }
}
