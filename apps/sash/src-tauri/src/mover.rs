//! Moving the window in front, on a thread of its own: a program that isn't answering can hold
//! up a move, and it mustn't hold up Sash's settings or its shortcuts.
//!
//! Each move puts what's seen of the window (its extended frame bounds, from DWM) where `layout`
//! says, so the invisible resize borders Windows 10 and 11 put around most windows don't leave
//! gaps. Moved onto a screen with different scaling, a window rescales itself, borders and all,
//! so the move is checked and made again once it has.

use crate::actions::{self, Do};
use crate::ffi::*;
use crate::layout::{self, Place, FULL};
use mullion_core::{thumbs, wins};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender};
use std::time::Duration;

/// What a shortcut asks, with the settings it needs.
pub struct Job {
    pub action: Do,
    pub cycle: bool,
    /// Between windows and around the screen's edges, in pixels at 100% scaling.
    pub gap: u32,
}

/// Why a window wasn't moved, when Sash should say so.
#[derive(Debug, PartialEq)]
pub enum Refused {
    /// A program run as administrator: Windows doesn't let other programs move its windows.
    Elevated(String),
    /// Not answering, so not moved rather than wait on it.
    Hung(String),
}

/// Start the thread; `refused` hears (on it) about each window it couldn't move.
pub fn start(refused: impl Fn(Refused) + Send + 'static) -> Sender<Job> {
    let (tx, rx) = channel::<Job>();
    let _ = std::thread::Builder::new().name("mover".into()).spawn(move || {
        let mut mover = Mover::default();
        for job in rx {
            if let Err(r) = mover.run(&job) {
                refused(r);
            }
        }
    });
    tx
}

#[derive(Clone, Copy)]
struct Screen {
    monitor: isize,
    whole: RECT,
    work: RECT,
    dpi: u32,
}

fn screen_of(monitor: isize) -> Option<Screen> {
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return None;
    }
    let (mut x, mut y) = (96, 96);
    unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) };
    Some(Screen { monitor, whole: info.rcMonitor, work: info.rcWork, dpi: x.max(1) })
}

/// The work area and scaling (dpi) of the screen the window in front is on, for Sash's note.
pub fn front_screen() -> Option<(RECT, u32)> {
    screen_of(unsafe { MonitorFromWindow(GetForegroundWindow(), MONITOR_DEFAULTTONEAREST) }).map(|s| (s.work, s.dpi))
}

fn screens() -> Vec<Screen> {
    unsafe extern "system" fn each(monitor: isize, _dc: isize, _rect: *mut RECT, out: isize) -> i32 {
        (*(out as *mut Vec<isize>)).push(monitor);
        1
    }
    let mut all: Vec<isize> = Vec::new();
    unsafe { EnumDisplayMonitors(0, std::ptr::null(), each, (&mut all as *mut Vec<isize>) as isize) };
    all.into_iter().filter_map(screen_of).collect()
}

/// What's seen of a window, and the whole of it with its invisible borders.
fn rects(hwnd: isize) -> Option<(RECT, RECT)> {
    let mut whole = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut whole) } == 0 {
        return None;
    }
    let seen = thumbs::bounds(hwnd).map_or(whole, |(seen, _)| seen);
    Some((seen, whole))
}

fn gap_on(screen: &Screen, gap: u32) -> i32 {
    (gap * screen.dpi / 96) as i32
}

/// Where Sash last put a window, and where it was before that.
struct Memory {
    before: RECT,
    placed: RECT,
}

#[derive(Default)]
struct Mover {
    /// By window, for Restore. Forgotten when the window's gone.
    memory: HashMap<isize, Memory>,
}

impl Mover {
    fn run(&mut self, job: &Job) -> Result<(), Refused> {
        let hwnd = unsafe { GetForegroundWindow() };
        // Only windows Alt+Tab would list: not the desktop, the taskbar, Start or a menu.
        let Some(w) = wins::read(hwnd).filter(|w| wins::listed(w, std::process::id()) && !w.minimized) else { return Ok(()) };
        if unsafe { IsHungAppWindow(hwnd) } != 0 {
            return Err(Refused::Hung(w.title));
        }
        let Some(here) = screen_of(unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) }) else { return Ok(()) };
        let Some((seen, _)) = rects(hwnd) else { return Ok(()) };
        let zoomed = unsafe { IsZoomed(hwnd) } != 0;
        let gap = gap_on(&here, job.gap);
        let refused = |e: Option<()>| e.ok_or_else(|| Refused::Elevated(w.title.clone()));
        match job.action {
            Do::Arrow(dir) => {
                let now = if zoomed { Some(Place { h: FULL, v: FULL }) } else { layout::detect(seen, here.work, gap) };
                let place = layout::step(now, dir, job.cycle);
                refused(self.put(hwnd, layout::rect(here.work, place, gap), Some(place), &here, zoomed))
            }
            Do::Put(place) => refused(self.put(hwnd, layout::rect(here.work, place, gap), Some(place), &here, zoomed)),
            Do::Centre => {
                // A maximized window is centred at its own size, as it was before.
                let from = if zoomed { restore(hwnd).ok_or_else(|| Refused::Elevated(w.title.clone()))? } else { seen };
                refused(self.put(hwnd, layout::centre(from, here.work, gap), None, &here, false))
            }
            Do::Maximize => {
                if zoomed {
                    refused(restore(hwnd).map(|_| ()))
                } else {
                    refused(probe(hwnd))?;
                    self.remember(hwnd, seen);
                    unsafe { ShowWindow(hwnd, SW_MAXIMIZE) };
                    Ok(())
                }
            }
            Do::Restore => match self.memory.remove(&hwnd) {
                Some(m) => {
                    let to = screen_of(unsafe { MonitorFromRect(&m.before, MONITOR_DEFAULTTONEAREST) }).unwrap_or(here);
                    refused(self.put(hwnd, m.before, None, &to, zoomed))?;
                    self.memory.remove(&hwnd);
                    Ok(())
                }
                None if zoomed => refused(restore(hwnd).map(|_| ())),
                None => Ok(()),
            },
            Do::Screen(dir) => {
                let all = screens();
                let Some(i) = all.iter().position(|s| s.monitor == here.monitor) else { return Ok(()) };
                let to = all[layout::next_screen(&all.iter().map(|s| s.whole).collect::<Vec<_>>(), i, dir)];
                if to.monitor == here.monitor {
                    return Ok(());
                }
                let scale = to.dpi as f64 / here.dpi as f64;
                if zoomed {
                    // Restored, carried over, and maximized there.
                    let from = restore(hwnd).ok_or_else(|| Refused::Elevated(w.title.clone()))?;
                    refused(self.put(hwnd, layout::carry(from, here.work, to.work, scale), None, &to, false))?;
                    unsafe { ShowWindow(hwnd, SW_MAXIMIZE) };
                    return Ok(());
                }
                let to_gap = gap_on(&to, job.gap);
                match layout::detect(seen, here.work, gap) {
                    Some(place) => refused(self.put(hwnd, layout::rect(to.work, place, to_gap), Some(place), &to, false)),
                    None => refused(self.put(hwnd, layout::carry(seen, here.work, to.work, scale), None, &to, false)),
                }
            }
        }
    }

    /// Keep where a window was before Sash moves it, unless it's still where Sash last put it.
    fn remember(&mut self, hwnd: isize, seen: RECT) {
        self.memory.retain(|&h, _| unsafe { IsWindow(h) } != 0);
        match self.memory.get(&hwnd) {
            Some(m) if near(m.placed, seen) => {}
            _ => {
                self.memory.insert(hwnd, Memory { before: seen, placed: seen });
            }
        }
    }

    /// Put what's seen of `hwnd` at `target` on `screen`. None if Windows refused.
    fn put(&mut self, hwnd: isize, target: RECT, place: Option<Place>, screen: &Screen, zoomed: bool) -> Option<()> {
        probe(hwnd)?;
        if zoomed {
            restore(hwnd)?;
        }
        if let Some((seen, _)) = rects(hwnd) {
            self.remember(hwnd, seen);
        }
        let from_dpi = screen_of(unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) }).map_or(screen.dpi, |s| s.dpi);
        place_exactly(hwnd, target)?;
        if from_dpi != screen.dpi {
            // The window rescales itself for the new screen (WM_DPICHANGED), and some programs do
            // that a moment later: let it, then put it right.
            std::thread::sleep(Duration::from_millis(60));
            place_exactly(hwnd, target)?;
        }
        // A window with a smallest (or fixed) size keeps to its place's side of the screen.
        if let Some((seen, _)) = rects(hwnd) {
            let (w, h) = (seen.right - seen.left, seen.bottom - seen.top);
            if !near(seen, target) {
                let at = layout::fit(target, place, w, h, screen.work);
                place_exactly(hwnd, at)?;
            }
        }
        if let (Some(m), Some((seen, _))) = (self.memory.get_mut(&hwnd), rects(hwnd)) {
            m.placed = seen;
        }
        Some(())
    }
}

fn near(a: RECT, b: RECT) -> bool {
    (a.left - b.left).abs() <= 1 && (a.top - b.top).abs() <= 1 && (a.right - b.right).abs() <= 1 && (a.bottom - b.bottom).abs() <= 1
}

/// Whether Windows lets Sash move this window: not if it belongs to a program run as
/// administrator (User Interface Privilege Isolation), which Sash learns by asking Windows to
/// redraw its frame where it is, rather than by looking into the program.
fn probe(hwnd: isize) -> Option<()> {
    let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_FRAMECHANGED;
    if unsafe { SetWindowPos(hwnd, 0, 0, 0, 0, 0, flags) } == 0 && unsafe { GetLastError() } == ERROR_ACCESS_DENIED {
        return None;
    }
    Some(())
}

/// Un-maximize a window; says where it went.
fn restore(hwnd: isize) -> Option<RECT> {
    probe(hwnd)?;
    unsafe { ShowWindow(hwnd, SW_RESTORE) };
    rects(hwnd).map(|(seen, _)| seen)
}

/// Move and size the whole window so what's seen of it is `target`, with its borders as they are
/// now.
fn place_exactly(hwnd: isize, target: RECT) -> Option<()> {
    let (seen, whole) = rects(hwnd)?;
    let x = target.left - (seen.left - whole.left);
    let y = target.top - (seen.top - whole.top);
    let w = (target.right - target.left) + (seen.left - whole.left) + (whole.right - seen.right);
    let h = (target.bottom - target.top) + (seen.top - whole.top) + (whole.bottom - seen.bottom);
    let ok = unsafe { SetWindowPos(hwnd, 0, x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER) } != 0;
    if !ok && unsafe { GetLastError() } == ERROR_ACCESS_DENIED {
        return None;
    }
    Some(())
}

/// The action a shortcut id stands for, as a job with today's settings.
pub fn job(hotkey_id: i32, cycle: bool, gap: u32) -> Option<Job> {
    actions::by_hotkey_id(hotkey_id).map(|a| Job { action: a.does, cycle, gap })
}
