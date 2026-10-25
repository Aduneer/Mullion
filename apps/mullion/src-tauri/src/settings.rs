//! Mullion's own settings, in %APPDATA%\Mullion\settings.json. Each app keeps its own beside it.

use mullion_core::settings::{dir, read_json, write_json};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The apps Mullion doesn't start, by name. Every other app it finds, it starts.
    pub off: Vec<String>,
    /// The welcome has been through: until then Mullion starts no apps, and opens the welcome
    /// even at sign-in. A file from before it existed was made by someone who's used Mullion.
    #[serde(default = "used_before")]
    pub welcomed: bool,
}

fn used_before() -> bool {
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

    pub fn set(&mut self, app: &str, on: bool) {
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
    fn only_someone_new_is_welcomed() {
        assert!(!Settings::default().welcomed);
        assert!(serde_json::from_str::<Settings>(r#"{"off":[]}"#).unwrap().welcomed);
        assert!(!serde_json::from_str::<Settings>(r#"{"welcomed":false}"#).unwrap().welcomed);
    }
}
