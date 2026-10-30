//! What a corner can do. The suite's apps are started by their exe, as Mullion does, with
//! `--toggle`: if Bay's overview or Fanlight's panel is already open, pushing the corner again
//! closes it. The desktop and Task View are Windows' own shortcuts, which close themselves the
//! same way.

use mullion_core::ffi::{send_keys, VK_LWIN, VK_TAB};
use mullion_core::suite::{self, Ask};
use serde::Serialize;

#[derive(Serialize, Clone, Copy, Debug)]
pub struct Action {
    pub id: &'static str,
    pub name: &'static str,
    /// The suite's app it needs, if any: without it, the action is greyed out.
    pub app: Option<&'static str>,
}

pub const NOTHING: &str = "nothing";

pub const ACTIONS: &[Action] = &[
    Action { id: NOTHING, name: "Nothing", app: None },
    Action { id: "bay", name: "Every window (Bay)", app: Some("Bay") },
    Action { id: "fanlight", name: "Search (Fanlight)", app: Some("Fanlight") },
    Action { id: "apps", name: "Apps (Fanlight)", app: Some("Fanlight") },
    Action { id: "desktop", name: "Desktop", app: None },
    Action { id: "task-view", name: "Task View", app: None },
    Action { id: "lock", name: "Lock", app: None },
    Action { id: "screens-off", name: "Screens off", app: None },
];

pub fn by_id(id: &str) -> Option<&'static Action> {
    ACTIONS.iter().find(|a| a.id == id)
}

/// Whether the app an action needs is installed; true for those that need none.
pub fn available(a: &Action) -> bool {
    a.app.is_none_or(|name| suite::APPS.iter().find(|x| x.name == name).and_then(suite::find_app).is_some())
}

/// Do it. `screens_off` is asked of the pointer's thread, whose window Windows takes it from.
pub fn run(id: &str, screens_off: impl FnOnce()) -> Result<(), String> {
    match id {
        "bay" => return open("Bay", &[]),
        "fanlight" => return open("Fanlight", &[]),
        "apps" => return open("Fanlight", &[suite::APPS_GRID]),
        // Win+D and Win+Tab: pressed again, each puts things back.
        "desktop" => send_keys(&[VK_LWIN, b'D' as u16]),
        "task-view" => send_keys(&[VK_LWIN, VK_TAB]),
        "lock" => return mullion_core::power::act("lock"),
        "screens-off" => screens_off(),
        _ => {}
    }
    Ok(())
}

/// Start one of the suite's apps, or reach the one running, asking it to open or close. Under
/// Mullion, as Mullion's; on its own, an app Mitre starts runs on its own too.
fn open(name: &str, extra: &[&str]) -> Result<(), String> {
    let app = suite::APPS.iter().find(|a| a.name == name).ok_or("no such app")?;
    let exe = suite::find_app(app).ok_or_else(|| format!("{name} isn’t installed"))?;
    let ask: &[&str] = if suite::is_run_by_mullion() { suite::args_for(Ask::Toggle) } else { &["--toggle"] };
    std::process::Command::new(exe).args(ask).args(extra).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_name_real_apps() {
        for a in ACTIONS {
            if let Some(app) = a.app {
                assert!(suite::APPS.iter().any(|x| x.name == app), "{}", a.id);
            }
        }
        assert!(by_id(NOTHING).is_some_and(available));
        assert!(by_id("lock").is_some_and(available));
        assert!(by_id("launch-missiles").is_none());
    }
}
