//! Sill hosts the notification area, beside Explorer rather than instead of it. It has a window of
//! the class programs look for (`Shell_TrayWnd`, kept above Explorer's so `FindWindow` finds it
//! first), so every program's `Shell_NotifyIcon` comes to Sill. Sill keeps its own table of the
//! icons and passes every message on to Explorer's tray unchanged, so Explorer's own taskbar and
//! flyout keep working and keep every icon, whatever happens to Sill.
//!
//! Every such call is a message a program sends and waits on, so the host's thread never waits
//! on anything itself: the icons are answered from the table at once and passed on to Explorer
//! from a thread of their own; the table is locked only to read or change it; and drawing the
//! icons, and telling the dock, happen elsewhere (`Event`). Only the app bar's messages (Sill's
//! own, Lintel's) wait for Explorer's answer, which they need, for at most a few seconds and not
//! at all when Explorer is hung.
//!
//! The table (`list`, `click`) is plain data, so another app can be given it later: Lintel's
//! right-hand side will ask Sill for it.

use crate::ffi::*;
use serde::Serialize;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

// --- what programs send ------------------------------------------------------------

/// NOTIFYICONDATAW as a 64-bit Explorer receives it: the handles are 32 bits wide, so it's the
/// same from 32- and 64-bit programs.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Nid {
    cb: u32,
    hwnd: u32,
    uid: u32,
    flags: u32,
    callback: u32,
    hicon: u32,
    tip: [u16; 128],
    state: u32,
    state_mask: u32,
    info: [u16; 256],
    version: u32,
    info_title: [u16; 64],
    info_flags: u32,
    guid: [u8; 16],
    balloon_icon: u32,
}

/// What `Shell_NotifyIcon` puts in its WM_COPYDATA (`dwData` 1).
#[repr(C)]
#[derive(Clone, Copy)]
struct TrayMessage {
    magic: u32,
    command: u32,
    nid: Nid,
}

const MAGIC: u32 = 0x3475_3423;
const NIM_ADD: u32 = 0;
const NIM_MODIFY: u32 = 1;
const NIM_DELETE: u32 = 2;
const NIM_SETFOCUS: u32 = 3;
const NIM_SETVERSION: u32 = 4;
const NIF_MESSAGE: u32 = 0x01;
const NIF_ICON: u32 = 0x02;
const NIF_TIP: u32 = 0x04;
const NIF_STATE: u32 = 0x08;
const NIF_GUID: u32 = 0x20;
const NIS_HIDDEN: u32 = 0x01;
const NIN_SELECT: u32 = WM_USER;
/// WM_COPYDATA's kind for a notify icon's message. (0 is an app bar's, 3 a request for an icon's
/// place: both go to Explorer as they are.)
const COPY_ICON: usize = 1;

/// One icon in the table.
#[derive(Clone, Copy)]
struct Entry {
    nid: Nid,
    pid: u32,
}

impl Entry {
    fn owner(&self) -> isize {
        self.nid.hwnd as i32 as isize
    }
    fn id(&self) -> String {
        if self.nid.flags & NIF_GUID != 0 && self.nid.guid != [0; 16] {
            self.nid.guid.iter().map(|b| format!("{b:02x}")).collect()
        } else {
            format!("{:x}-{}", self.nid.hwnd, self.nid.uid)
        }
    }
    fn same(&self, n: &Nid) -> bool {
        if n.flags & NIF_GUID != 0 && n.guid != [0; 16] && self.nid.flags & NIF_GUID != 0 {
            self.nid.guid == n.guid
        } else {
            self.nid.hwnd == n.hwnd && self.nid.uid == n.uid
        }
    }
}

/// An icon as the dock (and, later, Lintel) shows it.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct TrayIcon {
    /// Stays the same while the icon is there: its GUID, or its window and number.
    pub id: String,
    /// Its tooltip, which is usually its name and state ("Steam - synchronizing").
    pub tip: String,
    /// The program's file name, lowercased, as Windows' own settings list it.
    pub exe: String,
    #[serde(skip)]
    pub hicon: isize,
    /// Its picture, as a data: URL, filled in by whoever draws it.
    pub icon: Option<String>,
}

/// What the host tells Sill about, on the host's thread: answer at once, never wait.
pub enum Event {
    /// An icon came, changed or went.
    Icons,
    /// Explorer started again (it crashed, or was restarted): its taskbars are back, and app bars
    /// have to register again.
    Explorer,
    /// The host's 200 ms beat, for checks that must be quick (`taskbar::rehide`, full screen).
    Beat,
}

static TABLE: Mutex<Vec<Entry>> = Mutex::new(Vec::new());
static HOST: AtomicIsize = AtomicIsize::new(0);
static THREAD: AtomicU32 = AtomicU32::new(0);
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);
/// Set just before Sill broadcasts TaskbarCreated itself, so the host can tell that from Explorer's.
static OWN_BROADCAST: AtomicBool = AtomicBool::new(false);
/// While the icons re-add themselves after our broadcast: those adds aren't passed on, as Explorer
/// has them already (and some programs re-add without a tooltip, which would wipe Explorer's).
static QUIET_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);
static EVENTS: OnceLock<Box<dyn Fn(Event) + Send + Sync>> = OnceLock::new();
static FORWARD: OnceLock<Mutex<Sender<Forward>>> = OnceLock::new();

const QUIET: Duration = Duration::from_millis(2500);
const T_BEAT: usize = 1;
const T_PRUNE: usize = 2;
const T_AGAIN: usize = 3;
const T_REPLAY: usize = 4;

/// A message for Explorer's tray, passed on by the forwarding thread in the order they came.
struct Forward {
    kind: usize,
    from: usize,
    data: Vec<u8>,
}

/// Our host window, while there is one.
pub fn hwnd() -> isize {
    HOST.load(Ordering::Relaxed)
}

/// Explorer's own tray window (the taskbar), not ours.
pub fn explorer_tray() -> isize {
    let ours = hwnd();
    windows_of_class("Shell_TrayWnd").into_iter().find(|&h| h != ours).unwrap_or(0)
}

/// Start hosting, on a thread of its own. `events` runs on that thread, and must return at once.
pub fn start(events: impl Fn(Event) + Send + Sync + 'static) {
    let _ = EVENTS.set(Box::new(events));
    let (tx, rx) = channel::<Forward>();
    let _ = FORWARD.set(Mutex::new(tx));
    let _ = std::thread::Builder::new().name("tray-forward".into()).spawn(move || {
        for f in rx {
            send_to_explorer(f.kind, f.from, &f.data, 3000);
        }
    });
    let _ = std::thread::Builder::new().name("tray-host".into()).spawn(|| unsafe { run() });
}

/// Stop hosting: the host window goes, so programs find Explorer's tray again. Waits for it, a
/// second at most.
pub fn stop() {
    let thread = THREAD.load(Ordering::Relaxed);
    if thread == 0 {
        return;
    }
    unsafe { PostThreadMessageW(thread, WM_APP, 0, 0) };
    for _ in 0..50 {
        if hwnd() == 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn tell(e: Event) {
    if let Some(f) = EVENTS.get() {
        f(e);
    }
}

unsafe fn class(name: &str, proc_: WNDPROC) {
    let name = wide(name);
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
        // Classes live as long as the process.
        lpszClassName: name.leak().as_ptr(),
    };
    RegisterClassW(&wc);
}

unsafe fn run() {
    THREAD.store(GetCurrentThreadId(), Ordering::Relaxed);
    TASKBAR_CREATED.store(RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()), Ordering::Relaxed);
    class("Shell_TrayWnd", host_proc);
    class("TrayNotifyWnd", host_proc);
    let instance = GetModuleHandleW(std::ptr::null());
    // Never shown; topmost so it's above Explorer's in the z-order, where FindWindow looks first.
    let host = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
        wide("Shell_TrayWnd").as_ptr(),
        wide("").as_ptr(),
        WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
        0,
        0,
        0,
        0,
        0,
        0,
        instance,
        std::ptr::null(),
    );
    if host == 0 {
        THREAD.store(0, Ordering::Relaxed);
        return;
    }
    // Some programs look for the tray's child by name too.
    CreateWindowExW(0, wide("TrayNotifyWnd").as_ptr(), std::ptr::null(), WS_CHILD, 0, 0, 0, 0, host, 0, instance, std::ptr::null());
    HOST.store(host, Ordering::Relaxed);
    SetWindowPos(host, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    SetTimer(host, T_BEAT, 200, timer);
    SetTimer(host, T_PRUNE, 3000, timer);
    // Slow programs miss the first call to add their icons again; a second one catches them.
    SetTimer(host, T_AGAIN, 4000, timer);
    broadcast();

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, 0, 0, 0) > 0 {
        if msg.hwnd == 0 && msg.message == WM_APP {
            DestroyWindow(host);
            break;
        }
        DispatchMessageW(&msg);
    }
    HOST.store(0, Ordering::Relaxed);
    THREAD.store(0, Ordering::Relaxed);
}

/// Ask every program to add its icons again: they find our window first now.
fn broadcast() {
    *QUIET_UNTIL.lock().unwrap() = Some(Instant::now() + QUIET);
    OWN_BROADCAST.store(true, Ordering::Relaxed);
    unsafe { SendNotifyMessageW(HWND_BROADCAST, TASKBAR_CREATED.load(Ordering::Relaxed), 0, 0) };
}

fn quiet() -> bool {
    QUIET_UNTIL.lock().unwrap().is_some_and(|t| Instant::now() < t)
}

unsafe extern "system" fn timer(hwnd: isize, _msg: u32, id: usize, _time: u32) {
    match id {
        T_BEAT => {
            // Explorer raises its own tray now and then; ours has to stay in front of it.
            let class = wide("Shell_TrayWnd");
            if FindWindowW(class.as_ptr(), std::ptr::null()) != hwnd {
                SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
            tell(Event::Beat);
        }
        T_PRUNE => {
            // A program that ended without removing its icons.
            let gone = {
                let mut t = TABLE.lock().unwrap();
                let before = t.len();
                t.retain(|e| IsWindow(e.owner()) != 0);
                t.len() != before
            };
            if gone {
                tell(Event::Icons);
            }
        }
        T_AGAIN => {
            KillTimer(hwnd, T_AGAIN);
            broadcast();
        }
        T_REPLAY => {
            KillTimer(hwnd, T_REPLAY);
            replay();
        }
        _ => {}
    }
}

unsafe extern "system" fn host_proc(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize {
    let created = TASKBAR_CREATED.load(Ordering::Relaxed);
    if msg == created && created != 0 {
        if !OWN_BROADCAST.swap(false, Ordering::Relaxed) {
            // Explorer started again. Its tray is new and empty but for what programs add again by
            // themselves: give it our table once it's settled, and let Sill hide its taskbars and
            // register its app bar again.
            *QUIET_UNTIL.lock().unwrap() = None;
            SetTimer(hwnd, T_REPLAY, 1500, timer);
            tell(Event::Explorer);
        }
        return 0;
    }
    match msg {
        WM_COPYDATA if lparam != 0 => copy_data(wparam, &*(lparam as *const COPYDATASTRUCT)),
        WM_DESTROY => 0,
        // Whatever else programs send the tray (the taskbar's own messages), Explorer answers.
        m if (WM_USER..0xC000).contains(&m) => {
            let tray = explorer_tray();
            if tray == 0 {
                return 0;
            }
            let mut result = 0usize;
            SendMessageTimeoutW(tray, msg, wparam, lparam, SMTO_ABORTIFHUNG, 1000, &mut result);
            result as isize
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn copy_data(from: usize, c: &COPYDATASTRUCT) -> isize {
    if c.lpData.is_null() {
        return 0;
    }
    let data = std::slice::from_raw_parts(c.lpData as *const u8, c.cbData as usize);
    if c.dwData == COPY_ICON && data.len() >= std::mem::size_of::<TrayMessage>() {
        let m = std::ptr::read_unaligned(data.as_ptr() as *const TrayMessage);
        if m.magic == MAGIC {
            let (answer, changed) = note(m.command, &m.nid);
            if changed {
                tell(Event::Icons);
            }
            let swallowed = quiet() && (m.command == NIM_ADD || m.command == NIM_DELETE);
            if !swallowed {
                if let Some(tx) = FORWARD.get() {
                    let _ = tx.lock().unwrap().send(Forward { kind: c.dwData, from, data: data.to_vec() });
                }
            }
            return answer as isize;
        }
    }
    // App bars (Sill's and Lintel's) and icons' places: Explorer's answer is the answer.
    send_to_explorer(c.dwData, from, data, 3000)
}

fn send_to_explorer(kind: usize, from: usize, data: &[u8], ms: u32) -> isize {
    let tray = explorer_tray();
    if tray == 0 {
        return 0;
    }
    let c = COPYDATASTRUCT { dwData: kind, cbData: data.len() as u32, lpData: data.as_ptr() as *mut c_void };
    let mut result = 0usize;
    let ok = unsafe { SendMessageTimeoutW(tray, WM_COPYDATA, from, &c as *const COPYDATASTRUCT as isize, SMTO_ABORTIFHUNG, ms, &mut result) };
    if ok == 0 {
        0
    } else {
        result as isize
    }
}

/// Explorer started again: give its new tray every icon we know, from a thread of its own.
fn replay() {
    let all: Vec<Entry> = TABLE.lock().unwrap().clone();
    std::thread::spawn(move || {
        for e in all {
            let m = TrayMessage { magic: MAGIC, command: NIM_ADD, nid: e.nid };
            let bytes = unsafe { std::slice::from_raw_parts(&m as *const TrayMessage as *const u8, std::mem::size_of::<TrayMessage>()) };
            send_to_explorer(COPY_ICON, e.nid.hwnd as usize, bytes, 2000);
        }
    });
}

/// Put a message in the table. Returns what Explorer would answer, and whether the table changed.
fn note(command: u32, n: &Nid) -> (bool, bool) {
    let owner = n.hwnd as i32 as isize;
    let pid = pid_of(owner);
    // Explorer's own (the clock, volume, network) are Explorer's to show; Sill's own is Mullion's
    // business or its own tray icon's.
    if pid == std::process::id() || is_explorer(pid) {
        return (true, false);
    }
    let mut t = TABLE.lock().unwrap();
    let at = t.iter().position(|e| e.same(n));
    match command {
        NIM_ADD | NIM_MODIFY => {
            // A modify for an icon we've never seen (a program that missed our broadcast, running
            // elevated, say) is added: it's on Explorer's tray, so it should be on ours.
            if command == NIM_ADD && at.is_some() {
                return (false, false);
            }
            let mut e = at.map(|i| t[i]).unwrap_or(Entry { nid: Nid { flags: 0, state: 0, ..*n }, pid });
            merge(&mut e.nid, n);
            match at {
                Some(i) => t[i] = e,
                None => t.push(e),
            }
            (true, true)
        }
        NIM_DELETE => match at {
            Some(i) => {
                t.remove(i);
                (true, true)
            }
            None => (false, false),
        },
        NIM_SETVERSION => match at {
            Some(i) => {
                t[i].nid.version = n.version;
                (n.version <= 4, false)
            }
            None => (false, false),
        },
        NIM_SETFOCUS => (at.is_some(), false),
        _ => (false, false),
    }
}

/// Take the parts of `n` its flags say are there.
fn merge(e: &mut Nid, n: &Nid) {
    let f = n.flags;
    e.hwnd = n.hwnd;
    e.uid = n.uid;
    if f & NIF_MESSAGE != 0 {
        e.callback = n.callback;
    }
    if f & NIF_ICON != 0 {
        e.hicon = n.hicon;
    }
    if f & NIF_TIP != 0 {
        e.tip = n.tip;
    }
    if f & NIF_STATE != 0 {
        e.state = (e.state & !n.state_mask) | (n.state & n.state_mask);
    }
    if f & NIF_GUID != 0 {
        e.guid = n.guid;
    }
    e.flags |= f & (NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_STATE | NIF_GUID);
}

fn is_explorer(pid: u32) -> bool {
    static EXPLORERS: Mutex<Vec<(u32, bool)>> = Mutex::new(Vec::new());
    if let Some(&(_, yes)) = EXPLORERS.lock().unwrap().iter().find(|(p, _)| *p == pid) {
        return yes;
    }
    let yes = image_path(pid).is_some_and(|p| p.to_lowercase().ends_with(r"\explorer.exe"));
    let mut known = EXPLORERS.lock().unwrap();
    if known.len() > 64 {
        known.clear();
    }
    known.push((pid, yes));
    yes
}

/// The icons to show, in the order they came: not hidden, with a picture.
pub fn list() -> Vec<TrayIcon> {
    let t = TABLE.lock().unwrap().clone();
    t.iter()
        .filter(|e| e.nid.state & NIS_HIDDEN == 0 && e.nid.hicon != 0)
        .map(|e| TrayIcon {
            id: e.id(),
            tip: from_wide(&e.nid.tip),
            exe: image_path(e.pid).and_then(|p| p.rsplit('\\').next().map(str::to_lowercase)).unwrap_or_default(),
            hicon: e.nid.hicon as i32 as isize,
            icon: None,
        })
        .collect()
}

/// A click on an icon, as Explorer would pass it on: "left", "right" or "double".
pub fn click(id: &str, kind: &str) -> Result<(), String> {
    let e = TABLE.lock().unwrap().iter().find(|e| e.id() == id).copied().ok_or("that icon has gone")?;
    let owner = e.owner();
    if unsafe { IsWindow(owner) } == 0 {
        return Err("its program has gone".into());
    }
    let new = e.nid.version >= 3;
    let events: &[u32] = match kind {
        "left" if new => &[WM_LBUTTONDOWN, WM_LBUTTONUP, NIN_SELECT],
        "left" => &[WM_LBUTTONDOWN, WM_LBUTTONUP],
        "right" if new => &[WM_RBUTTONDOWN, WM_RBUTTONUP, WM_CONTEXTMENU],
        "right" => &[WM_RBUTTONDOWN, WM_RBUTTONUP],
        "double" => &[WM_LBUTTONDBLCLK, WM_LBUTTONUP],
        _ => return Err(format!("unknown click: {kind}")),
    };
    let mut at = POINT::default();
    unsafe {
        // The program opens its window or menu, and may bring it to the front.
        AllowSetForegroundWindow(pid_of(owner));
        GetCursorPos(&mut at);
    }
    for &event in events {
        let (wparam, lparam) = callback_params(e.nid.version, e.nid.uid, event, at);
        if unsafe { SendNotifyMessageW(owner, e.nid.callback, wparam, lparam) } == 0 {
            return Err(format!("Windows wouldn't pass the click on: {}", std::io::Error::last_os_error()));
        }
    }
    Ok(())
}

/// The process an icon belongs to.
pub fn pid(id: &str) -> Option<u32> {
    TABLE.lock().unwrap().iter().find(|e| e.id() == id).map(|e| e.pid)
}

/// The callback's wParam and lParam: version 4 packs the pointer and the icon's number in, older
/// ones the number alone.
fn callback_params(version: u32, uid: u32, event: u32, at: POINT) -> (usize, isize) {
    if version >= 4 {
        let point = ((at.y as u32 & 0xffff) << 16) | (at.x as u32 & 0xffff);
        (point as usize, (event | (uid << 16)) as isize)
    } else {
        (uid as usize, event as isize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nid(hwnd: u32, uid: u32, flags: u32, tip: &str) -> Nid {
        let mut n: Nid = unsafe { std::mem::zeroed() };
        n.hwnd = hwnd;
        n.uid = uid;
        n.flags = flags;
        for (i, c) in tip.encode_utf16().take(127).enumerate() {
            n.tip[i] = c;
        }
        n
    }

    #[test]
    fn the_message_is_laid_out_as_explorer_has_it() {
        assert_eq!(std::mem::size_of::<Nid>(), 956);
        assert_eq!(std::mem::size_of::<TrayMessage>(), 964);
    }

    #[test]
    fn changes_merge_only_what_they_carry() {
        let mut e = nid(1, 7, NIF_TIP | NIF_ICON, "Steam - synchronizing");
        e.hicon = 42;
        let mut m = nid(1, 7, NIF_STATE, "");
        m.state = NIS_HIDDEN;
        m.state_mask = NIS_HIDDEN;
        merge(&mut e, &m);
        assert_eq!((from_wide(&e.tip).as_str(), e.hicon, e.state), ("Steam - synchronizing", 42, NIS_HIDDEN));
        merge(&mut e, &nid(1, 7, NIF_TIP, "Steam"));
        assert_eq!(from_wide(&e.tip), "Steam");
    }

    #[test]
    fn icons_are_the_same_by_guid_or_window_and_number() {
        let e = Entry { nid: nid(1, 7, 0, ""), pid: 0 };
        assert!(e.same(&nid(1, 7, 0, "")) && !e.same(&nid(1, 8, 0, "")));
        let mut g = nid(1, 7, NIF_GUID, "");
        g.guid = [9; 16];
        let eg = Entry { nid: g, pid: 0 };
        assert!(eg.same(&Nid { hwnd: 2, uid: 1, ..g }));
        assert_eq!(eg.id(), "09".repeat(16));
        assert_eq!(e.id(), "1-7");
    }

    #[test]
    fn clicks_are_packed_by_version() {
        let at = POINT { x: 100, y: 1050 };
        assert_eq!(callback_params(4, 3, WM_LBUTTONUP, at), ((1050 << 16 | 100) as usize, (WM_LBUTTONUP | 3 << 16) as isize));
        assert_eq!(callback_params(0, 3, WM_LBUTTONUP, at), (3, WM_LBUTTONUP as isize));
        // A screen left of the main one: negative x stays in its 16 bits.
        let (w, _) = callback_params(4, 0, 0, POINT { x: -1, y: 0 });
        assert_eq!(w, 0xffff);
    }

    #[test]
    fn the_table_answers_as_explorer_would() {
        // A window that isn't ours or Explorer's (handle 0: no process).
        let n = nid(0x7fff_fff0, 1, NIF_TIP, "Test");
        assert_eq!(note(NIM_ADD, &n), (true, true));
        assert_eq!(note(NIM_ADD, &n), (false, false));
        assert_eq!(note(NIM_MODIFY, &n), (true, true));
        assert_eq!(note(NIM_DELETE, &n), (true, true));
        assert_eq!(note(NIM_DELETE, &n), (false, false));
        assert_eq!(note(NIM_SETVERSION, &n), (false, false));
    }
}
