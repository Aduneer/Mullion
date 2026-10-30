// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Sill: a dock. The apps you've pinned and the ones running, a dot for each window, the button for
//! Fanlight's apps, and the notification area's icons, on a shelf along the bottom of the screen.
//! By default it takes the taskbar's place, and the taskbar is hidden while it runs (`taskbar`);
//! or it floats above the taskbar and hides until the pointer reaches the edge (`dock`).
//!
//! It hosts the notification area itself (`host`), so the icons are there with the taskbar
//! hidden, and Explorer's tray keeps all of them too.

mod dock;
mod ffi;
mod host;
mod menu;
mod pins;
mod settings;
mod taskbar;
mod trayicons;

use mullion_core::apps;
use mullion_core::look;
use mullion_core::suite::{self, Ask};
use mullion_core::wallpaper;
use pins::Item;
use serde::Serialize;
use settings::{Pin, Settings};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, Once};
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::window::Color;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use trayicons::Shown;

/// Where to get Fanlight, when it isn't installed.
const RELEASES: &str = "https://github.com/Aduneer/Mullion/releases/latest";

struct SettingsState(Mutex<Settings>);
/// Pinned apps' icons, for when they aren't running, by key.
struct PinIcons(Mutex<HashMap<String, Option<String>>>);
/// The tray's icons as last drawn, and a way to have them drawn again.
struct Tray(Mutex<Vec<Shown>>, Mutex<Sender<()>>);
struct AutostartItem(CheckMenuItem<Wry>);

/// What the dock and the settings window are shown.
#[derive(Serialize)]
struct Hello {
    settings: Settings,
    items: Vec<Item>,
    tray: Vec<Shown>,
    /// Fanlight is installed, so its button works.
    fanlight: bool,
    autostart: bool,
    /// Mullion started Sill, and has its tray icon and sign-in entry.
    mullion: bool,
    /// Mullion's look for every app stands in for Sill's theme, palette and glass.
    shared_look: bool,
    icon: String,
    version: String,
    settings_path: String,
    /// The screens to put the dock on, the main one first.
    screens: Vec<ScreenChoice>,
}

/// A screen the dock can go on, as the settings list it.
#[derive(Serialize)]
struct ScreenChoice {
    /// What `Settings::screen` holds for it: empty for the main screen.
    id: String,
    name: String,
    size: String,
}

fn screen_choices() -> Vec<ScreenChoice> {
    let mut all: Vec<ScreenChoice> = ffi::screens()
        .into_iter()
        .map(|s| {
            // "\\.\DISPLAY2" is Screen 2, as Windows' display settings number them.
            let n: String = s.device.chars().rev().take_while(char::is_ascii_digit).collect::<Vec<_>>().into_iter().rev().collect();
            let name = if n.is_empty() { s.device.clone() } else { format!("Screen {n}") };
            let size = format!("{} × {}", s.rect.right - s.rect.left, s.rect.bottom - s.rect.top);
            ScreenChoice { id: if s.main { String::new() } else { s.device }, name: if s.main { format!("{name} (main)") } else { name }, size }
        })
        .collect();
    all.sort_by_key(|c| !c.id.is_empty());
    all
}

fn main() {
    // sill-guard.exe, after a Sill that hid the taskbar ended without putting it back, or an
    // uninstaller making sure.
    if std::env::args().any(|a| a == taskbar::RESTORE) {
        taskbar::restore_if_left();
        return;
    }
    tauri::Builder::default()
        // Starting it again (from Start, say) opens its settings. Mullion starts it again to ask
        // something of it (`mullion_core::suite`).
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let from_mullion = suite::from_mullion(&args);
            if from_mullion {
                leave_to_mullion(app);
            }
            match suite::ask(&args) {
                Ask::Quit => quit(app.clone()),
                Ask::Settings | Ask::Show | Ask::Toggle => show_settings(app),
                Ask::Look => look_again(app),
                Ask::Nothing if !from_mullion => show_settings(app),
                Ask::Nothing => {}
            }
        }))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .invoke_handler(tauri::generate_handler![
            hello, activate, new_window, app_menu, dock_menu, reorder, open_grid, tray_click, tray_more, shelf, wallpaper, wallpaper_image,
            save_settings, set_autostart, settings_ready, get_apps, pin_paths, unpin, start_menu_apps, picker_show, picker_pick,
            picker_close, picker_hide, quit
        ])
        .setup(|app| {
            let args: Vec<String> = std::env::args().collect();
            // A --quit that gets this far found no Sill running, so there's nothing to quit.
            if suite::ask(&args) == Ask::Quit {
                std::process::exit(0);
            }
            let first_run = !settings::path().exists();
            let mut settings = Settings::load().clamp();
            if first_run {
                // The dock starts with what the taskbar had pinned. COM on a thread of its own.
                settings.pins = std::thread::spawn(|| {
                    com();
                    pins::from_taskbar()
                })
                .join()
                .unwrap_or_default();
                let _ = settings.save();
            }
            if suite::from_mullion(&args) {
                leave_to_mullion(app.handle());
            } else {
                build_tray(app.handle())?;
            }
            app.manage(SettingsState(Mutex::new(settings.clone())));
            app.manage(PinIcons(Mutex::new(HashMap::new())));
            app.manage(wallpaper::Image::new());
            let (redraw, wanted) = channel::<()>();
            app.manage(Tray(Mutex::new(Vec::new()), Mutex::new(redraw.clone())));
            menu::theme(&settings.shown().theme);

            // The dock claims its strip first, while the shell still answers it directly; then
            // the taskbar goes, and then Sill takes over the notification area.
            let dock = create_dock(app.handle())?;
            dock::init(dock.hwnd()?.0 as isize);
            // A shortcut, program or folder dropped on the dock is pinned there.
            let drops = app.handle().clone();
            dock.on_window_event(move |e| {
                if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = e {
                    pin_files(&drops, paths);
                }
            });
            let slide = app.handle().clone();
            dock::on_slide(move |show| {
                let _ = slide.emit_to("dock", "slide", show);
            });
            ffi::set_screen(&settings.screen);
            dock::apply(layout(&settings));
            create_picker(app.handle())?;
            if settings.float {
                // Put back a taskbar a Sill that crashed left hidden.
                taskbar::restore();
            } else {
                taskbar::hide();
            }
            let handle = app.handle().clone();
            host::start(move |e| match e {
                host::Event::Icons => {
                    let _ = redraw.send(());
                }
                host::Event::Explorer => explorer_restarted(&handle),
                host::Event::Beat => {
                    taskbar::rehide();
                    dock::beat();
                }
            });
            draw_tray(app.handle().clone(), wanted);
            dock::watch_pointer();
            keep(app.handle().clone());

            let running = app.handle().clone();
            apps::start(|_| {}, move |_| emit_items(&running));
            load_pin_icons(app.handle(), settings.pins.clone());

            if matches!(suite::ask(&args), Ask::Show | Ask::Settings | Ask::Toggle) {
                show_settings(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Sill")
        .run(|_app, event| match event {
            // Closing the settings window leaves the dock.
            RunEvent::ExitRequested { code: None, api, .. } => api.prevent_exit(),
            RunEvent::Exit => clean_up(),
            _ => {}
        });
}

/// COM, for the shell's icons and shortcuts, on the calling thread.
fn com() {
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
    }
}

fn layout(s: &Settings) -> dock::Layout {
    dock::Layout { edge: dock::edge(&s.edge), thickness: s.thickness(), float: s.float }
}

/// Give everything back: the strip, the notification area, the taskbar. Once.
fn clean_up() {
    static DONE: Once = Once::new();
    DONE.call_once(|| {
        dock::unregister();
        host::stop();
        taskbar::restore();
    });
}

/// Explorer restarted: its new taskbars are showing and it knows no app bars. Once it's settled
/// (the host gives it our icons then too), hide them again and claim the strip again.
fn explorer_restarted(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1500));
        let a = app.clone();
        let _ = app.run_on_main_thread(move || {
            dock::register_again();
            if !a.state::<SettingsState>().0.lock().unwrap().float {
                taskbar::hide();
            }
        });
    });
}

/// Once a second, what waits on Explorer and so can't be on the host's beat: auto-hide kept on,
/// and the dock's strip kept out of the work area.
fn keep(app: AppHandle) {
    let _ = std::thread::Builder::new().name("keep".into()).spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        taskbar::keep_autohide();
        let l = dock::layout();
        let floating = app.state::<SettingsState>().0.lock().unwrap().float;
        if let (false, Some(rect)) = (floating, dock::rect()) {
            taskbar::keep_work_area(rect, l.edge);
        }
    });
}

// --- what the dock shows --------------------------------------------------------------

fn items(app: &AppHandle) -> Vec<Item> {
    let pins = app.state::<SettingsState>().0.lock().unwrap().pins.clone();
    let icons = app.state::<PinIcons>().0.lock().unwrap().clone();
    pins::items(&pins, &apps::running(), &icons)
}

/// To the dock, and the settings window's list of pinned apps.
fn emit_items(app: &AppHandle) {
    let _ = app.emit("items", items(app));
}

/// Pinned apps' own icons, for when they aren't running: the shell draws them on a thread of its own.
fn load_pin_icons(app: &AppHandle, pins: Vec<Pin>) {
    let app = app.clone();
    std::thread::spawn(move || {
        com();
        for p in pins {
            if app.state::<PinIcons>().0.lock().unwrap().contains_key(&p.key) {
                continue;
            }
            let icon = pins::icon(&p);
            app.state::<PinIcons>().0.lock().unwrap().insert(p.key, icon);
        }
        emit_items(&app);
    });
}

/// Draw the tray's icons whenever the host says they changed, a little after, so a burst of
/// changes (every program re-adding its icons) is drawn once.
fn draw_tray(app: AppHandle, wanted: std::sync::mpsc::Receiver<()>) {
    let _ = std::thread::Builder::new().name("tray-icons".into()).spawn(move || {
        let mut pictures = trayicons::Pictures::default();
        while wanted.recv().is_ok() {
            std::thread::sleep(Duration::from_millis(80));
            while wanted.try_recv().is_ok() {}
            let list = pictures.list();
            let tray = app.state::<Tray>();
            let mut last = tray.0.lock().unwrap();
            if *last != list {
                *last = list.clone();
                drop(last);
                let _ = app.emit_to("dock", "tray", list);
            }
        }
    });
}

// --- tray and settings window --------------------------------------------------------------

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
    let settings_i = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let autostart_i = CheckMenuItem::with_id(app, "autostart", "Start at sign-in", true, autostart_on, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit Sill", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings_i, &PredefinedMenuItem::separator(app)?, &autostart_i, &PredefinedMenuItem::separator(app)?, &quit_i])?;
    app.manage(AutostartItem(autostart_i));

    TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .tooltip("Sill")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => show_settings(app),
            "autostart" => {
                let on = !app.autolaunch().is_enabled().unwrap_or(false);
                let _ = set_autostart(app.clone(), on);
            }
            "quit" => quit(app.clone()),
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

/// Mullion started Sill, or reached the one running: Mullion's tray icon and sign-in entry stand
/// in for Sill's own, so there's one of each for the whole suite.
fn leave_to_mullion(app: &AppHandle) {
    let _ = app.remove_tray_by_id("main");
    let al = app.autolaunch();
    if al.is_enabled().unwrap_or(false) {
        let _ = al.disable();
    }
}

fn create_dock(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(app, "dock", WebviewUrl::App("dock.html".into()))
        .title("Sill")
        .inner_size(800.0, 66.0)
        .decorations(false)
        .resizable(false)
        .shadow(false)
        .skip_taskbar(true)
        .always_on_top(true)
        // Clicking the dock leaves the focus with the app you're in, as the taskbar does.
        .focusable(false)
        .visible(false)
        // See-through: the page draws the shelf, and the rest of the strip is the wallpaper.
        .transparent(true)
        .background_color(Color(0, 0, 0, 0))
        .additional_browser_args(look::BROWSER_ARGS)
        .build()
}

/// The settings window: made when it opens and dropped when it closes.
fn show_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    // Built from a thread of its own, and once. On the main thread, WebView2 pumps messages while
    // Tauri holds its webviews' lock, so a second ask arriving then (Mullion's, a second click)
    // would wait on that lock for good.
    static OPENING: AtomicBool = AtomicBool::new(false);
    if OPENING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        // Hidden until the page has drawn itself (`settings_ready`), so it never flashes white.
        let _ = WebviewWindowBuilder::new(&app, "settings", WebviewUrl::App("settings.html".into()))
            .title("Sill Settings")
            .inner_size(600.0, 720.0)
            .min_inner_size(480.0, 420.0)
            .center()
            .visible(false)
            .additional_browser_args(look::BROWSER_ARGS)
            .build();
        OPENING.store(false, Ordering::SeqCst);
    });
}

/// Mullion's look for every app changed, or went on or off: show what Sill looks like now.
fn look_again(app: &AppHandle) {
    let shown = app.state::<SettingsState>().0.lock().unwrap().shown();
    menu::theme(&shown.theme);
    let _ = app.emit("look", look::shared().is_some());
    let _ = app.emit("settings", &shown);
}

fn fanlight() -> Option<std::path::PathBuf> {
    suite::APPS.iter().find(|a| a.name == "Fanlight").and_then(suite::find_app)
}

// --- the dock's commands -------------------------------------------------------------------

#[tauri::command]
fn hello(app: AppHandle, settings: State<SettingsState>, tray: State<Tray>) -> Hello {
    // Ask for the tray's icons too, in case they came before the page was listening.
    let _ = tray.1.lock().unwrap().send(());
    // Each lock taken and let go on its own line: a guard in the struct below would be held to
    // its end, and `items` takes the settings' lock too.
    let shown = settings.0.lock().unwrap().shown();
    let icons = tray.0.lock().unwrap().clone();
    Hello {
        settings: shown,
        items: items(&app),
        tray: icons,
        fanlight: fanlight().is_some(),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        mullion: suite::is_run_by_mullion(),
        shared_look: look::shared().is_some(),
        icon: icon(),
        version: env!("CARGO_PKG_VERSION").into(),
        settings_path: settings::path().display().to_string(),
        screens: screen_choices(),
    }
}

/// Sill's own icon, for the settings window's header.
fn icon() -> String {
    use base64::Engine;
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(include_bytes!("../icons/128x128.png")))
}

/// A click on an app: bring it forward, or minimize it if it's already in front; with several
/// windows, a menu of them to pick from; open it if it isn't running. Says which ("focus",
/// "minimize", "menu", "launch"), so the page can bounce a launch. `x` and `y` are where a menu
/// opens from, on the page.
#[tauri::command]
fn activate(app: AppHandle, window: WebviewWindow, key: String, x: f64, y: f64) -> Result<&'static str, String> {
    activate_at(&app, &window, &key, x, y)
}

fn activate_at(app: &AppHandle, window: &WebviewWindow, key: &str, x: f64, y: f64) -> Result<&'static str, String> {
    if let Some(a) = apps::running().into_iter().find(|a| a.key == key) {
        if mullion_core::ffi::above_us(a.pid) {
            return out_of_reach(app, window, &a.name, &a.launch, x, y);
        }
        if a.windows.len() > 1 {
            return pick_window(app, window, &a, x, y);
        }
        let w = a.windows.first().ok_or("it has no windows open")?;
        if w.front {
            apps::minimize(w.hwnd, w.pid)?;
            return Ok("minimize");
        }
        apps::focus(w.hwnd, w.pid)?;
        return Ok("focus");
    }
    let pin = app.state::<SettingsState>().0.lock().unwrap().pins.iter().find(|p| p.key == key).cloned().ok_or("that app isn't in the dock")?;
    mullion_core::ffi::shell_open(&pin.launch, None, "open")?;
    Ok("launch")
}

/// A window's title as a menu shows it: long ones cut short.
fn menu_title(title: &str) -> String {
    if title.chars().count() > 60 {
        format!("{}…", title.chars().take(59).collect::<String>())
    } else {
        title.to_string()
    }
}

/// What the picker shows: an app's windows, in the dock's look.
#[derive(Serialize, Clone)]
struct Picking {
    name: String,
    icon: Option<String>,
    edge: String,
    theme: String,
    palette: String,
    windows: Vec<PickWin>,
}

#[derive(Serialize, Clone)]
struct PickWin {
    hwnd: u64,
    pid: u32,
    title: String,
    front: bool,
    minimized: bool,
}

/// Where the picker opens from (physical pixels, on the dock's edge), and whose windows it shows.
static PICK_AT: Mutex<ffi::POINT> = Mutex::new(ffi::POINT { x: 0, y: 0 });
static PICK_PID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// The picker: made once, with the dock, and shown when wanted (a window made later, from a
/// command, risks the lock `show_settings` tells of).
fn create_picker(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let picker = WebviewWindowBuilder::new(app, "picker", WebviewUrl::App("picker.html".into()))
        .title("Sill")
        .inner_size(300.0, 160.0)
        .decorations(false)
        .resizable(false)
        .shadow(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(false)
        .focused(false)
        .transparent(true)
        .background_color(Color(0, 0, 0, 0))
        .additional_browser_args(look::BROWSER_ARGS)
        .build()?;
    // A click anywhere else puts it away.
    let a = app.clone();
    picker.on_window_event(move |e| {
        if let tauri::WindowEvent::Focused(false) = e {
            hide_picker(&a);
        }
    });
    Ok(picker)
}

fn hide_picker(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("picker") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
            let _ = app.emit_to("picker", "picker_hidden", ());
        }
    }
    PICK_PID.store(0, Ordering::SeqCst);
    dock::MENU_OPEN.store(false, Ordering::Relaxed);
}

/// An app with several windows open: which one, as the taskbar's thumbnails ask. The picker
/// draws them, then asks to be shown (`picker_show`). Clicking the app again puts it away.
fn pick_window(app: &AppHandle, window: &WebviewWindow, a: &apps::App, x: f64, y: f64) -> Result<&'static str, String> {
    if PICK_PID.load(Ordering::SeqCst) == a.pid {
        hide_picker(app);
        return Ok("menu");
    }
    let shown = app.state::<SettingsState>().0.lock().unwrap().shown();
    let windows = apps::windows_of(a.pid)
        .into_iter()
        .map(|w| PickWin { hwnd: w.hwnd, pid: w.pid, title: w.title, front: w.front, minimized: w.minimized })
        .collect();
    *PICK_AT.lock().unwrap() = on_screen(window, x, y);
    PICK_PID.store(a.pid, Ordering::SeqCst);
    dock::MENU_OPEN.store(true, Ordering::Relaxed);
    let picking = Picking { name: a.name.clone(), icon: a.icon.clone(), edge: shown.edge, theme: shown.theme, palette: shown.palette, windows };
    app.emit_to("picker", "pick", picking).map_err(|e| e.to_string())?;
    Ok("menu")
}

/// Where a picker of `w` by `h` goes: off the dock's `edge` at `at`, inside `work`.
fn picker_spot(at: ffi::POINT, edge: &str, w: i32, h: i32, work: ffi::RECT) -> (i32, i32) {
    let (x, y) = match edge {
        "left" => (at.x, at.y - h / 2),
        "right" => (at.x - w, at.y - h / 2),
        _ => (at.x - w / 2, at.y - h),
    };
    (x.clamp(work.left, (work.right - w).max(work.left)), y.clamp(work.top, (work.bottom - h).max(work.top)))
}

/// The picker has drawn its windows, at this size (CSS pixels): put it by the dock and show it.
#[tauri::command]
fn picker_show(app: AppHandle, window: WebviewWindow, width: f64, height: f64) {
    if PICK_PID.load(Ordering::SeqCst) == 0 {
        return;
    }
    let k = window.scale_factor().unwrap_or(1.0);
    let (w, h) = ((width * k).ceil() as i32, (height * k).ceil() as i32);
    let edge = app.state::<SettingsState>().0.lock().unwrap().edge.clone();
    let at = *PICK_AT.lock().unwrap();
    let (_, work) = ffi::screen();
    let (x, y) = picker_spot(at, &edge, w, h, work);
    let _ = window.set_size(tauri::PhysicalSize::new(w as u32, h as u32));
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    let _ = window.show();
    let _ = window.set_focus();
}

#[tauri::command]
fn picker_pick(app: AppHandle, hwnd: u64, pid: u32) -> Result<(), String> {
    hide_picker(&app);
    apps::focus(hwnd, pid)
}

/// The × on one of the picker's windows: asked to close, as its own × does.
#[tauri::command]
fn picker_close(hwnd: u64, pid: u32) -> Result<(), String> {
    apps::close(hwnd, pid)
}

/// Escape (`back`: the focus goes back to the app that had it), or no windows left to pick from.
#[tauri::command]
fn picker_hide(app: AppHandle, back: bool) {
    hide_picker(&app);
    if back {
        apps::restore_focus();
    }
}

/// An app running as administrator, while Sill isn't: Windows keeps Sill from its windows and its
/// tray icon. Say so, and offer to open it again, as Fanlight or Start would; one that runs once
/// (Task Manager) comes forward then.
fn out_of_reach(app: &AppHandle, window: &WebviewWindow, name: &str, launch: &str, x: f64, y: f64) -> Result<&'static str, String> {
    let entries = [
        menu::Entry::Item { text: format!("{name} runs as administrator, out of Sill’s reach"), checked: false, enabled: false },
        menu::Entry::Separator,
        menu::item(format!("Open {name}")),
    ];
    match popup(app, window, &entries, x, y) {
        Some(2) => {
            mullion_core::ffi::shell_open(launch, None, "open")?;
            Ok("focus")
        }
        _ => {
            apps::restore_focus();
            Ok("menu")
        }
    }
}

/// A middle click: a new window of the app, as on the taskbar (or the app, if it isn't running).
#[tauri::command]
fn new_window(state: State<SettingsState>, key: String) -> Result<(), String> {
    if apps::running().iter().any(|a| a.key == key) {
        return apps::launch(&key);
    }
    let pin = state.0.lock().unwrap().pins.iter().find(|p| p.key == key).cloned().ok_or("that app isn't in the dock")?;
    mullion_core::ffi::shell_open(&pin.launch, None, "open")
}

/// A point on the dock's page (CSS pixels) on the screen.
fn on_screen(window: &WebviewWindow, x: f64, y: f64) -> ffi::POINT {
    let pos = window.outer_position().unwrap_or_default();
    let k = window.scale_factor().unwrap_or(1.0);
    ffi::POINT { x: pos.x + (x * k).round() as i32, y: pos.y + (y * k).round() as i32 }
}

/// Show a menu from the dock, held open while it is. The index of what was picked.
fn popup(app: &AppHandle, window: &WebviewWindow, entries: &[menu::Entry], x: f64, y: f64) -> Option<usize> {
    let hwnd = window.hwnd().ok()?.0 as isize;
    let edge = app.state::<SettingsState>().0.lock().unwrap().edge.clone();
    dock::MENU_OPEN.store(true, std::sync::atomic::Ordering::Relaxed);
    let picked = menu::popup(hwnd, entries, on_screen(window, x, y), &edge);
    dock::MENU_OPEN.store(false, std::sync::atomic::Ordering::Relaxed);
    picked
}

/// What an app's menu can do.
enum Do {
    Focus(u64, u32),
    NewWindow,
    Pin,
    Unpin,
    Open,
    Quit(u32),
}

/// A right click on an app: its windows, a new one, Pin or Unpin, and Quit. `x` and `y` are where
/// the menu opens from, on the page.
#[tauri::command]
fn app_menu(app: AppHandle, window: WebviewWindow, key: String, x: f64, y: f64) -> Result<(), String> {
    let running = apps::running().into_iter().find(|a| a.key == key);
    let pinned = app.state::<SettingsState>().0.lock().unwrap().is_pinned(&key);
    let mut entries = Vec::new();
    let mut acts = Vec::new();
    let mut add = |e: menu::Entry, d: Option<Do>| {
        entries.push(e);
        acts.push(d);
    };
    if let Some(a) = &running {
        for w in apps::windows_of(a.pid) {
            add(menu::Entry::Item { text: menu_title(&w.title), checked: w.front, enabled: true }, Some(Do::Focus(w.hwnd, w.pid)));
        }
        add(menu::Entry::Separator, None);
        add(menu::item("New window"), Some(Do::NewWindow));
    } else {
        add(menu::item("Open"), Some(Do::Open));
    }
    add(menu::item(if pinned { "Unpin from Sill" } else { "Pin to Sill" }), Some(if pinned { Do::Unpin } else { Do::Pin }));
    if let Some(a) = &running {
        add(menu::Entry::Separator, None);
        add(menu::item(if a.windows.len() > 1 { "Quit (close every window)" } else { "Quit" }), Some(Do::Quit(a.pid)));
    }
    let picked = popup(&app, &window, &entries, x, y).and_then(|i| acts.into_iter().nth(i).flatten());
    match picked {
        Some(Do::Focus(hwnd, pid)) => apps::focus(hwnd, pid),
        Some(Do::NewWindow) => apps::launch(&key),
        Some(Do::Open) => {
            apps::restore_focus();
            activate_at(&app, &window, &key, x, y).map(|_| ())
        }
        Some(Do::Pin) => {
            let a = running.ok_or("that app has gone")?;
            app.state::<PinIcons>().0.lock().unwrap().insert(a.key.clone(), a.icon.clone());
            change_pins(&app, |pins| pins.push(Pin { key: a.key.clone(), name: a.name.clone(), launch: a.launch.clone() }));
            apps::restore_focus();
            Ok(())
        }
        Some(Do::Unpin) => {
            change_pins(&app, |pins| pins.retain(|p| p.key != key));
            apps::restore_focus();
            Ok(())
        }
        Some(Do::Quit(pid)) => apps::quit(pid).map(|_| ()),
        None => {
            apps::restore_focus();
            Ok(())
        }
    }
}

fn change_pins(app: &AppHandle, f: impl FnOnce(&mut Vec<Pin>)) {
    let state = app.state::<SettingsState>();
    let s = {
        let mut s = state.0.lock().unwrap();
        f(&mut s.pins);
        s.clone()
    };
    let _ = s.save();
    emit_items(app);
}

/// A right click on the dock itself, between the apps.
#[tauri::command]
fn dock_menu(app: AppHandle, window: WebviewWindow, x: f64, y: f64) {
    let float = app.state::<SettingsState>().0.lock().unwrap().float;
    let entries = [
        menu::item("Sill Settings…"),
        menu::Entry::Separator,
        menu::Entry::Item { text: "Float and auto-hide".into(), checked: float, enabled: true },
        menu::Entry::Separator,
        menu::item("Quit Sill"),
    ];
    match popup(&app, &window, &entries, x, y) {
        Some(0) => show_settings(&app),
        Some(2) => {
            apps::restore_focus();
            let s = Settings { float: !float, ..app.state::<SettingsState>().0.lock().unwrap().clone() };
            let _ = save_settings(app.clone(), app.state::<SettingsState>(), s);
        }
        Some(4) => quit(app.clone()),
        _ => apps::restore_focus(),
    }
}

/// Pin files to the dock (dropped on it, or picked in the settings), after the apps there,
/// each once. Their icons follow.
fn pin_files(app: &AppHandle, paths: &[std::path::PathBuf]) {
    let new: Vec<Pin> = paths.iter().filter_map(|p| pins::from_path(p)).collect();
    if new.is_empty() {
        return;
    }
    change_pins(app, |pins| {
        for p in &new {
            if !pins.iter().any(|q| q.key == p.key) {
                pins.push(p.clone());
            }
        }
    });
    load_pin_icons(app, new);
}

#[tauri::command]
fn pin_paths(app: AppHandle, paths: Vec<String>) {
    pin_files(&app, &paths.into_iter().map(Into::into).collect::<Vec<_>>());
}

#[tauri::command]
fn unpin(app: AppHandle, key: String) {
    change_pins(&app, |pins| pins.retain(|p| p.key != key));
}

/// An app in the Start menu, for the settings window's list to add from.
#[derive(Serialize)]
struct StartApp {
    key: String,
    name: String,
    launch: String,
    icon: Option<String>,
}

/// The Start menu's apps with their icons: there can be hundreds, so off the main thread.
#[tauri::command]
async fn start_menu_apps() -> Vec<StartApp> {
    tauri::async_runtime::spawn_blocking(|| {
        com();
        pins::start_menu()
            .into_iter()
            .map(|p| {
                let icon = pins::icon(&p);
                StartApp { key: p.key, name: p.name, launch: p.launch, icon }
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// The pinned apps dragged into a new order.
#[tauri::command]
fn reorder(app: AppHandle, keys: Vec<String>) {
    change_pins(&app, |pins| pins.sort_by_key(|p| keys.iter().position(|k| *k == p.key).unwrap_or(usize::MAX)));
}

/// The first button: Fanlight's Apps grid, or closed again if it's open. Under Mullion, as
/// Mullion's; on its own, a Fanlight Sill starts runs on its own too.
#[tauri::command]
fn open_grid() -> Result<(), String> {
    let exe = fanlight().ok_or("Fanlight isn’t installed")?;
    let ask: &[&str] = if suite::is_run_by_mullion() { suite::args_for(Ask::Toggle) } else { &["--toggle"] };
    std::process::Command::new(exe).args(ask).arg(suite::APPS_GRID).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// A click on one of the tray's icons: "left", "right" or "double". `x` and `y` are where a menu
/// opens from, on the page, should its program be out of reach.
#[tauri::command]
fn tray_click(app: AppHandle, window: WebviewWindow, id: String, kind: String, x: f64, y: f64) -> Result<(), String> {
    click_icon(&app, &window, &id, &kind, x, y)
}

fn click_icon(app: &AppHandle, window: &WebviewWindow, id: &str, kind: &str, x: f64, y: f64) -> Result<(), String> {
    if let Some(pid) = host::pid(id).filter(|&pid| mullion_core::ffi::above_us(pid)) {
        let path = mullion_core::ffi::image_path(pid).ok_or("its program has gone")?;
        return out_of_reach(app, window, &apps::program_name(&path), &path, x, y).map(|_| ());
    }
    host::click(id, kind)
}

/// The "more" button: the icons Windows keeps in its flyout, as a menu; picking one clicks it.
#[tauri::command]
fn tray_more(app: AppHandle, window: WebviewWindow, tray: State<Tray>, x: f64, y: f64) -> Result<(), String> {
    let hidden: Vec<Shown> = tray.0.lock().unwrap().iter().filter(|i| !i.promoted).cloned().collect();
    let entries: Vec<menu::Entry> = hidden.iter().map(|i| menu::item(if i.icon.tip.is_empty() { i.icon.exe.clone() } else { i.icon.tip.clone() })).collect();
    match popup(&app, &window, &entries, x, y) {
        Some(i) => click_icon(&app, &window, &hidden[i].icon.id, "left", x, y),
        None => {
            apps::restore_focus();
            Ok(())
        }
    }
}

/// The page's shelf is this big (CSS pixels): floating, the window is just that.
#[tauri::command]
fn shelf(window: WebviewWindow, width: f64, height: f64) {
    let k = window.scale_factor().unwrap_or(1.0);
    dock::shelf((width * k).ceil() as i32, (height * k).ceil() as i32);
}

/// Where the wallpaper behind the dock is, for the glass look; the picture itself comes from
/// `wallpaper_image`, and only when its `key` changes.
#[tauri::command]
fn wallpaper(window: WebviewWindow) -> Option<wallpaper::Wallpaper> {
    wallpaper::current(dock::rect().filter(|_| window.label() == "dock")?).map(|(w, _)| w)
}

#[tauri::command]
fn wallpaper_image(image: State<wallpaper::Image>) -> Option<String> {
    let (w, path) = wallpaper::current(dock::rect()?)?;
    image.get(&w.key, path.as_deref())
}

// --- the settings window's commands ----------------------------------------------------------

/// The edge, size, floating and the look; each works at once.
#[tauri::command]
fn save_settings(app: AppHandle, state: State<SettingsState>, settings: Settings) -> Result<Settings, String> {
    let (previous, settings) = {
        let mut s = state.0.lock().unwrap();
        // The page doesn't edit the pins; the dock does.
        let next = Settings { pins: s.pins.clone(), ..settings }.clamp().keep_look(&s);
        (std::mem::replace(&mut *s, next.clone()), next)
    };
    settings.save().map_err(|e| e.to_string())?;
    if previous.screen != settings.screen {
        // Off the old screen: its strip and its work area go back as they were.
        dock::unregister();
        taskbar::give_back_work_area();
        ffi::set_screen(&settings.screen);
        dock::apply(layout(&settings));
    } else if layout(&previous) != layout(&settings) {
        if settings.float && !previous.float {
            dock::apply(layout(&settings));
            taskbar::restore();
        } else {
            dock::apply(layout(&settings));
            if !settings.float {
                taskbar::hide();
            }
        }
    }
    let shown = settings.shown();
    menu::theme(&shown.theme);
    let _ = app.emit("settings", &shown);
    Ok(shown)
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
/// Fanlight when it isn't installed.
#[tauri::command]
fn get_apps() -> Result<(), String> {
    mullion_core::ffi::shell_open(RELEASES, None, "open")
}

#[tauri::command]
fn quit(app: AppHandle) {
    clean_up();
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picker_opens_off_the_dock_and_stays_on_screen() {
        let work = ffi::RECT { left: 0, top: 30, right: 2560, bottom: 1014 };
        let at = |x, y| ffi::POINT { x, y };
        // Above an app on the bottom, centred on it.
        assert_eq!(picker_spot(at(1280, 1010), "bottom", 300, 120, work), (1130, 890));
        // Beside it on a side, centred on it.
        assert_eq!(picker_spot(at(70, 500), "left", 300, 120, work), (70, 440));
        assert_eq!(picker_spot(at(2490, 500), "right", 300, 120, work), (2190, 440));
        // An app near a corner: kept inside the work area.
        assert_eq!(picker_spot(at(40, 1010), "bottom", 300, 120, work), (0, 890));
        assert_eq!(picker_spot(at(70, 40), "left", 300, 120, work), (70, 30));
    }

    #[test]
    fn this_pcs_screens_list_the_main_one_first() {
        let all = screen_choices();
        assert!(!all.is_empty() && all[0].id.is_empty() && all[0].name.ends_with("(main)"));
        assert!(all[1..].iter().all(|c| c.id.starts_with(r"\\.\") && c.name.starts_with("Screen ")));
    }
}
