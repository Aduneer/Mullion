//! Moving the thumbnails: each window's from where it is on the screen to its place in the grid
//! (or back), and the desktop's faded out (or in) over the same time, so the screen goes from
//! looking just as it did to the overview without a jump.
//!
//! Each step sets every thumbnail with `DwmUpdateThumbnailProperties`, then waits for DWM's next
//! frame (`DwmFlush`), so the loop runs at the screen's refresh rate with no timer. With screens
//! at different rates DWM wakes it for each one's frames, so steps come unevenly; each step's
//! positions come from the time gone by, not a count of frames, so the movement stays even.

use crate::grid::Rect;
use mullion_core::ffi::RECT;
use mullion_core::thumbs;
use std::time::Instant;

/// One window's thumbnail and where it goes: `from` is where the window is, `to` its place in
/// the grid, both in its screen's overview (physical pixels).
#[derive(Clone, Copy, Debug)]
pub struct Flight {
    pub id: isize,
    /// The part of the window to show: without its invisible resize borders.
    pub crop: Option<RECT>,
    pub from: Rect,
    pub to: Rect,
}

/// The desktop's thumbnail (wallpaper and icons) over a whole overview, under the windows':
/// opaque when closed, gone when open, so the page's backdrop shows through as it fades.
#[derive(Clone, Copy, Debug)]
pub struct Fade {
    pub id: isize,
    pub crop: Option<RECT>,
    pub at: Rect,
}

pub fn rect(r: Rect) -> RECT {
    RECT { left: r.x.round() as i32, top: r.y.round() as i32, right: (r.x + r.w).round() as i32, bottom: (r.y + r.h).round() as i32 }
}

/// Fast at first, settling gently into place.
pub fn ease(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

/// Everything `k` of the way from closed (0) to open (1).
pub fn place_all(flights: &[Flight], fades: &[Fade], k: f64) {
    for f in fades {
        thumbs::place(f.id, f.crop, rect(f.at), ((1.0 - k) * 255.0).round() as u8);
    }
    for f in flights {
        thumbs::place(f.id, f.crop, rect(Rect::lerp(f.from, f.to, k)), 255);
    }
}

/// Open (or close) over `ms`, one step per DWM frame. Blocks until done: run it on a thread of
/// its own. Returns the number of steps and the longest wait between two, in ms.
pub fn animate(flights: &[Flight], fades: &[Fade], opening: bool, ms: f64) -> (usize, f64) {
    // In step with DWM before the first step, and, opening, a frame for the overview's windows
    // to show before anything moves: the first frames after showing a window come late.
    for _ in 0..if opening { 2 } else { 1 } {
        thumbs::next_frame();
    }
    let start = Instant::now();
    let (mut steps, mut longest, mut last) = (0, 0.0f64, Instant::now());
    loop {
        let t = (start.elapsed().as_secs_f64() * 1000.0 / ms.max(1.0)).min(1.0);
        let k = if opening { ease(t) } else { 1.0 - ease(t) };
        place_all(flights, fades, k);
        thumbs::next_frame();
        steps += 1;
        longest = longest.max(last.elapsed().as_secs_f64() * 1000.0);
        last = Instant::now();
        if t >= 1.0 {
            return (steps, longest);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_starts_and_ends_in_place() {
        assert_eq!(ease(0.0), 0.0);
        assert_eq!(ease(1.0), 1.0);
        assert!(ease(0.5) > 0.5);
    }

    #[test]
    fn rects_round_to_pixels() {
        let r = rect(Rect { x: 10.4, y: 20.6, w: 100.2, h: 50.0 });
        assert_eq!((r.left, r.top, r.right, r.bottom), (10, 21, 111, 71));
    }
}
