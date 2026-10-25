//! The clipboard history: text you copy, kept to search and paste again. Windows says when the
//! clipboard changes (a clipboard format listener on a message-only window), so nothing is polled.
//!
//! What password managers and other apps mark as private is left out, by the same formats
//! Windows' own clipboard history honours. It's kept in memory only, unless "Keep the clipboard
//! history" is on, when it's also saved in %APPDATA%\Fanlight\clipboard.json.

use crate::ffi::*;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// Larger copies (a whole log file, say) aren't kept.
const MAX_TEXT: usize = 256 * 1024;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Clip {
    pub id: u64,
    pub text: String,
    /// Unix seconds.
    pub at: u64,
    /// The app it was copied from, when Windows says.
    pub app: String,
}

#[derive(Default)]
struct State {
    clips: VecDeque<Clip>,
    next_id: u64,
    on: bool,
    max: usize,
    keep: bool,
    /// Where it's saved while `keep` is on; none in tests.
    file: Option<PathBuf>,
}

#[derive(Clone, Default)]
pub struct Clipboard {
    state: Arc<Mutex<State>>,
}

/// The clipboard's sequence number after our own last write, so a paste from the history doesn't
/// come back as a new entry.
static OURS: AtomicU32 = AtomicU32::new(0);

fn store() -> PathBuf {
    crate::settings::dir().join("clipboard.json")
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Clipboard {
    pub fn start(on: bool, max: usize, keep: bool) -> Self {
        let cb = Clipboard::default();
        cb.state.lock().unwrap().file = Some(store());
        cb.configure(on, max, keep);
        if keep {
            if let Some(clips) = crate::settings::read_json::<VecDeque<Clip>>(&store()) {
                let mut s = cb.state.lock().unwrap();
                s.next_id = clips.iter().map(|c| c.id + 1).max().unwrap_or(0);
                s.clips = clips;
                s.clips.truncate(max);
            }
        }
        let me = cb.clone();
        let _ = std::thread::Builder::new().name("clipboard".into()).spawn(move || listen(me));
        cb
    }

    /// Settings changed: on or off, how many to keep, and whether to save them.
    pub fn configure(&self, on: bool, max: usize, keep: bool) {
        let mut s = self.state.lock().unwrap();
        s.on = on;
        s.max = max;
        s.keep = keep;
        if !on {
            s.clips.clear();
        }
        s.clips.truncate(max);
        save(&s);
    }

    pub fn clips(&self) -> Vec<Clip> {
        self.state.lock().unwrap().clips.iter().cloned().collect()
    }

    pub fn get(&self, id: u64) -> Option<Clip> {
        self.state.lock().unwrap().clips.iter().find(|c| c.id == id).cloned()
    }

    pub fn remove(&self, id: u64) {
        let mut s = self.state.lock().unwrap();
        s.clips.retain(|c| c.id != id);
        save(&s);
    }

    pub fn clear(&self) {
        let mut s = self.state.lock().unwrap();
        s.clips.clear();
        save(&s);
    }

    /// A copy was made: add it to the top, or move it there if it's already in the history.
    fn add(&self, text: String, app: String) {
        let mut s = self.state.lock().unwrap();
        if !s.on || text.trim().is_empty() || text.len() > MAX_TEXT {
            return;
        }
        let id = s.next_id;
        s.next_id += 1;
        s.clips.retain(|c| c.text != text);
        s.clips.push_front(Clip { id, text, at: now(), app });
        let max = s.max;
        s.clips.truncate(max);
        save(&s);
    }
}

fn save(s: &State) {
    let Some(file) = &s.file else { return };
    if s.keep {
        let _ = crate::settings::write_json(file, &s.clips, false);
    } else {
        let _ = std::fs::remove_file(file);
    }
}

/// Put text on the clipboard, as Ctrl+C would.
pub fn set_text(text: &str) -> Result<(), String> {
    let units: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    unsafe {
        if !open() {
            return Err("another app is using the clipboard".into());
        }
        EmptyClipboard();
        let mem = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2);
        let ok = !mem.is_null() && {
            let p = GlobalLock(mem) as *mut u16;
            !p.is_null() && {
                std::ptr::copy_nonoverlapping(units.as_ptr(), p, units.len());
                GlobalUnlock(mem);
                !SetClipboardData(CF_UNICODETEXT, mem).is_null()
            }
        };
        if !ok && !mem.is_null() {
            GlobalFree(mem);
        }
        CloseClipboard();
        OURS.store(GetClipboardSequenceNumber(), Ordering::Relaxed);
        if ok {
            Ok(())
        } else {
            Err("Windows wouldn't take the text".into())
        }
    }
}

/// Another app may have the clipboard open for a moment; try a few times.
unsafe fn open() -> bool {
    for _ in 0..10 {
        if OpenClipboard(0) != 0 {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    false
}

thread_local! {
    static WATCH: std::cell::RefCell<Option<Clipboard>> = const { std::cell::RefCell::new(None) };
}

fn listen(cb: Clipboard) {
    WATCH.with(|w| *w.borrow_mut() = Some(cb));
    unsafe {
        let class = wide("FanlightClipboard");
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: proc_,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: GetModuleHandleW(std::ptr::null()),
            hIcon: 0,
            hCursor: 0,
            hbrBackground: 0,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(0, class.as_ptr(), class.as_ptr(), 0, 0, 0, 0, 0, HWND_MESSAGE, 0, wc.hInstance, std::ptr::null_mut());
        if hwnd == 0 || AddClipboardFormatListener(hwnd) == 0 {
            return;
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn proc_(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize {
    if msg == WM_CLIPBOARDUPDATE && GetClipboardSequenceNumber() != OURS.load(Ordering::Relaxed) {
        if let Some((text, app)) = read() {
            WATCH.with(|w| {
                if let Some(cb) = w.borrow().as_ref() {
                    cb.add(text, app);
                }
            });
        }
        return 0;
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// The text just copied and the app it came from, unless it's marked private or isn't text.
unsafe fn read() -> Option<(String, String)> {
    if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 || private() {
        return None;
    }
    let app = {
        let mut pid = 0;
        GetWindowThreadProcessId(GetClipboardOwner(), &mut pid);
        (pid != 0 && pid != std::process::id())
            .then(|| image_path(pid))
            .flatten()
            .map(|p| file_description(&p).unwrap_or_else(|| std::path::Path::new(&p).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()))
            .unwrap_or_default()
    };
    if !open() {
        return None;
    }
    let mem = GetClipboardData(CF_UNICODETEXT);
    let text = (!mem.is_null()).then(|| {
        let p = GlobalLock(mem) as *const u16;
        let units = GlobalSize(mem) / 2;
        let s = if p.is_null() { None } else { Some(from_wide(std::slice::from_raw_parts(p, units))) };
        GlobalUnlock(mem);
        s
    });
    CloseClipboard();
    Some((text??, app))
}

/// Copies that ask to be left out of clipboard history and monitoring: password managers set
/// these, as do apps copying passwords and one-time codes.
unsafe fn private() -> bool {
    let format = |name: &str| RegisterClipboardFormatW(wide(name).as_ptr());
    let exclude = format("ExcludeClipboardContentFromMonitorProcessing");
    let ignore = format("Clipboard Viewer Ignore");
    if IsClipboardFormatAvailable(exclude) != 0 || IsClipboardFormatAvailable(ignore) != 0 {
        return true;
    }
    // A DWORD: 0 means "keep this out of the history".
    let include = format("CanIncludeInClipboardHistory");
    IsClipboardFormatAvailable(include) != 0 && dword(include) == Some(0)
}

unsafe fn dword(format: u32) -> Option<u32> {
    if !open() {
        return None;
    }
    let mem = GetClipboardData(format);
    let v = (!mem.is_null() && GlobalSize(mem) >= 4).then(|| {
        let p = GlobalLock(mem) as *const u32;
        let v = if p.is_null() { None } else { Some(*p) };
        GlobalUnlock(mem);
        v
    });
    CloseClipboard();
    v.flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_first_without_repeats() {
        let cb = Clipboard::default();
        {
            let mut s = cb.state.lock().unwrap();
            s.on = true;
            s.max = 3;
        }
        for t in ["one", "two", "three", "two", "four", "   "] {
            cb.add(t.into(), String::new());
        }
        let texts: Vec<String> = cb.clips().into_iter().map(|c| c.text).collect();
        assert_eq!(texts, ["four", "two", "three"]);
        let id = cb.clips()[1].id;
        cb.remove(id);
        assert_eq!(cb.clips().len(), 2);
        // Off: nothing is kept.
        cb.state.lock().unwrap().on = false;
        cb.add("five".into(), String::new());
        assert_eq!(cb.clips().len(), 2);
    }
}
