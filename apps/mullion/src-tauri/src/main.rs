// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Mullion.exe: the suite as one app. It has the one tray icon, the one sign-in entry and the
//! settings window, and starts each app that's on with `--mullion`, so the apps leave those to
//! it (`mullion_core::suite`). Each app is still its own process, so one crashing or quitting
//! leaves the others running.
//!
//! It keeps no WebView of its own while the settings window is closed: the tray menu is Windows'
//! own, and the window is made when it opens and dropped when it closes.

mod settings;

use mullion_core::suite::{self, Ask, APPS};
use base64::Engine;
use mullion_core::{ffi, look};
use serde::Serialize;
use settings::Settings;
use std::sync::Mutex;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

/// What the sign-in entry passes, so starting at sign-in doesn't open the settings.
const SIGN_IN: &str = "--sign-in";
/// Where "Get it" goes for an app that isn't installed.
const RELEASES: &str = "https://github.com/Aduneer/Mullion/releases/latest";

struct SettingsState(Mutex<Settings>);

#[derive(Serialize)]
struct AppInfo {
    name: &'static str,
    about: &'static str,
    /// It has a window to open and settings of its own to show.
    shows: bool,
    installed: bool,
    on: bool,
    running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
}

#[derive(Serialize)]
struct Hello {
    apps: Vec<AppInfo>,
    /// Mullion's own icon.
    icon: Option<String>,
    autostart: bool,
    /// Off until the welcome has been through, so the page shows it.
    welcomed: bool,
    version: String,
    settings_path: String,
}

fn main() {
    tauri::Builder::default()
        // Starting it again opens the settings; `--quit` (from the installer) quits it and its apps.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| match suite::ask(&args) {
            Ask::Quit => quit_all(app),
            _ if args.iter().any(|a| a == SIGN_IN) => {}
            _ => open_settings(app),
        }))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![SIGN_IN])))
        .invoke_handler(tauri::generate_handler![hello, apps, set_app, welcome, open_app, get_app, set_autostart, ready, quit])
        .setup(|app| {
            let args: Vec<String> = std::env::args().collect();
            // A --quit that gets this far found no Mullion running, so there's nothing to quit.
            if suite::ask(&args) == Ask::Quit {
                std::process::exit(0);
            }
            let settings = Settings::load();
            // Someone new picks their apps in the welcome first; it starts them.
            let welcomed = settings.welcomed;
            if welcomed {
                start_apps(&settings);
            }
            app.manage(SettingsState(Mutex::new(settings)));
            build_tray(app.handle())?;
            if !welcomed || !args.iter().any(|a| a == SIGN_IN) {
                open_settings(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Mullion")
        .run(|_app, event| {
            // Closing the settings window leaves Mullion in the tray; only "Quit Mullion" ends it.
            if let RunEvent::ExitRequested { code: None, api, .. } = event {
                api.prevent_exit();
            }
        });
}

// --- the apps --------------------------------------------------------------------------

fn app_named(name: &str) -> Result<&'static suite::App, String> {
    APPS.iter().find(|a| a.name == name).ok_or_else(|| format!("no app called {name}"))
}

fn list(settings: &Settings, with_icons: bool) -> Vec<AppInfo> {
    APPS.iter()
        .map(|a| {
            let exe = suite::find_app(a);
            AppInfo {
                name: a.name,
                about: a.about,
                shows: a.shows,
                installed: exe.is_some(),
                on: settings.is_on(a.name),
                running: exe.as_ref().is_some_and(|e| !suite::running(e).is_empty()),
                icon: exe.filter(|_| with_icons).and_then(|_| icon_of(a.name)),
            }
        })
        .collect()
}

/// An app's icon (or Mullion's own), as a data: URL. They're built in rather than asked of the
/// shell, whose icon cache can keep showing an exe's old icon after it's replaced.
fn icon_of(name: &str) -> Option<String> {
    let png: &[u8] = match name {
        "Mullion" => include_bytes!("../icons/128x128.png"),
        "Oriel" => include_bytes!("../../../oriel/src-tauri/icons/128x128.png"),
        "Lintel" => include_bytes!("../../../lintel/src-tauri/icons/128x128.png"),
        "Fanlight" => include_bytes!("../../../fanlight/src-tauri/icons/128x128.png"),
        _ => return None,
    };
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
}

/// Start every app that's on, or tell the ones already running that they're Mullion's now.
fn start_apps(settings: &Settings) {
    for a in APPS.iter().filter(|a| settings.is_on(a.name)) {
        if let Some(exe) = suite::find_app(a) {
            let _ = suite::send(&exe, Ask::Nothing);
        }
    }
}

/// Quit Mullion, and the apps it runs; an app that's off and running on its own stays.
fn quit_all(app: &AppHandle) {
    let settings = app.state::<SettingsState>().0.lock().unwrap().clone();
    for a in APPS.iter().filter(|a| settings.is_on(a.name)) {
        if let Some(exe) = suite::find_app(a).filter(|e| !suite::running(e).is_empty()) {
            let _ = suite::send(&exe, Ask::Quit);
        }
    }
    app.exit(0);
}

// --- the settings window -----------------------------------------------------------------

fn open_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let welcomed = app.state::<SettingsState>().0.lock().unwrap().welcomed;
    // Hidden until the page has drawn itself (`ready`), so it never flashes white.
    let _ = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("index.html".into()))
        .title(if welcomed { "Mullion Settings" } else { "Welcome to Mullion" })
        .inner_size(560.0, 680.0)
        .min_inner_size(460.0, 420.0)
        .center()
        .visible(false)
        .additional_browser_args(look::BROWSER_ARGS)
        .build();
}

// --- tray ------------------------------------------------------------------------------

fn tray_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let settings = app.state::<SettingsState>().0.lock().unwrap().clone();
    let menu = Menu::new(app)?;
    let mut opens = false;
    // Before the welcome no app is started, so there's none to open.
    for a in APPS.iter().filter(|a| settings.welcomed && a.shows && settings.is_on(a.name) && suite::find_app(a).is_some()) {
        menu.append(&MenuItem::with_id(app, format!("open:{}", a.name), format!("Open {}", a.name), true, None::<&str>)?)?;
        opens = true;
    }
    if opens {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
    menu.append(&MenuItem::with_id(app, "settings", "Mullion Settings…", true, None::<&str>)?)?;
    menu.append(&CheckMenuItem::with_id(app, "autostart", "Start at sign-in", true, autostart_on, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", "Quit Mullion", true, None::<&str>)?)?;
    Ok(menu)
}

fn refresh_tray(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id("main"), tray_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .tooltip("Mullion")
        .menu(&tray_menu(app)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => open_settings(app),
            "autostart" => {
                let on = !app.autolaunch().is_enabled().unwrap_or(false);
                if let Ok(on) = set_autostart(app.clone(), on) {
                    let _ = app.emit_to("settings", "autostart", on);
                }
            }
            "quit" => quit_all(app),
            id => {
                if let Some(Ok(a)) = id.strip_prefix("open:").map(app_named) {
                    if let Some(exe) = suite::find_app(a) {
                        let _ = suite::send(&exe, Ask::Show);
                    }
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                open_settings(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

// --- commands --------------------------------------------------------------------------

#[tauri::command]
fn hello(app: AppHandle, state: State<SettingsState>) -> Hello {
    let settings = state.0.lock().unwrap().clone();
    Hello {
        apps: list(&settings, true),
        icon: icon_of("Mullion"),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        welcomed: settings.welcomed,
        version: env!("CARGO_PKG_VERSION").into(),
        settings_path: settings::path().display().to_string(),
    }
}

/// Which apps are installed, on and running, without their icons: the page asks every few
/// seconds while it's open, as apps start and quit.
#[tauri::command]
fn apps(state: State<SettingsState>) -> Vec<AppInfo> {
    list(&state.0.lock().unwrap(), false)
}

/// Turn an app on (start it) or off (quit it, and don't start it again).
#[tauri::command]
fn set_app(app: AppHandle, state: State<SettingsState>, name: String, on: bool) -> Result<Vec<AppInfo>, String> {
    let a = app_named(&name)?;
    let settings = {
        let mut s = state.0.lock().unwrap();
        s.set(a.name, on);
        s.save().map_err(|e| e.to_string())?;
        s.clone()
    };
    if let Some(exe) = suite::find_app(a) {
        if on {
            suite::send(&exe, Ask::Nothing).map_err(|e| e.to_string())?;
        } else if !suite::running(&exe).is_empty() {
            suite::send(&exe, Ask::Quit).map_err(|e| e.to_string())?;
        }
    }
    refresh_tray(&app);
    Ok(list(&settings, false))
}

/// The welcome's Start: keep the apps it turned off, and start the rest.
#[tauri::command]
fn welcome(app: AppHandle, window: WebviewWindow, state: State<SettingsState>, off: Vec<String>) -> Result<(), String> {
    let settings = {
        let mut s = state.0.lock().unwrap();
        for a in APPS.iter() {
            s.set(a.name, !off.iter().any(|o| o == a.name));
        }
        s.welcomed = true;
        s.save().map_err(|e| e.to_string())?;
        s.clone()
    };
    start_apps(&settings);
    refresh_tray(&app);
    let _ = window.set_title("Mullion Settings");
    Ok(())
}

/// Open an app's window, or its settings.
#[tauri::command]
fn open_app(name: String, settings: bool) -> Result<(), String> {
    let a = app_named(&name)?;
    let exe = suite::find_app(a).ok_or_else(|| format!("{name} isn't installed"))?;
    suite::send(&exe, if settings { Ask::Settings } else { Ask::Show }).map_err(|e| e.to_string())
}

/// The release page, for an app that isn't installed.
#[tauri::command]
fn get_app() -> Result<(), String> {
    ffi::shell_open(RELEASES, None, "open")
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let al = app.autolaunch();
    if enabled { al.enable() } else { al.disable() }.map_err(|e| e.to_string())?;
    let on = al.is_enabled().map_err(|e| e.to_string())?;
    refresh_tray(&app);
    Ok(on)
}

/// The page has drawn itself: show its window.
#[tauri::command]
fn ready(window: WebviewWindow) {
    let _ = window.show();
    let _ = window.set_focus();
}

#[tauri::command]
fn quit(app: AppHandle) {
    quit_all(&app);
}
