// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! sill-guard.exe: started by Sill when it hides the taskbar, with Sill's process id and exe. It
//! waits for that Sill to end, and if it ended without putting the taskbar back (it crashed, or
//! was ended from Task Manager), has `sill.exe --restore-taskbar` do it. It's a separate exe with
//! its own name so that ending every sill.exe doesn't end it too; it does nothing else.

use std::ffi::c_void;

const SYNCHRONIZE: u32 = 0x0010_0000;
const INFINITE: u32 = u32::MAX;

#[link(name = "kernel32", kind = "raw-dylib")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn WaitForSingleObject(handle: *mut c_void, ms: u32) -> u32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(pid), Some(sill)) = (args.get(1).and_then(|p| p.parse::<u32>().ok()), args.get(2)) else { return };
    unsafe {
        let p = OpenProcess(SYNCHRONIZE, 0, pid);
        if p.is_null() {
            return;
        }
        WaitForSingleObject(p, INFINITE);
        CloseHandle(p);
    }
    // Sill removes the note of how the taskbar was once it has put it back.
    let base = std::env::var_os("APPDATA").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    if base.join("Sill").join("taskbar-state").exists() {
        let _ = std::process::Command::new(sill).arg("--restore-taskbar").status();
    }
}
