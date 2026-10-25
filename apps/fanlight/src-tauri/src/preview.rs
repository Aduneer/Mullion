//! What the preview pane shows for a result (Space): a picture at full size, the start of a text
//! or code file, what's in a folder, a clipboard entry in full, and the facts about it: its size,
//! when it was changed, where it is. Read only when asked, one result at a time; files in the
//! cloud only (OneDrive placeholders) aren't read, so a preview never downloads anything.

use serde::Serialize;
use std::io::Read;
use std::os::windows::fs::MetadataExt;
use std::path::Path;

/// The most of a text file that's read and shown.
const TEXT_BYTES: usize = 32 * 1024;
const TEXT_LINES: usize = 300;
/// Larger files of a type that isn't known to be text aren't looked into.
const SNIFF_MAX: u64 = 8 * 1024 * 1024;
const FOLDER_ITEMS: usize = 40;

const CLOUD_ONLY: u32 = 0x1000 | 0x4_0000 | 0x40_0000;

#[derive(Serialize, Default, Debug, PartialEq)]
pub struct Preview {
    /// An icon key for the `icon` command: a picture's large thumbnail, or a large icon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// `image` is the content itself (a picture or a video's frame), to fill the pane.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub picture: bool,
    /// The start of a text file, or a clipboard entry in full.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The text goes on past what's shown.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub more: bool,
    /// What's in a folder, folders first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<Item>,
    pub facts: Vec<Fact>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Item {
    pub name: String,
    pub dir: bool,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Fact {
    pub label: &'static str,
    pub value: String,
    /// Unix seconds, for the page to write as a local date and time (`value` is then empty).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<u64>,
}

fn fact(label: &'static str, value: impl Into<String>) -> Fact {
    Fact { label, value: value.into(), time: None }
}

fn when(label: &'static str, t: std::io::Result<std::time::SystemTime>) -> Option<Fact> {
    let secs = t.ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    Some(Fact { label, value: String::new(), time: Some(secs) })
}

/// A file or folder's preview.
pub fn path(path: &Path, home: &str) -> Option<Preview> {
    let meta = std::fs::metadata(path).ok()?;
    let mut p = Preview::default();
    let shown = path.to_string_lossy();
    let place = path.parent().map(|d| crate::search::tidy(&d.to_string_lossy(), home)).unwrap_or_default();
    if meta.is_dir() {
        p.image = Some(format!("big:folder:{shown}"));
        let mut items: Vec<Item> = std::fs::read_dir(path)
            .map(|rd| {
                rd.filter_map(|e| {
                    let e = e.ok()?;
                    let m = e.metadata().ok()?;
                    let hidden = m.file_attributes() & 0x6 != 0;
                    let name = e.file_name().to_string_lossy().into_owned();
                    (!hidden && !name.starts_with('.')).then_some(Item { name, dir: m.is_dir() })
                })
                .collect()
            })
            .unwrap_or_default();
        let count = items.len();
        items.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        items.truncate(FOLDER_ITEMS);
        p.more = count > FOLDER_ITEMS;
        p.items = items;
        p.facts.push(fact("Kind", "Folder"));
        p.facts.push(fact("Contains", match count {
            0 => "Nothing".to_string(),
            1 => "1 item".to_string(),
            n => format!("{n} items"),
        }));
    } else {
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let online_only = meta.file_attributes() & CLOUD_ONLY != 0;
        if !online_only && crate::search::is_picture(&shown) {
            p.image = Some(format!("big:thumb:{shown}"));
            p.picture = true;
        } else {
            p.image = Some(format!("big:file:{shown}"));
            if !online_only && (TEXT.contains(&ext.as_str()) || (meta.len() <= SNIFF_MAX && !BINARY.contains(&ext.as_str()))) {
                if let Some((text, more)) = read_text(path) {
                    p.text = Some(text);
                    p.more = more;
                }
            }
        }
        p.facts.push(fact("Kind", kind_name(&ext)));
        p.facts.push(fact("Size", bytes(meta.len())));
        if online_only {
            p.facts.push(fact("Stored", "In the cloud only"));
        }
    }
    p.facts.extend(when("Modified", meta.modified()));
    p.facts.extend(when("Created", meta.created()));
    p.facts.push(fact("Where", place));
    Some(p)
}

/// Types that are text whatever's in them; others are looked into, unless they're known not to be.
const TEXT: &[&str] = &[
    "txt", "md", "markdown", "log", "csv", "tsv", "json", "jsonc", "xml", "yml", "yaml", "toml", "ini", "cfg", "conf", "env", "rs", "js", "mjs", "ts", "tsx", "jsx",
    "py", "rb", "go", "java", "kt", "c", "h", "cpp", "hpp", "cs", "swift", "php", "lua", "sh", "ps1", "psm1", "bat", "cmd", "html", "htm", "css", "scss", "svg",
    "sql", "gitignore", "editorconfig", "lock", "srt", "vtt", "tex", "nsh",
];
const BINARY: &[&str] = &[
    "exe", "dll", "msi", "zip", "7z", "rar", "gz", "tar", "iso", "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "mp3", "flac", "wav", "ogg", "m4a",
    "mp4", "mkv", "avi", "mov", "wmv", "webm", "jpg", "jpeg", "png", "gif", "bmp", "webp", "heic", "ico", "psd", "lnk", "db", "sqlite", "bin", "dat", "pak",
];

/// The start of a file, if it's text: UTF-8, or UTF-16 with its byte-order mark. None if it
/// looks like anything else.
fn read_text(path: &Path) -> Option<(String, bool)> {
    let mut buf = Vec::with_capacity(TEXT_BYTES + 1);
    std::fs::File::open(path).ok()?.take(TEXT_BYTES as u64 + 1).read_to_end(&mut buf).ok()?;
    let cut = buf.len() > TEXT_BYTES;
    buf.truncate(TEXT_BYTES);
    let text = decode(&buf)?;
    let lines: Vec<&str> = text.lines().take(TEXT_LINES + 1).collect();
    let more = cut || lines.len() > TEXT_LINES;
    Some((lines.into_iter().take(TEXT_LINES).collect::<Vec<_>>().join("\n"), more))
}

fn decode(buf: &[u8]) -> Option<String> {
    if let Some(rest) = buf.strip_prefix(&[0xFF, 0xFE]) {
        let units: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        return Some(String::from_utf16_lossy(&units));
    }
    if let Some(rest) = buf.strip_prefix(&[0xFE, 0xFF]) {
        let units: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes(*c)).collect();
        return Some(String::from_utf16_lossy(&units));
    }
    let buf = buf.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(buf);
    // Binary files have NULs early on; text doesn't.
    if buf.iter().take(4096).any(|&b| b == 0) {
        return None;
    }
    // A cut can land inside a character; that's the one place lossy decoding is allowed.
    match std::str::from_utf8(buf) {
        Ok(s) => Some(s.to_string()),
        Err(e) if e.valid_up_to() + 4 >= buf.len() => Some(String::from_utf8_lossy(&buf[..e.valid_up_to()]).into_owned()),
        Err(_) => None,
    }
}

fn kind_name(ext: &str) -> String {
    let known = match ext {
        "" => "File",
        "txt" => "Text document",
        "md" | "markdown" => "Markdown document",
        "pdf" => "PDF document",
        "doc" | "docx" | "odt" | "rtf" => "Word document",
        "xls" | "xlsx" | "ods" | "csv" => "Spreadsheet",
        "ppt" | "pptx" | "odp" => "Presentation",
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "heic" | "tif" | "tiff" => "Picture",
        "mp4" | "mov" | "mkv" | "avi" | "wmv" | "webm" => "Video",
        "mp3" | "flac" | "wav" | "ogg" | "m4a" => "Audio",
        "zip" | "7z" | "rar" | "gz" | "tar" => "Archive",
        "exe" | "msi" => "Program",
        "lnk" => "Shortcut",
        _ => return format!("{} file", ext.to_uppercase()),
    };
    known.to_string()
}

/// 1234567 → "1.2 MB", in 1024s as File Explorer counts them.
pub fn bytes(n: u64) -> String {
    let units = ["bytes", "KB", "MB", "GB", "TB"];
    let (mut v, mut i) = (n as f64, 0);
    while v >= 1024.0 && i < units.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    match i {
        0 if n == 1 => "1 byte".into(),
        0 => format!("{n} bytes"),
        _ if v < 10.0 => format!("{v:.1} {}", units[i]),
        _ => format!("{v:.0} {}", units[i]),
    }
}

/// A clipboard entry in full, with where and when it was copied.
pub fn clip(c: &crate::clipboard::Clip) -> Preview {
    let mut facts = vec![fact("Kind", "Copied text")];
    if !c.app.is_empty() {
        facts.push(fact("From", c.app.clone()));
    }
    facts.push(Fact { label: "Copied", value: String::new(), time: Some(c.at) });
    let chars = c.text.chars().count();
    let lines = c.text.lines().count().max(1);
    facts.push(fact("Length", format!("{chars} characters{}", if lines > 1 { format!(", {lines} lines") } else { String::new() })));
    let more = chars > 20_000;
    Preview { text: Some(c.text.chars().take(20_000).collect()), more, facts, ..Default::default() }
}

/// An app's: its icon, large, and what sort of app it is.
pub fn app(id: &str) -> Preview {
    // Store apps are known by a package family and app name: "Publisher.App_hash!App".
    let store = id.contains('!') && !id.contains('\\');
    let mut facts = vec![fact("Kind", if store { "Store app" } else { "Application" })];
    if !store {
        if let Some(file) = id.rsplit('\\').next().filter(|f| f.contains('.')) {
            facts.push(fact("Program", file.to_string()));
        }
    }
    Preview { image: Some(format!("big:app:{id}")), facts, ..Default::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_as_explorer_writes_them() {
        assert_eq!(bytes(0), "0 bytes");
        assert_eq!(bytes(1), "1 byte");
        assert_eq!(bytes(1023), "1023 bytes");
        assert_eq!(bytes(1536), "1.5 KB");
        assert_eq!(bytes(250 * 1024 * 1024), "250 MB");
        assert_eq!(bytes(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn text_in_its_encodings() {
        assert_eq!(decode(b"hello\nworld").as_deref(), Some("hello\nworld"));
        assert_eq!(decode(b"\xEF\xBB\xBFbom").as_deref(), Some("bom"));
        assert_eq!(decode(&[0xFF, 0xFE, b'h', 0, b'i', 0]).as_deref(), Some("hi"));
        assert_eq!(decode(b"MZ\x90\x00\x03\x00"), None);
        // Cut inside "é": the rest is kept.
        assert_eq!(decode(b"caf\xC3").as_deref(), Some("caf"));
        assert_eq!(decode(b"\xFF\xFF\xFF\xFF\xFF\xFF bad"), None);
    }

    #[test]
    fn previews_files_and_folders() {
        let root = std::env::temp_dir().join(format!("fanlight-preview-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sub")).unwrap();
        let long: String = (0..500).map(|i| format!("line {i}\n")).collect();
        std::fs::write(root.join("notes.md"), &long).unwrap();
        std::fs::write(root.join("blob.bin"), [0u8, 1, 2, 3]).unwrap();
        let p = path(&root.join("notes.md"), "").unwrap();
        assert!(p.text.as_ref().unwrap().starts_with("line 0\nline 1") && p.more);
        assert_eq!(p.text.unwrap().lines().count(), TEXT_LINES);
        assert_eq!(p.facts[0].value, "Markdown document");
        assert!(path(&root.join("blob.bin"), "").unwrap().text.is_none());
        let d = path(&root, "").unwrap();
        assert_eq!(d.items[0], Item { name: "sub".into(), dir: true });
        assert_eq!(d.facts[1].value, "3 items");
        assert!(path(&root.join("missing"), "").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn apps() {
        assert_eq!(app("Microsoft.WindowsNotepad_8wekyb3d8bbwe!App").facts[0].value, "Store app");
        let code = app(r"{6D809377-6AF0-444B-8957-A3773F02200E}\Microsoft VS Code\Code.exe");
        assert_eq!((code.facts[0].value.as_str(), code.facts[1].value.as_str()), ("Application", "Code.exe"));
    }
}
