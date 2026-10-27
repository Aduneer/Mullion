//! A global shortcut ("alt+space", "win+ctrl+up"), registered with Windows on a thread of its own
//! that waits for it. Changing it swaps it on that thread, and says whether Windows took the new
//! one: another app may have it already.

use crate::ffi::*;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

const ID: i32 = 1;
/// The message that asks the thread to change the shortcut: wParam is the new one's modifiers,
/// lParam its key.
const WM_SET: u32 = WM_APP + 1;

pub struct Hotkey {
    thread: u32,
    replies: Mutex<Receiver<Result<(), String>>>,
}

/// "ctrl+alt+space" as RegisterHotKey's modifiers and key. At least one modifier, and one key:
/// Space, Tab, an arrow, a letter, a digit or F1 to F12.
pub fn parse(spec: &str) -> Option<(u32, u16)> {
    let mut mods = 0;
    let mut key = None;
    for part in spec.to_lowercase().split('+') {
        let k = match part.trim() {
            "alt" => {
                mods |= MOD_ALT;
                continue;
            }
            "ctrl" | "control" => {
                mods |= MOD_CONTROL;
                continue;
            }
            "shift" => {
                mods |= MOD_SHIFT;
                continue;
            }
            "win" => {
                mods |= MOD_WIN;
                continue;
            }
            "space" => VK_SPACE,
            "tab" => VK_TAB,
            "up" => VK_UP,
            "down" => VK_DOWN,
            "left" => VK_LEFT,
            "right" => VK_RIGHT,
            p if p.len() == 1 && p.as_bytes()[0].is_ascii_alphanumeric() => p.to_ascii_uppercase().as_bytes()[0] as u16,
            p => match p.strip_prefix('f').and_then(|n| n.parse::<u16>().ok()) {
                Some(n @ 1..=12) => VK_F1 + n - 1,
                _ => return None,
            },
        };
        if key.replace(k).is_some() {
            return None;
        }
    }
    key.filter(|_| mods != 0).map(|k| (mods, k))
}

/// "alt+space" as menus write it: "Alt+Space".
pub fn label(spec: &str) -> String {
    spec.split('+').map(|p| p[..1].to_uppercase() + &p[1..]).collect::<Vec<_>>().join("+")
}

impl Hotkey {
    /// Register `spec` and call `pressed` (on the shortcut's thread) each time it's pressed. The
    /// result says whether Windows took it; the thread stays either way, for a different one.
    pub fn start(spec: &str, pressed: impl Fn() + Send + 'static) -> (Self, Result<(), String>) {
        let (reply, replies) = channel();
        let (tid_tx, tid_rx) = channel();
        let first = parse(spec);
        let _ = std::thread::Builder::new().name("hotkey".into()).spawn(move || run(first, pressed, reply, tid_tx));
        let thread = tid_rx.recv().unwrap_or(0);
        let hk = Hotkey { thread, replies: Mutex::new(replies) };
        let first = hk.replies.lock().unwrap().recv().unwrap_or_else(|_| Err("the shortcut couldn't be set up".into()));
        (hk, first)
    }

    pub fn set(&self, spec: &str) -> Result<(), String> {
        let (mods, key) = parse(spec).ok_or_else(|| format!("not a shortcut: {spec}"))?;
        let replies = self.replies.lock().unwrap();
        if unsafe { PostThreadMessageW(self.thread, WM_SET, mods as usize, key as isize) } == 0 {
            return Err("the shortcut's thread has stopped".into());
        }
        replies.recv().unwrap_or_else(|_| Err("the shortcut's thread has stopped".into()))
    }
}

fn register((mods, key): (u32, u16)) -> Result<(), String> {
    unsafe {
        UnregisterHotKey(0, ID);
        if RegisterHotKey(0, ID, mods | MOD_NOREPEAT, key as u32) != 0 {
            Ok(())
        } else {
            Err("another app already uses that shortcut".into())
        }
    }
}

fn run(first: Option<(u32, u16)>, pressed: impl Fn(), reply: Sender<Result<(), String>>, tid: Sender<u32>) {
    unsafe {
        // Make the thread's message queue before anyone posts to it.
        let mut msg = MSG::default();
        PeekMessageW(&mut msg, 0, 0, 0, 0);
        let _ = tid.send(GetCurrentThreadId());
        let r = first.ok_or_else(|| "not a shortcut".to_string()).and_then(register);
        let mut current = first.filter(|_| r.is_ok());
        let _ = reply.send(r);
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            match msg.message {
                WM_HOTKEY if msg.wParam == ID as usize => pressed(),
                WM_SET => {
                    let wanted = (msg.wParam as u32, msg.lParam as u16);
                    let r = register(wanted);
                    match (&r, current) {
                        (Ok(()), _) => current = Some(wanted),
                        // Keep the one that worked.
                        (Err(_), Some(was)) => {
                            let _ = register(was);
                        }
                        (Err(_), None) => {}
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
    fn shortcuts_need_a_modifier_and_one_key() {
        assert_eq!(parse("alt+space"), Some((MOD_ALT, VK_SPACE)));
        assert_eq!(parse("Ctrl+Alt+Space"), Some((MOD_CONTROL | MOD_ALT, VK_SPACE)));
        assert_eq!(parse("win+ctrl+up"), Some((MOD_WIN | MOD_CONTROL, VK_UP)));
        assert_eq!(parse("win+shift+tab"), Some((MOD_WIN | MOD_SHIFT, VK_TAB)));
        assert_eq!(parse("ctrl+alt+f12"), Some((MOD_CONTROL | MOD_ALT, VK_F1 + 11)));
        assert_eq!(parse("ctrl+b"), Some((MOD_CONTROL, b'B' as u16)));
        assert_eq!(parse("space"), None);
        assert_eq!(parse("alt+f13"), None);
        assert_eq!(parse("alt+space+tab"), None);
        assert_eq!(parse("alt+pause"), None);
    }

    #[test]
    fn labels_read_as_in_menus() {
        assert_eq!(label("alt+space"), "Alt+Space");
        assert_eq!(label("win+ctrl+up"), "Win+Ctrl+Up");
    }
}
