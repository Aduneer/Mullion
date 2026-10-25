//! One search: everything that matches what was typed, from every source, scored on one scale so
//! the best of them all can be the Top Hit, and the rest grouped as Spotlight groups them.

use crate::calc;
use crate::catalog::{self, COMMANDS, PAGES};
use crate::clipboard::Clip;
use crate::files::Files;
use crate::history::History;
use crate::matcher::Query;
use crate::settings::Settings;
use crate::shell::App;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Hit {
    /// What it is and which, for opening it and for the history: "app:<id>", "file:<path>",
    /// "folder:<path>", "page:<uri>", "cmd:<id>", "win:<hwnd>:<pid>", "clip:<id>", "calc:<value>",
    /// "url:<url>", "web:<query>".
    pub key: String,
    /// The section it goes in, and which glyph stands in while its icon loads.
    pub kind: &'static str,
    pub title: String,
    pub subtitle: String,
    /// An icon key for the `icon` command (see `shell::make_icon`), if it has an icon of its own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip)]
    pub score: i32,
    /// Asks for a second Enter first (Restart, Shut down …).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub confirm: bool,
}

#[derive(Serialize, Debug, Default)]
pub struct Section {
    pub title: &'static str,
    pub hits: Vec<Hit>,
}

#[derive(Serialize, Debug, Default)]
pub struct Results {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top: Option<Hit>,
    pub sections: Vec<Section>,
}

/// What a search looks through.
pub struct Sources<'a> {
    pub settings: &'a Settings,
    pub apps: &'a [App],
    pub files: &'a Files,
    pub windows: &'a [crate::wins::Win],
    pub clips: &'a [Clip],
    pub history: &'a History,
    pub comma_decimal: bool,
    pub home: &'a str,
    pub now: u64,
    /// One of `CATEGORIES`: only that, and more of it; and with nothing typed, all of it.
    pub only: Option<&'a str>,
}

/// The buttons in the field (and Ctrl+1–4), and the kinds of result each keeps.
pub const CATEGORIES: &[(&str, &[&str])] =
    &[("apps", &["app", "window"]), ("files", &["file", "folder"]), ("actions", &["command", "page"]), ("clipboard", &["clip"])];
/// How many of a kind a category shows; with nothing typed, it shows them all (every app, for
/// the grid).
const MORE: usize = 60;

/// The sections, in the order they're shown, each with how many it shows and how much a match in
/// it counts for against the others when choosing the Top Hit.
const KINDS: &[(&str, &str, usize, i32)] = &[
    ("app", "Applications", 6, 100),
    ("command", "Commands", 3, 40),
    ("page", "System Settings", 4, -20),
    ("window", "Windows", 4, -60),
    ("clip", "Clipboard", 4, -140),
    ("folder", "Folders", 3, 10),
    ("file", "Documents", 8, 0),
];

pub const ENGINE_NAMES: &[(&str, &str, &str)] = &[
    ("google", "Google", "https://www.google.com/search?q="),
    ("bing", "Bing", "https://www.bing.com/search?q="),
    ("duckduckgo", "DuckDuckGo", "https://duckduckgo.com/?q="),
    ("brave", "Brave Search", "https://search.brave.com/search?q="),
    ("ecosia", "Ecosia", "https://www.ecosia.org/search?q="),
    ("startpage", "Startpage", "https://www.startpage.com/do/search?q="),
    ("kagi", "Kagi", "https://kagi.com/search?q="),
];

pub fn engine(id: &str) -> (&'static str, &'static str) {
    let (_, name, url) = ENGINE_NAMES.iter().find(|(i, _, _)| *i == id).unwrap_or(&ENGINE_NAMES[0]);
    (name, url)
}

pub fn search(text: &str, src: &Sources) -> Results {
    let text = text.trim();
    let only = src.only.and_then(|o| CATEGORIES.iter().find(|c| c.0 == o)).map(|c| c.1);
    let wants = |kind: &str| only.is_none_or(|kinds| kinds.contains(&kind));
    if text.is_empty() && only.is_none() {
        return Results::default();
    }
    let lower = text.to_lowercase();
    // The clipboard on its own: "cb", "clipboard", or "cb " and words to look for in it.
    if src.settings.clipboard && only.is_none() {
        if let Some(rest) = clipboard_mode(&lower) {
            return clipboard_only(&text[text.len() - rest.len()..], src);
        }
    }
    let q = Query::new(text);
    // One or two letters only match at the start of a name or a word, or as initials, or nearly
    // everything would.
    let floor = if q.whole.chars().count() < 3 { 600 } else { 0 };
    // With nothing typed (in a category), everything in it matches.
    let all = q.is_empty();
    let score = |name: &str| if all { Some(0) } else { q.score(name).filter(|s| *s >= floor) };
    let listed = |name: &str, words: &str| if all { Some(0) } else { catalog::score(&q, name, words).filter(|s| *s >= floor) };
    let mut hits: Vec<Hit> = Vec::new();

    if wants("app") {
        for a in src.apps {
            if let Some(s) = score(&a.name) {
                hits.push(hit(format!("app:{}", a.id), "app", a.name.clone(), "Application".into(), Some(format!("app:{}", a.id)), s));
            }
        }
    }
    if wants("page") {
        for p in PAGES {
            if let Some(s) = listed(p.name, p.words) {
                hits.push(hit(format!("page:{}", p.uri), "page", p.name.into(), "Settings".into(), None, s));
            }
        }
    }
    if wants("command") {
        for c in COMMANDS {
            if (c.id == "clipboard" || c.id == "clear-clipboard") && !src.settings.clipboard {
                continue;
            }
            if let Some(s) = listed(c.name, c.words) {
                hits.push(Hit { confirm: c.confirm, ..hit(format!("cmd:{}", c.id), "command", c.name.into(), c.detail.into(), None, s) });
            }
        }
    }
    if src.settings.windows && wants("window") {
        for w in src.windows {
            let s = if all { Some(0) } else { [score(&w.title), score(&w.app).map(|s| s - 50)].into_iter().flatten().max() };
            if let Some(s) = s {
                let icon = (!w.path.is_empty()).then(|| format!("file:{}", w.path));
                let sub = if w.minimized { format!("{} · minimized", w.app) } else { w.app.clone() };
                hits.push(hit(format!("win:{}:{}", w.hwnd, w.pid), "window", w.title.clone(), sub, icon, s));
            }
        }
    }
    // Clipboard entries in an ordinary search once there's enough typed; in their category, always.
    if src.settings.clipboard && wants("clip") && (only.is_some() || q.whole.chars().count() >= 3) {
        for c in src.clips {
            if let Some(s) = if all { Some(0) } else { q.score(&preview(&c.text, 2000)) } {
                hits.push(clip_hit(c, s, src.now));
            }
        }
    }
    if src.settings.files && wants("file") && !all {
        for f in src.files.search(&q, if only.is_some() { MORE } else { 40 }) {
            if f.score < floor {
                continue;
            }
            let path = f.path.to_string_lossy().into_owned();
            let folder = f.path.parent().map(|p| tidy(&p.to_string_lossy(), src.home)).unwrap_or_default();
            let (kind, icon) = if f.child.dir {
                ("folder", format!("folder:{path}"))
            } else if !f.child.online_only && is_picture(&path) {
                ("file", format!("thumb:{path}"))
            } else {
                ("file", format!("file:{path}"))
            };
            hits.push(hit(format!("{kind}:{path}"), kind, f.child.name.to_string(), folder, Some(icon), f.score));
        }
    }

    // What you picked before counts, and a match counts for more in some sections than others.
    for h in &mut hits {
        let weight = KINDS.iter().find(|k| k.0 == h.kind).map_or(0, |k| k.3);
        h.score += weight + src.history.boost(text, &h.key);
    }
    hits.sort_by_key(|h| -h.score);

    let shown = |l: usize| match (only.is_some(), all) {
        (true, true) => usize::MAX,
        (true, false) => MORE,
        _ => l,
    };
    let mut top = None;
    if only.is_some() {
        // A category is a list: no sums or addresses, and nothing typed has no Top Hit.
        if !all && !hits.is_empty() {
            top = Some(hits.remove(0));
        }
    } else if let Some(answer) = calc::answer(text, src.comma_decimal) {
        top = Some(hit(format!("calc:{}", answer.value), "calc", answer.value, answer.detail, None, i32::MAX));
    } else if let Some(url) = as_url(text) {
        top = Some(hit(format!("url:{url}"), "url", url.clone(), "Open in your browser".into(), None, i32::MAX));
    } else if let Some(dir) = as_path(text, src.home) {
        let kind = if dir.is_dir() { "folder" } else { "file" };
        let p = dir.to_string_lossy().into_owned();
        top = Some(hit(format!("{kind}:{p}"), kind, p.clone(), "Open".into(), Some(format!("{kind}:{p}")), i32::MAX));
    } else if !hits.is_empty() {
        top = Some(hits.remove(0));
    }

    let mut sections: Vec<Section> = KINDS
        .iter()
        .filter_map(|(kind, title, limit, _)| {
            let mine: Vec<Hit> = hits.iter().filter(|h| h.kind == *kind).take(shown(*limit)).cloned().collect();
            (!mine.is_empty()).then_some(Section { title, hits: mine })
        })
        .collect();
    // Always a way to the web, at the end (and first, for a query starting with "?").
    let (name, _) = engine(&src.settings.engine);
    let web_query = text.strip_prefix('?').map_or(text, str::trim);
    if !web_query.is_empty() && only.is_none() {
        let web = hit(format!("web:{web_query}"), "web", format!("Search {name} for “{web_query}”"), "Opens in your browser".into(), None, 0);
        if text.starts_with('?') {
            return Results { top: Some(web), sections: Vec::new() };
        }
        sections.push(Section { title: "Web", hits: vec![web] });
    }
    Results { top, sections }
}

fn hit(key: String, kind: &'static str, title: String, subtitle: String, icon: Option<String>, score: i32) -> Hit {
    Hit { key, kind, title, subtitle, icon, score, confirm: false }
}

fn clip_hit(c: &Clip, score: i32, now: u64) -> Hit {
    let when = ago(now.saturating_sub(c.at));
    let sub = if c.app.is_empty() { format!("Copied {when}") } else { format!("Copied from {} {when}", c.app) };
    hit(format!("clip:{}", c.id), "clip", preview(&c.text, 200), sub, None, score)
}

/// "cb", "clipboard", "cb foo" → the words to look for ("" for all).
fn clipboard_mode(lower: &str) -> Option<&str> {
    if lower == "cb" || lower == "clipboard" || lower == "clip" {
        return Some("");
    }
    ["cb ", "clipboard ", "clip "].iter().find_map(|p| lower.strip_prefix(p)).map(str::trim_start)
}

fn clipboard_only(rest: &str, src: &Sources) -> Results {
    let q = Query::new(rest);
    let hits: Vec<Hit> = src
        .clips
        .iter()
        .filter_map(|c| {
            let s = if q.is_empty() { 0 } else { q.score(&preview(&c.text, 2000))? };
            Some(clip_hit(c, s, src.now))
        })
        .take(50)
        .collect();
    let title = if src.clips.is_empty() { "Clipboard: nothing copied yet" } else { "Clipboard" };
    Results { top: None, sections: vec![Section { title, hits }] }
}

/// The start of a text, on one line, for a list.
pub fn preview(text: &str, max: usize) -> String {
    let one: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > max {
        one.chars().take(max).collect::<String>() + "…"
    } else {
        one
    }
}

fn ago(secs: u64) -> String {
    match secs {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=86399 => format!("{} h ago", secs / 3600),
        _ => format!("{} days ago", secs / 86400),
    }
}

/// A folder as people read it: the home folder as "~".
pub fn tidy(path: &str, home: &str) -> String {
    if !home.is_empty() && path.len() >= home.len() && path[..home.len()].eq_ignore_ascii_case(home) {
        format!("~{}", &path[home.len()..])
    } else {
        path.to_string()
    }
}

pub fn is_picture(path: &str) -> bool {
    let ext = Path::new(path).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    ["jpg", "jpeg", "png", "gif", "bmp", "webp", "heic", "tif", "tiff", "mp4", "mov", "mkv", "avi", "wmv"].contains(&ext.as_str())
}

/// A web address typed in: "https://…", or "example.com/page" without one.
pub fn as_url(text: &str) -> Option<String> {
    if text.contains(char::is_whitespace) {
        return None;
    }
    let lower = text.to_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return (text.len() > 8).then(|| text.to_string());
    }
    let host = text.split(['/', '?', '#']).next()?;
    let host = host.split(':').next()?;
    let labels: Vec<&str> = host.split('.').collect();
    let tld = labels.last()?;
    let ok = labels.len() >= 2
        && labels.iter().all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && tld.len() >= 2
        && tld.chars().all(|c| c.is_ascii_alphabetic())
        // Not a file name: report.pdf, notes.txt.
        && !FILE_LIKE.contains(&tld.to_lowercase().as_str());
    ok.then(|| format!("https://{text}"))
}

const FILE_LIKE: &[&str] = &[
    "pdf", "txt", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "png", "jpg", "jpeg", "gif", "exe", "zip", "rs", "js", "py", "json", "md", "csv", "mp3", "mp4", "html", "htm", "css", "lnk", "msi", "rar", "7z", "bat", "ps1", "toml", "yml", "yaml", "xml", "log", "ini", "cfg",
];

/// A path typed in, that's there: "C:\Users", "~\Downloads", "%APPDATA%".
fn as_path(text: &str, home: &str) -> Option<std::path::PathBuf> {
    let t = text.trim_matches('"');
    let expanded = if let Some(rest) = t.strip_prefix('~') {
        format!("{home}{rest}")
    } else if let Some(rest) = t.strip_prefix('%') {
        let (name, after) = rest.split_once('%')?;
        format!("{}{after}", std::env::var(name).ok()?)
    } else {
        t.to_string()
    };
    let looks = expanded.len() >= 3 && (expanded.as_bytes()[1] == b':' || expanded.starts_with(r"\\"));
    let p = std::path::PathBuf::from(expanded);
    (looks && p.exists()).then_some(p)
}

/// The query, ready to go after a search engine's "?q=": everything but letters, digits and
/// "-._~" percent-encoded, as UTF-8.
pub fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources<'a>(settings: &'a Settings, apps: &'a [App], files: &'a Files, clips: &'a [Clip], history: &'a History) -> Sources<'a> {
        Sources { settings, apps, files, windows: &[], clips, history, comma_decimal: false, home: r"C:\Users\me", now: 10_000, only: None }
    }

    fn app(name: &str) -> App {
        App { id: format!("{name}.exe"), name: name.into() }
    }

    #[test]
    fn top_hit_and_sections() {
        let (settings, files) = (Settings::default(), Files::default());
        let history = History::load(std::env::temp_dir().join(format!("fanlight-search-test-{}.json", std::process::id())));
        let apps = [app("Visual Studio Code"), app("Notepad"), app("Snipping Tool")];
        let r = search("code", &sources(&settings, &apps, &files, &[], &history));
        assert_eq!(r.top.as_ref().unwrap().title, "Visual Studio Code");
        assert_eq!(r.sections.last().unwrap().hits[0].kind, "web");
        // A command and a settings page by the words they go by.
        let r = search("reboot", &sources(&settings, &apps, &files, &[], &history));
        assert_eq!(r.top.as_ref().unwrap().key, "cmd:restart");
        assert!(r.top.unwrap().confirm);
        let r = search("wallpaper", &sources(&settings, &apps, &files, &[], &history));
        assert_eq!(r.top.unwrap().key, "page:ms-settings:personalization-background");
        // A sum comes first.
        let r = search("2+2", &sources(&settings, &apps, &files, &[], &history));
        assert_eq!(r.top.unwrap().title, "4");
        // Nothing typed, nothing shown.
        assert!(search("  ", &sources(&settings, &apps, &files, &[], &history)).top.is_none());
        // "?" sends it straight to the web.
        let r = search("?notepad", &sources(&settings, &apps, &files, &[], &history));
        assert_eq!(r.top.unwrap().key, "web:notepad");
        assert!(r.sections.is_empty());
    }

    #[test]
    fn picks_are_learned() {
        let (settings, files) = (Settings::default(), Files::default());
        let path = std::env::temp_dir().join(format!("fanlight-learn-test-{}.json", std::process::id()));
        let history = History::load(path);
        let apps = [app("Notepad"), app("Notepad++")];
        assert_eq!(search("note", &sources(&settings, &apps, &files, &[], &history)).top.unwrap().title, "Notepad");
        history.picked("note", "app:Notepad++.exe");
        assert_eq!(search("note", &sources(&settings, &apps, &files, &[], &history)).top.unwrap().title, "Notepad++");
        history.clear();
    }

    #[test]
    fn clipboard_searches() {
        let (settings, files) = (Settings::default(), Files::default());
        let history = History::load(std::env::temp_dir().join("fanlight-unused.json"));
        let clips = [
            Clip { id: 1, text: "the tracking number is 1Z999AA10123456784".into(), at: 9_000, app: "Outlook".into() },
            Clip { id: 2, text: "SELECT * FROM users".into(), at: 9_990, app: String::new() },
        ];
        let r = search("cb", &sources(&settings, &[], &files, &clips, &history));
        assert_eq!(r.sections[0].hits.len(), 2);
        let r = search("cb tracking", &sources(&settings, &[], &files, &clips, &history));
        assert_eq!(r.sections[0].hits.len(), 1);
        assert_eq!(r.sections[0].hits[0].subtitle, "Copied from Outlook 16 min ago");
        // In an ordinary search too, once there's enough typed.
        let r = search("tracking", &sources(&settings, &[], &files, &clips, &history));
        assert!(r.sections.iter().any(|s| s.title == "Clipboard") || r.top.is_some_and(|t| t.kind == "clip"));
        // With the history off, none of it.
        let off = Settings { clipboard: false, ..Default::default() };
        assert!(search("cb", &sources(&off, &[], &files, &clips, &history)).sections.iter().all(|s| s.title != "Clipboard"));
    }

    #[test]
    fn categories() {
        let (settings, files) = (Settings::default(), Files::default());
        let history = History::load(std::env::temp_dir().join("fanlight-unused.json"));
        let apps = [app("Calculator"), app("Notepad")];
        let clips = [Clip { id: 1, text: "calendar invite".into(), at: 9_000, app: String::new() }];
        let mut src = sources(&settings, &apps, &files, &clips, &history);
        src.only = Some("apps");
        // Nothing typed: every app.
        let r = search("", &src);
        assert!(r.top.is_none() && r.sections[0].hits.len() == 2);
        // Only apps, and no sum or web search.
        let r = search("cal", &src);
        assert_eq!(r.top.unwrap().title, "Calculator");
        assert!(r.sections.iter().all(|s| s.title == "Applications"));
        src.only = Some("clipboard");
        let r = search("ca", &src);
        assert_eq!(r.top.unwrap().key, "clip:1");
        src.only = Some("actions");
        assert!(search("", &src).sections.iter().any(|s| s.title == "Commands"));
        assert!(search("2+2", &src).top.is_none_or(|t| t.kind != "calc"));
        src.only = Some("files");
        assert!(search("", &src).sections.is_empty());
    }

    #[test]
    fn urls() {
        assert_eq!(as_url("github.com").as_deref(), Some("https://github.com"));
        assert_eq!(as_url("example.co.uk/a?b=c").as_deref(), Some("https://example.co.uk/a?b=c"));
        assert_eq!(as_url("http://localhost:8080").as_deref(), Some("http://localhost:8080"));
        assert_eq!(as_url("report.pdf"), None);
        assert_eq!(as_url("hello world.com"), None);
        assert_eq!(as_url("1.5"), None);
        assert_eq!(as_url("notepad"), None);
    }

    #[test]
    fn paths_and_words() {
        assert_eq!(tidy(r"C:\Users\me\Documents", r"C:\Users\me"), r"~\Documents");
        assert_eq!(tidy(r"D:\Work", r"C:\Users\me"), r"D:\Work");
        assert_eq!(encode("c++ & rust?"), "c%2B%2B+%26+rust%3F");
        assert_eq!(encode("café"), "caf%C3%A9");
        assert_eq!(preview("  a\n\n b  c ", 10), "a b c");
        assert_eq!(preview("abcdef", 3), "abc…");
        assert_eq!(clipboard_mode("cb  foo"), Some("foo"));
        assert_eq!(clipboard_mode("cbx"), None);
        let windir = std::env::var("WINDIR").unwrap();
        assert!(as_path(&windir, "").is_some());
        assert!(as_path("%WINDIR%", "").is_some());
        assert!(as_path(r"Q:\does\not\exist", "").is_none());
        assert!(as_path("notepad", "").is_none());
    }
}
