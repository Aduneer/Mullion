//! The desktop's wallpaper on each screen, for the overview's backdrop: which picture, how
//! Windows fits it, and the colour around it. The page blurs and dims it.
//!
//! The picture itself reaches the page through the `wallpaper` URI scheme (main.rs), by screen
//! number, so the page never names a file.

use serde::Serialize;
use std::path::PathBuf;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL};
use windows::Win32::UI::Shell::{DesktopWallpaper, IDesktopWallpaper, DWPOS_CENTER, DWPOS_FILL, DWPOS_FIT, DWPOS_SPAN, DWPOS_STRETCH, DWPOS_TILE};

/// One screen's wallpaper, as Windows has it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Paper {
    /// The screen, in physical pixels.
    pub monitor: [i32; 4],
    pub file: Option<PathBuf>,
}

/// Every screen's wallpaper, and what the page needs to draw it.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Desktop {
    #[serde(skip)]
    pub papers: Vec<Paper>,
    /// "fill", "fit", "stretch", "center", "tile" or "span".
    pub fit: &'static str,
    /// Around a picture that doesn't cover the screen, or instead of one: "#rrggbb".
    pub color: String,
}

/// Ask Windows (Explorer, through IDesktopWallpaper). The calling thread must have COM
/// initialized.
pub fn read() -> Desktop {
    let mut desk = Desktop { fit: "fill", color: "#000000".into(), ..Default::default() };
    unsafe {
        let Ok(dw) = CoCreateInstance::<_, IDesktopWallpaper>(&DesktopWallpaper, None, CLSCTX_ALL) else { return desk };
        if let Ok(pos) = dw.GetPosition() {
            desk.fit = match pos {
                DWPOS_CENTER => "center",
                DWPOS_TILE => "tile",
                DWPOS_STRETCH => "stretch",
                DWPOS_FIT => "fit",
                DWPOS_SPAN => "span",
                DWPOS_FILL => "fill",
                _ => "fill",
            };
        }
        if let Ok(c) = dw.GetBackgroundColor() {
            desk.color = format!("#{:02x}{:02x}{:02x}", c.0 & 0xff, (c.0 >> 8) & 0xff, (c.0 >> 16) & 0xff);
        }
        let count = dw.GetMonitorDevicePathCount().unwrap_or(0);
        for i in 0..count {
            let Ok(id) = dw.GetMonitorDevicePathAt(i) else { continue };
            let monitor = dw.GetMonitorRECT(PCWSTR(id.0)).ok();
            let file = dw.GetWallpaper(PCWSTR(id.0)).ok().map(|s| take(s));
            CoTaskMemFree(Some(id.0 as _));
            let Some(m) = monitor else { continue };
            let file = file.filter(|f| !f.is_empty()).map(PathBuf::from).filter(|p| p.is_file()).or_else(transcoded);
            desk.papers.push(Paper { monitor: [m.left, m.top, m.right, m.bottom], file });
        }
    }
    desk
}

/// A string COM handed over, freed.
unsafe fn take(s: PWSTR) -> String {
    let text = s.to_string().unwrap_or_default();
    CoTaskMemFree(Some(s.0 as _));
    text
}

/// The copy Windows keeps of the picture it last set: for a slideshow, or a wallpaper whose
/// own file has gone.
fn transcoded() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var_os("APPDATA")?).join(r"Microsoft\Windows\Themes\TranscodedWallpaper");
    p.is_file().then_some(p)
}

/// What kind of picture `bytes` is, for its Content-Type; TranscodedWallpaper has no extension.
pub fn mime(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0xff, 0xd8, 0xff, ..] => Some("image/jpeg"),
        [0x89, b'P', b'N', b'G', ..] => Some("image/png"),
        [b'B', b'M', ..] => Some("image/bmp"),
        [b'G', b'I', b'F', ..] => Some("image/gif"),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some("image/webp"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_pictures_are_served() {
        assert_eq!(mime(&[0xff, 0xd8, 0xff, 0xe0]), Some("image/jpeg"));
        assert_eq!(mime(b"\x89PNG\r\n"), Some("image/png"));
        assert_eq!(mime(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(mime(b"<html>"), None);
        assert_eq!(mime(b""), None);
    }
}
