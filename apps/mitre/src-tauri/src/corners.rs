//! The screens and their corners: which corner (if any) a point is in, and which corners stop
//! the pointer. Rects are physical pixels (Mitre is per-monitor DPI aware, as every Tauri app
//! is), so screens at other scaling, or left of or above the main one, need nothing special.

use crate::ffi::*;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    TopLeft = 0,
    TopRight = 1,
    BottomLeft = 2,
    BottomRight = 3,
}

/// The order Mitre keeps a screen's four actions in.
pub const CORNERS: [Corner; 4] = [Corner::TopLeft, Corner::TopRight, Corner::BottomLeft, Corner::BottomRight];

impl Corner {
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn left(self) -> bool {
        matches!(self, Corner::TopLeft | Corner::BottomLeft)
    }

    pub fn top(self) -> bool {
        matches!(self, Corner::TopLeft | Corner::TopRight)
    }
}

/// One screen as Windows has it now.
#[derive(Clone, Debug, PartialEq)]
pub struct Screen {
    /// Its lasting name, the same from one start to the next and whichever port it's on first:
    /// what Mitre keeps its corners under.
    pub key: String,
    pub monitor: isize,
    pub rect: RECT,
    pub dpi: u32,
    pub primary: bool,
}

/// What the settings page draws of a screen.
#[derive(Serialize, Clone, Debug)]
pub struct ScreenInfo {
    pub key: String,
    /// "Screen 1", numbered left to right.
    pub name: String,
    pub main: bool,
    /// [left, top, width, height], physical pixels.
    pub rect: [i32; 4],
    /// Scaling, in percent.
    pub scale: u32,
    /// Each corner stops the pointer (no screen past it), in `CORNERS` order.
    pub hard: [bool; 4],
}

/// The screens, left to right, then top to bottom.
pub fn screens() -> Vec<Screen> {
    unsafe extern "system" fn each(monitor: isize, _: isize, _: *mut RECT, out: isize) -> i32 {
        let out = &mut *(out as *mut Vec<Screen>);
        let mut info: MONITORINFOEXW = std::mem::zeroed();
        info.info.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(monitor, &mut info.info) != 0 {
            let (mut dpi, mut _y) = (96u32, 96u32);
            GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut _y);
            let device = from_wide(&info.szDevice);
            out.push(Screen {
                key: lasting_name(&device).unwrap_or(device),
                monitor,
                rect: info.info.rcMonitor,
                dpi,
                primary: info.info.dwFlags & MONITORINFOF_PRIMARY != 0,
            });
        }
        1
    }
    let mut out: Vec<Screen> = Vec::new();
    unsafe { EnumDisplayMonitors(0, std::ptr::null(), each, &mut out as *mut _ as isize) };
    out.sort_by_key(|s| (s.rect.left, s.rect.top));
    out
}

/// The monitor's device path (`\\?\DISPLAY#GSM5B7F#…`), which names the monitor itself, not the
/// `\\.\DISPLAY2` Windows numbers its outputs with, which can change at the next start.
fn lasting_name(device: &str) -> Option<String> {
    let name = wide(device);
    let mut dd: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
    dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
    let ok = unsafe { EnumDisplayDevicesW(name.as_ptr(), 0, &mut dd, EDD_GET_DEVICE_INTERFACE_NAME) } != 0;
    Some(from_wide(&dd.DeviceID)).filter(|id| ok && !id.is_empty())
}

pub fn info(screens: &[Screen]) -> Vec<ScreenInfo> {
    screens
        .iter()
        .enumerate()
        .map(|(i, s)| ScreenInfo {
            key: s.key.clone(),
            name: format!("Screen {}", i + 1),
            main: s.primary,
            rect: [s.rect.left, s.rect.top, s.rect.right - s.rect.left, s.rect.bottom - s.rect.top],
            scale: (s.dpi * 100 + 48) / 96,
            hard: CORNERS.map(|c| is_hard(screens, s, c)),
        })
        .collect()
}

fn on_a_screen(screens: &[Screen], x: i32, y: i32) -> bool {
    screens.iter().any(|s| x >= s.rect.left && x < s.rect.right && y >= s.rect.top && y < s.rect.bottom)
}

/// How near the corner counts as in it: a few pixels, more on a screen scaled up.
pub fn zone(s: &Screen) -> i32 {
    (4 * s.dpi as i32 / 96).max(4)
}

/// Which screen's corner the point is in, if any.
pub fn corner_at(screens: &[Screen], x: i32, y: i32) -> Option<(usize, Corner)> {
    let (i, s) = screens.iter().enumerate().find(|(_, s)| x >= s.rect.left && x < s.rect.right && y >= s.rect.top && y < s.rect.bottom)?;
    let (r, z) = (s.rect, zone(s));
    let (left, right, top, bottom) = (x < r.left + z, x >= r.right - z, y < r.top + z, y >= r.bottom - z);
    let c = match (left, right, top, bottom) {
        (true, _, true, _) => Corner::TopLeft,
        (_, true, true, _) => Corner::TopRight,
        (true, _, _, true) => Corner::BottomLeft,
        (_, true, _, true) => Corner::BottomRight,
        _ => return None,
    };
    Some((i, c))
}

/// A corner the pointer stops at: no other screen past it on either edge. Where two screens
/// meet, the pointer slides through the corner, so it's hard to hit.
pub fn is_hard(screens: &[Screen], s: &Screen, c: Corner) -> bool {
    let r = s.rect;
    let (x, dx) = if c.left() { (r.left, -1) } else { (r.right - 1, 1) };
    let (y, dy) = if c.top() { (r.top, -1) } else { (r.bottom - 1, 1) };
    !on_a_screen(screens, x + dx, y) && !on_a_screen(screens, x, y + dy) && !on_a_screen(screens, x + dx, y + dy)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(key: &str, left: i32, top: i32, w: i32, h: i32, dpi: u32) -> Screen {
        Screen { key: key.into(), monitor: 0, rect: RECT { left, top, right: left + w, bottom: top + h }, dpi, primary: left == 0 && top == 0 }
    }

    /// The user's own: a 2560×1080 main screen at 100%, and a 1920×1080 one at 125% to its right.
    fn two() -> Vec<Screen> {
        vec![screen("a", 0, 0, 2560, 1080, 96), screen("b", 2560, 0, 1920, 1080, 120)]
    }

    #[test]
    fn points_find_their_corner() {
        let s = two();
        assert_eq!(corner_at(&s, 0, 0), Some((0, Corner::TopLeft)));
        assert_eq!(corner_at(&s, 3, 1079), Some((0, Corner::BottomLeft)));
        assert_eq!(corner_at(&s, 2559, 0), Some((0, Corner::TopRight)));
        assert_eq!(corner_at(&s, 2560, 1079), Some((1, Corner::BottomLeft)));
        assert_eq!(corner_at(&s, 4479, 1079), Some((1, Corner::BottomRight)));
        // Scaled up, the zone is wider: 5 px at 125%.
        assert_eq!(corner_at(&s, 4475, 4), Some((1, Corner::TopRight)));
        assert_eq!(corner_at(&s, 4, 4), None);
        assert_eq!(corner_at(&s, 1280, 0), None);
        assert_eq!(corner_at(&s, -1, 0), None);
    }

    #[test]
    fn corners_where_screens_meet_are_inner() {
        let s = two();
        let hard = |i: usize| CORNERS.map(|c| is_hard(&s, &s[i], c));
        assert_eq!(hard(0), [true, false, true, false]);
        assert_eq!(hard(1), [false, true, false, true]);
        // A smaller screen beside a taller one, lower down: the taller one's corners all stop the
        // pointer (nothing past them), the smaller one's left-hand ones don't.
        let s = vec![screen("a", 0, 0, 1920, 1200, 96), screen("b", 1920, 100, 1280, 1024, 96)];
        let hard = |i: usize| CORNERS.map(|c| is_hard(&s, &s[i], c));
        assert_eq!(hard(0), [true, true, true, true]);
        assert_eq!(hard(1), [false, true, false, true]);
    }

    #[test]
    fn screens_left_of_the_main_one_work_too() {
        let s = vec![screen("b", -1920, 0, 1920, 1080, 96), screen("a", 0, 0, 2560, 1440, 144)];
        assert_eq!(corner_at(&s, -1920, 0), Some((0, Corner::TopLeft)));
        assert_eq!(corner_at(&s, -1, 1079), Some((0, Corner::BottomRight)));
        assert_eq!(corner_at(&s, 5, 1435), Some((1, Corner::BottomLeft)));
        let info = info(&s);
        assert_eq!((info[0].name.as_str(), info[1].name.as_str(), info[1].main, info[1].scale), ("Screen 1", "Screen 2", true, 150));
    }

    #[test]
    fn this_pc_has_a_screen_with_a_name() {
        let s = screens();
        assert!(!s.is_empty());
        assert!(s.iter().all(|s| !s.key.is_empty() && s.rect.right > s.rect.left));
    }
}
