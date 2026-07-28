//! Admin detection and UAC elevation.
//!
//! Detects whether the current process is running with administrator
//! privileges, and provides a mechanism to relaunch the process with
//! elevated privileges via ShellExecuteExW with the "runas" verb.
//!
//! Design decisions per D-06 through D-09:
//! - D-06: ShellExecuteExW with runas verb triggers UAC prompt
//! - D-07: On UAC decline, the app continues in non-admin mode
//! - D-08: No state transfer — new process performs fresh scan
//! - D-09: Admin check runs once at startup; result persists

use anyhow::Context;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

/// Check whether the current process is running with administrator privileges.
///
/// Uses `IsUserAnAdmin()` from shell32.dll per D-09. Called once at startup;
/// result is cached in `App.is_admin` and `App.admin_check_done`.
pub fn is_admin() -> bool {
    unsafe { windows::Win32::UI::Shell::IsUserAnAdmin().as_bool() }
}

/// Relaunch the current process with elevated privileges.
///
/// Uses `ShellExecuteExW` with the "runas" verb (D-06). This triggers the
/// Windows UAC consent prompt. If the user approves, the current process
/// exits immediately (D-08: no state transfer) and a new elevated instance
/// starts. If the user declines, this function returns `Ok(())` (D-07) and
/// the app continues in non-admin mode.
///
/// # Errors
///
/// Returns an error if `ShellExecuteExW` fails for a reason other than the
/// user cancelling the UAC prompt (ERROR_CANCELLED = 1223).
pub fn elevate_to_admin() -> anyhow::Result<()> {
    let exe_path = std::env::current_exe()
        .context("Failed to get current executable path")?;

    // Convert the path to a wide string for Windows API
    let wide_path: Vec<u16> = OsStr::new(&exe_path)
        .encode_wide()
        .chain(std::iter::once(0)) // null terminator
        .collect();

    let verb: Vec<u16> = OsStr::new("runas")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut sei = windows::Win32::UI::Shell::SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<windows::Win32::UI::Shell::SHELLEXECUTEINFOW>() as u32,
        fMask: 64            // SEE_MASK_NOCLOSEPROCESS
             | 0x00100000,   // SEE_MASK_FLAG_NO_UI = 0x00100000
        nShow: 5,            // SW_SHOW
        lpVerb: windows::core::PCWSTR::from_raw(verb.as_ptr()),
        lpFile: windows::core::PCWSTR::from_raw(wide_path.as_ptr()),
        ..Default::default()
    };

    let success = unsafe { windows::Win32::UI::Shell::ShellExecuteExW(&mut sei) };

    if success.is_ok() {
        // User approved elevation. Old process exits immediately (D-08).
        // The new elevated process performs a fresh scan on startup.
        std::process::exit(0);
    } else {
        // Check if the user declined the UAC prompt (ERROR_CANCELLED = 1223)
        let error = unsafe { windows::Win32::Foundation::GetLastError() };
        let error_code = error.0 as u32;

        if error_code == 1223 {
            // User clicked "No" on UAC prompt — continue in non-admin mode (D-07)
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Elevation failed: ShellExecuteExW returned error code {}",
                error_code
            ))
        }
    }
}
