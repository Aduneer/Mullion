//! Hand-written bindings for the flat Win32 / NT calls only Oriel uses. The ones another app uses
//! too are Mullion's core's, and come in with the first line. COM (DXGI) goes through the
//! `windows` crate instead.
#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms)]

pub use mullion_core::ffi::*;

use std::ffi::c_void;
use std::ptr::null_mut;

// --- NT process list -----------------------------------------------------------

pub const SYSTEM_PROCESS_INFORMATION_CLASS: u32 = 5;
pub const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;

#[repr(C)]
pub struct UNICODE_STRING {
    pub Length: u16,
    pub MaximumLength: u16,
    pub Buffer: *const u16,
}

/// Leading part of SYSTEM_PROCESS_INFORMATION; we never read past InheritedFromUniqueProcessId.
#[repr(C)]
pub struct SYSTEM_PROCESS_INFORMATION {
    pub NextEntryOffset: u32,
    pub NumberOfThreads: u32,
    pub WorkingSetPrivateSize: i64,
    pub HardFaultCount: u32,
    pub NumberOfThreadsHighWatermark: u32,
    pub CycleTime: u64,
    pub CreateTime: i64,
    pub UserTime: i64,
    pub KernelTime: i64,
    pub ImageName: UNICODE_STRING,
    pub BasePriority: i32,
    pub UniqueProcessId: usize,
    pub InheritedFromUniqueProcessId: usize,
}

#[link(name = "ntdll", kind = "raw-dylib")]
extern "system" {
    pub fn NtQuerySystemInformation(class: u32, info: *mut c_void, len: u32, ret_len: *mut u32) -> i32;
}

// --- kernel32 / advapi32 -------------------------------------------------------

#[repr(C)]
#[derive(Default)]
pub struct PERFORMANCE_INFORMATION {
    pub cb: u32,
    pub CommitTotal: usize,
    pub CommitLimit: usize,
    pub CommitPeak: usize,
    pub PhysicalTotal: usize,
    pub PhysicalAvailable: usize,
    pub SystemCache: usize,
    pub KernelTotal: usize,
    pub KernelPaged: usize,
    pub KernelNonpaged: usize,
    pub PageSize: usize,
    pub HandleCount: u32,
    pub ProcessCount: u32,
    pub ThreadCount: u32,
}

pub const DRIVE_FIXED: u32 = 3;

pub const PROCESS_TERMINATE: u32 = 0x0001;
pub const ERROR_ACCESS_DENIED: i32 = 5;

/// 100 ns ticks since 1601 (UTC), in two halves.
#[repr(C)]
#[derive(Default)]
pub struct FILETIME {
    pub lo: u32,
    pub hi: u32,
}

impl FILETIME {
    pub fn ticks(&self) -> i64 {
        ((self.hi as i64) << 32) | self.lo as i64
    }
}

#[link(name = "kernel32", kind = "raw-dylib")]
extern "system" {
    pub fn GetProcessTimes(process: HANDLE, created: *mut FILETIME, exited: *mut FILETIME, kernel: *mut FILETIME, user: *mut FILETIME) -> i32;
    pub fn TerminateProcess(process: HANDLE, exit_code: u32) -> i32;
    pub fn K32GetPerformanceInfo(info: *mut PERFORMANCE_INFORMATION, cb: u32) -> i32;
    pub fn GetTickCount64() -> u64;
    pub fn GetLogicalDriveStringsW(len: u32, buf: *mut u16) -> u32;
    pub fn GetDriveTypeW(root: *const u16) -> u32;
    pub fn GetDiskFreeSpaceExW(dir: *const u16, avail: *mut u64, total: *mut u64, free: *mut u64) -> i32;
    pub fn GetLogicalProcessorInformationEx(relation: u32, buf: *mut c_void, len: *mut u32) -> i32;
}

/// HKEY_LOCAL_MACHINE is a sign-extended 32-bit constant.
const HKEY_LOCAL_MACHINE: isize = 0x8000_0002_u32 as i32 as isize;
const RRF_RT_REG_SZ: u32 = 0x0000_0002;
const RRF_RT_REG_DWORD: u32 = 0x0000_0010;

#[link(name = "advapi32", kind = "raw-dylib")]
extern "system" {
    fn RegGetValueW(
        hkey: isize,
        subkey: *const u16,
        value: *const u16,
        flags: u32,
        ty: *mut u32,
        data: *mut c_void,
        len: *mut u32,
    ) -> i32;
}

// --- helpers -------------------------------------------------------------------

/// A string value under HKEY_LOCAL_MACHINE.
pub fn reg_string(path: &str, value: &str) -> Option<String> {
    let (p, v) = (wide(path), wide(value));
    let mut len = 0u32;
    unsafe {
        if RegGetValueW(HKEY_LOCAL_MACHINE, p.as_ptr(), v.as_ptr(), RRF_RT_REG_SZ, null_mut(), null_mut(), &mut len) != 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize / 2 + 1];
        if RegGetValueW(HKEY_LOCAL_MACHINE, p.as_ptr(), v.as_ptr(), RRF_RT_REG_SZ, null_mut(), buf.as_mut_ptr().cast(), &mut len) != 0 {
            return None;
        }
        Some(from_wide(&buf).trim().to_string())
    }
}

pub fn reg_dword(path: &str, value: &str) -> Option<u32> {
    let (p, v) = (wide(path), wide(value));
    let mut data = 0u32;
    let mut len = 4u32;
    let ok = unsafe {
        RegGetValueW(HKEY_LOCAL_MACHINE, p.as_ptr(), v.as_ptr(), RRF_RT_REG_DWORD, null_mut(), (&mut data as *mut u32).cast(), &mut len)
    } == 0;
    ok.then_some(data)
}

pub fn physical_cores() -> usize {
    const RELATION_PROCESSOR_CORE: u32 = 0;
    let mut len = 0u32;
    unsafe {
        GetLogicalProcessorInformationEx(RELATION_PROCESSOR_CORE, null_mut(), &mut len);
        let mut buf = vec![0u8; len as usize];
        if GetLogicalProcessorInformationEx(RELATION_PROCESSOR_CORE, buf.as_mut_ptr().cast(), &mut len) == 0 {
            return 0;
        }
        // Variable-size records: { Relationship: u32, Size: u32, ... }
        let (mut off, mut n) = (0usize, 0usize);
        while off + 8 <= len as usize {
            let size = u32::from_ne_bytes(buf[off + 4..off + 8].try_into().unwrap()) as usize;
            if size == 0 {
                break;
            }
            n += 1;
            off += size;
        }
        n
    }
}
