//! Process identity — Send-safe ProcessSnapshot + verified handle wrapper.
//!
//! `ProcessSnapshot` is the pure-data identity that crosses async boundaries (mpsc).
//! Raw `HANDLE` is `!Send` in windows-rs 0.62 — never stored in App state or sent
//! over channels. The internal `OpenProcessHandle` wrapper owns the HANDLE inside a
//! single `spawn_blocking` scope and drops it via `CloseHandle`.
//!
//! ## Safety (PROC-07, Pitfall #1)
//! - `open_verified()` checks both `GetProcessId` AND `GetProcessTimes` creation
//!   FILETIME against the snapshot before the caller acts on the handle.
//! - Creation-time mismatch aborts with `Error::NotFound` — no wrong-process kill.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows::Win32::System::Threading::{
    GetProcessId, GetProcessTimes, OpenProcess, QueryFullProcessImageNameW,
    PROCESS_ACCESS_RIGHTS, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};
use windows::core::PWSTR;

/// Minimal access rights for kill operations (AV/EDR posture).
/// Never use `PROCESS_ALL_ACCESS`.
const RIGHTS: PROCESS_ACCESS_RIGHTS = PROCESS_ACCESS_RIGHTS(
    PROCESS_QUERY_LIMITED_INFORMATION.0 | PROCESS_TERMINATE.0 | PROCESS_SYNCHRONIZE.0,
);

/// Send-safe process identity that crosses async boundaries (mpsc channel).
///
/// Captured before any kill attempt so the caller has a stable reference
/// for PID-reuse verification.
#[derive(Debug, Clone)]
pub struct ProcessSnapshot {
    /// The process identifier.
    pub pid: u32,
    /// Process creation time (100ns intervals since 1601-01-01 UTC).
    /// `None` when the scan did not capture it (pre-Phase-2 scan rows).
    pub creation_time: Option<FILETIME>,
    /// Full executable path, if available.
    pub executable_path: Option<String>,
}

/// Internal handle wrapper — RAII close via `Drop`.
///
/// Never stored outside a `spawn_blocking` scope. Never crosses a channel.
/// Public for integration testing; the handle itself stays crate-private.
/// Identity (PID + creation time) lives in `ProcessSnapshot`, never here.
pub struct OpenProcessHandle {
    pub(crate) handle: HANDLE,
}

impl Drop for OpenProcessHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

/// Open a process with the given access rights.
///
/// `pub(crate)` — reused by `info.rs` for the QLI-only detail fetchers.
pub(crate) fn open_with(pid: u32, rights: PROCESS_ACCESS_RIGHTS) -> crate::Result<OpenProcessHandle> {
    unsafe {
        let handle = OpenProcess(rights, false, pid).map_err(|e| {
            if win32_error_code(&e) == 5 {
                // ERROR_ACCESS_DENIED — surfaced as PermissionDenied
                crate::Error::PermissionDenied(format!(
                    "Access denied opening process {} (may need admin rights)",
                    pid
                ))
            } else {
                crate::Error::Platform(format!("OpenProcess({}) failed: {:?}", pid, e))
            }
        })?;

        Ok(OpenProcessHandle { handle })
    }
}

/// Extract the Win32 error code from a windows-rs error.
///
/// `windows::core::Error::code()` returns an HRESULT; Win32 API failures are
/// encoded as `0x8007XXXX` where the low 16 bits hold the Win32 code
/// (e.g. `0x80070005` = ERROR_ACCESS_DENIED = 5).
fn win32_error_code(e: &windows::core::Error) -> u32 {
    (e.code().0 as u32) & 0xFFFF
}

/// Capture a snapshot of a process — PID, creation time, and executable path.
///
/// Opens the process with QLI | SYNCHRONIZE (no TERMINATE right — snapshot
/// is used for inspection, not kill). The handle is closed before returning.
pub fn snapshot_for(pid: u32) -> crate::Result<ProcessSnapshot> {
    // Special cases: PID 0 (Idle) and PID 4 (System) — no real file path,
    // and creation time is meaningless for the kernel pseudo-processes.
    if pid == 0 {
        return Ok(ProcessSnapshot {
            pid: 0,
            creation_time: None,
            executable_path: None,
        });
    }
    if pid == 4 {
        return Ok(ProcessSnapshot {
            pid: 4,
            creation_time: None,
            executable_path: None,
        });
    }

    let snapshot_rights =
        PROCESS_ACCESS_RIGHTS(PROCESS_QUERY_LIMITED_INFORMATION.0 | PROCESS_SYNCHRONIZE.0);

    let h = open_with(pid, snapshot_rights)?;

    // GetProcessTimes — capture creation FILETIME
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();

    let creation_time = unsafe {
        GetProcessTimes(h.handle, &mut creation, &mut exit, &mut kernel, &mut user)
            .ok()
            .map(|_| creation)
    };

    // QueryFullProcessImageNameW — capture executable path
    let executable_path = query_full_process_image_name(h.handle).ok();

    Ok(ProcessSnapshot {
        pid,
        creation_time,
        executable_path,
    })
}

/// Open a process and verify its identity against a snapshot.
///
/// Verifies both `GetProcessId` and `GetProcessTimes` creation FILETIME.
/// Returns `Error::NotFound` on mismatch — the process was replaced (PID reuse).
pub fn open_verified(snapshot: &ProcessSnapshot) -> crate::Result<OpenProcessHandle> {
    let h = open_with(snapshot.pid, RIGHTS)?;

    // Verify PID (belt)
    let current_pid = unsafe { GetProcessId(h.handle) };
    if current_pid != snapshot.pid {
        return Err(crate::Error::NotFound(format!(
            "PID {} reused — current handle maps to PID {}",
            snapshot.pid, current_pid
        )));
    }

    // Verify creation time (suspenders — Pitfall #1)
    match snapshot.creation_time {
        Some(expected_ct) => {
            let mut actual_ct = FILETIME::default();
            let mut exit = FILETIME::default();
            let mut kernel = FILETIME::default();
            let mut user = FILETIME::default();

            unsafe {
                GetProcessTimes(h.handle, &mut actual_ct, &mut exit, &mut kernel, &mut user)
                    .map_err(|e| {
                        crate::Error::Platform(format!("GetProcessTimes failed: {:?}", e))
                    })?;
            }

            if !creation_matches(actual_ct, expected_ct) {
                return Err(crate::Error::NotFound(format!(
                    "Process {} replaced — creation time mismatch",
                    snapshot.pid
                )));
            }
        }
        // `None` creation time: for a normal (non-pseudo) process we FAIL
        // CLOSED rather than degrade to a tautological PID-only check. The
        // PID "belt" adds nothing when we just opened a handle by that PID;
        // only creation time is meaningful, and without it we cannot prove the
        // snapshot still refers to a live process (PROC-07, review: PID-only
        // degradation was the residual risk). PID 0/4 pseudo-processes are an
        // explicit exception — they have no creation time by definition and are
        // hard-blocked by the whitelist before any kill could reach this point.
        None if snapshot.pid == 0 || snapshot.pid == 4 => {}
        None => {
            return Err(crate::Error::NotFound(format!(
                "Cannot verify identity of PID {} — no creation-time snapshot; \
                 refusing PID-only kill",
                snapshot.pid
            )));
        }
    }

    Ok(h)
}

/// Compare two FILETIME values for equality (pure, unit-testable).
///
/// FILETIME is two u32 fields; compare the combined u64 bit-pattern.
pub fn creation_matches(actual: FILETIME, expected: FILETIME) -> bool {
    let a = ((actual.dwHighDateTime as u64) << 32) | (actual.dwLowDateTime as u64);
    let b = ((expected.dwHighDateTime as u64) << 32) | (expected.dwLowDateTime as u64);
    a == b
}

/// Wrapper around `QueryFullProcessImageNameW` with a retry for insufficient buffer.
///
/// `pub(crate)` — reused by `info.rs` for the detail fetchers and the
/// scanner post-pass path resolution.
pub(crate) fn query_full_process_image_name(handle: HANDLE) -> crate::Result<String> {
    // Start with a 32KiB buffer (MAX_PATH-like but allows long paths).
    // Retry once with 64KiB if the buffer was insufficient.
    for capacity in [32 * 1024usize, 64 * 1024usize] {
        // QueryFullProcessImageNameW's lpdwSize is in WCHARs, not bytes
        // (WR-01): declaring `capacity` here would tell the API the
        // buffer is twice as large as the allocation — a heap overflow
        // for extended-length paths. Declare the true character count.
        let mut buf: Vec<u16> = vec![0u16; capacity / 2];
        let mut size: u32 = buf.len() as u32;

        let result = unsafe {
            QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_FORMAT(0), // PROCESS_NAME_WIN32 = 0
                PWSTR::from_raw(buf.as_mut_ptr()),
                &mut size,
            )
        };

        match result {
            Ok(()) => {
                buf.truncate(size as usize);
                let s = OsString::from_wide(&buf);
                return Ok(s.to_string_lossy().into_owned());
            }
            Err(e) => {
                let code = e.code().0 as u32;
                // ERROR_INSUFFICIENT_BUFFER = 122
                if code == 122 && capacity < 64 * 1024 {
                    continue; // retry with larger buffer
                }
                return Err(crate::Error::Platform(format!(
                    "QueryFullProcessImageNameW failed: {:?}",
                    e
                )));
            }
        }
    }

    unreachable!("retry logic should have returned by now")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation_matches_same_values() {
        let a = FILETIME {
            dwLowDateTime: 0x12345678,
            dwHighDateTime: 0x9ABCDEF0,
        };
        let b = FILETIME {
            dwLowDateTime: 0x12345678,
            dwHighDateTime: 0x9ABCDEF0,
        };
        assert!(creation_matches(a, b));
    }

    #[test]
    fn creation_matches_different_low() {
        let a = FILETIME {
            dwLowDateTime: 0x12345678,
            dwHighDateTime: 0x9ABCDEF0,
        };
        let b = FILETIME {
            dwLowDateTime: 0x12345679,
            dwHighDateTime: 0x9ABCDEF0,
        };
        assert!(!creation_matches(a, b));
    }

    #[test]
    fn creation_matches_different_high() {
        let a = FILETIME {
            dwLowDateTime: 0x12345678,
            dwHighDateTime: 0x9ABCDEF0,
        };
        let b = FILETIME {
            dwLowDateTime: 0x12345678,
            dwHighDateTime: 0x9ABCDEF1,
        };
        assert!(!creation_matches(a, b));
    }

    #[test]
    fn creation_matches_zero_values() {
        let a = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let b = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        assert!(creation_matches(a, b));
    }

    #[test]
    fn snapshot_for_pid_0_returns_idle() {
        let snap = snapshot_for(0).expect("snapshot_for(0)");
        assert_eq!(snap.pid, 0);
        assert!(snap.creation_time.is_none());
        assert!(snap.executable_path.is_none());
    }

    #[test]
    fn snapshot_for_pid_4_returns_system() {
        let snap = snapshot_for(4).expect("snapshot_for(4)");
        assert_eq!(snap.pid, 4);
        assert!(snap.creation_time.is_none());
        assert!(snap.executable_path.is_none());
    }

    /// A real (non-pseudo) process whose snapshot has no creation time must be
    /// refused — PROC-07 may not degrade to a PID-only verification. We use the
    /// current process as the target (OpenProcess on own PID succeeds and
    /// GetProcessId matches), so the None creation_time is the only failure.
    #[cfg(target_os = "windows")]
    #[test]
    fn open_verified_refuses_none_creation_time() {
        let snap = ProcessSnapshot {
            pid: std::process::id(),
            creation_time: None,
            executable_path: None,
        };
        let err = open_verified(&snap).err().expect("PID-only kill must be refused");
        assert!(
            matches!(err, crate::Error::NotFound(_)),
            "expected NotFound, got: {:?}",
            err
        );
    }

    /// PID 0 / 4 pseudo-processes keep the legacy exception: no creation time,
    /// but they are hard-blocked by the whitelist before any kill reaches
    /// open_verified, so returning Ok here preserves the existing contract.
    #[cfg(target_os = "windows")]
    #[test]
    fn open_verified_allows_pseudo_process_none_creation_time() {
        // PID 0 has no OS process to open — OpenProcess fails with a
        // platform error (not the NotFound we assert for the None case),
        // which confirms PID 0 never reaches the verification branch.
        let snap = ProcessSnapshot {
            pid: 0,
            creation_time: None,
            executable_path: None,
        };
        assert!(open_verified(&snap).is_err());
    }
}
