//! The dock's menus (an app's windows, Pin and Quit; the dock's own): Windows' own popup menus,
//! in the theme, opening off the dock's edge. They run on the main thread, which owns the dock.

use crate::ffi::*;

pub enum Entry {
    Item { text: String, checked: bool, enabled: bool },
    Separator,
}

pub fn item(text: impl Into<String>) -> Entry {
    Entry::Item { text: text.into(), checked: false, enabled: true }
}

/// Follow the theme: "dark", "light", or as Windows is.
pub fn theme(theme: &str) {
    let mode = match theme {
        "dark" => APPMODE_FORCE_DARK,
        "light" => APPMODE_FORCE_LIGHT,
        _ => APPMODE_ALLOW_DARK,
    };
    unsafe {
        SetPreferredAppMode(mode);
        FlushMenuThemes();
    }
}

/// Show `entries` from `owner`, at `at` (physical pixels) on the dock's `edge`, and wait for a
/// choice: the index of the item picked, if any.
pub fn popup(owner: isize, entries: &[Entry], at: POINT, edge: &str) -> Option<usize> {
    unsafe {
        let menu = CreatePopupMenu();
        if menu == 0 {
            return None;
        }
        for (i, e) in entries.iter().enumerate() {
            match e {
                Entry::Item { text, checked, enabled } => {
                    let flags = MF_STRING | if *checked { MF_CHECKED } else { 0 } | if *enabled { 0 } else { MF_GRAYED };
                    // Menus treat & as the start of a shortcut; a window's title may have one.
                    let text = wide(&text.replace('&', "&&"));
                    AppendMenuW(menu, flags, i + 1, text.as_ptr());
                }
                Entry::Separator => {
                    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
                }
            }
        }
        // Off the dock, towards the screen: above it at the bottom, beside it on a side.
        let align = match edge {
            "left" => TPM_VCENTERALIGN,
            "right" => TPM_RIGHTALIGN | TPM_VCENTERALIGN,
            _ => TPM_CENTERALIGN | TPM_BOTTOMALIGN,
        };
        // A menu only closes on a click elsewhere if its window is in front.
        SetForegroundWindow(owner);
        let picked = TrackPopupMenuEx(menu, align | TPM_RETURNCMD | TPM_RIGHTBUTTON, at.x, at.y, owner, std::ptr::null());
        PostMessageW(owner, WM_NULL, 0, 0);
        DestroyMenu(menu);
        (picked > 0).then(|| picked as usize - 1)
    }
}
