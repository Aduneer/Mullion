// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Sash: the window in front snapped to a half, a third or a quarter of the screen, maximized,
//! centred or sent to the next screen, from the keyboard. It waits in the background (with a tray
//! icon, or in Mullion's) for its shortcuts, and has no window of its own but its settings, and a
//! note when Windows won't let it move a window.

mod actions;
mod ffi;
mod layout;
mod mover;
mod settings;

use actions::ACTIONS;
use mullion_core::hotkey::{self, Hotkeys};
use mullion_core::look;
use mullion_core::suite::{self, Ask};
use mover::Refused;
use serde::Serialize;
use settings::Settings;
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::webview::Color;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

pub struct SettingsState(pub Mutex<Settings>);
/// The shortcuts, and why each one that isn't working isn't, by action.
struct HotkeyState(Hotkeys, Mutex<BTreeMap<String, String>>);
/// The note on screen: what it says, and which showing of it this is (for hiding it later).
struct NoteState(Mutex<(u64, String)>);
struct AutostartItem(CheckMenuItem<Wry>);

/// What the settings window shows.
#[derive(Serialize)]
struct Hello {
    settings: Settings,
    actions: &'static [actions::Action],
    /// Why a shortcut isn't working, by action, for those that aren't.
    errors: BTreeMap<String, String>,
    gaps: &'static [u32],
    autostart: bool,
    /// Mullion started Sash, and has its tray icon and sign-in entry.
    mullion: bool,
    /// Mullion's look for every app stands in for Sash's theme and palette.
    shared_look: bool,
    icon: String,
    version: String,
    settings_path: String,
}

#[derive(Serialize)]
struct NoteHello {
    theme: String,
    palette: String,
    text: String,
    icon: String,
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
        .invoke_handler(tauri::generate_handler![
            hello,
            set_shortcut,
            recording,
            save_settings,
            set_autostart,
            settings_ready,
            snap_settings,
            quit,
            note_hello,
            note_ready
        ])
        .setup(|app| {
            let args: Vec<String> = std::env::args().collect();
            // A --quit that gets this far found no Sash running, so there's nothing to quit.
            if suite::ask(&args) == Ask::Quit {
                std::process::exit(0);
            }
            let first_run = !settings::path().exists();
            let settings = Settings::load().clamp();
            app.manage(SettingsState(Mutex::new(settings.clone())));
            app.manage(NoteState(Mutex::new((0, String::new()))));

            // Mullion's tray icon stands in for Sash's when Mullion started it.
            if suite::from_mullion(&args) {
                leave_to_mullion(app.handle());
            } else {
                build_tray(app.handle())?;
            }

            let handle = app.handle().clone();
            let moves = mover::start(move |r| {
                let app = handle.clone();
                let _ = handle.run_on_main_thread(move || show_note(&app, &note_text(&r)));
            });

            let handle = app.handle().clone();
            let specs: Vec<(i32, &str)> = ACTIONS.iter().map(|a| (actions::hotkey_id(a.id).unwrap_or(0), settings.shortcut(a.id))).collect();
            let (hk, results) = Hotkeys::start(&specs, move |id| {
                let s = handle.state::<SettingsState>().0.lock().unwrap().clone();
                if let Some(job) = mover::job(id, s.cycle, s.gap) {
                    let _ = moves.send(job);
                }
            });
            let errors = results
                .into_iter()
                .filter_map(|(id, r)| Some((actions::by_hotkey_id(id)?.id.to_string(), r.err()?)))
                .collect();
            app.manage(HotkeyState(hk, Mutex::new(errors)));

            match suite::ask(&args) {
                Ask::Show | Ask::Settings => show_settings(app.handle()),
                // Nothing on the screen says Sash has started, so the first time it shows its
                // settings, with the shortcuts.
                _ if first_run && !suite::is_run_by_mullion() => {
                    let _ = settings.save();
                    show_settings(app.handle());
                }
                _ => {}
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Sash")
        .run(|_app, event| {
            // Closing the settings window leaves Sash waiting for its shortcuts.
            if let RunEvent::ExitRequested { code: None, api, .. } = event {
                api.prevent_exit();
            }
        });
}

// --- tray and settings window ------------------------------------------------------------

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
    let settings_i = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let autostart_i = CheckMenuItem::with_id(app, "autostart", "Start at sign-in", true, autostart_on, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit Sash", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings_i, &PredefinedMenuItem::separator(app)?, &autostart_i, &PredefinedMenuItem::separator(app)?, &quit_i])?;
    app.manage(AutostartItem(autostart_i));

    TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .tooltip("Sash")
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

/// Mullion started Sash, or reached the one running: Mullion's tray icon and sign-in entry stand
/// in for Sash's own, so there's one of each for the whole suite.
fn leave_to_mullion(app: &AppHandle) {
    let _ = app.remove_tray_by_id("main");
    let al = app.autolaunch();
    if al.is_enabled().unwrap_or(false) {
        let _ = al.disable();
    }
}

/// The settings window: made when it opens and dropped when it closes, so Sash keeps no page at
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
        .title("Sash Settings")
        .inner_size(600.0, 720.0)
        .min_inner_size(480.0, 420.0)
        .center()
        .visible(false)
        .additional_browser_args(look::BROWSER_ARGS)
        .build();
}

// --- the note --------------------------------------------------------------------------------

/// A title short enough for one line of the note.
fn short(title: &str) -> String {
    let t: Vec<char> = title.chars().collect();
    if t.len() > 44 {
        format!("{}…", t[..42].iter().collect::<String>().trim_end())
    } else {
        title.to_string()
    }
}

fn note_text(r: &Refused) -> String {
    match r {
        Refused::Elevated(t) => format!("“{}” is running as administrator, so Windows won’t let Sash move it.", short(t)),
        Refused::Hung(t) => format!("“{}” isn’t responding, so Sash left it where it is.", short(t)),
    }
}

/// How long the note stays.
const NOTE_FOR: Duration = Duration::from_millis(3600);

/// Say why a window didn't move, in a small note at the bottom of its screen that never takes
/// the focus and goes by itself. Made each time and dropped after.
fn show_note(app: &AppHandle, text: &str) {
    let state = app.state::<NoteState>();
    let showing = {
        let mut note = state.0.lock().unwrap();
        note.0 += 1;
        note.1 = text.to_string();
        note.0
    };
    if let Some(w) = app.get_webview_window("note") {
        let _ = w.emit("note", text);
    } else {
        let _ = WebviewWindowBuilder::new(app, "note", WebviewUrl::App("note.html".into()))
            .title("Sash")
            .decorations(false)
            .resizable(false)
            .skip_taskbar(true)
            .always_on_top(true)
            .visible(false)
            .focused(false)
            .background_color(Color(13, 13, 13, 255))
            .additional_browser_args(look::BROWSER_ARGS)
            .build();
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(NOTE_FOR);
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if handle.state::<NoteState>().0.lock().unwrap().0 == showing {
                if let Some(w) = handle.get_webview_window("note") {
                    let _ = w.destroy();
                }
            }
        });
    });
}

#[tauri::command]
fn note_hello(settings: State<SettingsState>, note: State<NoteState>) -> NoteHello {
    let s = settings.0.lock().unwrap().shown();
    NoteHello { theme: s.theme, palette: s.palette, text: note.0.lock().unwrap().1.clone(), icon: icon() }
}

/// The note has drawn itself, `w` by `h` CSS pixels: put it at the bottom of the screen in front,
/// and show it without taking the focus from the window there.
#[tauri::command]
fn note_ready(window: WebviewWindow, w: f64, h: f64) {
    let Some((work, dpi)) = mover::front_screen() else { return };
    let scale = dpi as f64 / 96.0;
    let (pw, ph) = ((w * scale).round() as i32, (h * scale).round() as i32);
    let x = work.left + (work.right - work.left - pw) / 2;
    let y = work.bottom - ph - (32.0 * scale) as i32;
    let Ok(hwnd) = window.hwnd() else { return };
    let hwnd = hwnd.0 as isize;
    look::round_corners(hwnd);
    unsafe {
        ffi::SetWindowPos(hwnd, ffi::HWND_TOPMOST, x, y, pw, ph, ffi::SWP_NOACTIVATE);
        ffi::ShowWindow(hwnd, ffi::SW_SHOWNOACTIVATE);
    }
}

// --- the settings window's commands ----------------------------------------------------------

#[tauri::command]
fn hello(app: AppHandle, settings: State<SettingsState>, hotkeys: State<HotkeyState>) -> Hello {
    Hello {
        settings: settings.0.lock().unwrap().shown(),
        actions: ACTIONS,
        errors: hotkeys.1.lock().unwrap().clone(),
        gaps: settings::GAPS,
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        mullion: suite::is_run_by_mullion(),
        shared_look: look::shared().is_some(),
        icon: icon(),
        version: env!("CARGO_PKG_VERSION").into(),
        settings_path: settings::path().display().to_string(),
    }
}

/// Sash's own icon, for the settings window's header and the note.
fn icon() -> String {
    use base64::Engine;
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(include_bytes!("../icons/128x128.png")))
}

/// Give `action` the shortcut `spec`, or none if it's empty. Kept only if Windows takes it; the
/// shortcuts come back on either way, after `recording`.
#[tauri::command]
fn set_shortcut(app: AppHandle, state: State<SettingsState>, hotkeys: State<HotkeyState>, action: String, spec: String) -> Result<Settings, String> {
    hotkeys.0.pause(false);
    let id = actions::hotkey_id(&action).ok_or("Sash has no such action")?;
    let spec = settings::normal(&spec);
    if !spec.is_empty() {
        if hotkey::parse(&spec).is_none() {
            return Err(format!("{} can’t be a shortcut", hotkey::label(&spec)));
        }
        let s = state.0.lock().unwrap();
        if let Some(other) = ACTIONS.iter().find(|a| a.id != action && s.shortcut(a.id) == spec) {
            return Err(format!("{} is already {}", hotkey::label(&spec), other.name));
        }
    }
    hotkeys.0.set(id, &spec).map_err(|e| format!("{}: {e}", hotkey::label(&spec)))?;
    hotkeys.1.lock().unwrap().remove(&action);
    let settings = {
        let mut s = state.0.lock().unwrap();
        s.shortcuts.insert(action, spec);
        s.clone()
    };
    settings.save().map_err(|e| e.to_string())?;
    let shown = settings.shown();
    let _ = app.emit("settings", &shown);
    Ok(shown)
}

/// The settings page is listening for a new shortcut: let go of Sash's own meanwhile, so it can
/// hear them too.
#[tauri::command]
fn recording(hotkeys: State<HotkeyState>, on: bool) {
    hotkeys.0.pause(on);
}

/// The look, the gap and pressing again. Shortcuts change one at a time (`set_shortcut`), so the
/// ones sent here are ignored.
#[tauri::command]
fn save_settings(app: AppHandle, state: State<SettingsState>, settings: Settings) -> Result<Settings, String> {
    let settings = {
        let mut s = state.0.lock().unwrap();
        let shortcuts = s.shortcuts.clone();
        *s = Settings { shortcuts, ..settings }.clamp().keep_look(&s);
        s.clone()
    };
    settings.save().map_err(|e| e.to_string())?;
    let shown = settings.shown();
    let _ = app.emit("settings", &shown);
    Ok(shown)
}

/// Mullion's look for every app changed, or went on or off: show what Sash looks like now.
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

/// Windows' own Snap settings (Settings → System → Multitasking), where its snapping can be
/// turned off. Sash never changes them itself.
#[tauri::command]
fn snap_settings() -> Result<(), String> {
    mullion_core::ffi::shell_open("ms-settings:multitasking", None, "open")
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_titles_are_cut_short() {
        assert_eq!(short("Task Manager"), "Task Manager");
        let long = "a".repeat(60);
        assert_eq!(short(&long).chars().count(), 43);
        assert!(note_text(&Refused::Elevated("Task Manager".into())).contains("“Task Manager”"));
    }
}
