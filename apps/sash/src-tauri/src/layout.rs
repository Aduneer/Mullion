//! Where a window goes, worked out on rectangles alone so it can be tested without any windows.
//!
//! A screen's work area (the screen less the taskbar and app bars like Lintel's) is cut into
//! sixths each way, which gives halves, thirds and two-thirds; a place is a span of sixths across
//! and one down. Every place's edges come from the same cut, so two windows beside each other
//! meet exactly, or exactly a gap apart. Everything is in physical pixels, and every rectangle is
//! what's seen of a window (its extended frame bounds), not its invisible resize borders.

use mullion_core::ffi::RECT;

/// From sixth `.0` to sixth `.1` of one side of the work area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span(pub u8, pub u8);

pub const FULL: Span = Span(0, 6);
pub const FIRST_HALF: Span = Span(0, 3);
pub const SECOND_HALF: Span = Span(3, 6);

/// The spans Sash puts windows in: whole, halves, thirds and two-thirds.
const SPANS: [Span; 8] = [FULL, FIRST_HALF, SECOND_HALF, Span(0, 2), Span(2, 4), Span(4, 6), Span(0, 4), Span(2, 6)];

/// A place on the screen: a span across (`h`) and one down (`v`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub h: Span,
    pub v: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

/// How far a window's edge may be from a place's and still count as in it: some programs round
/// their size a little (to whole lines of text, say).
const TOLERANCE: i32 = 3;

/// One side's edges for `span` between `a` and `b`: a gap at the screen's edges, and a gap
/// between two windows, half taken from each.
fn cut(a: i32, b: i32, span: Span, gap: i32) -> (i32, i32) {
    let at = |k: u8| a + ((b - a) as i64 * k as i64 * 2 + 6).div_euclid(12) as i32;
    let lo = at(span.0) + if span.0 == 0 { gap } else { gap - gap / 2 };
    let hi = at(span.1) - if span.1 == 6 { gap } else { gap / 2 };
    (lo, hi)
}

/// Where `place` is on a work area, with `gap` pixels around and between windows.
pub fn rect(work: RECT, place: Place, gap: i32) -> RECT {
    let (left, right) = cut(work.left, work.right, place.h, gap);
    let (top, bottom) = cut(work.top, work.bottom, place.v, gap);
    RECT { left, top, right, bottom }
}

fn near(a: RECT, b: RECT) -> bool {
    (a.left - b.left).abs() <= TOLERANCE && (a.top - b.top).abs() <= TOLERANCE && (a.right - b.right).abs() <= TOLERANCE && (a.bottom - b.bottom).abs() <= TOLERANCE
}

/// The place a window already is in, if it's in one: put there by Sash, or by hand.
pub fn detect(frame: RECT, work: RECT, gap: i32) -> Option<Place> {
    SPANS.iter().flat_map(|&h| SPANS.iter().map(move |&v| Place { h, v })).find(|&p| near(frame, rect(work, p, gap)))
}

/// Half, two-thirds, a third, and back to half, kept to the same side.
fn next_size(span: Span, at_start: bool) -> Span {
    let n = match span.1 - span.0 {
        3 => 4,
        4 => 2,
        _ => 3,
    };
    if at_start {
        Span(0, n)
    } else {
        Span(6 - n, 6)
    }
}

/// Where an arrow sends a window from `current` (None: somewhere of its own).
///
/// - Anywhere else, it goes to that half of the screen.
/// - Already on that side, pressing again makes it two-thirds, then a third, then half again
///   (unless `cycle` is off).
/// - A top or bottom half (or a left or right one, the other way) goes to that quarter, as
///   Windows' own Win+arrows do. A quarter moves to the quarter beside it, or, pressed toward its
///   own side, fills that side.
pub fn step(current: Option<Place>, dir: Dir, cycle: bool) -> Place {
    let across_ways = matches!(dir, Dir::Left | Dir::Right);
    let to_start = matches!(dir, Dir::Left | Dir::Up);
    let half = if to_start { FIRST_HALF } else { SECOND_HALF };
    let (along, across) = match current {
        None => (half, FULL),
        Some(p) => {
            let (along, across) = if across_ways { (p.h, p.v) } else { (p.v, p.h) };
            let here = along != FULL && if to_start { along.0 == 0 } else { along.1 == 6 };
            if here && across == FULL {
                (if cycle { next_size(along, to_start) } else { along }, across)
            } else if here {
                (along, FULL)
            } else if across != FULL && (along == FULL || if to_start { along.1 == 6 } else { along.0 == 0 }) {
                (half, across)
            } else {
                (half, FULL)
            }
        }
    };
    if across_ways {
        Place { h: along, v: across }
    } else {
        Place { h: across, v: along }
    }
}

fn width(r: RECT) -> i32 {
    r.right - r.left
}

fn height(r: RECT) -> i32 {
    r.bottom - r.top
}

/// `frame` the same size (no bigger than the work area, less `gap` all round) in the middle of it.
pub fn centre(frame: RECT, work: RECT, gap: i32) -> RECT {
    let w = width(frame).min(width(work) - 2 * gap);
    let h = height(frame).min(height(work) - 2 * gap);
    let left = work.left + (width(work) - w) / 2;
    let top = work.top + (height(work) - h) / 2;
    RECT { left, top, right: left + w, bottom: top + h }
}

/// A window that's in no place of Sash's, carried from one work area to another: the same part of
/// the screen, `scale` times the size (the new screen's scaling over the old one's), kept inside.
pub fn carry(frame: RECT, from: RECT, to: RECT, scale: f64) -> RECT {
    let w = ((width(frame) as f64 * scale).round() as i32).clamp(1, width(to));
    let h = ((height(frame) as f64 * scale).round() as i32).clamp(1, height(to));
    // Where its middle was, as a share of the old work area.
    let fx = ((frame.left + frame.right) as f64 / 2.0 - from.left as f64) / width(from).max(1) as f64;
    let fy = ((frame.top + frame.bottom) as f64 / 2.0 - from.top as f64) / height(from).max(1) as f64;
    let cx = to.left + (fx * width(to) as f64).round() as i32;
    let cy = to.top + (fy * height(to) as f64).round() as i32;
    let left = (cx - w / 2).clamp(to.left, to.right - w);
    let top = (cy - h / 2).clamp(to.top, to.bottom - h);
    RECT { left, top, right: left + w, bottom: top + h }
}

/// A window that wouldn't take its place's size (it has a smallest size, or a fixed one), moved
/// to keep to its place's side of the screen: against the edge it's meant to touch, otherwise
/// centred on the place. Kept inside the work area if it fits.
pub fn fit(target: RECT, place: Option<Place>, w: i32, h: i32, work: RECT) -> RECT {
    let side = |lo: i32, hi: i32, size: i32, span: Option<Span>, wlo: i32, whi: i32| {
        let at = match span {
            Some(s) if s.0 == 0 && s.1 != 6 => lo,
            Some(s) if s.1 == 6 && s.0 != 0 => hi - size,
            _ => lo + (hi - lo - size) / 2,
        };
        if size <= whi - wlo {
            at.clamp(wlo, whi - size)
        } else {
            wlo
        }
    };
    let left = side(target.left, target.right, w, place.map(|p| p.h), work.left, work.right);
    let top = side(target.top, target.bottom, h, place.map(|p| p.v), work.top, work.bottom);
    RECT { left, top, right: left + w, bottom: top + h }
}

/// The screen after (`dir` 1) or before (-1) `current`, left to right, then top to bottom, round
/// and round.
pub fn next_screen(screens: &[RECT], current: usize, dir: i32) -> usize {
    let mut order: Vec<usize> = (0..screens.len()).collect();
    order.sort_by_key(|&i| (screens[i].left, screens[i].top));
    let Some(at) = order.iter().position(|&i| i == current) else { return current };
    let n = order.len() as i32;
    order[(at as i32 + dir).rem_euclid(n) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: RECT = RECT { left: 0, top: 0, right: 2560, bottom: 1032 };

    fn at(h: Span, v: Span) -> Place {
        Place { h, v }
    }

    #[test]
    fn halves_meet_exactly() {
        let l = rect(WORK, at(FIRST_HALF, FULL), 0);
        let r = rect(WORK, at(SECOND_HALF, FULL), 0);
        assert_eq!(l, RECT { left: 0, top: 0, right: 1280, bottom: 1032 });
        assert_eq!(l.right, r.left);
        assert_eq!(r.right, WORK.right);
        // Thirds of a width that doesn't divide by three still share their edges.
        let odd = RECT { left: -1920, top: 0, right: 1, bottom: 1080 };
        let thirds = [Span(0, 2), Span(2, 4), Span(4, 6)].map(|s| rect(odd, at(s, FULL), 0));
        assert_eq!(thirds[0].left, odd.left);
        assert_eq!(thirds[0].right, thirds[1].left);
        assert_eq!(thirds[1].right, thirds[2].left);
        assert_eq!(thirds[2].right, odd.right);
        assert_eq!(rect(odd, at(Span(0, 4), FULL), 0).right, thirds[1].right);
    }

    #[test]
    fn gaps_are_the_same_at_the_edges_and_between() {
        let g = 9;
        let l = rect(WORK, at(FIRST_HALF, FIRST_HALF), g);
        let r = rect(WORK, at(SECOND_HALF, FIRST_HALF), g);
        let below = rect(WORK, at(FIRST_HALF, SECOND_HALF), g);
        assert_eq!(l.left - WORK.left, g);
        assert_eq!(l.top - WORK.top, g);
        assert_eq!(r.left - l.right, g);
        assert_eq!(below.top - l.bottom, g);
        assert_eq!(WORK.right - r.right, g);
    }

    #[test]
    fn finds_the_place_a_window_is_in() {
        for g in [0, 8] {
            for h in SPANS {
                for v in SPANS {
                    let p = at(h, v);
                    let mut r = rect(WORK, p, g);
                    assert_eq!(detect(r, WORK, g), Some(p));
                    r.right -= 2;
                    assert_eq!(detect(r, WORK, g), Some(p), "a little off still counts");
                }
            }
        }
        assert_eq!(detect(RECT { left: 300, top: 200, right: 1500, bottom: 900 }, WORK, 0), None);
        // A half with a gap isn't the same half without one.
        assert_eq!(detect(rect(WORK, at(FIRST_HALF, FULL), 16), WORK, 0), None);
    }

    #[test]
    fn an_arrow_sends_a_window_to_that_half() {
        assert_eq!(step(None, Dir::Left, true), at(FIRST_HALF, FULL));
        assert_eq!(step(None, Dir::Down, true), at(FULL, SECOND_HALF));
        // From the other half, or a middle third, or the whole screen.
        assert_eq!(step(Some(at(SECOND_HALF, FULL)), Dir::Left, true), at(FIRST_HALF, FULL));
        assert_eq!(step(Some(at(Span(2, 4), FULL)), Dir::Right, true), at(SECOND_HALF, FULL));
        assert_eq!(step(Some(at(FULL, FULL)), Dir::Up, true), at(FULL, FIRST_HALF));
    }

    #[test]
    fn pressing_again_goes_through_two_thirds_and_a_third() {
        let mut p = at(FIRST_HALF, FULL);
        let mut seen = vec![];
        for _ in 0..3 {
            p = step(Some(p), Dir::Left, true);
            seen.push(p.h);
        }
        assert_eq!(seen, [Span(0, 4), Span(0, 2), FIRST_HALF]);
        assert_eq!(step(Some(at(SECOND_HALF, FULL)), Dir::Right, true).h, Span(2, 6));
        assert_eq!(step(Some(at(Span(4, 6), FULL)), Dir::Right, true).h, SECOND_HALF);
        // Turned off, it stays a half.
        assert_eq!(step(Some(at(FIRST_HALF, FULL)), Dir::Left, false), at(FIRST_HALF, FULL));
    }

    #[test]
    fn a_half_and_the_other_way_make_a_quarter() {
        assert_eq!(step(Some(at(FIRST_HALF, FULL)), Dir::Up, true), at(FIRST_HALF, FIRST_HALF));
        assert_eq!(step(Some(at(FULL, SECOND_HALF)), Dir::Right, true), at(SECOND_HALF, SECOND_HALF));
        // A third keeps its width as it goes to the top.
        assert_eq!(step(Some(at(Span(0, 2), FULL)), Dir::Up, true), at(Span(0, 2), FIRST_HALF));
        // A quarter moves to the one beside it…
        assert_eq!(step(Some(at(FIRST_HALF, FIRST_HALF)), Dir::Right, true), at(SECOND_HALF, FIRST_HALF));
        assert_eq!(step(Some(at(FIRST_HALF, FIRST_HALF)), Dir::Down, true), at(FIRST_HALF, SECOND_HALF));
        // …or fills its side.
        assert_eq!(step(Some(at(FIRST_HALF, FIRST_HALF)), Dir::Up, true), at(FULL, FIRST_HALF));
        assert_eq!(step(Some(at(FIRST_HALF, FIRST_HALF)), Dir::Left, true), at(FIRST_HALF, FULL));
    }

    #[test]
    fn centring_keeps_the_size_unless_it_is_too_big() {
        let c = centre(RECT { left: 10, top: 10, right: 810, bottom: 610 }, WORK, 0);
        assert_eq!(c, RECT { left: 880, top: 216, right: 1680, bottom: 816 });
        let c = centre(RECT { left: 0, top: 0, right: 4000, bottom: 600 }, WORK, 8);
        assert_eq!((c.left, c.right), (8, 2552));
    }

    #[test]
    fn carried_windows_keep_their_part_of_the_screen() {
        let right = RECT { left: 2560, top: 0, right: 4480, bottom: 1032 };
        // In the middle of the left screen, at 125% on the right one.
        let r = carry(RECT { left: 1080, top: 316, right: 1480, bottom: 716 }, WORK, right, 1.25);
        assert_eq!((r.right - r.left, r.bottom - r.top), (500, 500));
        assert_eq!((r.left + r.right) / 2, 3520);
        // Too big for the new screen: no bigger than it, and inside it.
        let r = carry(RECT { left: 0, top: 0, right: 2560, bottom: 1032 }, WORK, right, 1.0);
        assert_eq!(r, right);
    }

    #[test]
    fn a_window_too_big_for_its_place_keeps_to_its_side() {
        let place = at(SECOND_HALF, FULL);
        let target = rect(WORK, place, 0);
        let r = fit(target, Some(place), 1500, 1032, WORK);
        assert_eq!((r.left, r.right), (1060, 2560));
        let place = at(FIRST_HALF, FULL);
        let r = fit(rect(WORK, place, 0), Some(place), 1500, 1032, WORK);
        assert_eq!((r.left, r.right), (0, 1500));
        // A middle third: centred on it.
        let place = at(Span(2, 4), FULL);
        let r = fit(rect(WORK, place, 0), Some(place), 1000, 1032, WORK);
        assert_eq!((r.left + r.right) / 2, 1280);
    }

    #[test]
    fn screens_go_left_to_right_and_round() {
        let screens = [RECT { left: 2560, ..WORK }, WORK, RECT { left: -1920, ..WORK }];
        assert_eq!(next_screen(&screens, 1, 1), 0);
        assert_eq!(next_screen(&screens, 0, 1), 2);
        assert_eq!(next_screen(&screens, 2, -1), 0);
        assert_eq!(next_screen(&screens[..1], 0, 1), 0);
    }
}
