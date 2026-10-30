//! Hearing the pointer reach a corner, without a low-level hook (which looks like spyware to Smart
//! App Control and sits in every mouse event's path) and without polling fast.
//!
//! A thread of its own with a hidden window registers for Raw Input from the mouse
//! (`RIDEV_INPUTSINK`, so it hears it whatever's in front), and on each `WM_INPUT` only asks where
//! the pointer is; the raw data is never read. That wakes it only while the mouse moves. Arriving
//! in a corner starts a one-shot timer for the dwell, since a pointer resting there brings no more
//! messages; when it ends, the corner fires if the pointer is still there. Nothing fires while a
//! mouse button is held (a drag into the corner) or a full-screen app or game is in front on that
//! screen. A slow timer stands in, once a second, if Raw Input ever goes quiet while the pointer
//! moves (seen once in the spike, never explained). The window being a hidden top-level one, not
//! message-only, is what brings it `WM_DISPLAYCHANGE`.

use crate::corners::{self, Corner, Screen};
use crate::ffi::*;
use crate::glow::Glow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ptr::null_mut;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// What the thread needs to know from the settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Config {
    pub dwell_ms: u32,
    /// The glow's colour, or None for no glow.
    pub glow: Option<[u8; 3]>,
    /// The corners that do something, by screen key, in `CORNERS` order.
    pub armed: HashMap<String, [bool; 4]>,
}

pub enum Event {
    /// A screen's corner fired: its key, and which corner.
    Fire(String, Corner),
    /// Screens came, went, moved or changed scaling.
    Screens(Vec<Screen>),
}

const TIMER_DWELL: usize = 1;
const TIMER_BACKSTOP: usize = 2;
const TIMER_GLOW: usize = 3;
/// About one frame.
const GLOW_FRAME_MS: u32 = 15;
/// Asks the thread to put its screens and glow right after new settings.
const WM_CONFIG: u32 = WM_APP + 1;

pub struct Watch {
    hwnd: isize,
    config: Arc<Mutex<Config>>,
}

impl Watch {
    /// Start listening on a thread of its own; `on` hears each corner that fires, on that thread.
    pub fn start(config: Config, on: impl Fn(Event) + Send + 'static) -> Watch {
        let config = Arc::new(Mutex::new(config));
        let (tx, rx) = std::sync::mpsc::channel();
        let shared = config.clone();
        std::thread::spawn(move || run(shared, Box::new(on), tx));
        Watch { hwnd: rx.recv().unwrap_or(0), config }
    }

    pub fn set(&self, config: Config) {
        *self.config.lock().unwrap() = config;
        unsafe { PostMessageW(self.hwnd, WM_CONFIG, 0, 0) };
    }

    /// Turn the screens off, as the power button can: asked of Windows through Mitre's own window.
    pub fn screens_off(&self) {
        unsafe { PostMessageW(self.hwnd, WM_SYSCOMMAND, SC_MONITORPOWER, 2) };
    }
}

/// One stay in one corner.
struct Visit {
    screen: usize,
    corner: Corner,
    /// It fired, or can't (a button held, a full-screen app, a corner with nothing to do): either
    /// way, nothing more until the pointer leaves.
    done: bool,
}

struct State {
    hwnd: isize,
    config: Arc<Mutex<Config>>,
    on: Box<dyn Fn(Event)>,
    screens: Vec<Screen>,
    visit: Option<Visit>,
    glow: Glow,
    /// For the backstop: when Raw Input last spoke, and where the pointer was a second ago.
    heard: Instant,
    was: (i32, i32),
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn run(config: Arc<Mutex<Config>>, on: Box<dyn Fn(Event)>, tx: std::sync::mpsc::Sender<isize>) {
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("MitreCorners");
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: wndproc,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: 0,
            hCursor: 0,
            hbrBackground: 0,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(WS_EX_TOOLWINDOW, class.as_ptr(), class.as_ptr(), WS_POPUP, 0, 0, 0, 0, 0, 0, instance, null_mut());
        let glow_class = wide("MitreGlow");
        let wc = WNDCLASSW { lpfnWndProc: DefWindowProcW, lpszClassName: glow_class.as_ptr(), ..wc };
        RegisterClassW(&wc);
        let glow = Glow::new(&glow_class, instance);
        let _ = tx.send(hwnd);

        let dev = RAWINPUTDEVICE { usUsagePage: 0x01, usUsage: 0x02, dwFlags: RIDEV_INPUTSINK, hwndTarget: hwnd };
        if hwnd == 0 || RegisterRawInputDevices(&dev, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32) == 0 {
            eprintln!("Mitre can't hear the mouse: {}", std::io::Error::last_os_error());
            return;
        }
        SetTimer(hwnd, TIMER_BACKSTOP, 1000, null_mut());
        let screens = corners::screens();
        STATE.with(|s| *s.borrow_mut() = Some(State { hwnd, config, on, screens, visit: None, glow, heard: Instant::now(), was: (0, 0) }));

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: isize, msg: u32, w: usize, l: isize) -> isize {
    if matches!(msg, WM_INPUT | WM_TIMER | WM_DISPLAYCHANGE | WM_DPICHANGED | WM_CONFIG) {
        STATE.with(|s| {
            // A message sent while the state is borrowed (by a call below) just waits its turn.
            let Ok(mut s) = s.try_borrow_mut() else { return };
            let Some(st) = s.as_mut() else { return };
            match (msg, w) {
                (WM_INPUT, _) => {
                    st.heard = Instant::now();
                    sample(st);
                }
                (WM_TIMER, TIMER_DWELL) => dwell(st),
                (WM_TIMER, TIMER_BACKSTOP) => backstop(st),
                (WM_TIMER, TIMER_GLOW) => {
                    if !st.glow.tick() {
                        KillTimer(hwnd, TIMER_GLOW);
                    }
                }
                (WM_DISPLAYCHANGE | WM_DPICHANGED | WM_CONFIG, _) => refresh(st, msg != WM_CONFIG),
                _ => {}
            }
        });
    }
    // DefWindowProc frees each WM_INPUT's data, and turns the screens off for `screens_off`.
    DefWindowProcW(hwnd, msg, w, l)
}

/// The screens changed, or the settings did: start afresh.
fn refresh(st: &mut State, screens_changed: bool) {
    unsafe { KillTimer(st.hwnd, TIMER_DWELL) };
    st.visit = None;
    st.glow.hide();
    if screens_changed {
        st.screens = corners::screens();
        (st.on)(Event::Screens(st.screens.clone()));
    }
    sample(st);
}

fn cursor() -> Option<(i32, i32)> {
    let mut p = POINT::default();
    (unsafe { GetCursorPos(&mut p) } != 0).then_some((p.x, p.y))
}

/// The pointer moved: does a visit to a corner start or end?
fn sample(st: &mut State) {
    let Some((x, y)) = cursor() else { return };
    let here = corners::corner_at(&st.screens, x, y);
    match (&st.visit, here) {
        (Some(v), Some((s, c))) if v.screen == s && v.corner == c => {}
        (_, Some((s, c))) => {
            let config = st.config.lock().unwrap().clone();
            let screen = &st.screens[s];
            let armed = config.armed.get(&screen.key).is_some_and(|a| a[c.index()]);
            let can = armed && !button_held() && full_screen(screen).is_none();
            st.visit = Some(Visit { screen: s, corner: c, done: !can });
            unsafe { KillTimer(st.hwnd, TIMER_DWELL) };
            st.glow.hide();
            if can {
                unsafe { SetTimer(st.hwnd, TIMER_DWELL, config.dwell_ms, null_mut()) };
                if let Some(color) = config.glow {
                    st.glow.grow(screen, c, config.dwell_ms, color);
                    unsafe { SetTimer(st.hwnd, TIMER_GLOW, GLOW_FRAME_MS, null_mut()) };
                }
            }
        }
        (Some(_), None) => {
            st.visit = None;
            unsafe { KillTimer(st.hwnd, TIMER_DWELL) };
            st.glow.fade();
        }
        (None, None) => {}
    }
}

/// The dwell's over: fire, if the pointer is still in the same corner and nothing forbids it.
fn dwell(st: &mut State) {
    unsafe { KillTimer(st.hwnd, TIMER_DWELL) };
    // The pointer may have left with no WM_INPUT since (moved by a program, say): look again.
    let here = cursor().and_then(|(x, y)| corners::corner_at(&st.screens, x, y));
    let Some(v) = st.visit.as_mut() else { return };
    if v.done || here != Some((v.screen, v.corner)) {
        return;
    }
    v.done = true;
    let screen = &st.screens[v.screen];
    if button_held() || full_screen(screen).is_some() {
        st.glow.hide();
        return;
    }
    st.glow.flash();
    (st.on)(Event::Fire(screen.key.clone(), v.corner));
}

/// Once a second: if the pointer moved and Raw Input said nothing, look where it is anyway.
fn backstop(st: &mut State) {
    let Some(p) = cursor() else { return };
    let moved = p != st.was;
    st.was = p;
    if moved && st.heard.elapsed().as_millis() > 1000 {
        sample(st);
    }
}

fn button_held() -> bool {
    [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_XBUTTON1, VK_XBUTTON2].iter().any(|&vk| unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000 != 0)
}

/// The windows of these programs are never a full-screen app: Explorer's (the desktop, Task View,
/// Start) and the suite's own (Bay's overview), so pushing the corner again can close them.
fn ours(exe: &str) -> bool {
    let name = exe.rsplit(['\\', '/']).next().unwrap_or(exe).to_lowercase();
    let name = name.trim_end_matches(".exe");
    name == "explorer" || name == "mullion" || mullion_core::suite::APPS.iter().any(|a| a.exe == name)
}

/// Why this screen's corners shouldn't fire now, if they shouldn't: a full-screen app or game in
/// front on it, or Windows saying one is (or a presentation is) running.
fn full_screen(screen: &Screen) -> Option<&'static str> {
    unsafe {
        let fg = GetForegroundWindow();
        let mut on_this_screen = false;
        if fg != 0 && fg != GetShellWindow() {
            let mut pid = 0u32;
            GetWindowThreadProcessId(fg, &mut pid);
            if image_path(pid).is_some_and(|p| ours(&p)) {
                return None;
            }
            on_this_screen = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST) == screen.monitor;
            let style = GetWindowLongW(fg, GWL_STYLE) as u32;
            let mut wr = RECT::default();
            GetWindowRect(fg, &mut wr);
            let m = screen.rect;
            // A window with a title bar isn't full screen even if it covers the screen: with the
            // taskbar hidden (as Lintel can), a maximized one does.
            if on_this_screen && style & WS_CAPTION != WS_CAPTION && wr.left <= m.left && wr.top <= m.top && wr.right >= m.right && wr.bottom >= m.bottom {
                return Some("a full-screen window");
            }
        }
        let mut q = 0i32;
        if SHQueryUserNotificationState(&mut q) == 0 {
            match q {
                QUNS_RUNNING_D3D_FULL_SCREEN => return Some("a full-screen game"),
                QUNS_PRESENTATION_MODE => return Some("a presentation"),
                // "Busy" is any full-screen app; it says nothing of which screen, so only this
                // screen's window in front counts.
                QUNS_BUSY if on_this_screen => return Some("a full-screen app"),
                _ => {}
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explorer_and_the_suite_are_never_full_screen() {
        assert!(ours(r"C:\Windows\explorer.exe"));
        assert!(ours(r"C:\Users\a\AppData\Local\Mullion\bay.exe"));
        assert!(ours(r"C:\Users\a\AppData\Local\Mullion\Mullion.exe"));
        assert!(!ours(r"C:\Program Files\Game\game.exe"));
        assert!(!ours(r"C:\Apps\bayside.exe"));
    }
}
