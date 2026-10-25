//! The desktop wallpaper behind the bar, for its glass look: the bar paints a blurred copy of the
//! part of the wallpaper it covers (ui/bar.js), because Windows only blurs behind windows that have
//! the focus, and the bar never does. Since the bar reserves its strip of the screen, nothing but
//! the wallpaper is ever behind it.

use base64::Engine;
use serde::Serialize;
use std::sync::Mutex;
use windows::core::PCWSTR;
use windows::Win32::Foundation::RECT;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED};
use windows::Win32::UI::Shell::{DesktopWallpaper, IDesktopWallpaper, DESKTOP_WALLPAPER_POSITION};

/// Larger files aren't sent to the page; the bar stays solid-tinted instead.
const MAX_IMAGE: u64 = 40 * 1024 * 1024;

/// Where the wallpaper is and how Windows lays it out, all in physical pixels.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Wallpaper {
    /// Changes when the picture does: its path and when it was last written.
    pub key: String,
    /// The color around a picture that doesn't cover the screen, or of a plain-color desktop.
    pub color: String,
    /// "fill", "fit", "stretch", "center", "tile" or "span".
    pub position: &'static str,
    /// The screen the bar is on, the whole desktop (for "span"), and the bar.
    pub monitor: [i32; 4],
    pub desktop: [i32; 4],
    pub bar: [i32; 4],
}

fn xywh(r: RECT) -> [i32; 4] {
    [r.left, r.top, r.right - r.left, r.bottom - r.top]
}

/// The wallpaper of the screen `bar` is on, and the picture's path.
pub fn current(bar: crate::ffi::RECT) -> Option<(Wallpaper, Option<String>)> {
    let bar = RECT { left: bar.left, top: bar.top, right: bar.right, bottom: bar.bottom };
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let dw: IDesktopWallpaper = CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL).ok()?;
        let (cx, cy) = ((bar.left + bar.right) / 2, (bar.top + bar.bottom) / 2);
        let mut desktop = RECT { left: i32::MAX, top: i32::MAX, right: i32::MIN, bottom: i32::MIN };
        let mut found = None;
        for i in 0..dw.GetMonitorDevicePathCount().ok()? {
            let Ok(id) = dw.GetMonitorDevicePathAt(i) else { continue };
            let Ok(r) = dw.GetMonitorRECT(PCWSTR(id.0)) else {
                CoTaskMemFree(Some(id.0 as _));
                continue;
            };
            desktop = RECT {
                left: desktop.left.min(r.left),
                top: desktop.top.min(r.top),
                right: desktop.right.max(r.right),
                bottom: desktop.bottom.max(r.bottom),
            };
            if found.is_none() && (r.left..r.right).contains(&cx) && (r.top..r.bottom).contains(&cy) {
                let path = dw.GetWallpaper(PCWSTR(id.0)).ok().and_then(|p| {
                    let s = p.to_string().ok();
                    CoTaskMemFree(Some(p.0 as _));
                    s
                });
                found = Some((r, path.filter(|p| !p.is_empty())));
            }
            CoTaskMemFree(Some(id.0 as _));
        }
        let (monitor, path) = found?;
        let c = dw.GetBackgroundColor().map(|c| c.0).unwrap_or(0);
        let color = format!("#{:02x}{:02x}{:02x}", c & 0xff, (c >> 8) & 0xff, (c >> 16) & 0xff);
        let position = position_name(dw.GetPosition().unwrap_or(DESKTOP_WALLPAPER_POSITION(4)).0);
        let modified = path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok()?.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        let key = format!("{}|{modified}", path.as_deref().unwrap_or(""));
        Some((Wallpaper { key, color, position, monitor: xywh(monitor), desktop: xywh(desktop), bar: xywh(bar) }, path))
    }
}

fn position_name(n: i32) -> &'static str {
    match n {
        0 => "center",
        1 => "tile",
        2 => "stretch",
        3 => "fit",
        5 => "span",
        _ => "fill",
    }
}

/// The picture as a data: URL the page can draw, kept until the wallpaper changes.
pub struct Image(Mutex<Option<(String, Option<String>)>>);

impl Image {
    pub fn new() -> Self {
        Image(Mutex::new(None))
    }

    pub fn get(&self, key: &str, path: Option<&str>) -> Option<String> {
        let mut cache = self.0.lock().unwrap();
        if let Some((k, url)) = cache.as_ref() {
            if k == key {
                return url.clone();
            }
        }
        let url = path.and_then(data_url);
        *cache = Some((key.to_string(), url.clone()));
        url
    }
}

fn data_url(path: &str) -> Option<String> {
    let mime = match path.rsplit('.').next()?.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" | "jfif" => "image/jpeg",
        "png" => "image/png",
        "bmp" | "dib" => "image/bmp",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return None,
    };
    if std::fs::metadata(path).ok()?.len() > MAX_IMAGE {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_by_windows_number() {
        assert_eq!(["center", "tile", "stretch", "fit", "fill", "span"], [0, 1, 2, 3, 4, 5].map(position_name));
    }

    #[test]
    fn reads_this_pcs_wallpaper() {
        // Whatever this PC has: a screen at least, and a picture or a plain color.
        let (w, _) = current(crate::ffi::RECT { left: 10, top: 10, right: 20, bottom: 20 }).expect("a wallpaper");
        assert!(w.monitor[2] > 0 && w.desktop[2] >= w.monitor[2]);
        assert!(w.color.starts_with('#') && w.color.len() == 7);
    }
}
