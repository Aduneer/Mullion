//! The overview: a window over each screen's work area (so the taskbar and Lintel's bar stay), made
//! when Bay starts and kept hidden with its page loaded, so the shortcut only has to show it.
//!
//! Opening, it asks Windows for the windows Alt+Tab would list and registers a live DWM thumbnail
//! of each on its own screen's overview, exactly over the window, with one of the desktop
//! (wallpaper and icons) under them all. So when the overview shows, the screen looks just as it
//! did. Then the windows fly to a grid while the desktop fades out, and the page underneath shows
//! through: the wallpaper blurred and dimmed, and each window's icon and title. Closing runs it
//! backwards. DWM draws the thumbnails above the page, so the page draws only around them.
//!
//! A window dragged onto another screen moves there: its thumbnail follows the pointer from
//! overview to overview, and both screens' grids make room. Desktops are left to Windows' own
//! Task View, which the page's Desktops button opens as Bay closes.

use crate::ffi::*;
use crate::flight::{self, Fade, Flight};
use crate::grid::{self, Rect};
use crate::wallpaper::{self, Desktop};
use crate::SettingsState;
use mullion_core::{icons, look, thumbs, wins};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tauri::window::Color;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

/// Around the grid, the room under each thumbnail for its icon and title, and between cells, in
/// CSS pixels.
const MARGIN: f64 = 56.0;
const LABEL: f64 = 40.0;
const GAP: f64 = 40.0;
/// How long the windows take to fly, in ms; `BAY_MS` slows it down to look at.
fn duration() -> f64 {
    std::env::var("BAY_MS").ok().and_then(|s| s.parse().ok()).unwrap_or(250.0)
}

/// One screen and its overview window. Rects are [left, top, right, bottom] in physical pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Screen {
    pub label: String,
    pub hwnd: isize,
    pub monitor: [i32; 4],
    pub work: [i32; 4],
    pub scale: f64,
}

impl Screen {
    /// A rect on the screen, in this overview's client area.
    fn local(&self, r: RECT) -> Rect {
        Rect { x: (r.left - self.work[0]) as f64, y: (r.top - self.work[1]) as f64, w: (r.right - r.left) as f64, h: (r.bottom - r.top) as f64 }
    }
}

/// One window as its screen's page draws it: its label and ring, in CSS pixels.
#[derive(Serialize, Clone, Debug)]
pub struct Slot {
    pub hwnd: isize,
    pub pid: u32,
    pub title: String,
    /// What its icon is looked up by: the program's path.
    pub icon_key: String,
    pub icon: Option<String>,
    /// Drawn by the page as a card: a minimized window has no picture to show.
    pub minimized: bool,
    /// Its place in the stack of windows, 0 at the front.
    pub z: usize,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// What a page shows while the overview is open.
#[derive(Serialize, Clone, Debug, Default)]
pub struct Shown {
    pub slots: Vec<Slot>,
    pub ms: f64,
}

/// What a page draws behind the windows: its screen's wallpaper, and where it goes.
#[derive(Serialize, Clone, Debug)]
pub struct Backdrop {
    /// The picture (served by the `wallpaper` scheme), or None for a plain colour.
    pub url: Option<String>,
    pub fit: &'static str,
    pub color: String,
    pub monitor: [i32; 4],
    pub work: [i32; 4],
    /// Every screen together, for a picture spanned across them.
    pub span: [i32; 4],
    pub scale: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Closed,
    Opening,
    Open,
    Closing,
}

type Found = (usize, wins::Raw, RECT, Option<RECT>);

/// A window in the open overview, for picking it.
#[derive(Clone, Copy, Debug)]
struct Win {
    hwnd: isize,
    pid: u32,
    screen: usize,
    /// Its flight, or None for a minimized window's card.
    flight: Option<usize>,
    /// Where the window is, and its place in the grid, in its overview's client area.
    from: Rect,
    to: Rect,
}

pub struct Overview {
    screens: Vec<Screen>,
    phase: Phase,
    /// A close asked for while still opening, and what was picked: done when the opening ends.
    queued: Option<Option<(isize, u32)>>,
    flights: Vec<Flight>,
    fades: Vec<Fade>,
    windows: Vec<Win>,
    shown: Vec<Shown>,
    /// The window in front before Bay opened, to give the keyboard back to.
    before: isize,
    desktop: Desktop,
    /// Counts the wallpapers seen, so the pages ask for a new one.
    paper_version: u32,
    /// The window being dragged, until it and the grids it changed have settled. A close asked
    /// for meanwhile waits in `queued`.
    drag: Option<isize>,
    /// Open Windows' Task View once closed: the Desktops button.
    task_view: bool,
}

/// The overview, and the icons found so far by program path.
pub struct State(Mutex<Overview>, Mutex<HashMap<String, Option<String>>>);

impl State {
    pub fn new() -> Self {
        let o = Overview {
            screens: Vec::new(),
            phase: Phase::Closed,
            queued: None,
            flights: Vec::new(),
            fades: Vec::new(),
            windows: Vec::new(),
            shown: Vec::new(),
            before: 0,
            desktop: Desktop::default(),
            paper_version: 0,
            drag: None,
            task_view: false,
        };
        State(Mutex::new(o), Mutex::new(HashMap::new()))
    }
}

// --- screens ---------------------------------------------------------------------------

/// Make an overview window for every screen, or move them to where the screens are now. Only
/// does anything when screens come, go or change, so the first open after that is slower.
pub fn sync_screens(app: &AppHandle) {
    let mut want: Vec<([i32; 4], [i32; 4], f64)> = app
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            let (p, s, w) = (m.position(), m.size(), m.work_area());
            let monitor = [p.x, p.y, p.x + s.width as i32, p.y + s.height as i32];
            let work = [w.position.x, w.position.y, w.position.x + w.size.width as i32, w.position.y + w.size.height as i32];
            (monitor, work, m.scale_factor())
        })
        .collect();
    // Left to right, so the screens' numbers follow them.
    want.sort_by_key(|(m, ..)| (m[0], m[1]));
    {
        let o = app.state::<State>();
        let o = o.0.lock().unwrap();
        let same = o.screens.len() == want.len() && o.screens.iter().zip(&want).all(|(s, (m, w, f))| s.monitor == *m && s.work == *w && s.scale == *f);
        if same {
            return;
        }
    }
    // Not under the lock: making a window waits on the event loop, and a page may ask for it.
    let mut screens = Vec::new();
    for (i, (monitor, work, scale)) in want.into_iter().enumerate() {
        let label = format!("bay-{i}");
        let w = match app.get_webview_window(&label) {
            Some(w) => w,
            None => match make_window(app, &label) {
                Ok(w) => w,
                Err(_) => continue,
            },
        };
        // Moved first, so a screen with other scaling sizes it at that screen's scale.
        let _ = w.set_position(PhysicalPosition::new(work[0], work[1]));
        let _ = w.set_size(PhysicalSize::new((work[2] - work[0]) as u32, (work[3] - work[1]) as u32));
        let Ok(hwnd) = w.hwnd() else { continue };
        screens.push(Screen { label, hwnd: hwnd.0 as isize, monitor, work, scale });
    }
    for (label, w) in app.webview_windows() {
        if label.starts_with("bay-") && !screens.iter().any(|s| s.label == label) {
            let _ = w.destroy();
        }
    }
    app.state::<State>().0.lock().unwrap().screens = screens;
    refresh_backdrops(app);
}

fn make_window(app: &AppHandle, label: &str) -> tauri::Result<tauri::WebviewWindow> {
    WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title("Bay")
        .decorations(false)
        .resizable(false)
        .shadow(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(false)
        .focused(false)
        .background_color(Color(13, 13, 13, 255))
        .additional_browser_args(look::BROWSER_ARGS)
        .build()
}

/// Which screen a window is on (the one with most of it).
fn screen_of(screens: &[Screen], hwnd: isize) -> Option<usize> {
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    unsafe {
        let m = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if GetMonitorInfoW(m, &mut info) == 0 {
            return None;
        }
    }
    let r = info.rcMonitor;
    screens.iter().position(|s| s.monitor == [r.left, r.top, r.right, r.bottom])
}

/// The screen the pointer is on.
fn pointer_screen(screens: &[Screen]) -> usize {
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p) };
    screen_at(screens, p).unwrap_or(0)
}

/// The screen a point is on, if any.
fn screen_at(screens: &[Screen], p: POINT) -> Option<usize> {
    screens.iter().position(|s| p.x >= s.monitor[0] && p.x < s.monitor[2] && p.y >= s.monitor[1] && p.y < s.monitor[3])
}

/// Where a screen's grid goes: its overview less the margins.
fn grid_area(sc: &Screen) -> Rect {
    let m = MARGIN * sc.scale;
    Rect { x: m, y: m, w: (sc.work[2] - sc.work[0]) as f64 - 2.0 * m, h: (sc.work[3] - sc.work[1]) as f64 - 2.0 * m }
}

/// The desktop's own window: the wallpaper and the icons, across every screen.
fn desktop_window() -> Option<isize> {
    let class = wide("Progman");
    let hwnd = unsafe { FindWindowW(class.as_ptr(), std::ptr::null()) };
    (hwnd != 0).then_some(hwnd)
}

// --- opening and closing -----------------------------------------------------------------

pub fn toggle(app: &AppHandle) {
    let phase = app.state::<State>().0.lock().unwrap().phase;
    match phase {
        Phase::Closed => open(app),
        _ => close(app, None),
    }
}

/// Show the overview: every window where it is, then flying to its place. On the main thread.
pub fn open(app: &AppHandle) {
    sync_screens(app);
    let settings = app.state::<SettingsState>().0.lock().unwrap().clone();
    let state = app.state::<State>();
    let mut o = state.0.lock().unwrap();
    if o.phase != Phase::Closed || o.screens.is_empty() {
        return;
    }
    o.before = unsafe { GetForegroundWindow() };
    let own = std::process::id();
    // Each screen's windows: their place in the stack, what Windows says, where each is, and the
    // part of it to show.
    let mut per: Vec<Vec<Found>> = vec![Vec::new(); o.screens.len()];
    let listed = wins::enumerate().into_iter().filter(|w| wins::listed(w, own) && (settings.minimized || !w.minimized));
    for (z, raw) in listed.enumerate() {
        let Some((seen, crop)) = thumbs::bounds(raw.hwnd) else { continue };
        let Some(s) = screen_of(&o.screens, raw.hwnd) else { continue };
        per[s].push((z, raw, seen, crop));
    }

    let progman = desktop_window();
    let mut desktop_rect = RECT::default();
    if let Some(p) = progman {
        unsafe { GetWindowRect(p, &mut desktop_rect) };
    }
    let (mut flights, mut fades, mut windows, mut shown) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut missing = Vec::new();
    let known = state.1.lock().unwrap().clone();
    for (s, list) in per.into_iter().enumerate() {
        let sc = &o.screens[s];
        let (w, h) = ((sc.work[2] - sc.work[0]) as f64, (sc.work[3] - sc.work[1]) as f64);
        // The desktop first, so it's under every window.
        if let Some(id) = progman.and_then(|p| thumbs::register(sc.hwnd, p)) {
            let crop = RECT {
                left: sc.work[0] - desktop_rect.left,
                top: sc.work[1] - desktop_rect.top,
                right: sc.work[2] - desktop_rect.left,
                bottom: sc.work[3] - desktop_rect.top,
            };
            fades.push(Fade { id, crop: Some(crop), at: Rect { x: 0.0, y: 0.0, w, h } });
        }
        let froms: Vec<Rect> = list.iter().map(|(_, _, seen, _)| sc.local(*seen)).collect();
        let tos = grid::layout(&froms, grid_area(sc), GAP * sc.scale, LABEL * sc.scale);
        let mut slots = Vec::new();
        // Bottom-most first: later thumbnails draw on top, so the stack stays as it was.
        for (i, (z, raw, _, crop)) in list.into_iter().enumerate().rev() {
            let (from, to) = (froms[i], tos[i]);
            let flight = if raw.minimized {
                None
            } else {
                let Some(id) = thumbs::register(sc.hwnd, raw.hwnd) else { continue };
                flights.push(Flight { id, crop, from, to });
                Some(flights.len() - 1)
            };
            windows.push(Win { hwnd: raw.hwnd, pid: raw.pid, screen: s, flight, from, to });
            let icon_key = image_path(raw.pid).unwrap_or_default();
            let icon = known.get(&icon_key).cloned().flatten();
            if !known.contains_key(&icon_key) && !missing.contains(&icon_key) {
                missing.push(icon_key.clone());
            }
            let f = sc.scale;
            if std::env::var_os("BAY_TRACE").is_some() {
                let (cx, cy) = (sc.work[0] + (to.x + to.w / 2.0) as i32, sc.work[1] + (to.y + to.h / 2.0) as i32);
                eprintln!("bay: {:?} on screen {s}, its middle at {cx},{cy}", raw.title);
            }
            slots.push(Slot { hwnd: raw.hwnd, pid: raw.pid, title: raw.title, icon_key, icon, minimized: raw.minimized, z, x: to.x / f, y: to.y / f, w: to.w / f, h: to.h / f });
        }
        slots.reverse();
        shown.push(Shown { slots, ms: duration() });
    }
    flight::place_all(&flights, &fades, 0.0);
    o.phase = Phase::Opening;
    o.flights = flights.clone();
    o.fades = fades.clone();
    o.windows = windows;
    o.shown = shown.clone();
    let screens = o.screens.clone();
    drop(o);

    for (sc, sh) in screens.iter().zip(&shown) {
        let _ = app.emit_to(sc.label.as_str(), "open", sh);
    }
    let front = pointer_screen(&screens);
    for (i, sc) in screens.iter().enumerate() {
        if i != front {
            if let Some(w) = app.get_webview_window(&sc.label) {
                let _ = w.show();
            }
        }
    }
    if let Some(w) = app.get_webview_window(&screens[front].label) {
        let _ = w.show();
        let _ = w.set_focus();
    }
    find_icons(app, missing);
    refresh_backdrops(app);

    let app = app.clone();
    std::thread::spawn(move || {
        let (steps, longest) = flight::animate(&flights, &fades, true, duration());
        if std::env::var_os("BAY_TRACE").is_some() {
            eprintln!("bay: opened in {steps} steps, the longest {longest:.1} ms");
        }
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || opened(&handle));
    });
}

fn opened(app: &AppHandle) {
    let queued = {
        let state = app.state::<State>();
        let mut o = state.0.lock().unwrap();
        if o.phase != Phase::Opening {
            return;
        }
        o.phase = Phase::Open;
        o.queued.take()
    };
    let _ = app.emit("opened", ());
    if let Some(picked) = queued {
        close(app, picked);
    }
}

/// Close the overview: every window flies back, then it hides. With a window picked, that one is
/// brought forward first (while Bay is still in front, so Windows lets it), and its thumbnail
/// flies on top of the others to where it is.
pub fn close(app: &AppHandle, picked: Option<(isize, u32)>) {
    let state = app.state::<State>();
    let mut o = state.0.lock().unwrap();
    match o.phase {
        Phase::Closed | Phase::Closing => return,
        Phase::Opening => {
            o.queued = Some(picked);
            return;
        }
        Phase::Open if o.drag.is_some() => {
            o.queued = Some(picked);
            return;
        }
        Phase::Open => {}
    }
    o.phase = Phase::Closing;
    if let Some((hwnd, pid)) = picked {
        let _ = wins::focus(hwnd, pid);
        if let Some(win) = o.windows.iter().find(|w| w.hwnd == hwnd).copied() {
            let sc = o.screens[win.screen].clone();
            if let Some(i) = win.flight {
                thumbs::unregister(o.flights[i].id);
            }
            // Again, so it's on top; and from where it is now, as a minimized one has just been
            // restored.
            if let (Some((seen, crop)), Some(id)) = (thumbs::bounds(hwnd), thumbs::register(sc.hwnd, hwnd)) {
                let f = Flight { id, crop, from: sc.local(seen), to: win.to };
                thumbs::place(id, crop, flight::rect(win.to), 255);
                match win.flight {
                    Some(i) => o.flights[i] = f,
                    None => o.flights.push(f),
                }
            }
        }
    }
    let (flights, fades) = (o.flights.clone(), o.fades.clone());
    drop(o);
    let _ = app.emit("close", ());
    let app = app.clone();
    std::thread::spawn(move || {
        flight::animate(&flights, &fades, false, duration());
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || closed(&handle, picked));
    });
}

fn closed(app: &AppHandle, picked: Option<(isize, u32)>) {
    let state = app.state::<State>();
    let mut o = state.0.lock().unwrap();
    for sc in &o.screens {
        if let Some(w) = app.get_webview_window(&sc.label) {
            let _ = w.hide();
        }
    }
    let (flights, fades) = (std::mem::take(&mut o.flights), std::mem::take(&mut o.fades));
    for id in flights.iter().map(|f| f.id).chain(fades.iter().map(|f| f.id)) {
        thumbs::unregister(id);
    }
    o.windows.clear();
    o.shown.clear();
    o.phase = Phase::Closed;
    let before = o.before;
    let task_view = std::mem::take(&mut o.task_view);
    drop(o);
    let _ = app.emit("closed", ());
    match picked {
        Some((hwnd, pid)) => land(hwnd, pid),
        None => unsafe {
            if before != 0 && IsWindow(before) != 0 {
                SetForegroundWindow(before);
            }
        },
    }
    if task_view {
        send_keys(&[VK_LWIN, VK_TAB]);
    }
}

/// The Desktops button: close, then open Windows' Task View, where desktops are made, switched
/// and moved between. Bay leaves that to Windows: the only way to move another program's window
/// to another desktop is through interfaces Windows doesn't document, which change between
/// versions.
pub fn desktops(app: &AppHandle) {
    app.state::<State>().0.lock().unwrap().task_view = true;
    close(app, None);
}

/// With Bay gone, put the picked window on top and in front again (bringing it forward while Bay
/// covered the screen only made it the active one), and the pointer in its middle, where its
/// thumbnail landed.
fn land(hwnd: isize, pid: u32) {
    if wins::check(hwnd, pid).is_err() {
        return;
    }
    unsafe {
        SetWindowPos(hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
        BringWindowToTop(hwnd);
    }
    let _ = wins::focus(hwnd, pid);
    if let Some((seen, _)) = thumbs::bounds(hwnd) {
        unsafe { SetCursorPos((seen.left + seen.right) / 2, (seen.top + seen.bottom) / 2) };
    }
}

/// A window was picked on a page: only one the overview shows, still that program's.
pub fn pick(app: &AppHandle, hwnd: isize, pid: u32) -> Result<(), String> {
    let known = app.state::<State>().0.lock().unwrap().windows.iter().any(|w| w.hwnd == hwnd && w.pid == pid);
    if !known {
        return Err("not a window in the overview".into());
    }
    wins::check(hwnd, pid)?;
    close(app, Some((hwnd, pid)));
    Ok(())
}

/// An overview lost the keyboard. If it went to another of Bay's screens, that's Tab or a click;
/// anywhere else (the taskbar, Alt+Tab), the overview closes.
pub fn focus_left(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        // Once the new window has it.
        std::thread::sleep(Duration::from_millis(60));
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let (phase, ours) = {
                let state = handle.state::<State>();
                let o = state.0.lock().unwrap();
                let front = unsafe { GetForegroundWindow() };
                // A window moved to another screen can take the keyboard for a moment.
                (o.phase, o.drag.is_some() || o.screens.iter().any(|s| s.hwnd == front))
            };
            if matches!(phase, Phase::Open | Phase::Opening) && !ours {
                close(&handle, None);
            }
        });
    });
}

/// Tab past the last window on one screen: on to the next screen's overview (`dir` 1) or back to
/// the one before (-1).
pub fn next_screen(app: &AppHandle, from: &str, dir: i32) {
    let screens = app.state::<State>().0.lock().unwrap().screens.clone();
    let Some(i) = screens.iter().position(|s| s.label == from) else { return };
    let n = screens.len() as i32;
    let next = &screens[(i as i32 + dir).rem_euclid(n) as usize];
    if let Some(w) = app.get_webview_window(&next.label) {
        let _ = w.set_focus();
    }
    let _ = app.emit_to(next.label.as_str(), "enter", dir);
}

// --- dragging a window to another screen -----------------------------------------------------

/// What a page needs after its grid changed: the windows, and that they're moving.
#[derive(Serialize, Clone)]
struct Moved<'a> {
    shown: &'a Shown,
}

/// A window's thumbnail was pulled out of its place on a page, held `fx`, `fy` of the way across
/// it. From here the backend follows the pointer, not the page: a page only hears the mouse
/// over its own screen, and a thumbnail only shows in the overview it's registered with, so
/// when the pointer crosses to another screen the thumbnail goes with it.
pub fn drag(app: &AppHandle, hwnd: isize, pid: u32, fx: f64, fy: f64) -> Result<(), String> {
    let state = app.state::<State>();
    let mut o = state.0.lock().unwrap();
    if o.phase != Phase::Open || o.drag.is_some() {
        return Err("not now".into());
    }
    let win = *o.windows.iter().find(|w| w.hwnd == hwnd && w.pid == pid).ok_or("not a window in the overview")?;
    // A minimized window has no thumbnail to carry, and stays where it'll come back.
    let fi = win.flight.ok_or("a minimized window stays on its screen")?;
    wins::check(hwnd, pid)?;
    if elevated(pid) && !elevated(std::process::id()) {
        let title = o.shown[win.screen].slots.iter().find(|s| s.hwnd == hwnd).map(|s| s.title.clone()).unwrap_or_default();
        return Err(format!("{title} runs as administrator, so Windows won’t let Bay move it"));
    }
    o.drag = Some(hwnd);
    let (screens, flight) = (o.screens.clone(), o.flights[fi]);
    drop(o);
    let (fx, fy) = (fx.clamp(0.0, 1.0), fy.clamp(0.0, 1.0));
    let app = app.clone();
    std::thread::spawn(move || {
        let button = if unsafe { GetSystemMetrics(SM_SWAPBUTTON) } != 0 { VK_RBUTTON } else { VK_LBUTTON };
        let size = flight.to;
        let (mut id, mut on) = (flight.id, None);
        let at = loop {
            let mut p = POINT::default();
            unsafe { GetCursorPos(&mut p) };
            let s = screen_at(&screens, p).or(on).unwrap_or(win.screen);
            // Registered again on the first step too, even on its own screen: the newest
            // thumbnail draws on top, so it passes over the others.
            if on != Some(s) {
                if let Some(new) = thumbs::register(screens[s].hwnd, hwnd) {
                    thumbs::unregister(id);
                    id = new;
                    on = Some(s);
                    app.state::<State>().0.lock().unwrap().flights[fi].id = id;
                }
            }
            let sc = &screens[on.unwrap_or(win.screen)];
            let at = Rect { x: (p.x - sc.work[0]) as f64 - fx * size.w, y: (p.y - sc.work[1]) as f64 - fy * size.h, w: size.w, h: size.h };
            thumbs::place(id, flight.crop, flight::rect(at), 255);
            thumbs::next_frame();
            if unsafe { GetAsyncKeyState(button) } as u16 & 0x8000 == 0 {
                break at;
            }
        };
        let on = on.unwrap_or(win.screen);
        let moved = on != win.screen && move_window(hwnd, &screens[win.screen], &screens[on]);
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || dropped(&handle, hwnd, on, at, moved));
    });
    Ok(())
}

/// Move a window from one screen to another: to the same place on the new screen's work area,
/// relative to its size, and no bigger than it. A maximized window is maximized there. Blocks
/// while the window's program answers, so it runs off the main thread. False if the window
/// didn't move (an elevated program's windows can't be moved by Bay).
fn move_window(hwnd: isize, from: &Screen, to: &Screen) -> bool {
    let mut r = RECT::default();
    unsafe {
        let zoomed = IsZoomed(hwnd) != 0;
        if zoomed {
            ShowWindow(hwnd, SW_RESTORE);
        }
        if GetWindowRect(hwnd, &mut r) == 0 {
            return false;
        }
        let at = place_on(r, from.work, to.work);
        let ok = SetWindowPos(hwnd, 0, at.left, at.top, at.right - at.left, at.bottom - at.top, SWP_NOZORDER | SWP_NOACTIVATE) != 0;
        if zoomed {
            ShowWindow(hwnd, SW_MAXIMIZE);
        }
        if !ok {
            return false;
        }
    }
    // Once the window has moved, and resized for the new screen's scale if it does that.
    let mut last = RECT::default();
    for _ in 0..12 {
        thumbs::next_frame();
        let mut now = RECT::default();
        unsafe { GetWindowRect(hwnd, &mut now) };
        if now == last {
            break;
        }
        last = now;
    }
    screen_of(std::slice::from_ref(to), hwnd) == Some(0)
}

/// A window's rect on one work area, put in the same place on another: its middle as far across
/// and down, kept inside, and shrunk if it's bigger.
fn place_on(r: RECT, from: [i32; 4], to: [i32; 4]) -> RECT {
    let (fw, fh) = ((from[2] - from[0]).max(1) as f64, (from[3] - from[1]).max(1) as f64);
    let (tw, th) = (to[2] - to[0], to[3] - to[1]);
    let (w, h) = ((r.right - r.left).min(tw), (r.bottom - r.top).min(th));
    let cx = to[0] as f64 + ((r.left + r.right) as f64 / 2.0 - from[0] as f64) / fw * tw as f64;
    let cy = to[1] as f64 + ((r.top + r.bottom) as f64 / 2.0 - from[1] as f64) / fh * th as f64;
    let left = ((cx - w as f64 / 2.0).round() as i32).clamp(to[0], to[2] - w);
    let top = ((cy - h as f64 / 2.0).round() as i32).clamp(to[1], to[3] - h);
    RECT { left, top, right: left + w, bottom: top + h }
}

/// The drag ended with the thumbnail at `at` on screen `on`. If the window moved there, both
/// screens' grids are laid out again; either way the windows fly to their places.
fn dropped(app: &AppHandle, hwnd: isize, on: usize, at: Rect, moved: bool) {
    let state = app.state::<State>();
    let mut o = state.0.lock().unwrap();
    let Some(wi) = o.windows.iter().position(|w| w.hwnd == hwnd) else { return };
    let (origin, fi) = (o.windows[wi].screen, o.windows[wi].flight.unwrap_or(0));
    let mut flights = Vec::new();
    let mut from_at = at;
    if moved {
        let sc = o.screens[on].clone();
        o.windows[wi].screen = on;
        if let Some((seen, crop)) = thumbs::bounds(hwnd) {
            o.windows[wi].from = sc.local(seen);
            o.flights[fi].crop = crop;
        }
        o.flights[fi].from = o.windows[wi].from;
        if let Some(k) = o.shown[origin].slots.iter().position(|s| s.hwnd == hwnd) {
            let slot = o.shown[origin].slots.remove(k);
            let to = &mut o.shown[on].slots;
            let at = to.iter().position(|s| s.z > slot.z).unwrap_or(to.len());
            to.insert(at, slot);
        }
        for s in [origin, on] {
            for (i, old, new) in relayout(&mut o, s) {
                if let Some(f) = o.windows[i].flight.filter(|&f| f != fi) {
                    flights.push(Flight { from: old, to: new, ..o.flights[f] });
                }
            }
        }
    } else if on != origin {
        // It couldn't go: back to its own screen, from where it was dropped (as near as that
        // screen can show it).
        let sc = o.screens[origin].clone();
        if let Some(id) = thumbs::register(sc.hwnd, hwnd) {
            thumbs::unregister(o.flights[fi].id);
            o.flights[fi].id = id;
        }
        let a = grid_area(&sc);
        from_at = Rect { x: at.x.clamp(a.x, a.x + a.w - at.w), y: at.y.clamp(a.y, a.y + a.h - at.h), ..at };
    }
    let to = o.windows[wi].to;
    flights.push(Flight { from: from_at, to, ..o.flights[fi] });
    let label = |s: usize| o.screens[s].label.clone();
    for s in if moved { vec![origin, on] } else { vec![origin] } {
        let _ = app.emit_to(label(s).as_str(), "moved", Moved { shown: &o.shown[s] });
    }
    let to_focus = label(on);
    drop(o);
    if let Some(w) = app.get_webview_window(&to_focus) {
        let _ = w.set_focus();
    }
    let app = app.clone();
    std::thread::spawn(move || {
        flight::animate(&flights, &[], true, duration());
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let queued = {
                let state = handle.state::<State>();
                let mut o = state.0.lock().unwrap();
                o.drag = None;
                o.queued.take()
            };
            if let Some(picked) = queued {
                close(&handle, picked);
            }
        });
    });
}

/// Lay screen `s`'s grid out again for the windows on it now. Returns each window that's on it,
/// with its old place and its new one.
fn relayout(o: &mut Overview, s: usize) -> Vec<(usize, Rect, Rect)> {
    let sc = o.screens[s].clone();
    let on: Vec<usize> = (0..o.windows.len()).filter(|&i| o.windows[i].screen == s).collect();
    let froms: Vec<Rect> = on.iter().map(|&i| o.windows[i].from).collect();
    let tos = grid::layout(&froms, grid_area(&sc), GAP * sc.scale, LABEL * sc.scale);
    let mut out = Vec::new();
    for (&i, &to) in on.iter().zip(&tos) {
        let (hwnd, old) = (o.windows[i].hwnd, o.windows[i].to);
        o.windows[i].to = to;
        if let Some(f) = o.windows[i].flight {
            o.flights[f].to = to;
        }
        if let Some(slot) = o.shown[s].slots.iter_mut().find(|sl| sl.hwnd == hwnd) {
            let f = sc.scale;
            (slot.x, slot.y, slot.w, slot.h) = (to.x / f, to.y / f, to.w / f, to.h / f);
        }
        out.push((i, old, to));
    }
    out
}

// --- what the pages need ---------------------------------------------------------------------

/// What a page needs when it loads: its backdrop, and the overview if it's already open (a page
/// made as the overview opened, for a new screen, missed the "open").
#[derive(Serialize)]
pub struct PageHello {
    pub backdrop: Option<Backdrop>,
    pub shown: Option<Shown>,
}

pub fn page_hello(app: &AppHandle, label: &str) -> PageHello {
    let state = app.state::<State>();
    let o = state.0.lock().unwrap();
    let Some(i) = o.screens.iter().position(|s| s.label == label) else { return PageHello { backdrop: None, shown: None } };
    let shown = (o.phase != Phase::Closed).then(|| o.shown.get(i).cloned()).flatten();
    PageHello { backdrop: Some(backdrop(&o, i)), shown }
}

fn backdrop(o: &Overview, i: usize) -> Backdrop {
    let sc = &o.screens[i];
    let paper = o.desktop.papers.iter().find(|p| p.monitor == sc.monitor);
    let mut span = sc.monitor;
    for s in &o.screens {
        span = [span[0].min(s.monitor[0]), span[1].min(s.monitor[1]), span[2].max(s.monitor[2]), span[3].max(s.monitor[3])];
    }
    Backdrop {
        url: paper.and_then(|p| p.file.as_ref()).map(|_| format!("http://wallpaper.localhost/{i}?v={}", o.paper_version)),
        fit: o.desktop.fit,
        color: o.desktop.color.clone(),
        monitor: sc.monitor,
        work: sc.work,
        span,
        scale: sc.scale,
    }
}

/// Read the wallpapers again (in the background: it asks Explorer), and tell the pages if any
/// changed.
pub fn refresh_backdrops(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED) };
        let desk = wallpaper::read();
        let state = app.state::<State>();
        let mut o = state.0.lock().unwrap();
        if o.desktop == desk && o.paper_version > 0 {
            return;
        }
        o.desktop = desk;
        o.paper_version += 1;
        for (i, sc) in o.screens.iter().enumerate() {
            let _ = app.emit_to(sc.label.as_str(), "backdrop", backdrop(&o, i));
        }
    });
}

/// The wallpaper picture for screen `i`, and its type: only the file Windows says is on it.
pub fn wallpaper_file(app: &AppHandle, i: usize) -> Option<(Vec<u8>, &'static str)> {
    let file = {
        let state = app.state::<State>();
        let o = state.0.lock().unwrap();
        let sc = o.screens.get(i)?;
        o.desktop.papers.iter().find(|p| p.monitor == sc.monitor)?.file.clone()?
    };
    let bytes = std::fs::read(file).ok()?;
    let mime = wallpaper::mime(&bytes)?;
    Some((bytes, mime))
}

#[derive(Serialize, Clone)]
struct Icon {
    key: String,
    icon: Option<String>,
}

/// Look up programs' icons (the shell wants a single-threaded apartment), and send each to the
/// pages as it comes.
pub fn find_icons(app: &AppHandle, keys: Vec<String>) {
    if keys.is_empty() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED) };
        for key in keys {
            let icon = if key.is_empty() { None } else { icons::icon(&key, 32) };
            app.state::<State>().1.lock().unwrap().insert(key.clone(), icon.clone());
            let _ = app.emit("icon", Icon { key, icon });
        }
    });
}

/// The icons of the windows open now, so the first overview has them.
pub fn warm_icons(app: &AppHandle) {
    let own = std::process::id();
    let mut keys: Vec<String> = wins::enumerate().into_iter().filter(|w| wins::listed(w, own)).filter_map(|w| image_path(w.pid)).collect();
    keys.sort();
    keys.dedup();
    find_icons(app, keys);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rects_on_a_screen_are_from_its_work_area() {
        let s = Screen { label: "bay-1".into(), hwnd: 0, monitor: [2560, 0, 4480, 1080], work: [2560, 40, 4480, 1080], scale: 1.25 };
        let r = s.local(RECT { left: 2600, top: 100, right: 3600, bottom: 900 });
        assert_eq!(r, Rect { x: 40.0, y: 60.0, w: 1000.0, h: 800.0 });
    }

    const LEFT: [i32; 4] = [0, 0, 2560, 1040];
    const RIGHT: [i32; 4] = [2560, 0, 4480, 1040];

    #[test]
    fn a_moved_window_keeps_its_place_on_the_screen() {
        // In the middle of the left screen, so in the middle of the right one.
        let r = place_on(RECT { left: 980, top: 320, right: 1580, bottom: 720 }, LEFT, RIGHT);
        assert_eq!(r, RECT { left: 3220, top: 320, right: 3820, bottom: 720 });
    }

    #[test]
    fn a_moved_window_stays_on_the_screen_and_fits_it() {
        // Right at the edge: kept inside.
        let r = place_on(RECT { left: 2000, top: 100, right: 2560, bottom: 500 }, LEFT, RIGHT);
        assert_eq!((r.right, r.right - r.left), (4480, 560));
        // Wider than the new screen: as wide as it.
        let r = place_on(RECT { left: 0, top: 0, right: 2560, bottom: 1040 }, LEFT, RIGHT);
        assert_eq!(r, RECT { left: 2560, top: 0, right: 4480, bottom: 1040 });
    }
}
