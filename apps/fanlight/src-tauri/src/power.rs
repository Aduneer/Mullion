//! The Lock, Sleep, Sign out, Restart and Shut down commands, as the Start menu does them.

use crate::ffi::*;
use std::ptr::null_mut;

pub fn act(what: &str) -> Result<(), String> {
    let ok = unsafe {
        match what {
            "lock" => LockWorkStation() != 0,
            // Sleep, not hibernate; apps may still keep the PC awake, as with the Start menu.
            "sleep" => SetSuspendState(0, 0, 0) != 0,
            "signout" => ExitWindowsEx(EWX_LOGOFF, SHTDN_REASON_PLANNED) != 0,
            "restart" => shutdown_privilege() && ExitWindowsEx(EWX_REBOOT, SHTDN_REASON_PLANNED) != 0,
            // With fast startup on, the Start menu's Shut down is a hybrid one; without it, that
            // flag is refused, so fall back to a full shutdown.
            "shutdown" => {
                shutdown_privilege()
                    && (ExitWindowsEx(EWX_SHUTDOWN | EWX_POWEROFF | EWX_HYBRID_SHUTDOWN, SHTDN_REASON_PLANNED) != 0
                        || ExitWindowsEx(EWX_SHUTDOWN | EWX_POWEROFF, SHTDN_REASON_PLANNED) != 0)
            }
            _ => return Err(format!("unknown power action: {what}")),
        }
    };
    if ok {
        Ok(())
    } else {
        Err(format!("Windows refused to {what} ({})", std::io::Error::last_os_error()))
    }
}

/// Restarting and shutting down need a privilege every user has but no program has switched on.
unsafe fn shutdown_privilege() -> bool {
    let mut token = null_mut();
    if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
        return false;
    }
    let name = wide("SeShutdownPrivilege");
    let mut tp = TOKEN_PRIVILEGES { PrivilegeCount: 1, Luid: LUID::default(), Attributes: SE_PRIVILEGE_ENABLED };
    let ok = LookupPrivilegeValueW(std::ptr::null(), name.as_ptr(), &mut tp.Luid) != 0
        && AdjustTokenPrivileges(token, 0, &tp, 0, null_mut(), null_mut()) != 0
        && std::io::Error::last_os_error().raw_os_error() == Some(0);
    CloseHandle(token);
    ok
}
