// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod calc;
mod catalog;
mod clipboard;
mod ffi;
mod files;
mod history;
mod hotkey;
mod icons;
mod matcher;
mod power;
mod preview;
mod search;
mod settings;
mod shell;
mod wins;

use clipboard::Clipboard;
use files::Files;
use history::History;
use hotkey::Hotkey;
use serde::Serialize;
use settings::Settings;
use shell::Shell;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::window::{Color, Effect, EffectsBuilder};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, State, Theme, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

/// The panel's width, in CSS pixels. Its window is the panel itself: as tall as the page says it
/// is (`fit`), so Windows can round it, give it its shadow and, with glass, blur what's behind it.
const WIDTH: f64 = 680.0;
/// With the preview open beside the results (Space).
const WIDE: f64 = 1100.0;
/// The search field alone, before the page has said; and the most it can grow to.
const FIELD: f64 = 58.0;
const MAX_HEIGHT: f64 = 640.0;
/// How far down the screen the window's top is, as on a Mac: a little above the middle.
const DROP: f64 = 0.16;
const INTERNATIONAL: &str = r"Control Panel\International";

struct SettingsState(Mutex<Settings>);
struct HotkeyState(Hotkey, Mutex<Option<String>>);
struct AutostartItem(CheckMenuItem<Wry>);
/// When the panel last opened, so the focus wobbling as it does isn't taken for a click elsewhere.
struct Shown(Mutex<Option<Instant>>);

/// The window that had the focus before the panel opened, to give it back.
static BEFORE: AtomicIsize = AtomicIsize::new(0);

#[derive(Serialize)]
struct Hello {
    settings: Settings,
    /// Why the shortcut isn't working, if it isn't.
    hotkey_error: Option<String>,
    hotkeys: &'static [&'static str],
    engines: Vec<(&'static str, &'static str)>,
    autostart: bool,
    files: usize,
    version: String,
    settings_path: String,
}

fn main() {
    tauri::Builder::default()
        // Starting it again (from Start, say) opens the panel.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .invoke_handler(tauri::generate_handler![hello, search, icon, preview, open, recent, forget, hide, fit, save_settings, set_autostart, reindex, clear_history, quit])
        .setup(|app| {
            let settings = Settings::load().clamp();
            app.manage(Shell::start());
            let files = Files::default();
            if settings.files {
                files.refresh(&settings.folders, true);
            }
            app.manage(files);
            app.manage(Clipboard::start(settings.clipboard, settings.clipboard_size, settings.keep_clipboard));
            app.manage(History::load(settings::dir().join("history.json")));
            app.manage(Shown(Mutex::new(None)));

            create_panel(app.handle())?;
            build_tray(app.handle(), &settings.hotkey)?;
            let handle = app.handle().clone();
            let (hk, first) = Hotkey::start(&settings.hotkey, move || {
                let app = handle.clone();
                let _ = handle.run_on_main_thread(move || toggle(&app));
            });
            app.manage(HotkeyState(hk, Mutex::new(first.err())));
            apply_look(app.handle(), &settings);
            app.manage(SettingsState(Mutex::new(settings)));
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Clicking anywhere else closes the panel, as on a Mac.
            WindowEvent::Focused(false) => {
                let recent = window.app_handle().state::<Shown>().0.lock().unwrap().is_some_and(|t| t.elapsed() < Duration::from_millis(300));
                if !recent {
                    hide_panel(window.app_handle(), false);
                }
            }
            WindowEvent::CloseRequested { api, .. } => {
                // Alt+F4 just closes the panel; "Quit Fanlight" is in the tray menu.
                api.prevent_close();
                hide_panel(window.app_handle(), true);
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building Fanlight")
        .run(|_app, _event| {});
}

// --- the panel -------------------------------------------------------------------------

fn panel(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("search")
}

fn toggle(app: &AppHandle) {
    if panel(app).is_some_and(|w| w.is_visible().unwrap_or(false)) {
        hide_panel(app, true);
    } else {
        show(app);
    }
}

/// Open the panel on the screen the pointer is on, with the keyboard in the search field.
fn show(app: &AppHandle) {
    let Some(w) = panel(app) else { return };
    let front = unsafe { ffi::GetForegroundWindow() };
    if !w.hwnd().is_ok_and(|h| h.0 as isize == front) {
        BEFORE.store(front, Ordering::Relaxed);
    }
    place(app, &w);
    // What's new since last time: apps installed, files added.
    app.state::<Shell>().refresh_apps(false);
    let settings = app.state::<SettingsState>().0.lock().unwrap().clone();
    if settings.files {
        app.state::<Files>().refresh(&settings.folders, false);
    }
    *app.state::<Shown>().0.lock().unwrap() = Some(Instant::now());
    let _ = w.emit("open", ());
    let _ = w.show();
    let _ = w.set_focus();
}

fn place(app: &AppHandle, w: &WebviewWindow) {
    let pos = app.cursor_position().ok();
    let monitor = pos.and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten()).or_else(|| app.primary_monitor().ok().flatten());
    let Some(m) = monitor else { return };
    let (area, scale) = (m.work_area(), m.scale_factor());
    let x = area.position.x as f64 + (area.size.width as f64 - WIDTH * scale) / 2.0;
    let y = area.position.y as f64 + area.size.height as f64 * DROP;
    // Moving it first, so a screen with other scaling sizes it at that screen's scale.
    let _ = w.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
    let height = w.inner_size().ok().zip(w.scale_factor().ok()).map_or(FIELD, |(s, f)| s.height as f64 / f);
    let _ = w.set_size(LogicalSize::new(WIDTH, height));
}

/// Close the panel; `restore` gives the focus back to the window that had it.
fn hide_panel(app: &AppHandle, restore: bool) {
    let Some(w) = panel(app) else { return };
    if !w.is_visible().unwrap_or(false) {
        return;
    }
    let _ = w.hide();
    let _ = w.emit("closed", ());
    if restore {
        restore_focus();
    }
}

fn restore_focus() {
    let hwnd = BEFORE.load(Ordering::Relaxed);
    unsafe {
        if hwnd != 0 && ffi::IsWindow(hwnd) != 0 {
            ffi::SetForegroundWindow(hwnd);
        }
    }
}

/// The same flags for every window: WebView2 refuses a second with different ones. Tauri's
/// defaults (which setting our own would otherwise drop), plus no GPU process: a list of text
/// doesn't need one, and it's the biggest part of a WebView's memory.
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-gpu";

fn create_panel(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(app, "search", WebviewUrl::App("index.html".into()))
        .title("Fanlight")
        .inner_size(WIDTH, FIELD)
        .decorations(false)
        .resizable(false)
        // Windows draws the shadow and rounds the corners; with glass, the page lets the blur
        // through, so the webview is see-through.
        .shadow(true)
        .transparent(true)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(false)
        .background_color(Color(0, 0, 0, 0))
        .additional_browser_args(BROWSER_ARGS)
        .build()
}

/// The window follows the theme (the acrylic's own tint is dark or light with it), and with glass
/// gets Windows' acrylic: what's behind it, blurred, as Start and the quick settings have.
fn apply_look(app: &AppHandle, settings: &Settings) {
    let native = match settings.theme.as_str() {
        "dark" => Some(Theme::Dark),
        "light" => Some(Theme::Light),
        _ => None,
    };
    let Some(w) = panel(app) else { return };
    let _ = w.set_theme(native);
    let effects = settings.glass.then(|| EffectsBuilder::new().effect(Effect::Acrylic).build());
    let _ = w.set_effects(effects);
    if let Ok(hwnd) = w.hwnd() {
        ffi::round_corners(hwnd.0 as isize);
    }
}

// --- tray ------------------------------------------------------------------------------

/// "alt+space" as menus write it: "Alt+Space".
fn shortcut_label(spec: &str) -> String {
    spec.split('+').map(|p| p[..1].to_uppercase() + &p[1..]).collect::<Vec<_>>().join("+")
}

fn build_tray(app: &AppHandle, hotkey: &str) -> tauri::Result<()> {
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
    let open_i = MenuItem::with_id(app, "open", format!("Open Fanlight\t{}", shortcut_label(hotkey)), true, None::<&str>)?;
    let settings_i = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let autostart_i = CheckMenuItem::with_id(app, "autostart", "Start at sign-in", true, autostart_on, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit Fanlight", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&open_i, &settings_i, &PredefinedMenuItem::separator(app)?, &autostart_i, &PredefinedMenuItem::separator(app)?, &quit_i],
    )?;
    app.manage(AutostartItem(autostart_i));
    app.manage(OpenItem(open_i));

    TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .tooltip("Fanlight")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show(app),
            "settings" => {
                show(app);
                if let Some(w) = panel(app) {
                    let _ = w.emit("view", "settings");
                }
            }
            "autostart" => {
                let on = !app.autolaunch().is_enabled().unwrap_or(false);
                let _ = set_autostart(app.clone(), on);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

struct OpenItem(MenuItem<Wry>);

// --- commands --------------------------------------------------------------------------

#[tauri::command]
fn hello(app: AppHandle, settings: State<SettingsState>, hotkey: State<HotkeyState>, files: State<Files>) -> Hello {
    Hello {
        settings: settings.0.lock().unwrap().clone(),
        hotkey_error: hotkey.1.lock().unwrap().clone(),
        hotkeys: settings::HOTKEYS,
        engines: search::ENGINE_NAMES.iter().map(|(id, name, _)| (*id, *name)).collect(),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        files: files.count(),
        version: env!("CARGO_PKG_VERSION").into(),
        settings_path: settings::path().display().to_string(),
    }
}

/// What was typed, and a number the page counts up, handed back so it can drop answers to
/// searches it has typed past.
#[tauri::command]
async fn search(app: AppHandle, text: String, seq: u64, only: Option<String>) -> (u64, search::Results) {
    let settings = app.state::<SettingsState>().0.lock().unwrap().clone();
    let apps = app.state::<Shell>().apps();
    let windows = if settings.windows { wins::list() } else { Vec::new() };
    let clips = app.state::<Clipboard>().clips();
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    let src = search::Sources {
        settings: &settings,
        apps: &apps,
        files: &app.state::<Files>(),
        windows: &windows,
        clips: &clips,
        history: &app.state::<History>(),
        comma_decimal: ffi::reg_user_string(INTERNATIONAL, "sDecimal").as_deref() == Some(","),
        home: &home,
        now: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()),
        only: only.as_deref(),
    };
    (seq, search::search(&text, &src))
}

#[tauri::command]
async fn icon(shell: State<'_, Shell>, key: String) -> Result<Option<String>, ()> {
    Ok(shell.icon(&key))
}

/// What the preview pane shows for a result. As with `open`, only for what the search gives: an
/// app Start lists, a file that's there, a clipboard entry, a window that's open, the catalog's.
#[tauri::command]
async fn preview(app: AppHandle, key: String) -> Option<preview::Preview> {
    use preview::{Fact, Preview};
    let fact = |label: &'static str, value: &str| Fact { label, value: value.to_string(), time: None };
    let (kind, rest) = key.split_once(':')?;
    let p = match kind {
        "app" => app.state::<Shell>().apps().iter().any(|a| a.id == rest).then(|| preview::app(rest))?,
        "file" | "folder" => {
            let path = std::path::Path::new(rest);
            if !path.is_absolute() {
                return None;
            }
            preview::path(path, &std::env::var("USERPROFILE").unwrap_or_default())?
        }
        "clip" => preview::clip(&app.state::<Clipboard>().get(rest.parse().ok()?)?),
        "win" => {
            let hwnd: isize = rest.split_once(':')?.0.parse().ok()?;
            let w = wins::list().into_iter().find(|w| w.hwnd == hwnd)?;
            let mut facts = vec![fact("Kind", "Open window"), fact("App", &w.app), fact("Title", &w.title)];
            if w.minimized {
                facts.push(fact("State", "Minimized"));
            }
            if !w.path.is_empty() {
                facts.push(fact("Program", &w.path));
            }
            Preview { image: (!w.path.is_empty()).then(|| format!("big:file:{}", w.path)), facts, ..Default::default() }
        }
        "page" => {
            let page = catalog::page_by_uri(rest)?;
            Preview { facts: vec![fact("Kind", "Settings page"), fact("Opens", page.uri), fact("Also found by", page.words)], ..Default::default() }
        }
        "cmd" => {
            let c = catalog::command_by_id(rest)?;
            let mut facts = vec![fact("Kind", "Command"), fact("Does", c.detail)];
            if c.confirm {
                facts.push(fact("Asks first", "Yes, press Enter twice"));
            }
            Preview { facts, ..Default::default() }
        }
        "calc" => Preview { text: Some(rest.to_string()), facts: vec![fact("Kind", "Answer"), fact("Enter", "Copies it")], ..Default::default() },
        "url" => Preview { facts: vec![fact("Kind", "Web address"), fact("Opens", &search::as_url(rest)?)], ..Default::default() },
        "web" => {
            let engine = app.state::<SettingsState>().0.lock().unwrap().engine.clone();
            let (name, base) = search::engine(&engine);
            Preview { facts: vec![fact("Kind", "Web search"), fact("With", name), fact("Opens", &format!("{base}{}", search::encode(rest)))], ..Default::default() }
        }
        _ => return None,
    };
    Some(p)
}

/// What the page does next, after `open`.
#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Next {
    /// Done: the panel has closed.
    Done,
    /// Stay open and search for this instead.
    Query(String),
    /// Stay open and show a view ("settings").
    View(&'static str),
    /// Stay open with a short note in the panel.
    Note(String),
}

/// Open a result: `how` is "open" (Enter), "reveal" (Ctrl+Enter: show in File Explorer), "copy"
/// (Ctrl+C on a result: its path, value or text), "paste" (a clipboard entry, into the app you
/// were in) or "admin" (Ctrl+Shift+Enter: an app, as administrator). The key must be one the
/// search gave: an app Start lists, a file that's there, a Settings page or command from the
/// catalog, an open window. The page can't name anything else to run.
#[tauri::command]
async fn open(app: AppHandle, key: String, how: String, query: String) -> Result<Next, String> {
    let next = open_result(&app, &key, &how, &query)?;
    app.state::<History>().searched(&query);
    Ok(next)
}

/// The searches you opened something from, newest first, for ↑.
#[tauri::command]
fn recent(history: State<History>) -> Vec<String> {
    history.recent()
}

fn open_result(app: &AppHandle, key: &str, how: &str, query: &str) -> Result<Next, String> {
    let app = app.clone();
    let (key, how, query) = (key.to_string(), how.to_string(), query.to_string());
    let (kind, rest) = key.split_once(':').ok_or("not a result")?;
    let remember = |app: &AppHandle| app.state::<History>().picked(&query, &key);
    match (kind, how.as_str()) {
        ("app", "open" | "admin") => {
            if !app.state::<Shell>().apps().iter().any(|a| a.id == rest) {
                return Err("that app isn't in Start any more".into());
            }
            hide_panel(&app, false);
            ffi::shell_open(&format!(r"shell:AppsFolder\{rest}"), None, if how == "admin" { "runas" } else { "open" })?;
            remember(&app);
        }
        ("file" | "folder", "open" | "reveal" | "copy") => {
            let path = std::path::Path::new(rest);
            if !path.is_absolute() || !path.exists() {
                return Err("it isn't there any more".into());
            }
            match how.as_str() {
                "copy" => {
                    clipboard::set_text(rest)?;
                    return Ok(Next::Note("Path copied".into()));
                }
                "reveal" => {
                    hide_panel(&app, false);
                    ffi::shell_open("explorer.exe", Some(&format!("/select,\"{rest}\"")), "open")?;
                }
                _ => {
                    hide_panel(&app, false);
                    ffi::shell_open(rest, None, "open")?;
                }
            }
            remember(&app);
        }
        ("page", "open") => {
            let page = catalog::page_by_uri(rest).ok_or("not a Settings page")?;
            hide_panel(&app, false);
            ffi::shell_open(page.uri, None, "open")?;
            remember(&app);
        }
        ("cmd", "open") => {
            let next = command(&app, rest)?;
            remember(&app);
            return Ok(next);
        }
        ("win", "open") => {
            let (hwnd, pid) = rest.split_once(':').ok_or("not a window")?;
            let (hwnd, pid) = (hwnd.parse::<isize>().map_err(|_| "not a window")?, pid.parse::<u32>().map_err(|_| "not a window")?);
            hide_panel(&app, false);
            wins::focus(hwnd, pid)?;
        }
        ("clip", "paste" | "open" | "copy") => {
            let id: u64 = rest.parse().map_err(|_| "not a clipboard entry")?;
            let clip = app.state::<Clipboard>().get(id).ok_or("that's no longer in the clipboard history")?;
            clipboard::set_text(&clip.text)?;
            if how == "copy" {
                hide_panel(&app, true);
            } else {
                // Back to the app you were in, then Ctrl+V there.
                hide_panel(&app, true);
                std::thread::spawn(|| {
                    std::thread::sleep(Duration::from_millis(120));
                    ffi::send_keys(&[ffi::VK_CONTROL, b'V' as u16]);
                });
            }
        }
        ("calc", _) => {
            clipboard::set_text(rest)?;
            hide_panel(&app, true);
        }
        ("url", "open") => {
            let url = search::as_url(rest).ok_or("not a web address")?;
            hide_panel(&app, false);
            ffi::shell_open(&url, None, "open")?;
        }
        ("url" | "web", "copy") => {
            clipboard::set_text(rest)?;
            return Ok(Next::Note("Copied".into()));
        }
        ("web", "open") => {
            let engine = app.state::<SettingsState>().0.lock().unwrap().engine.clone();
            let (_, base) = search::engine(&engine);
            hide_panel(&app, false);
            ffi::shell_open(&format!("{base}{}", search::encode(rest)), None, "open")?;
        }
        _ => return Err(format!("can't {how} that")),
    }
    Ok(Next::Done)
}

/// The commands from the catalog.
fn command(app: &AppHandle, id: &str) -> Result<Next, String> {
    catalog::command_by_id(id).ok_or("not a command")?;
    match id {
        "lock" | "sleep" | "signout" | "restart" | "shutdown" => {
            hide_panel(app, false);
            power::act(id)?;
        }
        "desktop" => {
            hide_panel(app, false);
            ffi::send_keys(&[ffi::VK_LWIN, b'D' as u16]);
        }
        "screenshot" => {
            hide_panel(app, true);
            // Once the panel is gone, or it'd be in the picture.
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_millis(200));
                ffi::send_keys(&[ffi::VK_LWIN, ffi::VK_SHIFT, b'S' as u16]);
            });
        }
        "emoji" => {
            hide_panel(app, true);
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_millis(150));
                ffi::send_keys(&[ffi::VK_LWIN, 0xBE]); // VK_OEM_PERIOD
            });
        }
        "clipboard" => return Ok(Next::Query("cb ".into())),
        "clear-clipboard" => {
            app.state::<Clipboard>().clear();
            return Ok(Next::Note("Clipboard history cleared".into()));
        }
        "settings" => return Ok(Next::View("settings")),
        "reindex" => {
            let settings = app.state::<SettingsState>().0.lock().unwrap().clone();
            app.state::<Files>().refresh(&settings.folders, true);
            return Ok(Next::Note("Looking through your folders again".into()));
        }
        "quit" => app.exit(0),
        _ => return Err("not a command".into()),
    }
    Ok(Next::Done)
}

/// Delete on a result: a clipboard entry is forgotten; anything else just loses its history.
#[tauri::command]
fn forget(clipboard: State<Clipboard>, history: State<History>, key: String) {
    if let Some(id) = key.strip_prefix("clip:").and_then(|i| i.parse().ok()) {
        clipboard.remove(id);
    }
    history.forget(&key);
}

/// Escape.
#[tauri::command]
fn hide(app: AppHandle, restore: bool) {
    hide_panel(&app, restore);
}

/// The page laid itself out at `height` CSS pixels: the window grows or shrinks to match, from the
/// top, as results come and go; and widens about its middle for the preview (`wide`).
#[tauri::command]
fn fit(window: WebviewWindow, height: f64, wide: Option<bool>) {
    let height = height.clamp(FIELD, MAX_HEIGHT).ceil();
    let width = if wide.unwrap_or(false) { WIDE } else { WIDTH };
    let Some(scale) = window.scale_factor().ok() else { return };
    let Some(size) = window.inner_size().ok() else { return };
    let now = ((size.width as f64 / scale).round(), (size.height as f64 / scale).round());
    if now == (width, height) {
        return;
    }
    if now.0 != width {
        if let Ok(pos) = window.outer_position() {
            let x = pos.x as f64 + (now.0 - width) * scale / 2.0;
            let _ = window.set_position(PhysicalPosition::new(x.round() as i32, pos.y));
        }
    }
    let _ = window.set_size(LogicalSize::new(width, height));
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    state: State<SettingsState>,
    hotkey: State<HotkeyState>,
    files: State<Files>,
    clipboard: State<Clipboard>,
    settings: Settings,
) -> Result<Settings, String> {
    let mut settings = settings.clamp();
    let previous = state.0.lock().unwrap().clone();
    if previous.hotkey != settings.hotkey {
        // Only keep the new shortcut if Windows took it.
        match hotkey.0.set(&settings.hotkey) {
            Ok(()) => {
                *hotkey.1.lock().unwrap() = None;
                let _ = app.state::<OpenItem>().0.set_text(format!("Open Fanlight\t{}", shortcut_label(&settings.hotkey)));
            }
            Err(e) => {
                settings.hotkey = previous.hotkey.clone();
                return Err(format!("{}: {e}", shortcut_label(&settings.hotkey)));
            }
        }
    }
    *state.0.lock().unwrap() = settings.clone();
    settings.save().map_err(|e| e.to_string())?;
    if (&previous.theme, previous.glass) != (&settings.theme, settings.glass) {
        apply_look(&app, &settings);
    }
    if (previous.clipboard, previous.clipboard_size, previous.keep_clipboard) != (settings.clipboard, settings.clipboard_size, settings.keep_clipboard) {
        clipboard.configure(settings.clipboard, settings.clipboard_size, settings.keep_clipboard);
    }
    if !settings.files {
        files.clear();
    } else if !previous.files || previous.folders != settings.folders {
        files.refresh(&settings.folders, true);
    }
    let _ = app.emit("settings", &settings);
    Ok(settings)
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let al = app.autolaunch();
    if enabled { al.enable() } else { al.disable() }.map_err(|e| e.to_string())?;
    let on = al.is_enabled().map_err(|e| e.to_string())?;
    if let Some(item) = app.try_state::<AutostartItem>() {
        let _ = item.0.set_checked(on);
    }
    Ok(on)
}

#[tauri::command]
fn reindex(state: State<SettingsState>, files: State<Files>) {
    let settings = state.0.lock().unwrap().clone();
    files.refresh(&settings.folders, true);
}

#[tauri::command]
fn clear_history(history: State<History>) {
    history.clear();
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_read_as_in_menus() {
        assert_eq!(shortcut_label("alt+space"), "Alt+Space");
        assert_eq!(shortcut_label("ctrl+shift+space"), "Ctrl+Shift+Space");
    }
}
