//! Files and folders by name, from an index of their own kept in memory: Desktop, Documents,
//! Downloads, Pictures, Music, Videos and any folders added in settings, without hidden and
//! system files, dot-folders, build output and package caches.
//!
//! The first walk reads every folder; after that, a folder whose last-modified time hasn't changed
//! has the same files in it (Windows updates it when a file is added, removed or renamed), so only
//! changed folders are read again and the index can be brought up to date each time the panel
//! opens, in a fraction of the time.

use crate::matcher::Query;
use std::collections::HashMap;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, UNIX_EPOCH};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Music, FOLDERID_Pictures, FOLDERID_Videos, SHGetKnownFolderPath, KF_FLAG_DEFAULT,
};

/// How stale the index may be before opening the panel brings it up to date.
const FRESH: Duration = Duration::from_secs(15);
/// Enough for any home folder; past this the walk stops, rather than eat memory.
const MAX_ENTRIES: usize = 500_000;
const MAX_DEPTH: usize = 20;

const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
/// OneDrive and other cloud files that aren't on this PC: listed, but nothing reads them.
const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x4_0000;
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;

/// Folders that are never worth searching: package caches and tool state.
const SKIP_DIRS: &[&str] = &["node_modules", "__pycache__", "bower_components", "site-packages", "appdata", "$recycle.bin", "system volume information"];

#[derive(Clone, Debug)]
pub struct Child {
    pub name: Box<str>,
    pub dir: bool,
    /// Unix seconds.
    pub modified: u64,
    /// In the cloud only, so not to be read for an icon or thumbnail.
    pub online_only: bool,
}

#[derive(Clone, Debug, Default)]
struct Listing {
    /// The folder's own last-modified time, in 100 ns ticks since 1601.
    stamp: u64,
    /// When it was read, the same way.
    read: u64,
    children: Vec<Child>,
}

/// Windows' clock moves in steps of about 15 ms, so a file added in the same step as a listing was
/// read leaves the folder's time as it was. A listing read this soon after the folder changed is
/// read again next time.
const SETTLED: u64 = 2 * 10_000_000;

fn ticks_now() -> u64 {
    std::time::SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64 / 100) + 11_644_473_600 * 10_000_000
}

#[derive(Default)]
struct Index {
    roots: Vec<PathBuf>,
    /// Every folder walked, with what's in it.
    dirs: Vec<(PathBuf, Arc<Listing>)>,
    entries: usize,
}

/// One match: where it is, and how well it matched.
pub struct Found {
    pub path: PathBuf,
    pub child: Child,
    pub score: i32,
}

#[derive(Clone, Default)]
pub struct Files {
    index: Arc<Mutex<Arc<Index>>>,
    state: Arc<Mutex<(Option<Instant>, bool)>>,
}

impl Files {
    /// Bring the index up to date in the background, unless it's fresh (or `force`), or a walk is
    /// already going.
    pub fn refresh(&self, extra: &[String], force: bool) {
        let mut st = self.state.lock().unwrap();
        if st.1 || (!force && st.0.is_some_and(|t| t.elapsed() < FRESH)) {
            return;
        }
        st.1 = true;
        drop(st);
        let roots = roots(extra);
        let me = self.clone();
        let _ = std::thread::Builder::new().name("files".into()).spawn(move || {
            let old = me.index.lock().unwrap().clone();
            let next = walk(roots, &old);
            *me.index.lock().unwrap() = Arc::new(next);
            *me.state.lock().unwrap() = (Some(Instant::now()), false);
        });
    }

    pub fn clear(&self) {
        *self.index.lock().unwrap() = Arc::default();
        *self.state.lock().unwrap() = (None, false);
    }

    /// How many files and folders are in the index, for settings.
    pub fn count(&self) -> usize {
        self.index.lock().unwrap().entries
    }

    /// The best `limit` matches, best first. The folders are shared out among the CPU's cores.
    pub fn search(&self, q: &Query, limit: usize) -> Vec<Found> {
        let index = self.index.lock().unwrap().clone();
        let mut found: Vec<Found> = index
            .roots
            .iter()
            .filter_map(|r| {
                let name = r.file_name()?.to_string_lossy();
                let score = q.score(&name)? + 30;
                let child = Child { name: name.into(), dir: true, modified: 0, online_only: false };
                Some(Found { path: r.clone(), child, score })
            })
            .collect();
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
        let chunk = index.dirs.len().div_ceil(threads).max(1);
        let now = std::time::SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        std::thread::scope(|s| {
            let parts: Vec<_> = index
                .dirs
                .chunks(chunk)
                .map(|dirs| {
                    s.spawn(move || {
                        let mut best: Vec<(i32, &Path, &Child)> = Vec::new();
                        for (dir, listing) in dirs {
                            for c in &listing.children {
                                if let Some(score) = q.score(&c.name) {
                                    best.push((score + bonus(c, dir, now), dir, c));
                                }
                            }
                            if best.len() > limit * 8 {
                                keep_best(&mut best, limit);
                            }
                        }
                        keep_best(&mut best, limit);
                        best.into_iter().map(|(score, dir, c)| Found { path: dir.join(&*c.name), child: c.clone(), score }).collect::<Vec<_>>()
                    })
                })
                .collect();
            for p in parts {
                found.extend(p.join().unwrap_or_default());
            }
        });
        found.sort_by_key(|f| -f.score);
        found.truncate(limit);
        found
    }
}

fn keep_best(best: &mut Vec<(i32, &Path, &Child)>, limit: usize) {
    best.sort_by_key(|(s, _, _)| -s);
    best.truncate(limit);
}

/// A little for files changed lately, and a little against ones buried deep.
fn bonus(c: &Child, dir: &Path, now: u64) -> i32 {
    let age_days = now.saturating_sub(c.modified) / 86400;
    let recent = match age_days {
        0..=1 => 40,
        2..=7 => 25,
        8..=30 => 10,
        _ => 0,
    };
    recent - (dir.components().count() as i32 - 4).clamp(0, 10) * 3
}

/// The folders searched: the user's own (wherever they've been moved to, OneDrive included) and
/// those added in settings, without any that lie inside another.
fn roots(extra: &[String]) -> Vec<PathBuf> {
    let mut all: Vec<PathBuf> = [FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Pictures, FOLDERID_Music, FOLDERID_Videos]
        .iter()
        .filter_map(|id| unsafe {
            let p = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
            let s = p.to_string().ok();
            CoTaskMemFree(Some(p.0 as _));
            s.map(PathBuf::from)
        })
        .chain(extra.iter().map(PathBuf::from))
        .filter(|p| p.is_dir())
        .collect();
    nest(&mut all);
    all
}

/// Drop folders that are inside another one in the list (or the same), keeping the order.
fn nest(all: &mut Vec<PathBuf>) {
    let key = |p: &Path| p.to_string_lossy().to_lowercase().trim_end_matches('\\').to_string() + "\\";
    let keys: Vec<String> = all.iter().map(|p| key(p)).collect();
    let mut keep = Vec::new();
    for (i, k) in keys.iter().enumerate() {
        let inside = keys.iter().enumerate().any(|(j, o)| j != i && k.starts_with(o.as_str()) && (k != o || j < i));
        keep.push(!inside);
    }
    let mut it = keep.into_iter();
    all.retain(|_| it.next().unwrap_or(true));
}

fn stamp(meta: &std::fs::Metadata) -> u64 {
    meta.last_write_time()
}

fn walk(roots: Vec<PathBuf>, old: &Index) -> Index {
    let previous: HashMap<&Path, &Arc<Listing>> = old.dirs.iter().map(|(p, l)| (p.as_path(), l)).collect();
    let mut dirs = Vec::new();
    let mut entries = 0;
    for root in &roots {
        let mut stack = vec![(root.clone(), 0usize)];
        while let Some((dir, depth)) = stack.pop() {
            let Ok(meta) = std::fs::metadata(&dir) else { continue };
            let st = stamp(&meta);
            let listing = match previous.get(dir.as_path()) {
                Some(l) if l.stamp == st && l.read > st + SETTLED => Arc::clone(l),
                _ => Arc::new(Listing { stamp: st, read: ticks_now(), children: list(&dir) }),
            };
            // Build output (Cargo's target, and others that mark themselves) isn't searched inside;
            // the folder itself is, from its parent.
            if listing.children.iter().any(|c| !c.dir && c.name.eq_ignore_ascii_case("CACHEDIR.TAG")) {
                continue;
            }
            entries += listing.children.len();
            if depth < MAX_DEPTH {
                for c in listing.children.iter().filter(|c| c.dir && descend(&c.name)) {
                    stack.push((dir.join(&*c.name), depth + 1));
                }
            }
            dirs.push((dir, listing));
            if entries > MAX_ENTRIES {
                return Index { roots, dirs, entries };
            }
        }
    }
    Index { roots, dirs, entries }
}

/// What's in one folder, less what isn't searched.
fn list(dir: &Path) -> Vec<Child> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    rd.filter_map(|e| {
        let e = e.ok()?;
        // On Windows this comes with the listing, so it costs nothing extra.
        let meta = e.metadata().ok()?;
        let attrs = meta.file_attributes();
        if attrs & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0 {
            return None;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name.starts_with('~') {
            return None;
        }
        // Links and junctions are listed but not followed, so nothing is walked twice or forever.
        let dir = meta.is_dir() && attrs & FILE_ATTRIBUTE_REPARSE_POINT == 0;
        let ticks = meta.last_write_time();
        Some(Child {
            name: name.into(),
            dir,
            // 100 ns ticks since 1601 to Unix seconds.
            modified: (ticks / 10_000_000).saturating_sub(11_644_473_600),
            online_only: attrs & (FILE_ATTRIBUTE_OFFLINE | FILE_ATTRIBUTE_RECALL_ON_OPEN | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS) != 0,
        })
    })
    .collect()
}

fn descend(name: &str) -> bool {
    let n = name.to_lowercase();
    !SKIP_DIRS.contains(&n.as_str()) && !n.starts_with('$')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_folders_are_searched_once() {
        let mut all = vec![PathBuf::from(r"C:\Users\a\Documents"), PathBuf::from(r"C:\Users\a\Documents\Work"), PathBuf::from(r"D:\Photos"), PathBuf::from(r"c:\users\a\documents\")];
        nest(&mut all);
        assert_eq!(all, [PathBuf::from(r"C:\Users\a\Documents"), PathBuf::from(r"D:\Photos")]);
        // A folder whose name starts like another's isn't inside it.
        let mut all = vec![PathBuf::from(r"D:\Work"), PathBuf::from(r"D:\Workshop")];
        nest(&mut all);
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn skips_caches() {
        assert!(descend("Projects"));
        assert!(!descend("node_modules"));
        assert!(!descend("AppData"));
        assert!(!descend("$RECYCLE.BIN"));
    }

    #[test]
    fn indexes_finds_and_updates() {
        let root = std::env::temp_dir().join(format!("fanlight-files-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("Quarterly Reports")).unwrap();
        std::fs::create_dir_all(root.join("node_modules").join("left-pad")).unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::write(root.join("target").join("CACHEDIR.TAG"), "Signature: 8a477f597d28d172789f06886806bc55").unwrap();
        std::fs::write(root.join("target").join("build-report.txt"), "").unwrap();
        std::fs::write(root.join("Quarterly Reports").join("Budget 2026.xlsx"), "x").unwrap();
        std::fs::write(root.join(".secret-report"), "").unwrap();

        let files = Files::default();
        let first = walk(vec![root.clone()], &Index::default());
        *files.index.lock().unwrap() = Arc::new(first);
        let names = |q: &str| files.search(&Query::new(q), 10).into_iter().map(|f| f.child.name.to_string()).collect::<Vec<_>>();
        assert_eq!(names("budget"), ["Budget 2026.xlsx"]);
        assert_eq!(names("qr"), ["Quarterly Reports"]);
        // Not in hidden, dot, cache or build folders.
        assert!(names("left").is_empty());
        assert!(names("secret").is_empty());
        assert!(names("build report").is_empty());

        // A new file: its folder's time changes, so the next walk reads that folder again.
        std::fs::write(root.join("Quarterly Reports").join("Forecast.docx"), "").unwrap();
        let old = files.index.lock().unwrap().clone();
        *files.index.lock().unwrap() = Arc::new(walk(vec![root.clone()], &old));
        assert_eq!(names("forecast"), ["Forecast.docx"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn finds_the_users_folders() {
        assert!(!roots(&[]).is_empty());
    }
}
