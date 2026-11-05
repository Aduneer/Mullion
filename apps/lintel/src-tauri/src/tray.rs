//! The notification area's icons on the bar's right, as a Mac's menu bar shows its extras. Lintel
//! doesn't host them: only one program can, and Apron does, in the taskbar's place. Apron passes
//! them on over a pipe (`mullion_core::tray`) and gets the clicks back, which it hands to each
//! icon's program as Explorer would. Without Apron there's nothing here, and Windows' taskbar has
//! them as ever.

use crate::{emit_bars, Anchor, OpenMenu, SettingsState};
use mullion_core::ffi;
use mullion_core::tray::{self as wire, Icon};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

pub struct Tray {
    link: wire::Link,
    icons: Mutex<Vec<Icon>>,
}

impl Tray {
    pub fn icons(&self) -> Vec<Icon> {
        self.icons.lock().unwrap().clone()
    }

    /// Apron is running, and passing its icons on.
    pub fn host(&self) -> bool {
        self.link.connected()
    }

    /// Tell Apron whether the bar shows the icons, so its dock can leave them out.
    pub fn showing(&self, on: bool) {
        self.link.send(&wire::ToHost::Showing(on));
    }

    fn icon(&self, id: &str) -> Result<Icon, String> {
        self.icons.lock().unwrap().iter().find(|i| i.id == id).cloned().ok_or_else(|| "that icon has gone".into())
    }
}

/// Look for Apron, and keep looking: it may start later, or quit and start again.
pub fn start(app: &AppHandle) {
    let link = wire::Link::default();
    app.manage(Tray { link: link.clone(), icons: Mutex::new(Vec::new()) });
    let app = app.clone();
    wire::connect(wire::pipe_name(), "apron.exe", link, move |e| {
        let tray = app.state::<Tray>();
        match e {
            wire::Event::Connected => {
                tray.showing(app.state::<SettingsState>().0.lock().unwrap().tray);
                emit_all(&app, "tray-host", true);
            }
            wire::Event::Heard(wire::FromHost::Icons(icons)) => {
                *tray.icons.lock().unwrap() = icons.clone();
                emit_all(&app, "tray", icons);
            }
            wire::Event::Lost => {
                tray.icons.lock().unwrap().clear();
                emit_all(&app, "tray", Vec::<Icon>::new());
                emit_all(&app, "tray-host", false);
            }
        }
    });
}

fn emit_all<S: serde::Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    emit_bars(app, event, payload.clone());
    let _ = app.emit_to("menu", event, payload);
}

/// Where an item is on the screen, in physical pixels: left, top, right, bottom.
fn on_screen(w: &WebviewWindow, a: Anchor) -> [i32; 4] {
    let pos = w.outer_position().unwrap_or_default();
    let k = w.scale_factor().unwrap_or(1.0);
    let at = |x: f64, y: f64| (pos.x + (x * k).round() as i32, pos.y + (y * k).round() as i32);
    let (l, t) = at(a.left, a.top);
    let (r, b) = at(a.left + a.width, a.top + a.height);
    [l, t, r, b]
}

/// A click on an icon: "left", "right" or "double". `item` is its place on the bar; from the menu
/// of hidden icons, the menu's own item stands in. Some(name) when its program runs as
/// administrator, out of reach: the bar says so instead.
#[tauri::command]
pub fn tray_click(app: AppHandle, window: WebviewWindow, tray: State<Tray>, id: String, kind: String, item: Option<Anchor>) -> Result<Option<String>, String> {
    let icon = tray.icon(&id)?;
    if ffi::above_us(icon.pid) {
        return Ok(Some(program(&icon)));
    }
    let rect = place(&app, &window, item);
    // Lintel had the click, so it's Lintel that may let the program come to the front.
    unsafe { ffi::AllowSetForegroundWindow(icon.pid) };
    if !tray.link.send(&wire::ToHost::Click { id, kind, rect }) {
        return Err("Apron isn’t running".into());
    }
    Ok(None)
}

/// The pointer came onto an icon, or left it: its program may show a tooltip of its own.
#[tauri::command]
pub fn tray_hover(window: WebviewWindow, tray: State<Tray>, id: String, on: bool, item: Option<Anchor>) {
    let m = match (on, item) {
        (true, Some(a)) => wire::ToHost::Hover { id, rect: on_screen(&window, a) },
        (false, _) => wire::ToHost::Leave { id },
        _ => return,
    };
    tray.link.send(&m);
}

/// The program an icon belongs to, by name, for the menu that says it's out of reach.
#[tauri::command]
pub fn tray_program(tray: State<Tray>, id: String) -> Result<String, String> {
    Ok(program(&tray.icon(&id)?))
}

/// Open an icon's program again, as Start would: one that runs once (Task Manager) comes forward.
#[tauri::command]
pub fn tray_open(tray: State<Tray>, id: String) -> Result<(), String> {
    let path = ffi::image_path(tray.icon(&id)?.pid).ok_or("its program has gone")?;
    ffi::shell_open(&path, None, "open")
}

fn program(icon: &Icon) -> String {
    ffi::image_path(icon.pid).map(|p| mullion_core::apps::program_name(&p)).unwrap_or_else(|| icon.exe.trim_end_matches(".exe").to_string())
}

/// The clicked item's place on the screen: its own box on a bar, or for the menu of hidden icons,
/// the bar item that menu hangs from.
fn place(app: &AppHandle, window: &WebviewWindow, item: Option<Anchor>) -> [i32; 4] {
    if window.label() == "menu" {
        let open = app.state::<OpenMenu>().0.lock().unwrap().open.as_ref().map(|(_, a, bar)| (*a, bar.clone()));
        if let Some((a, w)) = open.and_then(|(a, bar)| Some((a, app.get_webview_window(&bar)?))) {
            return on_screen(&w, a);
        }
    }
    item.map(|a| on_screen(window, a)).unwrap_or_default()
}
