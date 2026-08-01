//! Smart kill escalation pipeline (PROC-02, RESEARCH Pattern 2).
//!
//! Flow: protection gate → open+verify → route strategy → graceful signal →
//! timeout wait → force kill if needed.
//!
//! ## Execution (all inside one `spawn_blocking` scope):
//! 1. Re-read settings.toml (D-15) → protection_status (Pitfall #11: BEFORE OpenProcess)
//! 2. open_verified(&snapshot)
//! 3. GetExitCodeProcess != STILL_ACTIVE → AlreadyExited
//! 4. Probe: EnumWindows for visible windows → WM_CLOSE
//! 5. No windows → Ctrl+C helper (also serves as console probe):
//!    exit 0 = delivered → wait graceful timeout
//!    exit 1 = no console → force directly
//! 6. WaitForSingleObject(timeout) → Graceful or TerminateProcess → ForceKilled

use std::os::windows::process::CommandExt;
use std::process::Command;

use windows::Win32::Foundation::{
    HANDLE, HWND, LPARAM, WPARAM, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::core::BOOL;
use windows::Win32::System::Threading::{
    GetExitCodeProcess, GetProcessId, TerminateProcess, WaitForSingleObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowLongW, GetWindowThreadProcessId, IsWindowVisible, PostMessageW,
    GWL_EXSTYLE, WM_CLOSE, WS_EX_TOOLWINDOW,
};

use super::handle::{open_verified, ProcessSnapshot};
use super::whitelist::{protection_status, Protection};
use crate::config::load_settings;

/// `GetExitCodeProcess` returns `STILL_ACTIVE` (259) when the process is running.
const STILL_ACTIVE: u32 = 259;

// ----------------------------------------------------------------
// Pure data types
// ----------------------------------------------------------------

/// Graceful-kill strategy selected before execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// Send WM_CLOSE to a visible top-level window.
    WmClose,
    /// Deliver Ctrl+C to the target's console via helper process.
    ConsoleCtrlC,
    /// No graceful channel available — terminate immediately.
    ForceDirect,
}

/// Outcome of a kill attempt.
#[derive(Debug, Clone)]
pub enum KillOutcome {
    /// WM_CLOSE or Ctrl+C caused the process to exit within the timeout.
    Graceful,
    /// Graceful signal timed out; TerminateProcess succeeded.
    ForceKilled,
    /// Process had no GUI window or console; terminated immediately.
    Direct,
    /// GetExitCodeProcess != STILL_ACTIVE before any attempt.
    AlreadyExited,
    /// OpenProcess or TerminateProcess returned ERROR_ACCESS_DENIED.
    AccessDenied,
    /// Built-in whitelist blocks the kill — reason attached.
    HardBlocked(&'static str),
    /// Any other failure with a descriptive reason.
    Failed(String),
}

// ----------------------------------------------------------------
// Strategy routing (pure — unit-testable without Windows)
// ----------------------------------------------------------------

/// Route a kill strategy based on process characteristics.
///
/// Pure function. `has_visible_windows` comes from `EnumWindows` probing.
/// `has_console` comes from spawning the Ctrl+C helper and checking its
/// exit code (the helper serves as the probe since `AttachConsole` cannot
/// be called from a process that already owns a console).
pub fn route_strategy(has_visible_windows: bool, has_console: bool) -> Strategy {
    match (has_visible_windows, has_console) {
        (true, _) => Strategy::WmClose,
        (false, true) => Strategy::ConsoleCtrlC,
        (false, false) => Strategy::ForceDirect,
    }
}

// ----------------------------------------------------------------
// Kill pipeline (async entry → one spawn_blocking scope)
// ----------------------------------------------------------------

/// Terminate a process through the full escalation pipeline.
///
/// All Win32 calls run inside a single `tokio::task::spawn_blocking` scope.
/// `on_timeout` is called (in the blocking scope) when the graceful timeout
/// fires — the TUI uses this to update the status bar before the force kill.
pub async fn kill(
    snapshot: ProcessSnapshot,
    timeout_secs: u64,
    on_timeout: impl FnOnce() + Send + 'static,
) -> KillOutcome {
    tokio::task::spawn_blocking(move || kill_blocking(snapshot, timeout_secs, on_timeout))
        .await
        .unwrap_or_else(|e| KillOutcome::Failed(format!("spawn_blocking join error: {}", e)))
}

/// Synchronous kill logic — runs inside spawn_blocking.
fn kill_blocking(
    snapshot: ProcessSnapshot,
    timeout_secs: u64,
    on_timeout: impl FnOnce(),
) -> KillOutcome {
    // Step 1: Re-read settings.toml (D-15 — fresh, <1ms)
    let settings = match load_settings() {
        Ok(s) => s,
        Err(e) => return KillOutcome::Failed(format!("Failed to load settings: {}", e)),
    };

    // Extract basename from path for protection check
    let basename = snapshot
        .executable_path
        .as_deref()
        .and_then(|p| {
            std::path::Path::new(p)
                .file_name()
                .and_then(|f| f.to_str())
        })
        .unwrap_or("<unknown>");

    // Protection gate (Pitfall #11: BEFORE OpenProcess)
    let prot = protection_status(
        snapshot.pid,
        basename,
        snapshot.executable_path.as_deref(),
        &settings,
    );

    if let Protection::HardBlocked(reason) = prot {
        return KillOutcome::HardBlocked(reason);
    }

    // Step 2: Open + verify (PROC-07 creation-time check inside)
    let handle = match open_verified(&snapshot) {
        Ok(h) => h,
        Err(crate::Error::NotFound(msg)) => {
            return KillOutcome::Failed(format!("Process identity check failed: {}", msg));
        }
        Err(crate::Error::PermissionDenied(_)) => {
            return KillOutcome::AccessDenied;
        }
        Err(e) => {
            return KillOutcome::Failed(format!("OpenProcess failed: {}", e));
        }
    };

    // Step 3: Check if already exited
    let mut exit_code: u32 = 0;
    unsafe {
        if GetExitCodeProcess(handle.handle, &mut exit_code).is_ok() && exit_code != STILL_ACTIVE {
            return KillOutcome::AlreadyExited;
        }
    }

    let pid = snapshot.pid;

    // Step 4: Probe visible windows via EnumWindows. The probe is keyed on
    // the VERIFIED handle, not the raw PID (WR-02): the callback compares
    // the window's PID against GetProcessId(handle) — the handle keeps the
    // process object alive, so the numeric PID cannot be recycled and a
    // matching window is guaranteed to belong to the verified process.
    // WM_CLOSE is never posted to a PID-reused impostor.
    let has_visible_windows = has_visible_window(handle.handle);

    let graceful_dispatched = if has_visible_windows {
        // Send WM_CLOSE to the first visible top-level window.
        // UIPI may silently block cross-integrity — the timeout handles that.
        post_wm_close(handle.handle);
        true
    } else {
        // Step 5: Try the Ctrl+C helper (also serves as console probe).
        // Exit code 0 = delivered, 1 = no console → force directly.
        spawn_ctrl_c_helper(pid) == 0
    };

    if !graceful_dispatched {
        // No graceful channel — terminate immediately (ForceDirect path).
        let result = terminate_and_wait(handle.handle, pid);
        match result {
            KillOutcome::Direct => KillOutcome::Direct,
            other => other,
        }
    } else {
        // Step 6: Wait for graceful exit
        let timeout_ms = timeout_secs * 1000;
        unsafe {
            match WaitForSingleObject(handle.handle, timeout_ms as u32) {
                WAIT_OBJECT_0 => KillOutcome::Graceful,
                WAIT_TIMEOUT => {
                    on_timeout();
                    let result = terminate_and_wait(handle.handle, pid);
                    match result {
                        KillOutcome::Direct => KillOutcome::ForceKilled,
                        other => other,
                    }
                }
                _ => KillOutcome::Failed("WaitForSingleObject returned unexpected value".into()),
            }
        }
    }
}

// ----------------------------------------------------------------
// Terminate helper
// ----------------------------------------------------------------

/// Execute TerminateProcess and wait up to 3s for exit (Assumption A5).
fn terminate_and_wait(handle: HANDLE, pid: u32) -> KillOutcome {
    unsafe {
        match TerminateProcess(handle, 1) {
            Ok(()) => match WaitForSingleObject(handle, 3000) {
                WAIT_OBJECT_0 => KillOutcome::Direct,
                _ => KillOutcome::Failed(format!(
                    "Process {} did not exit after force kill",
                    pid
                )),
            },
            Err(e) => {
                // windows-rs errors carry Win32 codes as HRESULT 0x8007XXXX —
                // mask the low 16 bits to get ERROR_ACCESS_DENIED (5).
                let win32_code = (e.code().0 as u32) & 0xFFFF;
                if win32_code == 5 {
                    KillOutcome::AccessDenied
                } else {
                    KillOutcome::Failed(format!("TerminateProcess({}) failed: {:?}", pid, e))
                }
            }
        }
    }
}

// ----------------------------------------------------------------
// EnumWindows helpers (called inside spawn_blocking only)
// ----------------------------------------------------------------

/// Context for the enum_window_callback — passed via LPARAM.
///
/// Holds the VERIFIED process handle (WR-02): the callback compares the
/// window's PID against `GetProcessId(handle)`. The handle keeps the
/// process object alive, so the numeric PID cannot be recycled while it
/// is open — a window PID equal to `GetProcessId(handle)` belongs to the
/// verified process, never a PID-reused impostor.
struct WindowEnumCtx {
    handle: HANDLE,
    found: bool,
}

/// Callback for `EnumWindows` — checks if any visible top-level window
/// belongs to the verified process (handle-anchored, WR-02).
/// Skips tool windows (WS_EX_TOOLWINDOW).
///
/// Returns TRUE (1) to continue enumeration, FALSE (0) to stop.
unsafe extern "system" fn enum_window_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = unsafe { &mut *(lparam.0 as *mut WindowEnumCtx) };
    if ctx.found {
        return BOOL(0);
    }

    let mut window_pid: u32 = 0;
    unsafe {
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
    }

    if window_pid == GetProcessId(ctx.handle) {
        let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) };
        if (ex_style as u32 & WS_EX_TOOLWINDOW.0) == 0 {
            let visible = unsafe { IsWindowVisible(hwnd) };
            if visible.as_bool() {
                ctx.found = true;
                return BOOL(0);
            }
        }
    }

    BOOL(1)
}

/// Check if the verified process has any visible top-level window.
/// Skips tool windows (WS_EX_TOOLWINDOW).
fn has_visible_window(handle: HANDLE) -> bool {
    let mut ctx = WindowEnumCtx {
        handle,
        found: false,
    };

    unsafe {
        let _ = EnumWindows(
            Some(enum_window_callback),
            LPARAM(&mut ctx as *mut WindowEnumCtx as isize),
        );
    }

    ctx.found
}

/// Context for the post_wm_close_callback.
struct PostWmCloseCtx {
    handle: HANDLE,
    done: bool,
}

/// Callback for `EnumWindows` — sends WM_CLOSE to the first visible
/// top-level window of the verified process (handle-anchored, WR-02).
///
/// Returns TRUE (1) to continue, FALSE (0) to stop after posting.
unsafe extern "system" fn post_wm_close_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = unsafe { &mut *(lparam.0 as *mut PostWmCloseCtx) };
    if ctx.done {
        return BOOL(0);
    }

    let mut window_pid: u32 = 0;
    unsafe {
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
    }

    if window_pid == GetProcessId(ctx.handle) {
        let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) };
        if (ex_style as u32 & WS_EX_TOOLWINDOW.0) == 0 {
            let visible = unsafe { IsWindowVisible(hwnd) };
            if visible.as_bool() {
                unsafe {
                    let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
                ctx.done = true;
                return BOOL(0);
            }
        }
    }

    BOOL(1)
}

/// Send WM_CLOSE to the first visible top-level window of the verified
/// process. A window whose PID does not match `GetProcessId(handle)` is
/// never signaled (WR-02 — no signaling of PID-reused impostors).
fn post_wm_close(handle: HANDLE) {
    let mut ctx = PostWmCloseCtx {
        handle,
        done: false,
    };

    unsafe {
        let _ = EnumWindows(
            Some(post_wm_close_callback),
            LPARAM(&mut ctx as *mut PostWmCloseCtx as isize),
        );
    }
}

// ----------------------------------------------------------------
// Ctrl+C helper
// ----------------------------------------------------------------

/// Spawn the Ctrl+C helper process.
///
/// Primary: port-core's own `ctrl_c_helper` binary (WR-03), located next
/// to the calling executable — cargo builds all workspace binaries into
/// the same target dir, so dev and packaged builds find it. This keeps
/// the kill pipeline frontend-agnostic: any binary linking port-core
/// delivers Ctrl+C correctly instead of re-executing itself with an
/// undocumented flag only the TUI understands.
///
/// Fallback: re-exec `current_exe` with `--ctrl-c <pid>` — the TUI
/// implements this internal mode; preserved for single-crate builds
/// (`cargo run -p port-tui`) where the helper binary is not built.
///
/// Returns the helper's exit code: 0 = delivered, 1 = no console,
/// 2 = delivery failed, other = error. The helper uses
/// `CREATE_NO_WINDOW` (0x08000000) so it never flashes.
///
/// **Caveats** (documented):
/// - Ctrl+C broadcasts to ALL processes on the target's console.
/// - Processes created with `CREATE_NEW_PROCESS_GROUP` ignore Ctrl+C.
/// - A pending `ReadConsole` may not be interrupted.
/// - "Delivered" does not guarantee "exited" — the WaitForSingleObject timeout
///   is the real arbiter (Pitfall 4).
fn spawn_ctrl_c_helper(pid: u32) -> i32 {
    // Primary: the port-core helper binary as a sibling of the calling exe.
    // Candidate 1: same dir as the caller (target/debug layout).
    // Candidate 2: parent dir — integration tests run from
    //   target/debug/deps/, where the helper lives one level up.
    let helper_name = if cfg!(windows) {
        "ctrl_c_helper.exe"
    } else {
        "ctrl_c_helper"
    };
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        candidates.push(exe.with_file_name(helper_name));
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join(helper_name));
        }
    }
    for helper in candidates {
        if helper.is_file() {
            match Command::new(&helper)
                .arg("--ctrl-c")
                .arg(pid.to_string())
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .status()
            {
                Ok(status) => return status.code().unwrap_or(1),
                Err(_) => break, // fall through to the re-exec fallback
            }
        }
    }

    // Fallback: re-exec current_exe with the internal --ctrl-c mode.
    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return 1,
    };

    match Command::new(current_exe)
        .arg("--ctrl-c")
        .arg(pid.to_string())
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .status()
    {
        Ok(status) => status.code().unwrap_or(1),
        Err(_) => 1,
    }
}

// ----------------------------------------------------------------
// Tests
// ----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_wmclose_when_has_windows() {
        assert_eq!(route_strategy(true, true), Strategy::WmClose);
        assert_eq!(route_strategy(true, false), Strategy::WmClose);
    }

    #[test]
    fn route_consolectrl_when_console_only() {
        assert_eq!(route_strategy(false, true), Strategy::ConsoleCtrlC);
    }

    #[test]
    fn route_forcedirect_when_neither() {
        assert_eq!(route_strategy(false, false), Strategy::ForceDirect);
    }

    #[test]
    fn route_strategy_full_matrix() {
        let cases = [
            ((true, true), Strategy::WmClose),
            ((true, false), Strategy::WmClose),
            ((false, true), Strategy::ConsoleCtrlC),
            ((false, false), Strategy::ForceDirect),
        ];

        for ((w, c), expected) in cases {
            assert_eq!(
                route_strategy(w, c),
                expected,
                "route_strategy({}, {}) returned wrong variant",
                w,
                c
            );
        }
    }

    #[test]
    fn kill_outcome_debug_formatting() {
        let outcomes = [
            KillOutcome::Graceful,
            KillOutcome::ForceKilled,
            KillOutcome::Direct,
            KillOutcome::AlreadyExited,
            KillOutcome::AccessDenied,
            KillOutcome::HardBlocked("test reason"),
            KillOutcome::Failed("test error".into()),
        ];
        for o in &outcomes {
            let _ = format!("{:?}", o);
        }
    }
}
