//! The tray's icons as the dock shows them: each program's picture drawn into an image the page
//! can show, and which ones Windows' own settings keep on the taskbar ("promoted") rather than in
//! the flyout of hidden icons, so the dock shows the same few and keeps the rest under "more".
//! All of it off the host's thread, which only says something changed.

use crate::ffi::*;
use crate::host::{self, TrayIcon};
use std::collections::HashMap;

/// Drawn at twice the size the dock shows them at 100%, so they're sharp up to 200%.
const PX: i32 = 32;

/// An icon in the dock: the host's, with its picture, and whether it's shown or under "more".
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct Shown {
    #[serde(flatten)]
    pub icon: TrayIcon,
    pub promoted: bool,
}

/// The pictures already drawn, by icon handle: programs make a new handle for a new picture.
#[derive(Default)]
pub struct Pictures(HashMap<isize, Option<String>>);

impl Pictures {
    /// The host's icons now, drawn.
    pub fn list(&mut self) -> Vec<Shown> {
        let promoted = promoted();
        let icons = host::list();
        self.0.retain(|h, _| icons.iter().any(|i| i.hicon == *h));
        icons
            .into_iter()
            .map(|mut icon| {
                icon.icon = self.0.entry(icon.hicon).or_insert_with(|| draw(icon.hicon)).clone();
                let promoted = promoted.get(&icon.exe).copied().unwrap_or(true);
                Shown { icon, promoted }
            })
            .collect()
    }
}

/// An HICON (any process's: they're shared) as a data: URL.
fn draw(hicon: isize) -> Option<String> {
    let bgra = pixels(hicon, DI_NORMAL)?;
    // An icon without an alpha channel draws all transparent: its mask says what's there.
    let bgra = if bgra.chunks(4).all(|p| p[3] == 0) {
        let mask = pixels(hicon, DI_MASK)?;
        bgra.chunks(4).zip(mask.chunks(4)).flat_map(|(p, m)| [p[0], p[1], p[2], if m[0] == 0 { 255 } else { 0 }]).collect()
    } else {
        bgra
    };
    bgra.chunks(4).any(|p| p[3] > 0).then(|| mullion_core::icons::pixels_url(PX as u32, PX as u32, bgra))
}

fn pixels(hicon: isize, how: u32) -> Option<Vec<u8>> {
    unsafe {
        let dc = CreateCompatibleDC(0);
        if dc == 0 {
            return None;
        }
        let info = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: PX,
            biHeight: -PX, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let bmp = CreateDIBSection(dc, &info, 0, &mut bits, 0, 0);
        if bmp == 0 || bits.is_null() {
            DeleteDC(dc);
            return None;
        }
        let old = SelectObject(dc, bmp);
        let ok = DrawIconEx(dc, 0, 0, hicon, PX, PX, 0, 0, how) != 0;
        GdiFlush();
        let out = ok.then(|| std::slice::from_raw_parts(bits as *const u8, (PX * PX * 4) as usize).to_vec());
        SelectObject(dc, old);
        DeleteObject(bmp);
        DeleteDC(dc);
        out
    }
}

const SETTINGS: &str = r"Control Panel\NotifyIconSettings";

/// Which programs' icons Windows keeps on the taskbar, by file name (lowercased). A program with
/// several entries (it's moved, or had several icons) counts as shown if any says so; one with
/// none is shown.
fn promoted() -> HashMap<String, bool> {
    let mut out: HashMap<String, bool> = HashMap::new();
    unsafe {
        let mut key = 0isize;
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide(SETTINGS).as_ptr(), 0, KEY_READ, &mut key) != 0 {
            return out;
        }
        for i in 0.. {
            let mut name = [0u16; 128];
            let mut len = name.len() as u32;
            if RegEnumKeyExW(key, i, name.as_mut_ptr(), &mut len, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) != 0 {
                break;
            }
            let sub = &name[..len as usize + 1];
            let Some(path) = string(key, sub, "ExecutablePath") else { continue };
            let Some(exe) = path.rsplit(['\\', '/']).next().map(str::to_lowercase) else { continue };
            let yes = dword(key, sub, "IsPromoted").unwrap_or(0) != 0;
            *out.entry(exe).or_insert(false) |= yes;
        }
        RegCloseKey(key);
    }
    out
}

unsafe fn string(key: isize, sub: &[u16], value: &str) -> Option<String> {
    let mut buf = [0u16; 520];
    let mut len = (buf.len() * 2) as u32;
    let ok = RegGetValueW(key, sub.as_ptr(), wide(value).as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut len) == 0;
    ok.then(|| from_wide(&buf))
}

unsafe fn dword(key: isize, sub: &[u16], value: &str) -> Option<u32> {
    let mut v = 0u32;
    let mut len = 4u32;
    let ok = RegGetValueW(key, sub.as_ptr(), wide(value).as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), (&mut v as *mut u32).cast(), &mut len) == 0;
    ok.then_some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_which_icons_windows_shows() {
        // Whatever this PC has: names only, lowercased.
        for exe in promoted().keys() {
            assert_eq!(exe, &exe.to_lowercase());
            assert!(!exe.contains('\\'));
        }
    }

    #[test]
    fn a_handle_that_isnt_an_icon_draws_nothing() {
        assert_eq!(draw(0), None);
    }
}
