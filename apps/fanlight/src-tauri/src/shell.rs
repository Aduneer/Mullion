//! The shell's side of things, on a thread of its own with COM: the apps Start lists under All
//! apps (desktop programs and Store apps alike, from the shell's Applications folder), and the
//! icons for results, made once and kept.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::{
    FOLDERID_AppsFolder, IEnumShellItems, IShellItem, SHGetKnownFolderItem, BHID_EnumItems, KF_FLAG_DEFAULT, SIGDN, SIGDN_NORMALDISPLAY, SIGDN_PARENTRELATIVEPARSING,
};

/// The app list is read again when the panel opens, at most this often, to catch new installs.
const APPS_FRESH: Duration = Duration::from_secs(60);

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct App {
    /// What the shell calls it: a Store app's id, or a program's path (often under a known folder's
    /// GUID). `shell:AppsFolder\` and this opens it.
    pub id: String,
    pub name: String,
}

enum Job {
    Apps,
    Icon(String, Sender<Option<String>>),
}

/// The app list, and when it was read.
type AppList = Arc<Mutex<(Arc<Vec<App>>, Option<Instant>)>>;

#[derive(Clone)]
pub struct Shell {
    jobs: Sender<Job>,
    apps: AppList,
}

impl Shell {
    pub fn start() -> Self {
        let (jobs, rx) = channel::<Job>();
        let apps: AppList = Arc::default();
        let list = apps.clone();
        std::thread::Builder::new()
            .name("shell".into())
            .spawn(move || {
                unsafe {
                    let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                }
                let mut icons: HashMap<String, Option<String>> = HashMap::new();
                while let Ok(job) = rx.recv() {
                    match job {
                        Job::Apps => {
                            let found = read_apps();
                            // An empty list is the shell not being ready yet (just after sign-in): keep the old one.
                            let mut l = list.lock().unwrap();
                            if !found.is_empty() || l.0.is_empty() {
                                *l = (Arc::new(found), Some(Instant::now()));
                            }
                        }
                        // The preview's large images are made each time: they're big, and one at a time.
                        Job::Icon(key, reply) if key.starts_with("big:") => {
                            let _ = reply.send(make_icon(&key));
                        }
                        Job::Icon(key, reply) => {
                            let cache = cache_key(&key);
                            let url = match icons.get(&cache) {
                                Some(u) => u.clone(),
                                None => {
                                    let u = make_icon(&key);
                                    icons.insert(cache, u.clone());
                                    u
                                }
                            };
                            let _ = reply.send(url);
                        }
                    }
                }
            })
            .expect("failed to start the shell thread");
        let shell = Shell { jobs, apps };
        shell.refresh_apps(true);
        shell
    }

    /// Read the app list again, unless it was read lately (or `now`).
    pub fn refresh_apps(&self, now: bool) {
        let stale = self.apps.lock().unwrap().1.is_none_or(|at| at.elapsed() > APPS_FRESH);
        if now || stale {
            let _ = self.jobs.send(Job::Apps);
        }
    }

    pub fn apps(&self) -> Arc<Vec<App>> {
        self.apps.lock().unwrap().0.clone()
    }

    /// The image for a result's icon key (see `make_icon`), as a data: URL. Waits for the shell
    /// thread, so it's only called from async commands.
    pub fn icon(&self, key: &str) -> Option<String> {
        let (tx, rx) = channel();
        self.jobs.send(Job::Icon(key.to_string(), tx)).ok()?;
        rx.recv_timeout(Duration::from_secs(5)).ok().flatten()
    }
}

/// Files of a kind share one icon, so it's made once per extension; programs, shortcuts and the
/// like have their own, and so do folders (Downloads, Pictures …) and thumbnails.
fn cache_key(key: &str) -> String {
    if let Some(path) = key.strip_prefix("file:") {
        let ext = std::path::Path::new(path).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !OWN_ICONS.contains(&ext.as_str()) {
            return format!("ext:{ext}");
        }
    }
    key.to_string()
}

/// Files that carry an icon of their own rather than their type's.
const OWN_ICONS: &[&str] = &["exe", "lnk", "ico", "url", "appref-ms", "msc", "cpl", "scr", "website"];

/// The preview's icons, drawn this many pixels square, and its pictures.
const BIG_ICON: i32 = 256;
const BIG_THUMB: i32 = 768;

/// `app:<id>` an app's icon, `file:<path>` and `folder:<path>` a file's or folder's,
/// `thumb:<path>` a picture's thumbnail (or its icon, if it has none). `big:` before any of them,
/// the same for the preview, larger.
fn make_icon(key: &str) -> Option<String> {
    let (big, key) = key.strip_prefix("big:").map_or((false, key), |k| (true, k));
    let (icon, thumb) = if big { (BIG_ICON, BIG_THUMB) } else { (crate::icons::SIZE_PX, crate::icons::SIZE_PX) };
    let (kind, rest) = key.split_once(':')?;
    match kind {
        "app" => crate::icons::icon(&format!(r"shell:AppsFolder\{rest}"), icon),
        "file" | "folder" => crate::icons::icon(rest, icon),
        "thumb" => crate::icons::thumbnail(rest, thumb).or_else(|| crate::icons::icon(rest, icon)),
        _ => None,
    }
}

fn read_apps() -> Vec<App> {
    unsafe {
        let Ok(folder) = SHGetKnownFolderItem::<IShellItem>(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None) else { return Vec::new() };
        let Ok(items) = folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems) else { return Vec::new() };
        let mut apps: Vec<App> = Vec::new();
        loop {
            let mut one = [None];
            let mut fetched = 0u32;
            if items.Next(&mut one, Some(&mut fetched)).is_err() || fetched == 0 {
                break;
            }
            let Some(item) = one[0].take() else { break };
            let (Some(name), Some(id)) = (display_name(&item, SIGDN_NORMALDISPLAY), display_name(&item, SIGDN_PARENTRELATIVEPARSING)) else { continue };
            if keep(&name, &id) && !apps.iter().any(|a| a.id == id) {
                apps.push(App { id, name });
            }
        }
        apps
    }
}

unsafe fn display_name(item: &IShellItem, how: SIGDN) -> Option<String> {
    let p = item.GetDisplayName(how).ok()?;
    let s = p.to_string().ok();
    CoTaskMemFree(Some(p.0 as _));
    s.filter(|s| !s.trim().is_empty())
}

/// Start also lists web links, help files and uninstallers; only apps belong in the results.
fn keep(name: &str, id: &str) -> bool {
    let lower = id.to_lowercase();
    let n = name.to_lowercase();
    !(lower.starts_with("http://")
        || lower.starts_with("https://")
        || [".url", ".chm", ".htm", ".html", ".pdf", ".txt", ".rtf"].iter().any(|e| lower.ends_with(e))
        || n.starts_with("uninstall")
        || n.contains("uninstaller"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_apps_not_links() {
        assert!(keep("Notepad", "Microsoft.WindowsNotepad_8wekyb3d8bbwe!App"));
        assert!(keep("Visual Studio Code", r"{6D809377-6AF0-444B-8957-A3773F02200E}\Microsoft VS Code\Code.exe"));
        assert!(!keep("Website", "https://example.com/"));
        assert!(!keep("Uninstall Foo", r"C:\Foo\unins000.exe"));
        assert!(!keep("Foo Help", r"C:\Foo\help.chm"));
    }

    #[test]
    fn files_share_their_types_icon() {
        assert_eq!(cache_key(r"file:C:\a\report.PDF"), "ext:pdf");
        assert_eq!(cache_key(r"file:C:\b\other.pdf"), "ext:pdf");
        assert_eq!(cache_key(r"file:C:\b\setup.exe"), r"file:C:\b\setup.exe");
        assert_eq!(cache_key(r"folder:C:\b"), r"folder:C:\b");
        assert_eq!(cache_key("app:Microsoft.WindowsNotepad_8wekyb3d8bbwe!App"), "app:Microsoft.WindowsNotepad_8wekyb3d8bbwe!App");
    }

    #[test]
    fn reads_starts_apps() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        let apps = read_apps();
        // Every Windows has some: File Explorer, Settings, Notepad or the like.
        assert!(apps.len() > 5, "{} apps", apps.len());
        assert!(apps.iter().all(|a| !a.name.is_empty() && !a.id.is_empty()));
    }
}
