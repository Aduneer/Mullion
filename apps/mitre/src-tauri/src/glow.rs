//! The glow: a soft quarter-circle of the palette's accent that grows in the corner while the
//! pointer waits there, so a push feels noticed and a swipe through only flickers. Flashes as the
//! corner fires, and fades if the pointer leaves first.
//!
//! A native layered window, click-through, never active, above everything, drawn a pixel at a
//! time into a small bitmap: no page and no browser for it. It lives on the pointer's thread
//! (`watch`), whose timer moves it on a frame at a time, and only while it's on screen.

use crate::corners::{Corner, Screen};
use crate::ffi::*;
use std::ffi::c_void;
use std::ptr::null_mut;
use std::time::Instant;

/// The glow's full reach at 100% scaling, in pixels, and how much more it reaches as it flashes.
const REACH: f64 = 84.0;
const FLASH_REACH: f64 = 1.3;
/// How strong it is at its fullest (0–1), and how long the flash and the fade take, in ms.
const STRENGTH: f64 = 0.8;
const FLASH_MS: f64 = 260.0;
const FADE_MS: f64 = 140.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Hidden,
    /// Growing over the dwell, `ms` long.
    Growing(f64),
    Flash,
    /// Fading from how far it had grown.
    Fading(f64),
}

pub struct Glow {
    hwnd: isize,
    phase: Phase,
    since: Instant,
    /// Where it is: the window's top-left, its side, and which corner of it is the screen's.
    at: (i32, i32),
    side: i32,
    corner: Corner,
    color: [u8; 3],
    dc: isize,
    bitmap: isize,
    bits: *mut u32,
}

impl Glow {
    /// The glow's window, on the calling thread; hidden until `grow`.
    pub fn new(class: &[u16], instance: isize) -> Glow {
        let ex = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        let hwnd = unsafe { CreateWindowExW(ex, class.as_ptr(), class.as_ptr(), WS_POPUP, 0, 0, 0, 0, 0, 0, instance, null_mut()) };
        Glow { hwnd, phase: Phase::Hidden, since: Instant::now(), at: (0, 0), side: 0, corner: Corner::TopLeft, color: [0; 3], dc: 0, bitmap: 0, bits: null_mut() }
    }

    /// Start growing in `corner` of `screen`, to its fullest in `dwell_ms`.
    pub fn grow(&mut self, screen: &Screen, corner: Corner, dwell_ms: u32, color: [u8; 3]) {
        let side = (REACH * FLASH_REACH * screen.dpi as f64 / 96.0).ceil() as i32;
        let r = screen.rect;
        let x = if corner.left() { r.left } else { r.right - side };
        let y = if corner.top() { r.top } else { r.bottom - side };
        if side != self.side {
            self.free();
            self.side = side;
        }
        (self.at, self.corner, self.color) = ((x, y), corner, color);
        self.phase = Phase::Growing(dwell_ms.max(1) as f64);
        self.since = Instant::now();
        self.draw();
        unsafe {
            SetWindowPos(self.hwnd, HWND_TOPMOST, x, y, side, side, SWP_NOACTIVATE);
            ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
        }
    }

    /// The corner fired.
    pub fn flash(&mut self) {
        if self.phase != Phase::Hidden {
            self.phase = Phase::Flash;
            self.since = Instant::now();
        }
    }

    /// The pointer left before the corner fired.
    pub fn fade(&mut self) {
        if let Phase::Growing(ms) = self.phase {
            self.phase = Phase::Fading((self.since.elapsed().as_secs_f64() * 1000.0 / ms).min(1.0));
            self.since = Instant::now();
        }
    }

    /// Gone at once (a button pressed, the screens changed).
    pub fn hide(&mut self) {
        if self.phase != Phase::Hidden {
            self.phase = Phase::Hidden;
            unsafe { ShowWindow(self.hwnd, SW_HIDE) };
        }
    }

    /// The next frame. False once it's gone, so the timer can stop.
    pub fn tick(&mut self) -> bool {
        if self.phase == Phase::Hidden {
            return false;
        }
        if self.size_and_strength().is_none() {
            self.hide();
            self.free();
            return false;
        }
        self.draw();
        true
    }

    /// How far it reaches (of `REACH`) and how strong it is now, or None when it's done.
    fn size_and_strength(&self) -> Option<(f64, f64)> {
        let ms = self.since.elapsed().as_secs_f64() * 1000.0;
        let ease = |t: f64| 1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3);
        match self.phase {
            Phase::Hidden => None,
            Phase::Growing(dwell) => {
                let t = ease(ms / dwell);
                Some((0.35 + 0.65 * t, STRENGTH * t))
            }
            Phase::Flash => {
                let t = ms / FLASH_MS;
                (t < 1.0).then(|| (1.0 + (FLASH_REACH - 1.0) * ease(t), (STRENGTH + 0.2).min(1.0) * (1.0 - t)))
            }
            Phase::Fading(from) => {
                let t = ms / FADE_MS;
                (t < 1.0).then_some((0.35 + 0.65 * from, STRENGTH * from * (1.0 - t)))
            }
        }
    }

    fn draw(&mut self) {
        let Some((reach, strength)) = self.size_and_strength() else { return };
        if self.bits.is_null() && !self.make_bitmap() {
            return;
        }
        let side = self.side;
        let radius = reach * side as f64 / FLASH_REACH;
        let pixels = unsafe { std::slice::from_raw_parts_mut(self.bits, (side * side) as usize) };
        let [r, g, b] = self.color.map(|c| c as f64);
        for y in 0..side {
            // The bitmap is top-down; measure from the corner that's the screen's.
            let dy = if self.corner.top() { y } else { side - 1 - y } as f64 + 0.5;
            for x in 0..side {
                let dx = if self.corner.left() { x } else { side - 1 - x } as f64 + 0.5;
                let d = (dx * dx + dy * dy).sqrt() / radius;
                let a = if d >= 1.0 { 0.0 } else { strength * (1.0 - d).powf(1.3) };
                // Premultiplied BGRA, as UpdateLayeredWindow wants it.
                let px = |c: f64| (c * a).round() as u32;
                pixels[(y * side + x) as usize] = ((a * 255.0).round() as u32) << 24 | px(r) << 16 | px(g) << 8 | px(b);
            }
        }
        let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA };
        let pos = POINT { x: self.at.0, y: self.at.1 };
        let size = SIZE { cx: side, cy: side };
        let origin = POINT { x: 0, y: 0 };
        unsafe { UpdateLayeredWindow(self.hwnd, 0, &pos, &size, self.dc, &origin, 0, &blend, ULW_ALPHA) };
    }

    fn make_bitmap(&mut self) -> bool {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: self.side,
                biHeight: -self.side,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [0],
        };
        unsafe {
            self.dc = CreateCompatibleDC(0);
            let mut bits: *mut c_void = null_mut();
            self.bitmap = CreateDIBSection(self.dc, &info, DIB_RGB_COLORS, &mut bits, 0, 0);
            if self.bitmap == 0 || bits.is_null() {
                self.free();
                return false;
            }
            SelectObject(self.dc, self.bitmap);
            self.bits = bits.cast();
        }
        true
    }

    /// Let go of the bitmap while there's no glow.
    fn free(&mut self) {
        unsafe {
            if self.dc != 0 {
                DeleteDC(self.dc);
            }
            if self.bitmap != 0 {
                DeleteObject(self.bitmap);
            }
        }
        (self.dc, self.bitmap, self.bits) = (0, 0, null_mut());
    }
}

/// Each palette's accent, as the swatches in the settings show it, half way to white so
/// it shows on a dark wallpaper too.
pub fn accent(palette: &str) -> [u8; 3] {
    let [r, g, b] = match palette {
        "catppuccin" => [0xcb, 0xa6, 0xf7],
        "nord" => [0x88, 0xc0, 0xd0],
        "gruvbox" => [0x8e, 0xc0, 0x7c],
        "rose-pine" => [0xc4, 0xa7, 0xe7],
        "solarized" => [0x2a, 0xa1, 0x98],
        _ => [0x39, 0x87, 0xe5],
    };
    [r, g, b].map(|c| c + (255 - c) / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_palette_has_an_accent() {
        for p in mullion_core::settings::PALETTES {
            assert!(*p == "oriel" || accent(p) != accent("oriel"), "{p}");
        }
    }
}
