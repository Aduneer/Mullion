//! The shortcut that opens the panel (Alt+Space unless changed), registered with Windows on a
//! thread of its own that waits for it. Changing it in settings swaps it on that thread, and says
//! whether Windows took the new one: another app may have it already.

use crate::ffi::*;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

const ID: i32 = 1;
/// The message that asks the thread to change the shortcut; its wParam is the new one's modifiers.
const WM_SET: u32 = WM_APP + 1;

pub struct Hotkey {
    thread: u32,
    replies: Mutex<Receiver<Result<(), String>>>,
}

/// "ctrl+alt+space" as RegisterHotKey's modifiers. Only shortcuts with Space are offered.
pub fn parse(spec: &str) -> Option<u32> {
    let mut mods = 0;
    let mut space = false;
    for part in spec.to_lowercase().split('+') {
        match part.trim() {
            "alt" => mods |= MOD_ALT,
            "ctrl" | "control" => mods |= MOD_CONTROL,
            "shift" => mods |= MOD_SHIFT,
            "win" => mods |= MOD_WIN,
            "space" => space = true,
            _ => return None,
        }
    }
    (space && mods != 0).then_some(mods)
}

impl Hotkey {
    /// Register `spec` and call `pressed` (on the hotkey's thread) each time it's pressed. The
    /// result says whether Windows took it; the thread stays either way, for a different one.
    pub fn start(spec: &str, pressed: impl Fn() + Send + 'static) -> (Self, Result<(), String>) {
        let (reply, replies) = channel();
        let (tid_tx, tid_rx) = channel();
        let first = parse(spec).unwrap_or(MOD_ALT);
        let _ = std::thread::Builder::new().name("hotkey".into()).spawn(move || run(first, pressed, reply, tid_tx));
        let thread = tid_rx.recv().unwrap_or(0);
        let hk = Hotkey { thread, replies: Mutex::new(replies) };
        let first = hk.replies.lock().unwrap().recv().unwrap_or_else(|_| Err("the shortcut couldn't be set up".into()));
        (hk, first)
    }

    pub fn set(&self, spec: &str) -> Result<(), String> {
        let mods = parse(spec).ok_or_else(|| format!("not a shortcut Fanlight offers: {spec}"))?;
        let replies = self.replies.lock().unwrap();
        if unsafe { PostThreadMessageW(self.thread, WM_SET, mods as usize, 0) } == 0 {
            return Err("the shortcut's thread has stopped".into());
        }
        replies.recv().unwrap_or_else(|_| Err("the shortcut's thread has stopped".into()))
    }
}

fn register(mods: u32) -> Result<(), String> {
    unsafe {
        UnregisterHotKey(0, ID);
        if RegisterHotKey(0, ID, mods | MOD_NOREPEAT, VK_SPACE as u32) != 0 {
            Ok(())
        } else {
            Err("another app already uses that shortcut".into())
        }
    }
}

fn run(first: u32, pressed: impl Fn(), reply: Sender<Result<(), String>>, tid: Sender<u32>) {
    unsafe {
        // Make the thread's message queue before anyone posts to it.
        let mut msg = MSG::default();
        PeekMessageW(&mut msg, 0, 0, 0, 0);
        let _ = tid.send(GetCurrentThreadId());
        let mut current = first;
        let r = register(current);
        if r.is_err() {
            current = 0;
        }
        let _ = reply.send(r);
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            match msg.message {
                WM_HOTKEY if msg.wParam == ID as usize => pressed(),
                WM_SET => {
                    let wanted = msg.wParam as u32;
                    let r = register(wanted);
                    match r {
                        Ok(()) => current = wanted,
                        // Keep the one that worked.
                        Err(_) if current != 0 => {
                            let _ = register(current);
                        }
                        Err(_) => {}
                    }
                    let _ = reply.send(r);
                }
                _ => {
                    DispatchMessageW(&msg);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_with_space() {
        assert_eq!(parse("alt+space"), Some(MOD_ALT));
        assert_eq!(parse("Ctrl+Alt+Space"), Some(MOD_CONTROL | MOD_ALT));
        assert_eq!(parse("win+alt+space"), Some(MOD_WIN | MOD_ALT));
        assert_eq!(parse("space"), None);
        assert_eq!(parse("alt+f4"), None);
        for h in crate::settings::HOTKEYS {
            assert!(parse(h).is_some(), "{h}");
        }
    }
}
