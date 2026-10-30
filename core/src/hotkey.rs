//! Global shortcuts ("alt+space", "win+ctrl+up"), registered with Windows on a thread of their own
//! that waits for them. Changing one swaps it on that thread, and says whether Windows took the
//! new one: another app may have it already.
//!
//! `Hotkey` is one shortcut (Fanlight's, Bay's); `Hotkeys` is a set of them, each with an id of
//! the app's choosing (Sash's).

use crate::ffi::*;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

/// The message that asks the thread to change a shortcut: wParam is its id, lParam the new one's
/// modifiers and key (`pack`), or 0 for none.
const WM_SET: u32 = WM_APP + 1;
/// The message that lets go of every shortcut (wParam 1), or takes them all back (0).
const WM_PAUSE: u32 = WM_APP + 2;

/// "ctrl+alt+space" as RegisterHotKey's modifiers and key. At least one modifier, and one key:
/// Space, Tab, Enter, Backspace, Delete, Insert, Home, End, Page Up or Down, an arrow, a letter,
/// a digit or F1 to F12.
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
            "enter" => VK_RETURN,
            "backspace" => VK_BACK,
            "delete" => VK_DELETE,
            "insert" => VK_INSERT,
            "home" => VK_HOME,
            "end" => VK_END,
            "pageup" => VK_PRIOR,
            "pagedown" => VK_NEXT,
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
    spec.split('+')
        .map(|p| match p {
            "pageup" => "PgUp".into(),
            "pagedown" => "PgDn".into(),
            p => p[..1].to_uppercase() + &p[1..],
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// RegisterHotKey's modifiers and key.
type Keys = (u32, u16);
/// Whether Windows took each shortcut, by id.
type Taken = Vec<(i32, Result<(), String>)>;
/// Each shortcut to start with, by id: its keys, if they're a shortcut, and whether it's meant to
/// be none.
type First = Vec<(i32, Option<Keys>, bool)>;

fn pack((mods, key): Keys) -> isize {
    ((mods as isize) << 16) | key as isize
}

fn unpack(l: isize) -> Option<Keys> {
    (l != 0).then_some(((l >> 16) as u32, (l & 0xffff) as u16))
}

/// A set of shortcuts on one thread.
pub struct Hotkeys {
    thread: u32,
    replies: Mutex<Receiver<Result<(), String>>>,
}

impl Hotkeys {
    /// Register each `(id, spec)` (an empty spec is none) and call `pressed` with its id (on the
    /// shortcuts' thread) each time one is pressed. Says, for each, whether Windows took it; the
    /// thread stays either way, for different ones.
    pub fn start(specs: &[(i32, &str)], pressed: impl Fn(i32) + Send + 'static) -> (Self, Taken) {
        let (reply, replies) = channel();
        let (tid_tx, tid_rx) = channel();
        let first: First = specs.iter().map(|&(id, s)| (id, parse(s), s.is_empty())).collect();
        let n = first.len();
        let _ = std::thread::Builder::new().name("hotkey".into()).spawn(move || run(first, pressed, reply, tid_tx));
        let thread = tid_rx.recv().unwrap_or(0);
        let hk = Hotkeys { thread, replies: Mutex::new(replies) };
        let results = {
            let replies = hk.replies.lock().unwrap();
            specs.iter().take(n).map(|&(id, _)| (id, replies.recv().unwrap_or_else(|_| Err("the shortcut couldn't be set up".into())))).collect()
        };
        (hk, results)
    }

    /// Change shortcut `id` to `spec`, or to none if it's empty. If Windows won't take the new
    /// one, the old one stays.
    pub fn set(&self, id: i32, spec: &str) -> Result<(), String> {
        let wanted = if spec.is_empty() { 0 } else { pack(parse(spec).ok_or_else(|| format!("not a shortcut: {spec}"))?) };
        let replies = self.replies.lock().unwrap();
        if unsafe { PostThreadMessageW(self.thread, WM_SET, id as usize, wanted) } == 0 {
            return Err("the shortcut's thread has stopped".into());
        }
        replies.recv().unwrap_or_else(|_| Err("the shortcut's thread has stopped".into()))
    }

    /// Let go of every shortcut, so the keys reach other apps (and a settings page recording a new
    /// one), or take them back. Changes while paused are kept and registered on resuming.
    pub fn pause(&self, paused: bool) {
        let replies = self.replies.lock().unwrap();
        if unsafe { PostThreadMessageW(self.thread, WM_PAUSE, paused as usize, 0) } != 0 {
            let _ = replies.recv();
        }
    }
}

/// One shortcut.
pub struct Hotkey(Hotkeys);

const ID: i32 = 1;

impl Hotkey {
    /// Register `spec` and call `pressed` (on the shortcut's thread) each time it's pressed. The
    /// result says whether Windows took it; the thread stays either way, for a different one.
    pub fn start(spec: &str, pressed: impl Fn() + Send + 'static) -> (Self, Result<(), String>) {
        // An empty spec here is a mistake, not "none".
        let spec = if spec.is_empty() { "?" } else { spec };
        let (hks, mut results) = Hotkeys::start(&[(ID, spec)], move |_| pressed());
        let first = results.pop().map(|(_, r)| r).unwrap_or_else(|| Err("the shortcut couldn't be set up".into()));
        (Hotkey(hks), first)
    }

    pub fn set(&self, spec: &str) -> Result<(), String> {
        if spec.is_empty() {
            return Err("not a shortcut".into());
        }
        self.0.set(ID, spec)
    }
}

fn register(id: i32, (mods, key): Keys) -> Result<(), String> {
    unsafe {
        UnregisterHotKey(0, id);
        if RegisterHotKey(0, id, mods | MOD_NOREPEAT, key as u32) != 0 {
            Ok(())
        } else {
            Err("another app already uses that shortcut".into())
        }
    }
}

fn run(first: First, pressed: impl Fn(i32), reply: Sender<Result<(), String>>, tid: Sender<u32>) {
    unsafe {
        // Make the thread's message queue before anyone posts to it.
        let mut msg = MSG::default();
        PeekMessageW(&mut msg, 0, 0, 0, 0);
        let _ = tid.send(GetCurrentThreadId());
        // The shortcuts Windows has taken, by id.
        let mut current: HashMap<i32, Keys> = HashMap::new();
        let mut paused = false;
        for (id, keys, none) in first {
            let r = match keys {
                Some(k) => register(id, k).map(|()| {
                    current.insert(id, k);
                }),
                None if none => Ok(()),
                None => Err("not a shortcut".into()),
            };
            let _ = reply.send(r);
        }
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            match msg.message {
                WM_HOTKEY if !paused => pressed(msg.wParam as i32),
                WM_SET => {
                    let id = msg.wParam as i32;
                    let r = match unpack(msg.lParam) {
                        None => {
                            UnregisterHotKey(0, id);
                            current.remove(&id);
                            Ok(())
                        }
                        Some(wanted) if paused => {
                            current.insert(id, wanted);
                            Ok(())
                        }
                        Some(wanted) => {
                            let r = register(id, wanted);
                            match (&r, current.get(&id).copied()) {
                                (Ok(()), _) => {
                                    current.insert(id, wanted);
                                }
                                // Keep the one that worked.
                                (Err(_), Some(was)) => {
                                    let _ = register(id, was);
                                }
                                (Err(_), None) => {}
                            }
                            r
                        }
                    };
                    let _ = reply.send(r);
                }
                WM_PAUSE => {
                    let pause = msg.wParam != 0;
                    if pause != paused {
                        paused = pause;
                        for (&id, &keys) in &current {
                            if paused {
                                UnregisterHotKey(0, id);
                            } else {
                                let _ = register(id, keys);
                            }
                        }
                    }
                    let _ = reply.send(Ok(()));
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
        assert_eq!(parse("ctrl+alt+enter"), Some((MOD_CONTROL | MOD_ALT, VK_RETURN)));
        assert_eq!(parse("ctrl+alt+backspace"), Some((MOD_CONTROL | MOD_ALT, VK_BACK)));
        assert_eq!(parse("ctrl+alt+shift+pagedown"), Some((MOD_CONTROL | MOD_ALT | MOD_SHIFT, VK_NEXT)));
        assert_eq!(parse("space"), None);
        assert_eq!(parse(""), None);
        assert_eq!(parse("alt+f13"), None);
        assert_eq!(parse("alt+space+tab"), None);
        assert_eq!(parse("alt+pause"), None);
    }

    #[test]
    fn labels_read_as_in_menus() {
        assert_eq!(label("alt+space"), "Alt+Space");
        assert_eq!(label("win+ctrl+up"), "Win+Ctrl+Up");
        assert_eq!(label("ctrl+alt+pageup"), "Ctrl+Alt+PgUp");
    }

    #[test]
    fn keys_survive_the_trip_to_the_thread() {
        for spec in ["alt+space", "ctrl+alt+shift+win+f12", "ctrl+alt+backspace"] {
            let keys = parse(spec).unwrap();
            assert_eq!(unpack(pack(keys)), Some(keys));
        }
        assert_eq!(unpack(0), None);
    }
}
