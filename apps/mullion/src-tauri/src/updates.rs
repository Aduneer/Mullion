//! Mullion's check for updates: GitHub's API for the suite's latest release, a minute after
//! Mullion starts and then once a day, while the setting is on. Nothing is sent but the request
//! itself (with "Mullion/<version>" as its User-Agent).
//!
//! When there's a newer version, Mullion Settings and the tray say so. Installed with Mullion's
//! installer, Update downloads that release's `Mullion-<version>-setup.exe`, checks it against
//! the release's `SHA256SUMS.txt`, and runs it with `/S /update`: it quits Mullion and its apps,
//! keeps the apps that are installed, and starts Mullion again. Anywhere else (the portable zip),
//! or if Windows won't run the installer (Smart App Control, as releases aren't signed yet),
//! Update opens the release page instead.

use mullion_core::{ffi, net};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const LATEST: &str = "https://api.github.com/repos/Aduneer/Mullion/releases/latest";
/// Where a release's page and files must be: anything else in the API's reply is ignored.
const PAGES: &str = "https://github.com/Aduneer/Mullion/releases/";
const FILES: &str = "https://github.com/Aduneer/Mullion/releases/download/";
const MAX_REPLY: usize = 1 << 20;
const MAX_SETUP: usize = 64 << 20;
const TIMEOUT: Duration = Duration::from_secs(20);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);
/// The first check waits this long after Mullion starts, so it's not in the way of signing in.
pub const FIRST_CHECK: Duration = Duration::from_secs(60);
pub const EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// What the page shows about updates.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Status {
    pub checking: bool,
    pub installing: bool,
    /// When the last check came back, in seconds since 1970.
    pub checked: Option<u64>,
    /// A newer release than this Mullion, if there is one.
    pub newer: Option<Release>,
    pub error: Option<String>,
    /// Update can download and run the installer, rather than open the release page.
    pub can_install: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Release {
    /// "0.5.0", without the tag's "v".
    pub version: String,
    pub page: String,
    /// The release notes, as written (Markdown).
    pub notes: String,
    #[serde(skip)]
    setup: Option<String>,
    #[serde(skip)]
    sums: Option<String>,
}

#[derive(Deserialize)]
struct Reply {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// "v1.2.3" or "1.2.3" as numbers to compare. Anything else (a suffix, a fourth part) isn't a
/// version we release.
fn version(v: &str) -> Option<(u32, u32, u32)> {
    let mut parts = v.strip_prefix('v').unwrap_or(v).split('.').map(|p| p.parse::<u32>().ok());
    let v = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(v)
}

/// The latest release from the API's reply, if it's newer than `current`.
fn newer(reply: &[u8], current: &str) -> Result<Option<Release>, String> {
    let r: Reply = serde_json::from_slice(reply).map_err(|_| "GitHub's reply didn't make sense".to_string())?;
    let latest = version(&r.tag_name).ok_or("GitHub's latest release has an odd version")?;
    if r.draft || r.prerelease || !r.html_url.starts_with(PAGES) || version(current).is_none_or(|c| latest <= c) {
        return Ok(None);
    }
    let v = format!("{}.{}.{}", latest.0, latest.1, latest.2);
    let file = |name: &str| r.assets.iter().find(|a| a.name == name && a.browser_download_url.starts_with(FILES)).map(|a| a.browser_download_url.clone());
    Ok(Some(Release {
        setup: file(&format!("Mullion-{v}-setup.exe")),
        sums: file("SHA256SUMS.txt"),
        version: v,
        page: r.html_url,
        notes: r.body.unwrap_or_default(),
    }))
}

/// Ask GitHub for the latest release. Blocks for up to `TIMEOUT`, so off the main thread.
pub fn check() -> Result<Option<Release>, String> {
    let reply = net::get(LATEST, "application/vnd.github+json", MAX_REPLY, TIMEOUT).map_err(|e| e.text())?;
    newer(&reply, env!("CARGO_PKG_VERSION"))
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// The folder Mullion's installer put it in, if this Mullion is the one it installed. Only then
/// can its installer update it.
pub fn installed_here() -> bool {
    let Some(dir) = ffi::reg_user_string(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Mullion", "InstallLocation") else { return false };
    let here = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf()));
    here.is_some_and(|h| same_folder(&h, &PathBuf::from(dir.trim_matches('"'))))
}

fn same_folder(a: &std::path::Path, b: &std::path::Path) -> bool {
    let norm = |p: &std::path::Path| p.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase();
    norm(a) == norm(b)
}

/// The installer's SHA-256 from the release's SHA256SUMS.txt ("<hex>  <name>" lines).
fn listed_sum(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let (hex, file) = l.trim().split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name && hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then(|| hex.to_lowercase())
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Download the release's installer, check it, and start it. Blocks while it downloads.
/// Err says why it couldn't, and the page then offers the release page.
pub fn install(r: &Release) -> Result<(), String> {
    let path = download(r)?;
    // Silent, keeping the apps that are installed; /update starts Mullion again at the end.
    std::process::Command::new(&path).args(["/S", "/update"]).spawn().map(|_| ()).map_err(|e| {
        if e.raw_os_error() == Some(4551) {
            "Windows blocked the installer (Smart App Control)".to_string()
        } else {
            format!("The installer didn't start: {e}")
        }
    })
}

/// The release's installer, downloaded into %TEMP%\Mullion and checked against the release's
/// SHA256SUMS.txt.
fn download(r: &Release) -> Result<PathBuf, String> {
    let (setup, sums) = r.setup.as_deref().zip(r.sums.as_deref()).ok_or("This release has no installer to download")?;
    let name = format!("Mullion-{}-setup.exe", r.version);
    let sums = net::get(sums, "text/plain, */*", MAX_REPLY, TIMEOUT).map_err(|e| e.text())?;
    let want = listed_sum(&String::from_utf8_lossy(&sums), &name).ok_or("The release doesn't list the installer's checksum")?;
    let body = net::get(setup, "application/octet-stream", MAX_SETUP, DOWNLOAD_TIMEOUT).map_err(|e| e.text())?;
    if hex(&Sha256::digest(&body)) != want {
        return Err("The download didn't match the release's checksum".into());
    }
    let dir = std::env::temp_dir().join("Mullion");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(&name);
    std::fs::write(&path, &body).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(tag: &str) -> String {
        format!(
            r#"{{"tag_name":"{tag}","html_url":"https://github.com/Aduneer/Mullion/releases/tag/{tag}","body":"Notes","draft":false,"prerelease":false,
               "assets":[{{"name":"Mullion-0.5.0-setup.exe","browser_download_url":"https://github.com/Aduneer/Mullion/releases/download/v0.5.0/Mullion-0.5.0-setup.exe"}},
                         {{"name":"SHA256SUMS.txt","browser_download_url":"https://github.com/Aduneer/Mullion/releases/download/v0.5.0/SHA256SUMS.txt"}},
                         {{"name":"Bay-0.5.0-setup.exe","browser_download_url":"https://evil.example/Bay-0.5.0-setup.exe"}}]}}"#
        )
    }

    #[test]
    fn versions_compare_as_numbers() {
        assert_eq!(version("v0.4.1"), Some((0, 4, 1)));
        assert_eq!(version("1.10.0"), Some((1, 10, 0)));
        assert!(version("v0.10.0") > version("v0.9.9"));
        for odd in ["", "v", "1.2", "1.2.3.4", "1.2.3-beta", "one.two.three"] {
            assert_eq!(version(odd), None, "{odd}");
        }
    }

    #[test]
    fn only_a_newer_release_is_offered() {
        let r = newer(reply("v0.5.0").as_bytes(), "0.4.1").unwrap().unwrap();
        assert_eq!(r.version, "0.5.0");
        assert_eq!(r.page, "https://github.com/Aduneer/Mullion/releases/tag/v0.5.0");
        assert_eq!(r.notes, "Notes");
        assert!(r.setup.as_deref().unwrap().ends_with("/v0.5.0/Mullion-0.5.0-setup.exe"));
        assert!(r.sums.is_some());
        assert_eq!(newer(reply("v0.5.0").as_bytes(), "0.5.0").unwrap(), None);
        assert_eq!(newer(reply("v0.4.0").as_bytes(), "0.4.1").unwrap(), None);
        assert!(newer(b"not json", "0.4.1").is_err());
        assert!(newer(reply("nightly").as_bytes(), "0.4.1").is_err());
    }

    #[test]
    fn files_must_come_from_the_release() {
        // A download elsewhere is as good as none: Update then opens the release page.
        let elsewhere = reply("v0.5.0").replace("https://github.com/Aduneer/Mullion/releases/download/", "https://evil.example/");
        let r = newer(elsewhere.as_bytes(), "0.4.1").unwrap().unwrap();
        assert_eq!((r.setup, r.sums), (None, None));
        // And a page elsewhere isn't a release of ours at all.
        let page = reply("v0.5.0").replace("\"html_url\":\"https://github.com/Aduneer/", "\"html_url\":\"https://evil.example/");
        assert_eq!(newer(page.as_bytes(), "0.4.1").unwrap(), None);
    }

    #[test]
    fn drafts_and_prereleases_are_not_offered() {
        for flag in ["draft", "prerelease"] {
            let r = reply("v0.5.0").replace(&format!("\"{flag}\":false"), &format!("\"{flag}\":true"));
            assert_eq!(newer(r.as_bytes(), "0.4.1").unwrap(), None, "{flag}");
        }
    }

    #[test]
    fn the_checksum_is_read_from_the_release_list() {
        let a = "a".repeat(64);
        let sums = format!("{}  Bay-0.5.0-setup.exe\r\n{}  Mullion-0.5.0-setup.exe\r\n", "b".repeat(64), a.to_uppercase());
        assert_eq!(listed_sum(&sums, "Mullion-0.5.0-setup.exe"), Some(a));
        assert_eq!(listed_sum(&sums, "Mullion-0.6.0-setup.exe"), None);
        assert_eq!(listed_sum("xyz  Mullion-0.5.0-setup.exe", "Mullion-0.5.0-setup.exe"), None);
        assert_eq!(hex(&Sha256::digest(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    // Goes online (GitHub's API), so it only runs on request: cargo test -p mullion -- --ignored
    #[test]
    #[ignore = "needs the internet"]
    fn github_answers() {
        net::user_agent(format!("Mullion/{}", env!("CARGO_PKG_VERSION")));
        let reply = net::get(LATEST, "application/vnd.github+json", MAX_REPLY, TIMEOUT).expect("GitHub answered");
        // As if this were 0.0.1: the real latest release, with its installer and checksums.
        let r = newer(&reply, "0.0.1").unwrap().expect("a newer release");
        assert!(r.page.starts_with(PAGES), "{}", r.page);
        assert!(r.setup.is_some() && r.sums.is_some());
        println!("latest: {} ({})", r.version, r.page);
        assert!(check().is_ok());
        // Update's download (about 8 MB), checked against the release's list; not run.
        let path = download(&r).expect("the installer downloaded and matched its checksum");
        println!("downloaded: {} ({} bytes)", path.display(), std::fs::metadata(&path).unwrap().len());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn folders_compare_without_case_or_a_trailing_slash() {
        assert!(same_folder(std::path::Path::new(r"C:\Users\x\AppData\Local\Mullion"), std::path::Path::new(r"c:\users\x\appdata\local\mullion\")));
        assert!(!same_folder(std::path::Path::new(r"C:\Mullion"), std::path::Path::new(r"C:\Mullion2")));
    }
}
