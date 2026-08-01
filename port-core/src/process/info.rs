//! Process detail fetchers — executable path, command line, start time,
//! parent PID, and digital signature (PROC-06).
//!
//! All Win32 calls run inside `spawn_blocking` scopes (Pitfall #9).
//! Every per-field fetcher returns `Option` — a failure renders "—" in the
//! detail panel (UI-SPEC Detail Panel States), never a whole-panel error.
//!
//! ## Command line (PROC-06)
//! `NtQueryInformationProcess` class 60 (two-call pattern, RESEARCH Code
//! Example 3). Requires only PROCESS_QUERY_LIMITED_INFORMATION — works
//! against elevated processes (psutil/wazuh production pattern since 2019).
//! Undocumented but stable on Win 8.1+ (Assumption A6); failure renders "—".
//!
//! ## Start time identity (D-08, PROC-07)
//! Start time derives from the GetProcessTimes creation FILETIME — the same
//! identity the kill verifies against. The kill re-captures it FRESH via
//! `snapshot_for(pid)` at kill time (plan 02-01 Task 2); this module never
//! caches the FILETIME, so the verified identity is never a stale panel copy.
//!
//! ## Signature (D-07)
//! WinVerifyTrustEx with WTD_CACHE_ONLY_URL_RETRIEVAL (A8: offline
//! cache-first — no network hang) and WTD_STATEACTION_CLOSE cleanup
//! (Pitfall 5). "Signed" = any valid signature chain, NOT trusted-publisher
//! semantics (RESEARCH Open Question 3 resolution).

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr;

use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, HWND};
use windows::Win32::Security::WinTrust::{
    WinVerifyTrustEx, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA,
    WINTRUST_DATA_0, WINTRUST_DATA_UICONTEXT, WINTRUST_FILE_INFO,
    WTD_CACHE_ONLY_URL_RETRIEVAL, WTD_CHOICE_FILE, WTD_REVOKE_NONE,
    WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY, WTD_UI_NONE,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
    TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    GetProcessTimes, PROCESS_ACCESS_RIGHTS, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_SYNCHRONIZE,
};
use windows::core::PCWSTR;

use crate::config::{default_settings, load_settings};
use crate::models::ProcessInfo;
use crate::process::handle;
use crate::process::whitelist::{protection_status, Protection};

/// NTSTATUS: the supplied buffer was too small.
const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004u32 as i32;
/// NtQueryInformationProcess class: process command line (Win 8.1+).
const PROCESS_COMMAND_LINE_INFORMATION: u32 = 60;
/// TRUST_E_NOSIGNATURE — the file has no signature.
const TRUST_E_NOSIGNATURE: u32 = 0x800B_0100;

/// Manual FFI to ntdll — NtQueryInformationProcess is not in the windows
/// crate (RESEARCH Code Example 3; T-02-SC: linked from the OS, not a crate).
#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQueryInformationProcess(
        processhandle: *mut core::ffi::c_void,
        processinformationclass: u32,
        processinformation: *mut core::ffi::c_void,
        processinformationlength: u32,
        returnlength: *mut u32,
    ) -> i32; // NTSTATUS
}

/// Layout of the UNICODE_STRING written by NtQueryInformationProcess class 60.
#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *mut u16,
}

/// Convert a FILETIME (100ns intervals since 1601-01-01 UTC) to a SystemTime.
///
/// Pure helper (RESEARCH Code Example 6):
/// `value = (dwHighDateTime << 32) | dwLowDateTime`;
/// `secs = value / 10_000_000 saturating_sub 11_644_473_600` (1601→1970).
/// The epoch case: FILETIME 116444736000000000 → 1970-01-01T00:00:00Z.
/// Returns `Some` always (a pre-epoch FILETIME saturates to the epoch);
/// the `Option` shape matches the per-field-failure contract.
pub fn filetime_to_systemtime(ft: FILETIME) -> Option<std::time::SystemTime> {
    let value = ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64;
    let secs = (value / 10_000_000).saturating_sub(11_644_473_600);
    Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
}

/// Defensively extract a wide string from a caller-owned buffer.
///
/// `length_bytes` is the UNICODE_STRING `Length` field (bytes). Defensive
/// bounds (RESEARCH Code Example 3 / Assumption A6): if the claimed length
/// exceeds the allocation's byte capacity, the Buffer pointer lies outside
/// the allocation — return `None` rather than reading out of bounds
/// (T-02-09). Trailing NULs are dropped; embedded NULs are preserved.
pub fn extract_unicode_string(buffer: &[u16], length_bytes: u32) -> Option<String> {
    let claimed_chars = (length_bytes as usize) / 2;
    if claimed_chars > buffer.len() {
        return None;
    }
    let mut end = claimed_chars;
    while end > 0 && buffer[end - 1] == 0 {
        end -= 1;
    }
    Some(String::from_utf16_lossy(&buffer[..end]))
}

/// Resolve the full executable path for a PID (~1ms, RESEARCH Pattern 4).
///
/// Used by the scanner post-pass: only same-basename processes pay the
/// path query. PID 0/4 (kernel pseudo-processes) have no file path.
pub(crate) fn query_full_path(pid: u32) -> Option<String> {
    if pid == 0 || pid == 4 {
        return None;
    }
    let h = handle::open_with(pid, PROCESS_QUERY_LIMITED_INFORMATION).ok()?;
    handle::query_full_process_image_name(h.handle).ok()
}

/// Fetch the full detail set for a process (PROC-06, D-08).
///
/// One `spawn_blocking` scope (Pitfall #9). Per-field failures return `None`
/// (the panel renders "—") — never a whole-panel `Err`. The async wrapper
/// only errors if the blocking scope itself cannot run.
pub async fn fetch_details(pid: u32) -> crate::Result<ProcessInfo> {
    tokio::task::spawn_blocking(move || fetch_details_blocking(pid))
        .await
        .map_err(|e| crate::Error::Platform(format!("spawn_blocking join error: {}", e)))?
}

fn fetch_details_blocking(pid: u32) -> crate::Result<ProcessInfo> {
    // PID 0/4 are kernel pseudo-processes — no handle, no path, no start time.
    if pid == 0 || pid == 4 {
        return Ok(ProcessInfo {
            pid,
            name: String::new(),
            executable_path: None,
            command_line: None,
            start_time: None,
            is_signed: None,
            is_system_critical: true,
            user_protected: false,
            parent_pid: None,
        });
    }

    let h = handle::open_with(
        pid,
        PROCESS_ACCESS_RIGHTS(PROCESS_QUERY_LIMITED_INFORMATION.0 | PROCESS_SYNCHRONIZE.0),
    )?;

    // 1. Executable path — QueryFullProcessImageNameW (size-in/out, retry).
    let executable_path = handle::query_full_process_image_name(h.handle).ok();

    // 2. Start time — GetProcessTimes creation FILETIME (D-08, PROC-07).
    //    The kill re-captures this identity FRESH via snapshot_for(pid) at
    //    kill time; the panel never caches the FILETIME.
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let start_time = unsafe {
        GetProcessTimes(h.handle, &mut creation, &mut exit, &mut kernel, &mut user)
            .ok()
            .and_then(|_| filetime_to_systemtime(creation))
    };

    // 3. Command line — NtQueryInformationProcess class 60 (two-call).
    let command_line = query_command_line(h.handle);

    // 4. Parent PID — Toolhelp32 snapshot.
    let parent_pid = query_parent_pid(pid);

    // 5. Name — prefer the path basename; empty when unknown (the caller's
    //    row name wins in that case).
    let name = executable_path
        .as_deref()
        .and_then(|p| std::path::Path::new(p).file_name())
        .and_then(|f| f.to_str())
        .map(|s| s.to_string())
        .unwrap_or_default();

    // 6. Protection markers — the same whitelist logic the kill gate uses
    //    (D-15: fresh settings; built-in tier first, Pitfall #6).
    let settings = load_settings().unwrap_or_else(|_| default_settings());
    let basename = if name.is_empty() {
        "<unknown>"
    } else {
        name.as_str()
    };
    let (is_system_critical, user_protected) = match protection_status(
        pid,
        basename,
        executable_path.as_deref(),
        &settings,
    ) {
        Protection::HardBlocked(_) => (true, false),
        Protection::UserConfirm => (false, true),
        Protection::None => (false, false),
    };

    Ok(ProcessInfo {
        pid,
        name,
        executable_path,
        command_line,
        start_time,
        is_signed: None, // populated on demand via verify_signature (D-07)
        is_system_critical,
        user_protected,
        parent_pid,
    })
}

/// Retrieve the process command line via NtQueryInformationProcess class 60.
///
/// Two-call pattern (RESEARCH Code Example 3): the first call with a null
/// buffer returns STATUS_INFO_LENGTH_MISMATCH and the required size; the
/// second call writes a UNICODE_STRING whose Buffer points INTO the caller's
/// allocation (Win 8.1+). The allocation is bounds-checked before any
/// dereference (T-02-09) — a corrupt return cannot read out of bounds.
/// Requires only PROCESS_QUERY_LIMITED_INFORMATION — works against elevated
/// processes. Failure (access denied / invalid NTSTATUS) -> None.
fn query_command_line(process_handle: HANDLE) -> Option<String> {
    let handle_ptr = process_handle.0 as *mut core::ffi::c_void;
    let mut needed: u32 = 0;

    let status = unsafe {
        NtQueryInformationProcess(
            handle_ptr,
            PROCESS_COMMAND_LINE_INFORMATION,
            ptr::null_mut(),
            0,
            &mut needed,
        )
    };
    if status != STATUS_INFO_LENGTH_MISMATCH || needed == 0 {
        return None;
    }

    let mut buffer: Vec<u16> = vec![0u16; (needed as usize + 1) / 2];
    let mut written: u32 = 0;
    let status = unsafe {
        NtQueryInformationProcess(
            handle_ptr,
            PROCESS_COMMAND_LINE_INFORMATION,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
            (buffer.len() * 2) as u32,
            &mut written,
        )
    };
    if status != 0 {
        return None;
    }

    // The returned UNICODE_STRING sits at the start of our buffer; its Buffer
    // points into this same allocation (Win 8.1+). read_unaligned: the
    // buffer is only u16-aligned, the struct contains a pointer field.
    if (written as usize) < std::mem::size_of::<UnicodeString>() {
        return None;
    }
    let us: UnicodeString = unsafe { ptr::read_unaligned(buffer.as_ptr() as *const UnicodeString) };

    // Bounds-check BEFORE dereferencing (T-02-09): Buffer must point inside
    // the allocation and Length must not exceed it.
    if us.buffer.is_null() {
        return None;
    }
    let offset = (us.buffer as usize).wrapping_sub(buffer.as_ptr() as usize) / 2;
    let claimed_chars = (us.length as usize) / 2;
    if offset + claimed_chars > buffer.len() {
        return None; // Buffer points outside the allocation — corrupt return
    }

    let slice = &buffer[offset..offset + claimed_chars];
    extract_unicode_string(slice, us.length as u32)
}

/// Find a process's parent PID via a Toolhelp32 snapshot.
///
/// Sets `entry.dwSize` BEFORE Process32FirstW (required by the API), iterates
/// until the target PID matches, and always closes the snapshot handle
/// (PITFALLS.md handle-leak gotcha).
fn query_parent_pid(pid: u32) -> Option<u32> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.ok()?;

    // RAII close — the snapshot handle must never leak.
    struct Snapshot(HANDLE);
    impl Drop for Snapshot {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let snapshot = Snapshot(snapshot);

    let mut entry = PROCESSENTRY32W::default();
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    let mut ok = unsafe { Process32FirstW(snapshot.0, &mut entry) }.is_ok();
    while ok {
        if entry.th32ProcessID == pid {
            return Some(entry.th32ParentProcessID);
        }
        ok = unsafe { Process32NextW(snapshot.0, &mut entry) }.is_ok();
    }
    None
}

/// Verify a file's digital signature via WinVerifyTrustEx (D-07).
///
/// Returns:
/// - `Some(true)` — WinVerifyTrust returned 0: the file carries a valid
///   signature chain. **Scope note (RESEARCH Open Question 3):** "Signed"
///   means ANY valid signature, NOT trusted-publisher semantics — a
///   self-signed certificate in the root store also passes.
/// - `Some(false)` — TRUST_E_NOSIGNATURE: no signature present.
/// - `None` — anything else (access denied, revocation, corrupt catalog):
///   the UI renders "Unknown".
///
/// Runs in `spawn_blocking` (WinVerifyTrust costs 10-100ms/file, A2) with
/// WTD_CACHE_ONLY_URL_RETRIEVAL (A8: offline cache-first — no network hang)
/// and a second WTD_STATEACTION_CLOSE call to release resources (Pitfall 5).
pub async fn verify_signature(path: &str) -> Option<bool> {
    let path = path.to_string();
    tokio::task::spawn_blocking(move || verify_signature_blocking(&path))
        .await
        .ok()
        .flatten()
}

fn verify_signature_blocking(path: &str) -> Option<bool> {
    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut file_info = WINTRUST_FILE_INFO {
        cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: PCWSTR(wide.as_ptr()),
        hFile: HANDLE(ptr::null_mut()),
        pgKnownSubject: ptr::null_mut(),
    };
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    let mut data = WINTRUST_DATA {
        cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
        pPolicyCallbackData: ptr::null_mut(),
        pSIPClientData: ptr::null_mut(),
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 {
            pFile: &mut file_info,
        },
        dwStateAction: WTD_STATEACTION_VERIFY,
        hWVTStateData: HANDLE(ptr::null_mut()),
        pwszURLReference: windows::core::PWSTR(ptr::null_mut()),
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL,
        dwUIContext: WINTRUST_DATA_UICONTEXT(0),
        pSignatureSettings: ptr::null_mut(),
    };

    let null_hwnd = HWND(ptr::null_mut());
    let result = unsafe { WinVerifyTrustEx(null_hwnd, &mut action, &mut data) };

    // Resource cleanup (Pitfall 5): the CLOSE pass releases the state
    // allocated by the VERIFY pass.
    data.dwStateAction = WTD_STATEACTION_CLOSE;
    let _ = unsafe { WinVerifyTrustEx(null_hwnd, &mut action, &mut data) };

    if result == 0 {
        Some(true)
    } else if (result as u32) == TRUST_E_NOSIGNATURE {
        Some(false)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ft(low: u32, high: u32) -> FILETIME {
        FILETIME {
            dwLowDateTime: low,
            dwHighDateTime: high,
        }
    }

    // ── filetime_to_systemtime (RESEARCH Code Example 6) ──

    /// FILETIME 116444736000000000 (1601-01-01 + 11644473600s) == UNIX_EPOCH.
    #[test]
    fn filetime_epoch_maps_to_unix_epoch() {
        // 11_644_473_600 * 10_000_000 = 0x019DB1DED53E8000
        let st = filetime_to_systemtime(ft(0xD53E_8000, 0x019D_B1DE))
            .expect("epoch conversion must succeed");
        assert_eq!(st, std::time::UNIX_EPOCH);
    }

    /// A known 2026 instant round-trips and displays as
    /// "09:41:12 31-Jul-2026" (UI-SPEC detail panel row 5 format).
    #[test]
    fn filetime_2026_round_trips_through_chrono_display() {
        let instant = chrono::DateTime::parse_from_rfc3339("2026-07-31T09:41:12Z")
            .expect("parse instant")
            .with_timezone(&chrono::Utc);
        let ft_value = (instant.timestamp() as u64 + 11_644_473_600) * 10_000_000;
        let st = filetime_to_systemtime(ft(
            (ft_value & 0xFFFF_FFFF) as u32,
            (ft_value >> 32) as u32,
        ))
        .expect("2026 conversion must succeed");
        let display = chrono::DateTime::<chrono::Utc>::from(st)
            .format("%H:%M:%S %d-%b-%Y")
            .to_string();
        assert_eq!(display, "09:41:12 31-Jul-2026");
    }

    // ── extract_unicode_string (defensive bounds, A6) ──

    /// L"ab" with NUL terminator — trailing NUL dropped.
    #[test]
    fn extract_unicode_string_plain() {
        let buf = [97u16, 98, 0]; // wide L"ab" + NUL terminator (6 bytes)
        assert_eq!(extract_unicode_string(&buf, 6).as_deref(), Some("ab"));
    }

    /// Length claims more bytes than the allocation holds — the Buffer
    /// pointer would lie outside the allocation; must be None, not OOB read.
    #[test]
    fn extract_unicode_string_claims_beyond_allocation() {
        let buf = [97u16, 98, 0]; // 3 wide chars allocated (6 bytes)
        assert_eq!(extract_unicode_string(&buf, 8), None);
    }

    /// Zero length yields an empty string.
    #[test]
    fn extract_unicode_string_zero_length() {
        let buf = [0u16];
        assert_eq!(extract_unicode_string(&buf, 0).as_deref(), Some(""));
    }

    /// Embedded NULs are preserved; only trailing NULs are trimmed.
    #[test]
    fn extract_unicode_string_embedded_nul_preserved_trailing_trimmed() {
        let buf = [97u16, 0, 98, 0]; // wide L"a\0b" + NUL terminator (8 bytes)
        assert_eq!(extract_unicode_string(&buf, 8).as_deref(), Some("a\u{0}b"));
    }

    // ── verify_signature (D-07, Windows-gated) ──

    /// The test binary itself: WinVerifyTrust must return SOME verdict
    /// (signed or unsigned) — None would indicate broken wiring, not an
    /// unsigned file.
    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn verify_signature_current_exe_returns_verdict() {
        let path = std::env::current_exe().expect("current_exe");
        let verdict = verify_signature(&path.to_string_lossy()).await;
        assert!(
            verdict.is_some(),
            "WinVerifyTrust must return a verdict (Some), got None for {}",
            path.display()
        );
    }

    // ── fetch_details integration (D-08, Windows-gated) ──

    /// On the current process all three always-populatable fields must be
    /// present: path, start time, and parent PID (PROC-06 success path).
    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn fetch_details_populates_current_process() {
        let pid = std::process::id();
        let info = fetch_details(pid)
            .await
            .expect("fetch_details on self must succeed");
        assert_eq!(info.pid, pid);
        assert!(
            info.executable_path.is_some(),
            "executable path must populate for the current process"
        );
        assert!(
            info.start_time.is_some(),
            "start time must populate for the current process"
        );
        assert!(
            info.parent_pid.is_some(),
            "parent PID must populate for the current process"
        );
    }
}
