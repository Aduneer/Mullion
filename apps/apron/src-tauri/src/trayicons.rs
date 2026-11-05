//! The tray's icons as the dock and Lintel show them: each program's picture drawn into an image
//! a page can show, and which ones Windows' own settings keep on the taskbar ("promoted") rather
//! than in the flyout of hidden icons, so the dock shows the same few and keeps the rest under
//! "more". All of it off the host's thread, which only says something changed.

use crate::ffi::*;
use crate::host;
use mullion_core::tray::Icon;
use std::collections::HashMap;

/// Drawn at twice the size the dock shows them at 100%, so they're sharp up to 200%.
const PX: i32 = 32;

/// A picture as drawn: its data: URL, and whether it's a glyph in one colour.
type Picture = Option<(String, bool)>;

/// The pictures already drawn, by icon handle: programs make a new handle for a new picture.
#[derive(Default)]
pub struct Pictures(HashMap<isize, Picture>);

impl Pictures {
    /// The host's icons now, drawn.
    pub fn list(&mut self) -> Vec<Icon> {
        let promoted = promoted();
        let icons = host::list();
        self.0.retain(|h, _| icons.iter().any(|i| i.hicon == *h));
        icons
            .into_iter()
            .map(|i| {
                let picture = self.0.entry(i.hicon).or_insert_with(|| draw(i.hicon)).clone();
                Icon {
                    promoted: promoted.get(&i.exe).copied().unwrap_or(true),
                    glyph: picture.as_ref().is_some_and(|p| p.1),
                    icon: picture.map(|p| p.0),
                    id: i.id,
                    tip: i.tip,
                    exe: i.exe,
                    pid: i.pid,
                    tooltip: i.tooltip,
                }
            })
            .collect()
    }
}

/// Whether a picture (BGRA) is a glyph in one colour: every pixel that shows is gray and about
/// as light as the rest, as Windows' own and most programs' tray icons are (white for a dark
/// taskbar, black for a light one). Coloured pictures, and gray ones with shading, aren't.
fn is_glyph(bgra: &[u8]) -> bool {
    let shown: Vec<(f32, f32)> = bgra
        .chunks(4)
        .filter(|p| p[3] >= 96)
        .map(|p| {
            // Colours are premultiplied: undo that first.
            let a = p[3] as f32 / 255.0;
            let [b, g, r] = [p[0], p[1], p[2]].map(|c| (c as f32 / 255.0 / a).min(1.0));
            let (hi, lo) = (r.max(g).max(b), r.min(g).min(b));
            (hi - lo, 0.3 * r + 0.59 * g + 0.11 * b)
        })
        .collect();
    if shown.len() < 8 {
        return false;
    }
    let n = shown.len() as f32;
    let tint = shown.iter().map(|s| s.0).sum::<f32>() / n;
    let light = shown.iter().map(|s| s.1).sum::<f32>() / n;
    let spread = (shown.iter().map(|s| (s.1 - light).powi(2)).sum::<f32>() / n).sqrt();
    tint < 0.08 && spread < 0.12
}

/// An HICON (any process's: they're shared) as a data: URL, and whether it's a glyph.
fn draw(hicon: isize) -> Picture {
    let bgra = pixels(hicon, DI_NORMAL)?;
    // An icon without an alpha channel draws all transparent: its mask says what's there.
    let bgra = if bgra.chunks(4).all(|p| p[3] == 0) {
        let mask = pixels(hicon, DI_MASK)?;
        bgra.chunks(4).zip(mask.chunks(4)).flat_map(|(p, m)| [p[0], p[1], p[2], if m[0] == 0 { 255 } else { 0 }]).collect()
    } else {
        bgra
    };
    let glyph = is_glyph(&bgra);
    bgra.chunks(4).any(|p| p[3] > 0).then(|| (mullion_core::icons::pixels_url(PX as u32, PX as u32, bgra), glyph))
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

    #[test]
    fn one_colour_pictures_are_glyphs() {
        let px = |b: u8, g: u8, r: u8, a: u8| [b, g, r, a];
        let picture = |pixels: &[[u8; 4]]| pixels.iter().cycle().take(64).flatten().copied().collect::<Vec<u8>>();
        // White, and white with soft (premultiplied) edges.
        assert!(is_glyph(&picture(&[px(255, 255, 255, 255), px(128, 128, 128, 128), px(0, 0, 0, 0)])));
        // Black.
        assert!(is_glyph(&picture(&[px(0, 0, 0, 255), px(10, 10, 10, 255)])));
        // Steam's blue, and a gray with dark and light parts.
        assert!(!is_glyph(&picture(&[px(200, 120, 20, 255)])));
        assert!(!is_glyph(&picture(&[px(255, 255, 255, 255), px(20, 20, 20, 255)])));
        // Nothing much there.
        assert!(!is_glyph(&[0; 64]));
    }
}
