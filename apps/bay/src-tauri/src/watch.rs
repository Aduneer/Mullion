//! Windows coming and going while the overview is open: opened, closed on their own, hidden away
//! to the notification area, minimized (when Bay leaves minimized windows out), or renamed.
//! Windows says so (WinEvent hooks), so nothing is polled: the hooks run on a thread of their own
//! that just waits for messages, and once a burst of changes settles, the open overview looks
//! again (`overview::sync_windows`). Bay's own windows are left out.

use crate::ffi::*;
use std::cell::Cell;
use std::sync::OnceLock;
use tauri::AppHandle;

const GA_ROOT: u32 = 2;

static APP: OnceLock<AppHandle> = OnceLock::new();

thread_local! {
    static TIMER: Cell<usize> = const { Cell::new(0) };
}

pub fn start(app: &AppHandle) {
    if APP.set(app.clone()).is_err() {
        return;
    }
    let _ = std::thread::Builder::new().name("watch".into()).spawn(|| unsafe {
        let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
        for (from, to) in [
            (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
            (EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE),
            (EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_NAMECHANGE),
            (EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED),
        ] {
            SetWinEventHook(from, to, 0, hook, 0, 0, flags);
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            DispatchMessageW(&msg);
        }
    });
}

unsafe extern "system" fn hook(_hook: isize, event: u32, hwnd: isize, object: i32, child: i32, _thread: u32, _time: u32) {
    // OBJID_WINDOW and CHILDID_SELF: the window itself, not a part of it. Every window's title
    // changes count, but only a top-level one's matter.
    if object != 0 || child != 0 || (event == EVENT_OBJECT_NAMECHANGE && GetAncestor(hwnd, GA_ROOT) != hwnd) {
        return;
    }
    // Windows come and go in bursts (an app opening, a tab torn off): look once it settles.
    TIMER.with(|t| {
        if t.get() != 0 {
            KillTimer(0, t.get());
        }
        t.set(SetTimer(0, 0, 120, settled));
    });
}

unsafe extern "system" fn settled(_hwnd: isize, _msg: u32, _id: usize, _time: u32) {
    TIMER.with(|t| {
        KillTimer(0, t.replace(0));
    });
    let Some(app) = APP.get() else { return };
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || crate::overview::sync_windows(&handle));
}
