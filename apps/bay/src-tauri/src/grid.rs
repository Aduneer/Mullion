//! Where each window goes: a grid that fits every window on the screen, each in its own shape,
//! and each near where it is, so the windows fly short ways and keep their places relative to
//! each other.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    /// The way from `a` to `b`, `t` of the way along (0 to 1).
    pub fn lerp(a: Rect, b: Rect, t: f64) -> Rect {
        let at = |p: f64, q: f64| p + (q - p) * t;
        Rect { x: at(a.x, b.x), y: at(a.y, b.y), w: at(a.w, b.w), h: at(a.h, b.h) }
    }

    fn centre(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

/// A rect inside `area` for each of `windows` (where each window is now), in the same order.
///
/// Every column count is tried, and the one that shows the most of the windows wins; on a tie,
/// more columns. A cell is a window's thumbnail plus `label` below it for its icon and title,
/// with `gap` around it. A window is never shown bigger than it is, and each row is centred, so
/// a short last row sits in the middle. The windows go into rows top to bottom by where their
/// middles are, and each row left to right, so a window on the left of the screen stays on the
/// left.
pub fn layout(windows: &[Rect], area: Rect, gap: f64, label: f64) -> Vec<Rect> {
    let n = windows.len();
    // The cells are all one size, so how much shows doesn't depend on which window is where.
    let mut best: Option<(f64, usize, f64, f64)> = None;
    for cols in (1..=n).rev() {
        let rows = n.div_ceil(cols);
        let cell_w = (area.w - gap * (cols - 1) as f64) / cols as f64;
        let cell_h = (area.h - gap * (rows - 1) as f64) / rows as f64 - label;
        if cell_w <= 1.0 || cell_h <= 1.0 {
            continue;
        }
        let shown: f64 = windows.iter().map(|w| w.w * w.h * fit(w, cell_w, cell_h).powi(2)).sum();
        if best.is_none_or(|(most, ..)| shown > most) {
            best = Some((shown, cols, cell_w, cell_h));
        }
    }
    let Some((_, cols, cell_w, cell_h)) = best else { return Vec::new() };

    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| windows[a].centre().1.total_cmp(&windows[b].centre().1));
    let rows = n.div_ceil(cols);
    let grid_h = (cell_h + label) * rows as f64 + gap * (rows - 1) as f64;
    let mut rects = vec![Rect::default(); n];
    for (row, chunk) in order.chunks_mut(cols).enumerate() {
        chunk.sort_by(|&a, &b| windows[a].centre().0.total_cmp(&windows[b].centre().0));
        let row_w = cell_w * chunk.len() as f64 + gap * (chunk.len() - 1) as f64;
        let x0 = area.x + (area.w - row_w) / 2.0;
        let y0 = area.y + (area.h - grid_h) / 2.0 + row as f64 * (cell_h + label + gap);
        for (i, &at) in chunk.iter().enumerate() {
            let s = fit(&windows[at], cell_w, cell_h);
            let (w, h) = (windows[at].w * s, windows[at].h * s);
            let x = x0 + i as f64 * (cell_w + gap) + (cell_w - w) / 2.0;
            rects[at] = Rect { x, y: y0 + (cell_h - h) / 2.0, w, h };
        }
    }
    rects
}

/// How much a window shrinks to fit a cell: never more than its own size.
fn fit(w: &Rect, cell_w: f64, cell_h: f64) -> f64 {
    (cell_w / w.w.max(1.0)).min(cell_h / w.h.max(1.0)).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect { x: 40.0, y: 60.0, w: 2400.0, h: 1300.0 };

    fn windows(n: usize) -> Vec<Rect> {
        (0..n)
            .map(|i| Rect { x: (i * 137 % 1900) as f64, y: (i * 89 % 700) as f64, w: 800.0 + (i % 5) as f64 * 300.0, h: 600.0 + (i % 3) as f64 * 250.0 })
            .collect()
    }

    fn inside(r: &Rect, a: &Rect) -> bool {
        r.x >= a.x - 0.01 && r.y >= a.y - 0.01 && r.x + r.w <= a.x + a.w + 0.01 && r.y + r.h <= a.y + a.h + 0.01
    }

    fn overlap(a: &Rect, b: &Rect) -> bool {
        a.x < b.x + b.w - 0.01 && b.x < a.x + a.w - 0.01 && a.y < b.y + b.h - 0.01 && b.y < a.y + a.h - 0.01
    }

    #[test]
    fn every_window_fits_in_its_own_shape() {
        for n in [1, 2, 3, 7, 12, 20, 45] {
            let wins = windows(n);
            let rects = layout(&wins, SCREEN, 24.0, 40.0);
            assert_eq!(rects.len(), n);
            for (r, w) in rects.iter().zip(&wins) {
                assert!(inside(r, &SCREEN), "{n} windows: {r:?} is off the screen");
                assert!((r.w / r.h - w.w / w.h).abs() < 1e-6, "{n} windows: {r:?} isn't {w:?}'s shape");
                assert!(r.w <= w.w && r.h <= w.h);
            }
            for (i, a) in rects.iter().enumerate() {
                assert!(rects[i + 1..].iter().all(|b| !overlap(a, b)), "{n} windows overlap");
            }
        }
    }

    #[test]
    fn small_windows_stay_their_size_side_by_side() {
        let w = Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 };
        let rects = layout(&[w, w], SCREEN, 24.0, 40.0);
        assert_eq!((rects[0].w, rects[0].h), (400.0, 300.0));
        assert_eq!(rects[0].y, rects[1].y);
    }

    #[test]
    fn windows_keep_their_places() {
        // Four windows in the corners of the screen, listed in no particular order.
        let at = |x: f64, y: f64| Rect { x, y, w: 900.0, h: 600.0 };
        let wins = [at(1500.0, 700.0), at(0.0, 0.0), at(0.0, 700.0), at(1500.0, 0.0)];
        let r = layout(&wins, SCREEN, 24.0, 40.0);
        let (top_left, top_right, bottom_left, bottom_right) = (r[1], r[3], r[2], r[0]);
        assert!(top_left.x < top_right.x && bottom_left.x < bottom_right.x);
        assert!(top_left.y < bottom_left.y && top_right.y < bottom_right.y);
    }

    #[test]
    fn nothing_to_lay_out() {
        assert!(layout(&[], SCREEN, 24.0, 40.0).is_empty());
    }

    #[test]
    fn halfway() {
        let a = Rect { x: 0.0, y: 0.0, w: 100.0, h: 50.0 };
        let b = Rect { x: 100.0, y: 50.0, w: 200.0, h: 150.0 };
        assert_eq!(Rect::lerp(a, b, 0.5), Rect { x: 50.0, y: 25.0, w: 150.0, h: 100.0 });
    }
}
