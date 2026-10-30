// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Bay: every open window at once, as live thumbnails, to pick one. It waits in the background
//! (with a tray icon, or in Mullion's) until its shortcut, then shows the overview on every
//! screen (`overview`).

mod ffi;
mod flight;
mod grid;
mod overview;
mod settings;
mod wallpaper;

use mullion_core::hotkey::{self, Hotkey};
use mullion_core::look;
use mullion_core::suite::{self, Ask};
use serde::Serialize;
use settings::Settings;
use std::sync::Mutex;
use tauri::http::{Response, StatusCode};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

pub struct SettingsState(pub Mutex<Settings>);
struct HotkeyState(Hotkey, Mutex<Option<String>>);
struct AutostartItem(CheckMenuItem<Wry>);
struct OpenItem(MenuItem<Wry>);

/// What the settings window shows.
#[derive(Serialize)]
struct Hello {
    settings: Settings,
    /// Why the shortcut isn't working, if it isn't.
    hotkey_error: Option<String>,
    hotkeys: &'static [&'static str],
    autostart: bool,
    /// Mullion started Bay, and has its tray icon and sign-in entry.
    mullion: bool,
    /// Mullion's look for every app stands in for Bay's theme and palette.
    shared_look: bool,
    icon: String,
    version: String,
    settings_path: String,
}

/// What the overview's page needs: its look, and its screen's backdrop.
#[derive(Serialize)]
struct PageHello {
    theme: String,
    palette: String,
    backdrop_kind: String,
    #[serde(flatten)]
    screen: overview::PageHello,
}

fn main() {
    tauri::Builder::default()
        // Starting it again (from Start, say) opens the overview. Mullion starts it again to ask
        // something of it (`mullion_core::suite`).
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let from_mullion = suite::from_mullion(&args);
            if from_mullion {
                leave_to_mullion(app);
            }
            match suite::ask(&args) {
                Ask::Quit => app.exit(0),
                Ask::Settings => show_settings(app),
                Ask::Show => overview::open(app),
                // Mitre's corner, pushed again, closes what it opened.
                Ask::Toggle => overview::toggle(app),
                Ask::Look => look_again(app),
                Ask::Nothing if !from_mullion => overview::open(app),
                Ask::Nothing => {}
            }
        }))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        // The wallpaper behind each screen's overview, by screen number: the page can't name a
        // file, only a screen.
        .register_uri_scheme_protocol("wallpaper", |ctx, request| {
            let screen = request.uri().path().trim_start_matches('/').parse().ok();
            match screen.and_then(|i| overview::wallpaper_file(ctx.app_handle(), i)) {
                Some((bytes, mime)) => Response::builder().header("Content-Type", mime).header("Access-Control-Allow-Origin", "*").body(bytes),
                None => Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()),
            }
            .unwrap_or_default()
        })
        .invoke_handler(tauri::generate_handler![page, pick, dismiss, next_screen, drag, let_go, desktops, hello, save_settings, set_autostart, settings_ready, quit])
        .setup(|app| {
            let args: Vec<String> = std::env::args().collect();
            // A --quit that gets this far found no Bay running, so there's nothing to quit.
            if suite::ask(&args) == Ask::Quit {
                std::process::exit(0);
            }
            let first_run = !settings::path().exists();
            let settings = Settings::load().clamp();
            app.manage(overview::State::new());
            app.manage(SettingsState(Mutex::new(settings.clone())));
            overview::sync_screens(app.handle());
            overview::warm_icons(app.handle());

            // Mullion's tray icon stands in for Bay's when Mullion started it.
            if suite::from_mullion(&args) {
                leave_to_mullion(app.handle());
            } else {
                build_tray(app.handle(), &settings.hotkey)?;
            }
            let handle = app.handle().clone();
            let (hk, first) = Hotkey::start(&settings.hotkey, move || {
                let app = handle.clone();
                let _ = handle.run_on_main_thread(move || overview::toggle(&app));
            });
            app.manage(HotkeyState(hk, Mutex::new(first.err())));
            match suite::ask(&args) {
                Ask::Show | Ask::Toggle => overview::open(app.handle()),
                Ask::Settings => show_settings(app.handle()),
                // Nothing on the screen says Bay has started, so the first time it shows its
                // settings, with the shortcut.
                _ if first_run && !suite::is_run_by_mullion() => {
                    let _ = settings.save();
                    show_settings(app.handle());
                }
                _ => {}
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if !window.label().starts_with("bay-") {
                return;
            }
            match event {
                WindowEvent::Focused(false) => overview::focus_left(window.app_handle()),
                // Alt+F4 just closes the overview; "Quit Bay" is in the tray menu.
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    overview::close(window.app_handle(), None);
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building Bay")
        .run(|_app, event| {
            // Closing the settings window leaves Bay waiting for its shortcut.
            if let RunEvent::ExitRequested { code: None, api, .. } = event {
                api.prevent_exit();
            }
        });
}

// --- tray and settings window ------------------------------------------------------------

fn build_tray(app: &AppHandle, hotkey: &str) -> tauri::Result<()> {
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
    let open_i = MenuItem::with_id(app, "open", format!("Open Bay\t{}", hotkey::label(hotkey)), true, None::<&str>)?;
    let settings_i = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let autostart_i = CheckMenuItem::with_id(app, "autostart", "Start at sign-in", true, autostart_on, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit Bay", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&open_i, &settings_i, &PredefinedMenuItem::separator(app)?, &autostart_i, &PredefinedMenuItem::separator(app)?, &quit_i],
    )?;
    app.manage(AutostartItem(autostart_i));
    app.manage(OpenItem(open_i));

    TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .tooltip("Bay")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => overview::open(app),
            "settings" => show_settings(app),
            "autostart" => {
                let on = !app.autolaunch().is_enabled().unwrap_or(false);
                let _ = set_autostart(app.clone(), on);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                overview::open(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Mullion started Bay, or reached the one running: Mullion's tray icon and sign-in entry stand
/// in for Bay's own, so there's one of each for the whole suite.
fn leave_to_mullion(app: &AppHandle) {
    let _ = app.remove_tray_by_id("main");
    let al = app.autolaunch();
    if al.is_enabled().unwrap_or(false) {
        let _ = al.disable();
    }
}

/// The settings window: made when it opens and dropped when it closes, so Bay keeps only the
/// overview's pages while it waits.
fn show_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    // Hidden until the page has drawn itself (`settings_ready`), so it never flashes white.
    let _ = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("Bay Settings")
        .inner_size(560.0, 640.0)
        .min_inner_size(460.0, 420.0)
        .center()
        .visible(false)
        .additional_browser_args(look::BROWSER_ARGS)
        .build();
}

// --- the overview's commands ---------------------------------------------------------------

#[tauri::command]
async fn page(app: AppHandle, window: WebviewWindow) -> PageHello {
    let s = app.state::<SettingsState>().0.lock().unwrap().shown();
    PageHello { theme: s.theme, palette: s.palette, backdrop_kind: s.backdrop, screen: overview::page_hello(&app, window.label()) }
}

/// A window was clicked, or Enter pressed on it: bring it forward, and close.
#[tauri::command]
fn pick(app: AppHandle, hwnd: u64, pid: u32) -> Result<(), String> {
    overview::pick(&app, mullion_core::wins::handle(hwnd)?, pid)
}

/// Esc, the shortcut again, or a click beside the windows.
#[tauri::command]
fn dismiss(app: AppHandle) {
    overview::close(&app, None);
}

#[tauri::command]
fn next_screen(app: AppHandle, window: WebviewWindow, dir: i32) {
    overview::next_screen(&app, window.label(), dir.signum());
}

/// A window's thumbnail pulled out of its place, held `fx`, `fy` of the way across it: the
/// backend follows the pointer from here, onto another screen if it goes there.
#[tauri::command]
fn drag(app: AppHandle, hwnd: u64, pid: u32, fx: f64, fy: f64) -> Result<(), String> {
    overview::drag(&app, mullion_core::wins::handle(hwnd)?, pid, fx, fy)
}

#[tauri::command]
fn let_go() {
    overview::let_go();
}

/// The Desktops button: close, and open Windows' Task View.
#[tauri::command]
fn desktops(app: AppHandle) {
    overview::desktops(&app);
}

// --- the settings window's commands ----------------------------------------------------------

#[tauri::command]
fn hello(app: AppHandle, settings: State<SettingsState>, hotkey: State<HotkeyState>) -> Hello {
    Hello {
        settings: settings.0.lock().unwrap().shown(),
        hotkey_error: hotkey.1.lock().unwrap().clone(),
        hotkeys: settings::HOTKEYS,
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        mullion: suite::is_run_by_mullion(),
        shared_look: look::shared().is_some(),
        icon: icon(),
        version: env!("CARGO_PKG_VERSION").into(),
        settings_path: settings::path().display().to_string(),
    }
}

/// Bay's own icon, for the settings window's header.
fn icon() -> String {
    use base64::Engine;
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(include_bytes!("../icons/128x128.png")))
}

#[tauri::command]
fn save_settings(app: AppHandle, state: State<SettingsState>, hotkey: State<HotkeyState>, settings: Settings) -> Result<Settings, String> {
    let previous = state.0.lock().unwrap().clone();
    let mut settings = settings.clamp().keep_look(&previous);
    if previous.hotkey != settings.hotkey {
        // Only keep the new shortcut if Windows took it.
        match hotkey.0.set(&settings.hotkey) {
            Ok(()) => {
                *hotkey.1.lock().unwrap() = None;
                if let Some(item) = app.try_state::<OpenItem>() {
                    let _ = item.0.set_text(format!("Open Bay\t{}", hotkey::label(&settings.hotkey)));
                }
            }
            Err(e) => {
                settings.hotkey = previous.hotkey.clone();
                return Err(format!("{}: {e}", hotkey::label(&settings.hotkey)));
            }
        }
    }
    *state.0.lock().unwrap() = settings.clone();
    settings.save().map_err(|e| e.to_string())?;
    let shown = settings.shown();
    let _ = app.emit("settings", &shown);
    Ok(shown)
}

/// Mullion's look for every app changed, or went on or off: show what Bay looks like now.
fn look_again(app: &AppHandle) {
    let shown = app.state::<SettingsState>().0.lock().unwrap().shown();
    let _ = app.emit("look", look::shared().is_some());
    let _ = app.emit("settings", &shown);
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

/// The settings page has drawn itself: show its window.
#[tauri::command]
fn settings_ready(window: WebviewWindow) {
    let _ = window.show();
    let _ = window.set_focus();
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}
