// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod appbar;
mod apps;
mod audio;
mod ffi;
mod icons;
mod media;
mod pdh;
mod power;
mod settings;
mod stats;
mod wallpaper;

use apps::Front;
use audio::{Audio, AudioWatch};
use media::{Media, MediaWatch};
use serde::{Deserialize, Serialize};
use settings::Settings;
use stats::{Sampler, Stats};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::window::Color;
use tauri::window::{Effect, EffectsBuilder};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, RunEvent, State, Theme, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

const TICK: Duration = Duration::from_secs(1);
/// The menu window's size while it lays a menu out, before it says how big the menu is.
const MENU_ROOM: (f64, f64) = (560.0, 900.0);
const INTERNATIONAL: &str = r"Control Panel\International";

struct SettingsState(Mutex<Settings>);
struct LatestSample(Mutex<Option<Sample>>);
#[derive(Default)]
struct MenuState {
    /// The menu on screen, the bar item it hangs from, and that item's bar.
    open: Option<(String, Anchor, String)>,
    /// The menu that last closed because the focus went elsewhere, and when.
    closed: Option<(String, Instant)>,
}
struct OpenMenu(Mutex<MenuState>);
/// The bar windows: which screen each is on, by label ("bar" for the first, then "bar-2" …).
#[derive(Default)]
struct Bars {
    windows: Mutex<Vec<(String, String)>>,
    /// Held while bars are added or removed, so two changes don't cross.
    syncing: Mutex<()>,
}

/// A bar item's box, in the bar's CSS pixels.
#[derive(Deserialize, Clone, Copy, Debug)]
struct Anchor {
    left: f64,
    top: f64,
}

/// How soon after a menu closed on losing the focus a click on its item counts as the click that closed it.
const BLUR_CLICK: Duration = Duration::from_millis(400);

/// What the bar gets every tick.
#[derive(Serialize, Clone)]
struct Sample {
    t: f64,
    stats: Stats,
    #[serde(skip_serializing_if = "Option::is_none")]
    media: Option<Media>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio: Option<Audio>,
}

#[derive(Serialize)]
struct Hello {
    settings: Settings,
    sample: Option<Sample>,
    front: Option<Front>,
    apps: Vec<apps::App>,
    has_battery: bool,
    /// Oriel is installed, so the stats menu can offer to open it.
    oriel: bool,
    autostart: bool,
    /// Windows' own clock and calendar settings, for the "auto" clock and the month view.
    clock24: bool,
    /// 0 = Monday … 6 = Sunday, as Windows numbers them.
    first_day: u8,
    version: String,
    host: String,
    settings_path: String,
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .invoke_handler(tauri::generate_handler![
            wallpaper,
            wallpaper_image,
            bar_info, screens,
            hello, save_settings, set_autostart, open_menu, menu_ready, close_menu, media_control, media_art, set_volume,
            app_windows, window_action, launch_app, quit_app, power, run_action, quit
        ])
        .setup(|app| {
            let settings = Settings::load().clamp();
            let media = MediaWatch::start();
            media.set_wanted(settings.media);
            app.manage(media.clone());
            let audio = AudioWatch::start();
            audio.set_wanted(settings.volume);
            app.manage(audio.clone());
            app.manage(SettingsState(Mutex::new(settings.clone())));
            app.manage(LatestSample(Mutex::new(None)));
            app.manage(OpenMenu(Mutex::new(MenuState::default())));
            app.manage(wallpaper::Image::new());
            app.manage(Bars::default());

            create_menu(app.handle())?;
            apply_look(app.handle(), &settings);
            let (full, backdrop, displays) = (app.handle().clone(), app.handle().clone(), app.handle().clone());
            appbar::on_events(
                // A game or a video went fullscreen: its bar has hidden itself; so does an open menu.
                move |hidden| {
                    if hidden {
                        hide_menu(&full, false);
                    }
                },
                move || emit_bars(&backdrop, "backdrop", ()),
                // A screen came or went: bars follow (with "all screens", or a screen of its own).
                move || sync_bars_later(&displays),
            );
            sync_bars_later(app.handle());

            let (handle, running) = (app.handle().clone(), app.handle().clone());
            apps::start(
                move |front| {
                    emit_bars(&handle, "front", front);
                    let _ = handle.emit_to("menu", "front", front);
                },
                move |apps| {
                    emit_bars(&running, "apps", apps);
                    let _ = running.emit_to("menu", "apps", apps);
                },
            );
            start_sampler(app.handle().clone(), media, audio);
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Clicking anywhere else closes a menu, as on a Mac.
            WindowEvent::Focused(false) if window.label() == "menu" => menu_blurred(window.app_handle()),
            WindowEvent::CloseRequested { api, .. } => {
                // Alt+F4 on a menu just closes the menu; "Quit Lintel" is in the logo menu.
                api.prevent_close();
                if window.label() == "menu" {
                    hide_menu(window.app_handle(), true);
                }
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building Lintel")
        .run(|_app, event| {
            if let RunEvent::Exit = event {
                appbar::unregister_all();
            }
        });
}

// --- sampling ------------------------------------------------------------------

fn start_sampler(app: AppHandle, media: MediaWatch, audio: AudioWatch) {
    std::thread::Builder::new()
        .name("sampler".into())
        .spawn(move || {
            let mut sampler = Sampler::new();
            let mut next = Instant::now();
            loop {
                // Nothing to show while a fullscreen app has the bar hidden.
                if !appbar::fullscreen() {
                    // The reads happen on their own threads, so these are the previous tick's: fine for a song.
                    media.poll();
                    audio.poll();
                    let sample = Sample {
                        t: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0),
                        stats: sampler.sample(),
                        media: media.latest(),
                        audio: audio.latest(),
                    };
                    // The menu gets them while hidden too, so the CPU and network graphs have history when it opens.
                    emit_bars(&app, "sample", &sample);
                    let _ = app.emit_to("menu", "sample", &sample);
                    *app.state::<LatestSample>().0.lock().unwrap() = Some(sample);
                }
                next += TICK;
                let now = Instant::now();
                if next > now {
                    std::thread::sleep(next - now);
                } else {
                    next = now; // fell behind (sleep/hibernate): don't try to catch up
                }
            }
        })
        .expect("failed to start sampler");
}

// --- commands --------------------------------------------------------------------

#[tauri::command]
fn hello(app: AppHandle, settings: State<SettingsState>, latest: State<LatestSample>) -> Hello {
    Hello {
        settings: settings.0.lock().unwrap().clone(),
        sample: latest.0.lock().unwrap().clone(),
        front: apps::latest(),
        apps: apps::running(),
        has_battery: stats::battery().is_some(),
        oriel: oriel_exe().is_some(),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        clock24: ffi::reg_user_string(INTERNATIONAL, "sShortTime").is_none_or(|f| f.contains('H')),
        first_day: ffi::reg_user_string(INTERNATIONAL, "iFirstDayOfWeek").and_then(|d| d.parse().ok()).filter(|d| *d < 7).unwrap_or(0),
        version: env!("CARGO_PKG_VERSION").into(),
        host: std::env::var("COMPUTERNAME").unwrap_or_default(),
        settings_path: settings::path().display().to_string(),
    }
}

#[tauri::command]
fn save_settings(app: AppHandle, state: State<SettingsState>, media: State<MediaWatch>, audio: State<AudioWatch>, settings: Settings) -> Result<Settings, String> {
    let settings = settings.clamp();
    let previous = std::mem::replace(&mut *state.0.lock().unwrap(), settings.clone());
    settings.save().map_err(|e| e.to_string())?;
    media.set_wanted(settings.media);
    audio.set_wanted(settings.volume);
    if (&previous.theme, previous.glass) != (&settings.theme, settings.glass) {
        apply_look(&app, &settings);
    }
    appbar::set_layout(appbar::edge(&settings.edge), settings.thickness());
    if previous.screens != settings.screens {
        sync_bars_later(&app);
    }
    let _ = app.emit("settings", &settings);
    Ok(settings)
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let al = app.autolaunch();
    if enabled { al.enable() } else { al.disable() }.map_err(|e| e.to_string())?;
    al.is_enabled().map_err(|e| e.to_string())
}

/// A bar item was clicked (or hovered while another menu is open). `item` is where it is, in
/// the bar's CSS pixels. Clicking the item of the open menu closes it.
#[tauri::command]
fn open_menu(app: AppHandle, window: WebviewWindow, open: State<OpenMenu>, id: String, item: Anchor, toggle: bool) {
    let bar = window.label().to_string();
    let mut state = open.0.lock().unwrap();
    if state.open.as_ref().is_some_and(|(o, _, b)| *o == id && *b == bar) {
        if toggle {
            drop(state);
            hide_menu(&app, true);
        }
        return;
    }
    // The click on the item took the focus from the menu first, which closed it; this click was
    // meant to close it, so don't open it again.
    if toggle && state.closed.take().is_some_and(|(o, at)| o == id && at.elapsed() < BLUR_CLICK) {
        drop(state);
        apps::restore_focus();
        return;
    }
    let Some(menu) = app.get_webview_window("menu") else { return };
    if state.open.is_none() {
        // Room to lay out any menu while it's still hidden; menu_ready then sets its real size.
        let _ = menu.set_size(LogicalSize::new(MENU_ROOM.0, MENU_ROOM.1));
    }
    if state.open.as_ref().is_some_and(|(_, _, b)| *b != bar) {
        // Another bar's menu: that bar stops showing its item as open.
        let _ = app.emit_to(state.open.as_ref().unwrap().2.as_str(), "menu-closed", ());
    }
    state.open = Some((id.clone(), item, bar));
    drop(state);
    let _ = app.emit_to("menu", "menu", &id);
}

/// The menu page has laid its menu out: size it and place it beside its bar item (below for a
/// bar along the top, above for one along the bottom, to the side for one on the left or right),
/// kept a few pixels inside the bar's length.
#[tauri::command]
fn menu_ready(app: AppHandle, open: State<OpenMenu>, settings: State<SettingsState>, id: String, width: f64, height: f64) {
    let Some((item, label)) = open.0.lock().unwrap().open.as_ref().filter(|(o, _, _)| *o == id).map(|(_, a, b)| (*a, b.clone())) else { return };
    let (Some(bar), Some(menu)) = (app.get_webview_window(&label), app.get_webview_window("menu")) else { return };
    let (Ok(pos), Ok(size), Ok(scale)) = (bar.outer_position(), bar.outer_size(), bar.scale_factor()) else { return };
    let edge = settings.0.lock().unwrap().edge.clone();
    let (w, h) = (width.clamp(80.0, MENU_ROOM.0), height.clamp(20.0, MENU_ROOM.1));
    let bar = Rect { x: pos.x as f64, y: pos.y as f64, w: size.width as f64, h: size.height as f64 };
    let (x, y) = place_menu(&edge, bar, item, (w * scale, h * scale), scale);
    let _ = menu.set_size(LogicalSize::new(w, h));
    let _ = menu.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
    if !menu.is_visible().unwrap_or(false) {
        let _ = menu.show();
    }
    let _ = menu.set_focus();
    let _ = app.emit_to(label.as_str(), "menu-opened", &id);
}

/// Physical pixels on the screen.
#[derive(Clone, Copy, Debug)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// Where a menu of `size` (physical pixels) goes for `item` on a bar at `bar` along `edge`.
fn place_menu(edge: &str, bar: Rect, item: Anchor, size: (f64, f64), scale: f64) -> (f64, f64) {
    let (gap, away) = (6.0 * scale, 4.0 * scale);
    let along = |start: f64, len: f64, from: f64, menu: f64| (start + from * scale).min(start + len - gap - menu).max(start + gap);
    match edge {
        "bottom" => (along(bar.x, bar.w, item.left, size.0), bar.y - away - size.1),
        "left" => (bar.x + bar.w + away, along(bar.y, bar.h, item.top, size.1)),
        "right" => (bar.x - away - size.0, along(bar.y, bar.h, item.top, size.1)),
        _ => (along(bar.x, bar.w, item.left, size.0), bar.y + bar.h + away),
    }
}

/// Escape, a click on an empty part of the bar, or a menu item that's done. `restore` gives the
/// focus back to the app that had it, unless the item itself brought another window forward.
#[tauri::command]
fn close_menu(app: AppHandle, restore: bool) {
    hide_menu(&app, restore);
}

fn hide_menu(app: &AppHandle, restore: bool) {
    if app.state::<OpenMenu>().0.lock().unwrap().open.take().is_none() {
        return;
    }
    if let Some(menu) = app.get_webview_window("menu") {
        let _ = menu.hide();
    }
    emit_bars(app, "menu-closed", ());
    let _ = app.emit_to("menu", "menu-closed", ());
    if restore {
        apps::restore_focus();
    }
}

/// The menu lost the focus: to another app (close, and leave that app in front), or to the bar
/// itself, when one of its items was clicked (close, and remember which, see `open_menu`).
fn menu_blurred(app: &AppHandle) {
    let id = app.state::<OpenMenu>().0.lock().unwrap().open.as_ref().map(|(o, _, _)| o.clone());
    let Some(id) = id else { return };
    let ours = apps::foreground_is_ours();
    hide_menu(app, ours);
    app.state::<OpenMenu>().0.lock().unwrap().closed = Some((id, Instant::now()));
}

/// The Now playing buttons. They act on the player Windows' media controls show, like the media
/// keys on a keyboard do.
#[tauri::command]
fn media_control(media: State<MediaWatch>, action: String) -> Result<(), String> {
    media.control(match action.as_str() {
        "toggle" => media::Action::Toggle,
        "next" => media::Action::Next,
        "previous" => media::Action::Previous,
        _ => return Err(format!("unknown media action: {action}")),
    });
    Ok(())
}

/// Where the wallpaper behind this bar is, for the glass look; the picture itself comes from
/// `wallpaper_image`, and only when its `key` changes.
#[tauri::command]
fn wallpaper(window: WebviewWindow) -> Option<wallpaper::Wallpaper> {
    wallpaper::current(appbar::rect(window.hwnd().ok()?.0 as isize)?).map(|(w, _)| w)
}

#[tauri::command]
fn wallpaper_image(window: WebviewWindow, image: State<wallpaper::Image>) -> Option<String> {
    let (w, path) = wallpaper::current(appbar::rect(window.hwnd().ok()?.0 as isize)?)?;
    image.get(&w.key, path.as_deref())
}

/// Which screen a bar is on (as the running apps' windows give theirs), and how many bars there
/// are, so each can show the apps on its own screen.
#[derive(Serialize)]
struct BarInfo {
    monitor: u64,
    bars: usize,
}

#[tauri::command]
fn bar_info(window: WebviewWindow, bars: State<Bars>) -> Option<BarInfo> {
    let hwnd = window.hwnd().ok()?.0 as isize;
    Some(BarInfo {
        monitor: unsafe { ffi::MonitorFromWindow(hwnd, ffi::MONITOR_DEFAULTTONEAREST) } as u64,
        bars: bars.windows.lock().unwrap().len(),
    })
}

/// The screens, for Menu Bar Settings.
#[tauri::command]
fn screens() -> Vec<appbar::Screen> {
    appbar::screens()
}

/// The current track's cover as a data: URL, fetched when a sample's `media.art` changes.
#[tauri::command]
fn media_art(media: State<MediaWatch>) -> Option<String> {
    media.art()
}

/// The volume menu's sliders and mute buttons, and the scroll wheel on the bar's speaker. `pid` is
/// an app's (0 for system sounds), or none for the output device itself; `volume` is 0–100.
#[tauri::command]
fn set_volume(audio: State<AudioWatch>, pid: Option<u32>, volume: Option<f64>, muted: Option<bool>) -> Result<(), String> {
    if let Some(v) = volume {
        if !(0.0..=100.0).contains(&v) {
            return Err(format!("volume out of range: {v}"));
        }
        audio.set(pid, audio::Change::Volume((v / 100.0) as f32));
    }
    if let Some(m) = muted {
        audio.set(pid, audio::Change::Mute(m));
    }
    Ok(())
}

/// The front app's windows, for its menu.
#[tauri::command]
fn app_windows(pid: u32) -> Vec<apps::Win> {
    apps::windows_of(pid)
}

/// The app menu's window items. `hwnd` and `pid` are what the menu listed (see `apps::target`).
#[tauri::command]
fn window_action(action: String, hwnd: u64, pid: u32) -> Result<(), String> {
    match action.as_str() {
        "focus" => apps::focus(hwnd, pid),
        "minimize" => apps::minimize(hwnd, pid),
        "close" => apps::close(hwnd, pid),
        _ => Err(format!("unknown window action: {action}")),
    }
}

/// A new window of a running app, as a middle click on a taskbar button opens.
#[tauri::command]
fn launch_app(key: String) -> Result<(), String> {
    apps::launch(&key)
}

#[tauri::command]
fn quit_app(pid: u32) -> Result<usize, String> {
    apps::quit(pid)
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum PowerAction {
    Lock,
    Sleep,
    Signout,
    Restart,
    Shutdown,
}

#[tauri::command]
fn power(action: PowerAction) -> Result<(), String> {
    power::act(match action {
        PowerAction::Lock => "lock",
        PowerAction::Sleep => "sleep",
        PowerAction::Signout => "signout",
        PowerAction::Restart => "restart",
        PowerAction::Shutdown => "shutdown",
    })
}

/// Things the menus can ask Windows to open. A fixed list, so the page can't launch arbitrary programs.
fn action_target(name: &str) -> Option<String> {
    match name {
        "task-manager" => Some("taskmgr.exe".into()),
        "explorer" => Some("explorer.exe".into()),
        "oriel" => oriel_exe(),
        // The Wi-Fi list Windows' own network button opens.
        "ms-availablenetworks:" => Some(name.into()),
        _ => {
            let page = name.strip_prefix("ms-settings:")?;
            page.chars().all(|c| c.is_ascii_alphanumeric() || c == '-').then(|| name.into())
        }
    }
}

#[tauri::command]
fn run_action(name: String) -> Result<(), String> {
    if name == "notifications" {
        // Windows has no API for its notification centre; Win+N is how everyone opens it.
        send_win_key(b'N' as u16);
        return Ok(());
    }
    let target = action_target(&name).ok_or_else(|| format!("unknown action: {name}"))?;
    let (op, file) = (ffi::wide("open"), ffi::wide(&target));
    // ShellExecute rather than spawning: Task Manager asks for elevation, and ms-settings: is a URI.
    let r = unsafe { ffi::ShellExecuteW(0, op.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), ffi::SW_SHOWNORMAL) };
    if r > 32 {
        Ok(())
    } else {
        Err(format!("couldn't open {target} (error {r})"))
    }
}

/// Oriel's exe, if it's installed for this user: where its installer says it put it.
fn oriel_exe() -> Option<String> {
    let icon = ffi::reg_user_string(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Oriel", "DisplayIcon")?;
    let path = icon.trim_matches('"').split(',').next()?.trim().to_string();
    (path.to_lowercase().ends_with(".exe") && std::path::Path::new(&path).is_file()).then_some(path)
}

fn send_win_key(key: u16) {
    use ffi::*;
    let input = |vk: u16, up: bool| INPUT {
        kind: INPUT_KEYBOARD,
        ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, dwExtraInfo: 0 },
        _pad: [0; 8],
    };
    let keys = [input(VK_LWIN, false), input(key, false), input(key, true), input(VK_LWIN, true)];
    unsafe { SendInput(keys.len() as u32, keys.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
}

#[tauri::command]
fn quit(app: AppHandle) {
    appbar::unregister_all();
    app.exit(0);
}

// --- windows -----------------------------------------------------------------------

/// The same flags for both windows: WebView2 refuses a second window with different ones.
/// Tauri's defaults (which setting our own would otherwise drop), plus no GPU process: a bar of
/// text doesn't need one, and it's the biggest part of a WebView's memory.
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-gpu";

/// Every bar window, for what they all show.
fn emit_bars<S: Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    for (label, _) in app.webview_windows() {
        if label.starts_with("bar") {
            let _ = app.emit_to(label.as_str(), event, payload.clone());
        }
    }
}

/// The screens that get a bar: "main", "all", or one screen by its id (the main one while that
/// screen isn't connected).
fn wanted_screens(setting: &str, screens: &[appbar::Screen]) -> Vec<String> {
    match setting {
        "all" => screens.iter().map(|s| s.id.clone()).collect(),
        id if id != "main" && screens.iter().any(|s| s.id == id) => vec![id.to_string()],
        _ => screens.first().map(|s| vec![s.id.clone()]).unwrap_or_default(),
    }
}

/// Add and remove bar windows so each wanted screen has one. Off the main thread: building a
/// WebView waits on the main thread, which may be what asked (a command, or a window message).
fn sync_bars_later(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || sync_bars(&app));
}

fn sync_bars(app: &AppHandle) {
    let bars = app.state::<Bars>();
    let _syncing = bars.syncing.lock().unwrap();
    let settings = app.state::<SettingsState>().0.lock().unwrap().clone();
    let wanted = wanted_screens(&settings.screens, &appbar::screens());
    let current = bars.windows.lock().unwrap().clone();
    let mut changed = false;
    for (screen, label) in &current {
        if !wanted.contains(screen) {
            if let Some(w) = app.get_webview_window(label) {
                if let Ok(hwnd) = w.hwnd() {
                    let hwnd = hwnd.0 as isize;
                    let _ = app.run_on_main_thread(move || appbar::unregister(hwnd));
                }
                let _ = w.destroy();
            }
            bars.windows.lock().unwrap().retain(|(_, l)| l != label);
            changed = true;
        }
    }
    for screen in wanted {
        if bars.windows.lock().unwrap().iter().any(|(s, _)| *s == screen) {
            continue;
        }
        let taken: Vec<String> = bars.windows.lock().unwrap().iter().map(|(_, l)| l.clone()).collect();
        let label = std::iter::once("bar".to_string())
            .chain((2..).map(|n| format!("bar-{n}")))
            .find(|l| !taken.contains(l) && app.get_webview_window(l).is_none())
            .unwrap();
        let Ok(bar) = create_bar(app, &label) else { continue };
        bars.windows.lock().unwrap().push((screen.clone(), label));
        apply_look(app, &settings);
        let (edge, thickness) = (appbar::edge(&settings.edge), settings.thickness());
        let _ = app.run_on_main_thread(move || {
            if let Ok(hwnd) = bar.hwnd() {
                appbar::register(hwnd.0 as isize, &screen, edge, thickness);
            }
            let _ = bar.show();
        });
        changed = true;
    }
    if changed {
        // Each bar asks again which screen it's on and how many there are.
        emit_bars(app, "backdrop", ());
    }
}

fn create_bar(app: &AppHandle, label: &str) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title("Lintel")
        .inner_size(1280.0, 30.0)
        .decorations(false)
        .resizable(false)
        .shadow(false)
        .skip_taskbar(true)
        .always_on_top(true)
        // Clicking the bar leaves the focus with the app you're in, as on a Mac.
        .focusable(false)
        .visible(false)
        .background_color(Color(0x0d, 0x0d, 0x0d, 255))
        .additional_browser_args(BROWSER_ARGS)
        .build()
}

fn create_menu(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(app, "menu", WebviewUrl::App("menu.html".into()))
        .title("Lintel menu")
        .inner_size(MENU_ROOM.0, MENU_ROOM.1)
        .decorations(false)
        .resizable(false)
        // Rounded corners and a drop shadow on Windows 11.
        .shadow(true)
        .skip_taskbar(true)
        .always_on_top(true)
        .transparent(true)
        .visible(false)
        .background_color(Color(0x1a, 0x1a, 0x19, 255))
        .additional_browser_args(BROWSER_ARGS)
        .build()
}

/// The windows follow the theme: their native frame, and the blank frame before a page paints.
/// With glass on, the menu is see-through and Windows blurs what's behind it (acrylic); the page
/// then paints only a tint over it.
fn apply_look(app: &AppHandle, settings: &Settings) {
    let (native, bar_bg, menu_bg) = match settings.theme.as_str() {
        "dark" => (Some(Theme::Dark), Color(0x0d, 0x0d, 0x0d, 255), Color(0x1a, 0x1a, 0x19, 255)),
        "light" => (Some(Theme::Light), Color(0xf9, 0xf9, 0xf7, 255), Color(0xfc, 0xfc, 0xfb, 255)),
        _ => (None, Color(0x0d, 0x0d, 0x0d, 255), Color(0x1a, 0x1a, 0x19, 255)),
    };
    for (label, w) in app.webview_windows() {
        let bg = if label == "menu" { menu_bg } else { bar_bg };
        {
            let _ = w.set_theme(native);
            // The menu floats over other windows, and has the focus while it's open, so Windows'
            // own acrylic works for it. The bar never has the focus, and Windows draws an
            // unfocused window's acrylic flat gray; only the wallpaper is ever behind the bar,
            // though, so it paints a blurred copy of that itself (ui/bar.js).
            let glass = settings.glass && label == "menu";
            let _ = w.set_background_color(Some(if glass { Color(0, 0, 0, 0) } else { bg }));
            if label == "menu" {
                let _ = w.set_effects(glass.then(|| EffectsBuilder::new().effect(Effect::Acrylic).build()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_go_on_the_screens_asked_for() {
        let screen = |id: &str, primary| appbar::Screen { id: id.into(), primary, rect: [0, 0, 1920, 1080], area: Default::default() };
        let screens = [screen(r"\\.\DISPLAY2", true), screen(r"\\.\DISPLAY1", false)];
        assert_eq!(wanted_screens("main", &screens), [r"\\.\DISPLAY2"]);
        assert_eq!(wanted_screens("all", &screens), [r"\\.\DISPLAY2", r"\\.\DISPLAY1"]);
        assert_eq!(wanted_screens(r"\\.\DISPLAY1", &screens), [r"\\.\DISPLAY1"]);
        // A screen that isn't connected: the main one meanwhile.
        assert_eq!(wanted_screens(r"\\.\DISPLAY9", &screens), [r"\\.\DISPLAY2"]);
        assert!(wanted_screens("main", &[]).is_empty());
    }

    #[test]
    fn menus_open_beside_their_item_and_stay_on_screen() {
        let item = Anchor { left: 100.0, top: 200.0 };
        let menu = (300.0, 400.0);
        // Along the top of a 1920-wide screen: under the item.
        let top = Rect { x: 0.0, y: 0.0, w: 1920.0, h: 30.0 };
        assert_eq!(place_menu("top", top, item, menu, 1.0), (100.0, 34.0));
        // An item near the right end: pulled in to stay 6 px inside the screen.
        assert_eq!(place_menu("top", top, Anchor { left: 1800.0, top: 0.0 }, menu, 1.0), (1614.0, 34.0));
        // Along the bottom: above the bar.
        let bottom = Rect { x: 0.0, y: 1050.0, w: 1920.0, h: 30.0 };
        assert_eq!(place_menu("bottom", bottom, item, menu, 1.0), (100.0, 646.0));
        // On the sides: next to the item, at its height.
        let left = Rect { x: 0.0, y: 0.0, w: 46.0, h: 1080.0 };
        assert_eq!(place_menu("left", left, item, menu, 1.0), (50.0, 200.0));
        let right = Rect { x: 1874.0, y: 0.0, w: 46.0, h: 1080.0 };
        assert_eq!(place_menu("right", right, item, menu, 1.0), (1570.0, 200.0));
        // At 150%, the item's CSS pixels are scaled to the screen's.
        assert_eq!(place_menu("top", top, item, menu, 1.5), (150.0, 36.0));
    }
}
