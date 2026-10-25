//! Hiding Windows' own taskbar while Lintel stands in for it: the taskbar is set to auto-hide, as
//! its own settings can, and put back as it was when Lintel quits or the setting is turned off.
//! Windows only auto-hides a taskbar at the bottom of the screen; elsewhere it ignores the request.
//!
//! What it was is kept in a file beside the settings until it's put back, so that if Lintel
//! doesn't get to quit cleanly, the next start (with the setting off) still restores it.

use crate::ffi::{wide, FindWindowW, SHAppBarMessage, ABE_BOTTOM, APPBARDATA, RECT};
use std::path::PathBuf;

const ABM_GETSTATE: u32 = 0x04;
const ABM_GETTASKBARPOS: u32 = 0x05;
const ABM_SETSTATE: u32 = 0x0a;
const ABS_AUTOHIDE: isize = 0x01;

fn saved() -> PathBuf {
    crate::settings::path().with_file_name("taskbar-state")
}

fn data(state: isize) -> APPBARDATA {
    let class = wide("Shell_TrayWnd");
    APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: unsafe { FindWindowW(class.as_ptr(), std::ptr::null()) },
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

/// Whether Windows will auto-hide the taskbar where it is: only at the bottom.
pub fn can_hide() -> bool {
    let mut d = data(0);
    unsafe { SHAppBarMessage(ABM_GETTASKBARPOS, &mut d) != 0 && d.uEdge == ABE_BOTTOM }
}

/// Hide the taskbar (or, with `hide` false, put it back as it was).
pub fn apply(hide: bool) {
    if !hide {
        return restore();
    }
    let now = state();
    // Only the first time: after that, what's on screen is our doing.
    if !saved().exists() && std::fs::write(saved(), now.to_string()).is_err() {
        return; // without a note of how it was, it couldn't be put back
    }
    if now & ABS_AUTOHIDE == 0 {
        set_state(now | ABS_AUTOHIDE);
    }
}

/// Put the taskbar back as it was before Lintel hid it, if it did.
pub fn restore() {
    let Ok(text) = std::fs::read_to_string(saved()) else { return };
    if let Ok(was) = text.trim().parse::<isize>() {
        set_state(was);
    }
    let _ = std::fs::remove_file(saved());
}
