//! What every app's windows do to look the part: follow the theme, and have Windows 11's corners.

use crate::ffi::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND};
use tauri::Theme;

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
    fn themes_follow_the_setting() {
        assert_eq!(native_theme("dark"), Some(Theme::Dark));
        assert_eq!(native_theme("light"), Some(Theme::Light));
        assert_eq!(native_theme("auto"), None);
    }
}
