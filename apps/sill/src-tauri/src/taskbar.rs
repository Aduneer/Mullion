//! Sill standing in for Windows' taskbar: every taskbar (Explorer's `Shell_TrayWnd`, and each
//! screen's `Shell_SecondaryTrayWnd`) is hidden outright, and set to auto-hide as its own settings
//! can, so it neither shows nor pops up when the pointer touches the edge. Explorer shows them
//! again now and then by itself, so each beat of the host hides any that are showing.
//!
//! Getting it back is what matters most. How it was is kept in a file beside the settings until
//! it's put back: on quit; by `sill-guard.exe`, which waits for Sill to end and runs
//! `sill.exe --restore-taskbar` if it ended without doing it (a crash, or ended from Task Manager);
//! and by the next start, which finds the file. Signing out or restarting Windows brings the
//! taskbar back too, since hiding it isn't saved anywhere; only the auto-hide is, and the file
//! has what it was.
//!
//! Hidden like this, Explorer stops keeping the dock's strip out of the work area (the part of
//! the screen windows maximize into), so Sill keeps it out itself (`keep_work_area`).

use crate::ffi::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// What `sill-guard.exe` and the uninstallers pass to have the taskbar put back.
pub const RESTORE: &str = "--restore-taskbar";

static HIDING: AtomicBool = AtomicBool::new(false);
static GUARDED: AtomicBool = AtomicBool::new(false);
/// The work area's edge before Sill moved it in, and where Sill moved it to.
static WORK: Mutex<Option<(RECT, RECT)>> = Mutex::new(None);

fn saved() -> PathBuf {
    crate::settings::dir().join("taskbar-state")
}

/// Whether Sill is hiding the taskbar now.
pub fn hiding() -> bool {
    HIDING.load(Ordering::Relaxed)
}

/// Explorer's taskbars, one per screen, not our host window.
fn taskbars() -> Vec<isize> {
    let ours = crate::host::hwnd();
    let mut all: Vec<isize> = windows_of_class("Shell_TrayWnd").into_iter().filter(|&h| h != ours).collect();
    all.extend(windows_of_class("Shell_SecondaryTrayWnd"));
    all
}

fn data(state: isize) -> APPBARDATA {
    APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: crate::host::explorer_tray(),
        uCallbackMessage: 0,
        uEdge: 0,
        rc: RECT::default(),
        lParam: state,
    }
}

fn state() -> isize {
    unsafe { SHAppBarMessage(ABM_GETSTATE, &mut data(0)) as isize }
}

fn set_state(state: isize) {
    unsafe { SHAppBarMessage(ABM_SETSTATE, &mut data(state)) };
}

/// Hide the taskbars, noting first how they were, and have the guard watch.
pub fn hide() {
    let now = state();
    // Only the first time: after that, what's on screen is our doing.
    if !saved().exists() {
        let _ = std::fs::create_dir_all(crate::settings::dir());
        if std::fs::write(saved(), now.to_string()).is_err() {
            return; // without a note of how it was, it couldn't be put back
        }
    }
    if now & ABS_AUTOHIDE == 0 {
        set_state(now | ABS_AUTOHIDE);
    }
    HIDING.store(true, Ordering::Relaxed);
    rehide();
    guard();
}

/// Hide any taskbar Explorer has shown again. Quick: it never waits on Explorer, so the host's
/// beat can call it.
pub fn rehide() {
    if !hiding() {
        return;
    }
    for t in taskbars() {
        if unsafe { IsWindowVisible(t) } != 0 {
            unsafe { ShowWindowAsync(t, SW_HIDE) };
        }
    }
}

/// Auto-hide set back by hand (or by Lintel, putting back what it found): set it again. Once a
/// second, off the host's thread, since this one waits on Explorer.
pub fn keep_autohide() {
    if hiding() {
        let now = state();
        if now & ABS_AUTOHIDE == 0 {
            set_state(now | ABS_AUTOHIDE);
        }
    }
}

/// For `--restore-taskbar` (sill-guard.exe, the uninstallers): put the taskbar back only if a Sill
/// that hid it ended without doing so, as the state file it leaves says. Otherwise leave it be:
/// Lintel may be hiding it. (The uninstallers quit Sill first.)
pub fn restore_if_left() {
    if saved().is_file() {
        restore();
    }
}

/// Put the taskbars back as they were, if Sill hid them (now, or in a run that didn't get to).
pub fn restore() {
    HIDING.store(false, Ordering::Relaxed);
    give_back_work_area();
    if let Ok(text) = std::fs::read_to_string(saved()) {
        if let Ok(was) = text.trim().parse::<isize>() {
            set_state(was);
        }
        let _ = std::fs::remove_file(saved());
    }
    for t in taskbars() {
        unsafe { ShowWindowAsync(t, SW_SHOWNA) };
    }
}

/// Start `sill-guard.exe` beside this exe, once, to put the taskbar back if Sill ends without
/// doing it. It's left alone if it's missing (a build without it): the next start still does.
fn guard() {
    if GUARDED.swap(true, Ordering::Relaxed) {
        return;
    }
    let Ok(me) = std::env::current_exe() else { return };
    let guard = me.with_file_name("sill-guard.exe");
    if !guard.is_file() {
        return;
    }
    use std::os::windows::process::CommandExt;
    // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP: no console, and not ended with Sill's.
    let _ = std::process::Command::new(guard).arg(std::process::id().to_string()).arg(&me).creation_flags(0x0000_0008 | 0x0000_0200).spawn();
}

/// Keep `dock` (the dock's strip) out of its screen's work area, so maximized windows stop at the
/// dock as they did at the taskbar. Only moves the edge the dock is on, and only inwards, so
/// Lintel's strip at the top stays as Explorer keeps it. Once a second, off the host's thread:
/// changing it tells every window.
pub fn keep_work_area(dock: RECT, edge: u32) {
    if !hiding() {
        return;
    }
    let (_, work) = screen_at(POINT { x: (dock.left + dock.right) / 2, y: (dock.top + dock.bottom) / 2 });
    let mut want = work;
    match edge {
        ABE_LEFT => want.left = want.left.max(dock.right),
        ABE_RIGHT => want.right = want.right.min(dock.left),
        _ => want.bottom = want.bottom.min(dock.top),
    }
    if want == work {
        return;
    }
    let mut w = WORK.lock().unwrap();
    let before = w.map_or(work, |(b, _)| b);
    let mut set = want;
    if unsafe { SystemParametersInfoW(SPI_SETWORKAREA, 0, (&mut set as *mut RECT).cast(), SPIF_SENDCHANGE) } != 0 {
        *w = Some((before, want));
    }
}

/// Undo `keep_work_area`, if the work area is still as Sill left it: on quitting, or before the
/// dock moves to another screen. Explorer works it out again when the taskbar's state is put back.
pub fn give_back_work_area() {
    let Some((before, ours)) = WORK.lock().unwrap().take() else { return };
    let (_, work) = screen_at(POINT { x: (ours.left + ours.right) / 2, y: (ours.top + ours.bottom) / 2 });
    let mut back = given_back(work, before, ours);
    if back != work {
        unsafe { SystemParametersInfoW(SPI_SETWORKAREA, 0, (&mut back as *mut RECT).cast(), SPIF_SENDCHANGE) };
    }
}

/// `work` with each edge Sill moved (from `before` to `ours`) put back, where it's still Sill's.
fn given_back(work: RECT, before: RECT, ours: RECT) -> RECT {
    let edge = |now: i32, b: i32, o: i32| if b != o && now == o { b } else { now };
    RECT {
        left: edge(work.left, before.left, ours.left),
        top: edge(work.top, before.top, ours.top),
        right: edge(work.right, before.right, ours.right),
        bottom: edge(work.bottom, before.bottom, ours.bottom),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_edges_sill_moved_go_back() {
        let r = |left, top, right, bottom| RECT { left, top, right, bottom };
        let before = r(0, 0, 2560, 1080);
        let ours = r(0, 0, 2560, 1016);
        // Lintel took the top meanwhile: that stays.
        assert_eq!(given_back(r(0, 30, 2560, 1016), before, ours), r(0, 30, 2560, 1080));
        // Someone else moved the bottom since: theirs now.
        assert_eq!(given_back(r(0, 0, 2560, 1000), before, ours), r(0, 0, 2560, 1000));
    }
}
