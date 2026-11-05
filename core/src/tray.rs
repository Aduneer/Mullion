//! The notification area's icons, passed from the app that hosts them to the apps that show them.
//! Only one program can host the tray (programs find it by its window class), and that's Apron;
//! Lintel shows the same icons on its bar. Apron serves its table over a named pipe on this PC,
//! open to this user's processes only, and Lintel sends back the clicks, which Apron passes on to
//! each icon's program as Explorer would.
//!
//! The wire is one JSON message per line, each way. Apron sends the whole list whenever anything
//! in it changes (an icon came, changed or went), and once when Lintel connects. Lintel says
//! whether it's showing them, and sends clicks and the pointer coming and going. That's all: no
//! files, no settings, and nothing that leaves the PC (the pipe refuses remote clients).
//!
//! Each end checks who is at the other: Apron only talks to a `lintel.exe`, Lintel only listens to
//! an `apron.exe`, both running as the same user.
#![allow(non_snake_case, clippy::upper_case_acronyms)]

use crate::ffi::{self, HANDLE};
use serde::{Deserialize, Serialize};
use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// An icon in the notification area, as Apron's dock and Lintel's bar show it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Icon {
    /// Stays the same while the icon is there: its GUID, or its window and number.
    pub id: String,
    /// Its tooltip, which is usually its name and state ("Steam - synchronizing").
    pub tip: String,
    /// The program's file name, lowercased, as Windows' own settings list it.
    pub exe: String,
    /// The program's process, so the app the click lands on can let it come to the front.
    pub pid: u32,
    /// Its picture, as a data: URL.
    pub icon: Option<String>,
    /// Windows keeps it on the taskbar, rather than among the hidden icons under ^.
    pub promoted: bool,
    /// A picture in one colour (most are white or black, for Windows' dark or light taskbar), so
    /// it can be drawn in the bar's own ink instead.
    pub glyph: bool,
    /// Show `tip` on hover. Otherwise the program draws its own, when told the pointer is there.
    pub tooltip: bool,
}

/// What Apron says.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FromHost {
    /// Every icon there is now, in order.
    Icons(Vec<Icon>),
}

/// What Lintel says. Rectangles are the icon's place on the screen, in physical pixels: left,
/// top, right, bottom. Programs ask where their icon is to put a menu or a popup beside it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ToHost {
    /// Lintel shows the icons (its setting is on), so the dock needn't as well.
    Showing(bool),
    /// "left", "right" or "double".
    Click { id: String, kind: String, rect: [i32; 4] },
    /// The pointer came onto an icon.
    Hover { id: String, rect: [i32; 4] },
    /// …and left it.
    Leave { id: String },
}

/// The pipe's name: one per Windows session, so each signed-in user's Apron has its own.
pub fn pipe_name() -> String {
    let mut session = 0u32;
    unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) };
    format!(r"\\.\pipe\Mullion.Apron.tray.{session}")
}

/// A message as a line on the wire.
pub fn line<T: Serialize>(m: &T) -> String {
    let mut s = serde_json::to_string(m).unwrap_or_default();
    s.push('\n');
    s
}

/// The longest line either end takes: a list of icons with their pictures is well under this.
const MAX_LINE: usize = 8 << 20;

// --- Apron's end -----------------------------------------------------------------------------

/// What the server hears from one of its clients.
pub enum Heard {
    Ask(ToHost),
    /// That client went away.
    Gone,
}

struct Clients {
    next: u64,
    all: Vec<(u64, Sender<String>)>,
    /// The last list, for a client that connects later.
    latest: Option<String>,
}

/// The serving end. Sending never waits on a client: each has a thread of its own that writes.
#[derive(Clone)]
pub struct Server(Arc<Mutex<Clients>>);

impl Server {
    pub fn send(&self, m: &FromHost) {
        let l = line(m);
        let mut c = self.0.lock().unwrap();
        c.latest = Some(l.clone());
        c.all.retain(|(_, tx)| tx.send(l.clone()).is_ok());
    }
}

/// Serve on `name`, to processes of this user whose exe is called `client` ("lintel.exe").
/// `on` hears each client's messages, by a number for that client, on that client's own thread.
pub fn serve(name: String, client: &'static str, on: impl Fn(u64, Heard) + Send + Sync + 'static) -> Server {
    let server = Server(Arc::new(Mutex::new(Clients { next: 1, all: Vec::new(), latest: None })));
    let s = server.clone();
    let on = Arc::new(on);
    let _ = std::thread::Builder::new().name("tray-pipe".into()).spawn(move || {
        let mut first = true;
        loop {
            // The first one claims the name, so no other program can have taken it already.
            let Some(pipe) = (unsafe { instance(&name, first) }) else {
                std::thread::sleep(Duration::from_secs(5));
                continue;
            };
            first = false;
            if !unsafe { accept(&pipe) } {
                continue;
            }
            let mut pid = 0u32;
            unsafe { GetNamedPipeClientProcessId(pipe.0, &mut pid) };
            if !is_exe(pid, client) {
                unsafe { DisconnectNamedPipe(pipe.0) };
                continue;
            }
            let pipe = Arc::new(pipe);
            let (tx, rx) = channel::<String>();
            let id = {
                let mut c = s.0.lock().unwrap();
                let id = c.next;
                c.next += 1;
                if let Some(l) = &c.latest {
                    let _ = tx.send(l.clone());
                }
                c.all.push((id, tx));
                id
            };
            let w = pipe.clone();
            let _ = std::thread::Builder::new().name("tray-pipe-write".into()).spawn(move || {
                for l in rx {
                    if !unsafe { write(&w, l.as_bytes()) } {
                        break;
                    }
                }
            });
            let (s, on) = (s.clone(), on.clone());
            let _ = std::thread::Builder::new().name("tray-pipe-read".into()).spawn(move || {
                unsafe { read_lines(&pipe, |l| if let Ok(m) = serde_json::from_slice::<ToHost>(l) { on(id, Heard::Ask(m)) }) };
                // Its writer ends when its sender goes, and the pipe closes with the last of the two.
                s.0.lock().unwrap().all.retain(|(i, _)| *i != id);
                on(id, Heard::Gone);
            });
        }
    });
    server
}

// --- Lintel's end ----------------------------------------------------------------------------

/// What the client hears.
pub enum Event {
    /// Apron is there.
    Connected,
    Heard(FromHost),
    /// Apron went away (quit, or isn't running): it's looked for again every couple of seconds.
    Lost,
}

/// The way back to the server, while there is one.
#[derive(Clone, Default)]
pub struct Link(Arc<Mutex<Option<Sender<String>>>>);

impl Link {
    /// Send if connected; says whether it was.
    pub fn send(&self, m: &ToHost) -> bool {
        self.0.lock().unwrap().as_ref().is_some_and(|tx| tx.send(line(m)).is_ok())
    }

    pub fn connected(&self) -> bool {
        self.0.lock().unwrap().is_some()
    }
}

/// Keep connected to `name`, served by an exe called `server` ("apron.exe"), through `link`.
/// `on` hears what happens, on the connection's thread.
pub fn connect(name: String, server: &'static str, link: Link, on: impl Fn(Event) + Send + 'static) {
    let _ = std::thread::Builder::new().name("tray-link".into()).spawn(move || loop {
        let Some(pipe) = (unsafe { open(&name) }) else {
            std::thread::sleep(Duration::from_secs(2));
            continue;
        };
        let mut pid = 0u32;
        unsafe { GetNamedPipeServerProcessId(pipe.0, &mut pid) };
        if !is_exe(pid, server) {
            drop(pipe);
            std::thread::sleep(Duration::from_secs(5));
            continue;
        }
        let pipe = Arc::new(pipe);
        let (tx, rx) = channel::<String>();
        *link.0.lock().unwrap() = Some(tx);
        let w = pipe.clone();
        let _ = std::thread::Builder::new().name("tray-link-write".into()).spawn(move || {
            for l in rx {
                if !unsafe { write(&w, l.as_bytes()) } {
                    break;
                }
            }
        });
        on(Event::Connected);
        unsafe { read_lines(&pipe, |l| if let Ok(m) = serde_json::from_slice::<FromHost>(l) { on(Event::Heard(m)) }) };
        *link.0.lock().unwrap() = None;
        on(Event::Lost);
        std::thread::sleep(Duration::from_secs(1));
    });
}

/// Whether `pid` runs from an exe called `name` (any folder: an installed copy, or a build).
fn is_exe(pid: u32, name: &str) -> bool {
    pid != 0 && ffi::image_path(pid).is_some_and(|p| p.rsplit(['\\', '/']).next().is_some_and(|f| f.eq_ignore_ascii_case(name)))
}

// --- the pipe itself ---------------------------------------------------------------------------

struct Pipe(HANDLE);
// The handle is used from a reading and a writing thread at once, which overlapped I/O allows.
unsafe impl Send for Pipe {}
unsafe impl Sync for Pipe {}

impl Drop for Pipe {
    fn drop(&mut self) {
        unsafe { ffi::CloseHandle(self.0) };
    }
}

#[repr(C)]
struct OVERLAPPED {
    internal: usize,
    internal_high: usize,
    offset: u32,
    offset_high: u32,
    event: HANDLE,
}

#[repr(C)]
struct SECURITY_ATTRIBUTES {
    nLength: u32,
    lpSecurityDescriptor: *mut c_void,
    bInheritHandle: i32,
}

const PIPE_ACCESS_DUPLEX: u32 = 0x3;
const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;
const FILE_FLAG_FIRST_PIPE_INSTANCE: u32 = 0x0008_0000;
const PIPE_REJECT_REMOTE_CLIENTS: u32 = 0x8;
const PIPE_UNLIMITED_INSTANCES: u32 = 255;
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const OPEN_EXISTING: u32 = 3;
const ERROR_IO_PENDING: u32 = 997;
const ERROR_PIPE_CONNECTED: u32 = 535;
const ERROR_PIPE_BUSY: u32 = 231;
const ERROR_MORE_DATA: u32 = 234;
const INVALID_HANDLE: HANDLE = -1isize as HANDLE;
const TOKEN_USER: u32 = 1;

#[link(name = "kernel32", kind = "raw-dylib")]
extern "system" {
    fn CreateNamedPipeW(name: *const u16, open: u32, mode: u32, max: u32, out: u32, inb: u32, timeout: u32, sa: *const SECURITY_ATTRIBUTES) -> HANDLE;
    fn ConnectNamedPipe(pipe: HANDLE, ov: *mut OVERLAPPED) -> i32;
    fn DisconnectNamedPipe(pipe: HANDLE) -> i32;
    fn WaitNamedPipeW(name: *const u16, ms: u32) -> i32;
    fn GetNamedPipeClientProcessId(pipe: HANDLE, pid: *mut u32) -> i32;
    fn GetNamedPipeServerProcessId(pipe: HANDLE, pid: *mut u32) -> i32;
    fn CreateFileW(name: *const u16, access: u32, share: u32, sa: *const SECURITY_ATTRIBUTES, disposition: u32, flags: u32, template: HANDLE) -> HANDLE;
    fn ReadFile(file: HANDLE, buf: *mut u8, len: u32, read: *mut u32, ov: *mut OVERLAPPED) -> i32;
    fn WriteFile(file: HANDLE, buf: *const u8, len: u32, written: *mut u32, ov: *mut OVERLAPPED) -> i32;
    fn GetOverlappedResult(file: HANDLE, ov: *const OVERLAPPED, n: *mut u32, wait: i32) -> i32;
    fn CreateEventW(sa: *const SECURITY_ATTRIBUTES, manual: i32, initial: i32, name: *const u16) -> HANDLE;
    fn GetLastError() -> u32;
    fn LocalFree(mem: *mut c_void) -> *mut c_void;
    fn ProcessIdToSessionId(pid: u32, session: *mut u32) -> i32;
    fn GetCurrentProcessId() -> u32;
}

#[link(name = "advapi32", kind = "raw-dylib")]
extern "system" {
    fn GetTokenInformation(token: HANDLE, class: u32, info: *mut c_void, len: u32, ret_len: *mut u32) -> i32;
    fn ConvertSidToStringSidW(sid: *mut c_void, out: *mut *mut u16) -> i32;
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl: *const u16, revision: u32, sd: *mut *mut c_void, len: *mut u32) -> i32;
}

/// A security descriptor that lets only this user (this user's SID, not everyone signed in, and
/// not the default's "Everyone may read") open the pipe. Made once.
fn only_this_user() -> *mut c_void {
    static SD: OnceLock<usize> = OnceLock::new();
    *SD.get_or_init(|| unsafe {
        let mut token: HANDLE = null_mut();
        if ffi::OpenProcessToken(ffi::GetCurrentProcess(), ffi::TOKEN_QUERY, &mut token) == 0 {
            return 0;
        }
        let mut len = 0u32;
        GetTokenInformation(token, TOKEN_USER, null_mut(), 0, &mut len);
        let mut buf = vec![0u64; (len as usize).div_ceil(8)];
        let ok = GetTokenInformation(token, TOKEN_USER, buf.as_mut_ptr().cast(), len, &mut len) != 0;
        ffi::CloseHandle(token);
        if !ok {
            return 0;
        }
        // TOKEN_USER starts with the SID's pointer.
        let sid = *(buf.as_ptr() as *const *mut c_void);
        let mut text: *mut u16 = null_mut();
        if ConvertSidToStringSidW(sid, &mut text) == 0 {
            return 0;
        }
        let sid = ffi::from_wide_ptr(text);
        LocalFree(text.cast());
        let mut sd: *mut c_void = null_mut();
        let sddl = ffi::wide(&format!("D:P(A;;GA;;;{sid})"));
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), 1, &mut sd, null_mut()) == 0 {
            return 0;
        }
        // Kept for as long as the process runs.
        sd as usize
    }) as *mut c_void
}

unsafe fn instance(name: &str, first: bool) -> Option<Pipe> {
    let sd = only_this_user();
    if sd.is_null() {
        return None;
    }
    let sa = SECURITY_ATTRIBUTES { nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32, lpSecurityDescriptor: sd, bInheritHandle: 0 };
    let open = PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | if first { FILE_FLAG_FIRST_PIPE_INSTANCE } else { 0 };
    let h = CreateNamedPipeW(ffi::wide(name).as_ptr(), open, PIPE_REJECT_REMOTE_CLIENTS, PIPE_UNLIMITED_INSTANCES, 1 << 16, 1 << 16, 0, &sa);
    (h != INVALID_HANDLE && !h.is_null()).then_some(Pipe(h))
}

unsafe fn open(name: &str) -> Option<Pipe> {
    let n = ffi::wide(name);
    for _ in 0..2 {
        let h = CreateFileW(n.as_ptr(), GENERIC_READ | GENERIC_WRITE, 0, std::ptr::null(), OPEN_EXISTING, FILE_FLAG_OVERLAPPED, null_mut());
        if h != INVALID_HANDLE {
            return Some(Pipe(h));
        }
        // Every instance taken for a moment (another bar connecting): wait for one, once.
        if GetLastError() != ERROR_PIPE_BUSY || WaitNamedPipeW(n.as_ptr(), 2000) == 0 {
            break;
        }
    }
    None
}

/// Run one overlapped call to its end. `start` begins it with the OVERLAPPED it's given.
unsafe fn overlapped(pipe: &Pipe, start: impl FnOnce(*mut OVERLAPPED) -> i32) -> Result<u32, u32> {
    let event = CreateEventW(std::ptr::null(), 1, 0, std::ptr::null());
    if event.is_null() {
        return Err(0);
    }
    let mut ov = OVERLAPPED { internal: 0, internal_high: 0, offset: 0, offset_high: 0, event };
    let started = start(&mut ov);
    let err = if started == 0 { GetLastError() } else { 0 };
    let mut n = 0u32;
    let out = if started != 0 || err == ERROR_IO_PENDING {
        if GetOverlappedResult(pipe.0, &ov, &mut n, 1) != 0 {
            Ok(n)
        } else {
            Err(GetLastError())
        }
    } else {
        Err(err)
    };
    ffi::CloseHandle(event);
    out
}

/// Wait for a client on a new instance.
unsafe fn accept(pipe: &Pipe) -> bool {
    match overlapped(pipe, |ov| ConnectNamedPipe(pipe.0, ov)) {
        Ok(_) => true,
        Err(e) => e == ERROR_PIPE_CONNECTED,
    }
}

unsafe fn write(pipe: &Pipe, mut data: &[u8]) -> bool {
    while !data.is_empty() {
        let len = data.len().min(1 << 16) as u32;
        match overlapped(pipe, |ov| WriteFile(pipe.0, data.as_ptr(), len, null_mut(), ov)) {
            Ok(0) | Err(_) => return false,
            Ok(n) => data = &data[n as usize..],
        }
    }
    true
}

/// Read until the other end goes, handing each whole line (without its newline) to `each`.
unsafe fn read_lines(pipe: &Pipe, mut each: impl FnMut(&[u8])) {
    let mut buf = vec![0u8; 1 << 16];
    let mut pending: Vec<u8> = Vec::new();
    loop {
        let n = match overlapped(pipe, |ov| ReadFile(pipe.0, buf.as_mut_ptr(), buf.len() as u32, null_mut(), ov)) {
            Ok(0) => return,
            Ok(n) => n as usize,
            Err(e) if e == ERROR_MORE_DATA => buf.len(),
            Err(_) => return,
        };
        pending.extend_from_slice(&buf[..n]);
        while let Some(at) = pending.iter().position(|&b| b == b'\n') {
            let rest = pending.split_off(at + 1);
            each(&pending[..at]);
            pending = rest;
        }
        if pending.len() > MAX_LINE {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::RecvTimeoutError;

    #[test]
    fn messages_are_one_line_of_json() {
        let m = ToHost::Click { id: "1-7".into(), kind: "right".into(), rect: [10, 0, 34, 30] };
        let l = line(&m);
        assert_eq!(l, "{\"click\":{\"id\":\"1-7\",\"kind\":\"right\",\"rect\":[10,0,34,30]}}\n");
        assert_eq!(serde_json::from_str::<ToHost>(l.trim()).unwrap(), m);
        assert_eq!(line(&ToHost::Showing(true)), "{\"showing\":true}\n");
        // A field a later Apron adds, or one an earlier one lacks, doesn't break the list.
        let icons: FromHost = serde_json::from_str(r#"{"icons":[{"id":"a","tip":"Steam","new":1}]}"#).unwrap();
        assert_eq!(icons, FromHost::Icons(vec![Icon { id: "a".into(), tip: "Steam".into(), ..Default::default() }]));
    }

    #[test]
    fn the_pipe_carries_both_ways() {
        // This test's own exe stands in for both apps.
        let me = std::env::current_exe().unwrap();
        let exe: &'static str = Box::leak(me.file_name().unwrap().to_string_lossy().into_owned().into_boxed_str());
        let name = format!(r"\\.\pipe\Mullion.test.tray.{}", std::process::id());
        let (heard_tx, heard) = channel();
        let heard_tx = Mutex::new(heard_tx);
        let server = serve(name.clone(), exe, move |_, h| {
            if let Heard::Ask(m) = h {
                let _ = heard_tx.lock().unwrap().send(m);
            }
        });
        let list = FromHost::Icons(vec![Icon { id: "a".into(), tip: "x".repeat(200_000), ..Default::default() }]);
        // Sent before anyone's connected: a client gets it when it does.
        server.send(&list);
        let (got_tx, got) = channel();
        let link = Link::default();
        connect(name, exe, link.clone(), move |e| {
            let _ = got_tx.send(e);
        });
        let wait = Duration::from_secs(10);
        assert!(matches!(got.recv_timeout(wait), Ok(Event::Connected)));
        match got.recv_timeout(wait) {
            Ok(Event::Heard(m)) => assert_eq!(m, list),
            _ => panic!("no list"),
        }
        assert!(link.connected());
        let click = ToHost::Click { id: "a".into(), kind: "left".into(), rect: [0, 0, 1, 1] };
        assert!(link.send(&click));
        assert_eq!(heard.recv_timeout(wait), Ok(click));
        // Changes after it connected arrive too.
        let empty = FromHost::Icons(Vec::new());
        server.send(&empty);
        match got.recv_timeout(wait) {
            Ok(Event::Heard(m)) => assert_eq!(m, empty),
            _ => panic!("no change"),
        }
        assert!(matches!(got.recv_timeout(Duration::from_millis(200)), Err(RecvTimeoutError::Timeout)));
    }

    #[test]
    fn a_stranger_is_not_listened_to() {
        let name = format!(r"\\.\pipe\Mullion.test.tray.stranger.{}", std::process::id());
        let server = serve(name.clone(), "lintel.exe", |_, _| {});
        server.send(&FromHost::Icons(Vec::new()));
        // This test isn't lintel.exe: the server hangs up on it, before saying anything.
        let (got_tx, got) = channel();
        let me = std::env::current_exe().unwrap();
        let exe: &'static str = Box::leak(me.file_name().unwrap().to_string_lossy().into_owned().into_boxed_str());
        connect(name, exe, Link::default(), move |e| {
            let _ = got_tx.send(matches!(e, Event::Heard(_)));
        });
        let heard: Vec<bool> = std::iter::from_fn(|| got.recv_timeout(Duration::from_secs(3)).ok()).take(4).collect();
        assert!(!heard.contains(&true), "{heard:?}");
    }
}
