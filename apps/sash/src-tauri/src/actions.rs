//! What Sash can do to the window in front, each with its own shortcut. The first ones have a
//! shortcut from the start; the rest are there to be given one in Settings.

use crate::layout::{Dir, Place, Span, FIRST_HALF, FULL, SECOND_HALF};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Do {
    /// An arrow: that half, then two-thirds and a third, or a quarter (`layout::step`).
    Arrow(Dir),
    /// Always this place.
    Put(Place),
    Maximize,
    Centre,
    /// Back to where it was before Sash moved it.
    Restore,
    /// The next screen (1) or the one before (-1), in the same place there.
    Screen(i32),
}

#[derive(Serialize)]
pub struct Action {
    pub id: &'static str,
    pub name: &'static str,
    /// The settings page's heading it goes under.
    pub group: &'static str,
    /// Its shortcut until you change it; empty for none.
    pub key: &'static str,
    #[serde(skip)]
    pub does: Do,
}

const fn put(h: Span, v: Span) -> Do {
    Do::Put(Place { h, v })
}

/// Ctrl+Alt with the arrows, and with keys that type nothing: on keyboards with AltGr, Ctrl+Alt
/// and a letter or digit is AltGr and that key, which types € or @ or ă, so none is taken unasked.
pub const ACTIONS: &[Action] = &[
    Action { id: "left", name: "Left half", group: "Halves", key: "ctrl+alt+left", does: Do::Arrow(Dir::Left) },
    Action { id: "right", name: "Right half", group: "Halves", key: "ctrl+alt+right", does: Do::Arrow(Dir::Right) },
    Action { id: "top", name: "Top half", group: "Halves", key: "ctrl+alt+up", does: Do::Arrow(Dir::Up) },
    Action { id: "bottom", name: "Bottom half", group: "Halves", key: "ctrl+alt+down", does: Do::Arrow(Dir::Down) },
    Action { id: "maximize", name: "Maximize", group: "The whole screen", key: "ctrl+alt+enter", does: Do::Maximize },
    Action { id: "centre", name: "Centre", group: "The whole screen", key: "ctrl+alt+space", does: Do::Centre },
    Action { id: "restore", name: "Restore", group: "The whole screen", key: "ctrl+alt+backspace", does: Do::Restore },
    Action { id: "next-screen", name: "Next screen", group: "The whole screen", key: "ctrl+alt+shift+right", does: Do::Screen(1) },
    Action { id: "previous-screen", name: "Previous screen", group: "The whole screen", key: "ctrl+alt+shift+left", does: Do::Screen(-1) },
    Action { id: "first-third", name: "Left third", group: "Thirds", key: "", does: put(Span(0, 2), FULL) },
    Action { id: "middle-third", name: "Middle third", group: "Thirds", key: "", does: put(Span(2, 4), FULL) },
    Action { id: "last-third", name: "Right third", group: "Thirds", key: "", does: put(Span(4, 6), FULL) },
    Action { id: "first-two-thirds", name: "Left two-thirds", group: "Thirds", key: "", does: put(Span(0, 4), FULL) },
    Action { id: "last-two-thirds", name: "Right two-thirds", group: "Thirds", key: "", does: put(Span(2, 6), FULL) },
    Action { id: "top-left", name: "Top-left quarter", group: "Quarters", key: "", does: put(FIRST_HALF, FIRST_HALF) },
    Action { id: "top-right", name: "Top-right quarter", group: "Quarters", key: "", does: put(SECOND_HALF, FIRST_HALF) },
    Action { id: "bottom-left", name: "Bottom-left quarter", group: "Quarters", key: "", does: put(FIRST_HALF, SECOND_HALF) },
    Action { id: "bottom-right", name: "Bottom-right quarter", group: "Quarters", key: "", does: put(SECOND_HALF, SECOND_HALF) },
];

/// An action's shortcut id, for `Hotkeys`: its place in `ACTIONS`, from 1.
pub fn hotkey_id(id: &str) -> Option<i32> {
    ACTIONS.iter().position(|a| a.id == id).map(|i| i as i32 + 1)
}

pub fn by_hotkey_id(n: i32) -> Option<&'static Action> {
    usize::try_from(n - 1).ok().and_then(|i| ACTIONS.get(i))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_is_a_shortcut_and_no_two_share_one() {
        let mut seen = std::collections::HashSet::new();
        for a in ACTIONS {
            assert!(a.key.is_empty() || mullion_core::hotkey::parse(a.key).is_some(), "{}", a.id);
            assert!(a.key.is_empty() || seen.insert(a.key), "{} twice", a.key);
            assert_eq!(by_hotkey_id(hotkey_id(a.id).unwrap()).unwrap().id, a.id);
        }
        assert!(by_hotkey_id(0).is_none());
    }

    #[test]
    fn no_default_is_altgr_and_a_key_that_types() {
        for a in ACTIONS.iter().filter(|a| !a.key.is_empty()) {
            let key = a.key.rsplit('+').next().unwrap();
            assert!(!(key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric()), "{}", a.key);
        }
    }
}
