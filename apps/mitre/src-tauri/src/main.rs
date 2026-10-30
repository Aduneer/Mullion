// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Mitre: hot corners. Push the pointer into a corner of a screen and it does what that corner
//! is set to: Bay's overview, Fanlight's search or its apps, the desktop, Task View, lock, or the
//! screens off. It waits in the background (with a tray icon, or in Mullion's), listening to the
//! pointer (`watch`), and has no window of its own but its settings and the glow in the corner.

mod actions;
mod corners;
mod ffi;
mod glow;
mod settings;
mod watch;

use corners::Screen;
use mullion_core::look;
use mullion_core::suite::{self, Ask};
use serde::Serialize;
use settings::Settings;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use watch::{Event, Watch};

/// Where to get the suite's apps Mitre's corners can open, when one isn't installed.
const RELEASES: &str = "https://github.com/Aduneer/Mullion/releases/latest";

struct SettingsState(Mutex<Settings>);
/// The screens as Windows has them now.
struct ScreensState(Mutex<Vec<Screen>>);
struct WatchState(Watch);
struct AutostartItem(CheckMenuItem<Wry>);

#[derive(Serialize)]
struct ActionInfo {
    #[serde(flatten)]
    action: actions::Action,
    /// Its app is installed (or it needs none).
    available: bool,
}

/// What the settings window shows.
#[derive(Serialize)]
struct Hello {
    settings: Settings,
    actions: Vec<ActionInfo>,
    screens: Vec<corners::ScreenInfo>,
    dwells: &'static [u32],
    autostart: bool,
    /// Mullion started Mitre, and has its tray icon and sign-in entry.
    mullion: bool,
    /// Mullion's look for every app stands in for Mitre's theme and palette.
    shared_look: bool,
    icon: String,
    version: String,
    settings_path: String,
}

fn main() {
    tauri::Builder::default()
        // Starting it again (from Start, say) opens its settings: there's nothing else to show.
        // Mullion starts it again to ask something of it (`mullion_core::suite`).
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let from_mullion = suite::from_mullion(&args);
            if from_mullion {
                leave_to_mullion(app);
            }
            match suite::ask(&args) {
                Ask::Quit => app.exit(0),
                Ask::Settings | Ask::Show | Ask::Toggle => show_settings(app),
                Ask::Look => look_again(app),
                Ask::Nothing if !from_mullion => show_settings(app),
                Ask::Nothing => {}
            }
        }))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .invoke_handler(tauri::generate_handler![hello, save_settings, set_autostart, settings_ready, get_apps, quit])
        .setup(|app| {
            let args: Vec<String> = std::env::args().collect();
            // A --quit that gets this far found no Mitre running, so there's nothing to quit.
            if suite::ask(&args) == Ask::Quit {
                std::process::exit(0);
            }
            let first_run = !settings::path().exists();
            let settings = Settings::load().clamp();
            // Mullion's tray icon stands in for Mitre's when Mullion started it (before the
            // watch's config, whose glow follows Mullion's look then).
            if suite::from_mullion(&args) {
                leave_to_mullion(app.handle());
            } else {
                build_tray(app.handle())?;
            }

            let screens = corners::screens();
            let handle = app.handle().clone();
            let watch = Watch::start(config(&settings, &screens), move |event| match event {
                Event::Fire(screen, corner) => {
                    let Some(settings) = handle.try_state::<SettingsState>() else { return };
                    let id = settings.0.lock().unwrap().action(&screen, corner).to_string();
                    let app = handle.clone();
                    // Off the pointer's thread, so it's listening again at once.
                    std::thread::spawn(move || {
                        let screens_off = || {
                            if let Some(w) = app.try_state::<WatchState>() {
                                w.0.screens_off();
                            }
                        };
                        if let Err(e) = actions::run(&id, screens_off) {
                            eprintln!("Mitre: {id}: {e}");
                        }
                    });
                }
                Event::Screens(screens) => {
                    let info = corners::info(&screens);
                    let Some(state) = handle.try_state::<ScreensState>() else { return };
                    *state.0.lock().unwrap() = screens;
                    let _ = handle.emit("screens", info);
                }
            });
            app.manage(SettingsState(Mutex::new(settings.clone())));
            app.manage(ScreensState(Mutex::new(screens)));
            app.manage(WatchState(watch));

            match suite::ask(&args) {
                Ask::Show | Ask::Settings | Ask::Toggle => show_settings(app.handle()),
                // Nothing on the screen says Mitre has started, and no corner does anything yet:
                // the first time, it shows its settings.
                _ if first_run && !suite::is_run_by_mullion() => {
                    let _ = settings.save();
                    show_settings(app.handle());
                }
                _ => {}
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Mitre")
        .run(|_app, event| {
            // Closing the settings window leaves Mitre waiting at the corners.
            if let RunEvent::ExitRequested { code: None, api, .. } = event {
                api.prevent_exit();
            }
        });
}

/// What the pointer's thread needs: the dwell, the glow's colour, and which corners do something
/// that can be done now (an action whose app isn't installed can't).
fn config(settings: &Settings, screens: &[Screen]) -> watch::Config {
    let shown = settings.shown();
    let can: HashMap<&str, bool> = actions::ACTIONS.iter().map(|a| (a.id, a.id != actions::NOTHING && actions::available(a))).collect();
    let armed = screens
        .iter()
        .map(|s| (s.key.clone(), corners::CORNERS.map(|c| can.get(settings.action(&s.key, c)).copied().unwrap_or(false))))
        .collect();
    watch::Config { dwell_ms: settings.dwell, glow: settings.glow.then(|| glow::accent(&shown.palette)), armed }
}

/// The settings or the screens changed: tell the pointer's thread.
fn rewatch(app: &AppHandle) {
    let settings = app.state::<SettingsState>().0.lock().unwrap().clone();
    let screens = app.state::<ScreensState>().0.lock().unwrap().clone();
    app.state::<WatchState>().0.set(config(&settings, &screens));
}

// --- tray and settings window ------------------------------------------------------------

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
    let settings_i = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let autostart_i = CheckMenuItem::with_id(app, "autostart", "Start at sign-in", true, autostart_on, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit Mitre", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings_i, &PredefinedMenuItem::separator(app)?, &autostart_i, &PredefinedMenuItem::separator(app)?, &quit_i])?;
    app.manage(AutostartItem(autostart_i));

    TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .tooltip("Mitre")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
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
                show_settings(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Mullion started Mitre, or reached the one running: Mullion's tray icon and sign-in entry stand
/// in for Mitre's own, so there's one of each for the whole suite.
fn leave_to_mullion(app: &AppHandle) {
    let _ = app.remove_tray_by_id("main");
    let al = app.autolaunch();
    if al.is_enabled().unwrap_or(false) {
        let _ = al.disable();
    }
}

/// The settings window: made when it opens and dropped when it closes, so Mitre keeps no page at
/// all while it waits.
fn show_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    // Hidden until the page has drawn itself (`settings_ready`), so it never flashes white.
    let _ = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("Mitre Settings")
        .inner_size(640.0, 760.0)
        .min_inner_size(520.0, 480.0)
        .center()
        .visible(false)
        .additional_browser_args(look::BROWSER_ARGS)
        .build();
}

// --- the settings window's commands ----------------------------------------------------------

#[tauri::command]
fn hello(app: AppHandle, settings: State<SettingsState>, screens: State<ScreensState>) -> Hello {
    // An app installed since Mitre started: its actions work from now.
    rewatch(&app);
    Hello {
        settings: settings.0.lock().unwrap().shown(),
        actions: actions::ACTIONS.iter().map(|a| ActionInfo { action: *a, available: actions::available(a) }).collect(),
        screens: corners::info(&screens.0.lock().unwrap()),
        dwells: settings::DWELLS,
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        mullion: suite::is_run_by_mullion(),
        shared_look: look::shared().is_some(),
        icon: icon(),
        version: env!("CARGO_PKG_VERSION").into(),
        settings_path: settings::path().display().to_string(),
    }
}

/// Mitre's own icon, for the settings window's header.
fn icon() -> String {
    use base64::Engine;
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(include_bytes!("../icons/128x128.png")))
}

/// The corners, the dwell, the glow and the look; each works at once.
#[tauri::command]
fn save_settings(app: AppHandle, state: State<SettingsState>, settings: Settings) -> Result<Settings, String> {
    let settings = {
        let mut s = state.0.lock().unwrap();
        *s = settings.clamp().keep_look(&s);
        s.clone()
    };
    settings.save().map_err(|e| e.to_string())?;
    rewatch(&app);
    let shown = settings.shown();
    let _ = app.emit("settings", &shown);
    Ok(shown)
}

/// Mullion's look for every app changed, or went on or off: show what Mitre looks like now, and
/// glow in its accent.
fn look_again(app: &AppHandle) {
    rewatch(app);
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

/// The suite's latest release on GitHub, in the browser (the page says so on the button), for
/// the apps a corner can open that aren't installed.
#[tauri::command]
fn get_apps() -> Result<(), String> {
    mullion_core::ffi::shell_open(RELEASES, None, "open")
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}
