//! Mullion as a suite: the apps it has, where each one's exe is, and how Mullion.exe (the hub in
//! apps/mullion) talks to them. It starts each app with `--mullion`, and from then on the app
//! leaves its tray icon and sign-in entry to Mullion. Later asks (`--show`, `--settings`,
//! `--quit`) reach the running app the way a second start always does: the single-instance plugin
//! hands the new process's arguments to the one already running, and the new one exits.
//!
//! An app started any other way runs on its own, exactly as it always has.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct App {
    pub name: &'static str,
    /// The exe's file name, without .exe: the crate's name.
    pub exe: &'static str,
    pub about: &'static str,
    /// Whether it has a window `--show` opens, and settings `--settings` opens. Lintel's bar is
    /// always on screen and its settings are a menu of the bar's.
    pub shows: bool,
}

pub const APPS: &[App] = &[
    App { name: "Oriel", exe: "oriel", about: "A command centre for your second screen", shows: true },
    App { name: "Lintel", exe: "lintel", about: "A menu bar along the top of the screen", shows: false },
    App { name: "Fanlight", exe: "fanlight", about: "Search and launch on Alt+Space", shows: true },
];

/// Mullion started this app: Mullion has the tray icon and the sign-in entry.
pub const RUN_BY_MULLION: &str = "--mullion";

/// What the process that started (or reached) an app asks of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    /// Nothing in particular: what starting it again has always done.
    Nothing,
    Show,
    Settings,
    Quit,
}

pub fn ask(args: &[String]) -> Ask {
    args.iter()
        .find_map(|a| match a.as_str() {
            "--show" => Some(Ask::Show),
            "--settings" => Some(Ask::Settings),
            "--quit" => Some(Ask::Quit),
            _ => None,
        })
        .unwrap_or(Ask::Nothing)
}

pub fn args_for(ask: Ask) -> &'static [&'static str] {
    match ask {
        Ask::Nothing => &[RUN_BY_MULLION],
        Ask::Show => &[RUN_BY_MULLION, "--show"],
        Ask::Settings => &[RUN_BY_MULLION, "--settings"],
        Ask::Quit => &[RUN_BY_MULLION, "--quit"],
    }
}

static RUN_BY: AtomicBool = AtomicBool::new(false);

/// Whether these arguments, the app's own or those a later start handed it, came from Mullion.
/// If they did, the app is Mullion's from then until it quits.
pub fn from_mullion(args: &[String]) -> bool {
    let yes = args.iter().any(|a| a == RUN_BY_MULLION);
    if yes {
        RUN_BY.store(true, Ordering::Relaxed);
    }
    yes
}

/// Whether Mullion has started or reached this app since it started.
pub fn is_run_by_mullion() -> bool {
    RUN_BY.load(Ordering::Relaxed)
}

const UNINSTALL: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";

/// Where an app's exe is: beside this exe (Mullion's folder, or target\debug while building),
/// then in Mullion's folder as its installer recorded it, then where the app's own installer
/// put it.
pub fn find_app(app: &App) -> Option<PathBuf> {
    let file = format!("{}.exe", app.exe);
    let here = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(&file)));
    let mullion = crate::ffi::reg_user_string(&format!(r"{UNINSTALL}\Mullion"), "InstallLocation").map(|d| Path::new(d.trim_matches('"')).join(&file));
    let own = crate::ffi::reg_user_string(&format!(r"{UNINSTALL}\{}", app.name), "DisplayIcon").and_then(|icon| exe_from_icon(&icon));
    [here, mullion, own].into_iter().flatten().find(|p| p.is_file())
}

/// The exe in an Installed apps entry's icon, `"C:\…\app.exe",0` or plain `C:\…\app.exe`.
fn exe_from_icon(icon: &str) -> Option<PathBuf> {
    let path = icon.trim().trim_matches('"').split(',').next()?.trim().trim_matches('"');
    path.to_lowercase().ends_with(".exe").then(|| PathBuf::from(path))
}

/// The processes running from this exe (this user's; another's can't be read).
pub fn running(exe: &Path) -> Vec<u32> {
    let want = exe.to_string_lossy().to_lowercase();
    pids().into_iter().filter(|&pid| crate::ffi::image_path(pid).is_some_and(|p| p.to_lowercase() == want)).collect()
}

fn pids() -> Vec<u32> {
    let mut buf = vec![0u32; 1024];
    loop {
        let mut needed = 0u32;
        let size = (buf.len() * 4) as u32;
        if unsafe { crate::ffi::K32EnumProcesses(buf.as_mut_ptr(), size, &mut needed) } == 0 {
            return Vec::new();
        }
        // A full buffer may have cut the list short.
        if needed < size {
            buf.truncate(needed as usize / 4);
            return buf;
        }
        buf.resize(buf.len() * 2, 0);
    }
}

/// Start an app, or reach the running one, with what's asked of it. Doesn't wait.
pub fn send(exe: &Path, ask: Ask) -> std::io::Result<()> {
    std::process::Command::new(exe).args(args_for(ask)).spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn asks_come_from_the_arguments() {
        assert_eq!(ask(&args(&["app.exe"])), Ask::Nothing);
        assert_eq!(ask(&args(&["app.exe", "--mullion", "--settings"])), Ask::Settings);
        assert_eq!(ask(&args(&["app.exe", "--quit"])), Ask::Quit);
        for a in [Ask::Nothing, Ask::Show, Ask::Settings, Ask::Quit] {
            let sent = args(&[&["app.exe"], args_for(a)].concat());
            assert_eq!(ask(&sent), a);
            assert!(sent.iter().any(|s| s == RUN_BY_MULLION));
        }
    }

    #[test]
    fn installed_apps_icons_name_their_exe() {
        assert_eq!(exe_from_icon(r#""C:\Users\a\AppData\Local\Oriel\oriel.exe",0"#), Some(PathBuf::from(r"C:\Users\a\AppData\Local\Oriel\oriel.exe")));
        assert_eq!(exe_from_icon(r"C:\Apps\Lintel\lintel.exe"), Some(PathBuf::from(r"C:\Apps\Lintel\lintel.exe")));
        assert_eq!(exe_from_icon(r"C:\Apps\Lintel\lintel.ico"), None);
    }

    #[test]
    fn finds_this_process_running() {
        let me = std::env::current_exe().unwrap();
        assert!(running(&me).contains(&std::process::id()));
        assert!(running(Path::new(r"C:\nowhere\nothing.exe")).is_empty());
    }

    #[test]
    fn every_app_has_its_own_exe() {
        for (i, a) in APPS.iter().enumerate() {
            assert_eq!(a.exe, a.name.to_lowercase());
            assert!(APPS[i + 1..].iter().all(|b| b.exe != a.exe));
        }
    }
}
