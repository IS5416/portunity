//! Process detail fetchers — executable path, command line, start time,
//! parent PID, and digital signature (PROC-06).
//!
//! RED step of the plan 02-02 Task 1 TDD cycle: this module contains only
//! the test contract. The implementation lands in the GREEN step.
//!
//! Planned API surface (from the plan's behavior block):
//! - `filetime_to_systemtime(FILETIME) -> Option<SystemTime>` — pure helper
//! - `extract_unicode_string(&[u16], u32) -> Option<String>` — defensive bounds
//! - `fetch_details(pid) -> Result<ProcessInfo>` — one spawn_blocking scope
//! - `verify_signature(path) -> Option<bool>` — WinVerifyTrustEx

use windows::Win32::Foundation::FILETIME;

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
        let buf = [97u16, 0, 98, 0, 0, 0]; // 6 bytes: L"ab" + terminator
        assert_eq!(extract_unicode_string(&buf, 6).as_deref(), Some("ab"));
    }

    /// Length claims more bytes than the allocation holds — the Buffer
    /// pointer would lie outside the allocation; must be None, not OOB read.
    #[test]
    fn extract_unicode_string_claims_beyond_allocation() {
        let buf = [97u16, 0, 98, 0, 0, 0]; // 6 bytes allocated
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
        let buf = [97u16, 0, 0, 0, 98, 0, 0, 0]; // L"a\0b" + terminator
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
        let info = fetch_details(pid).expect("fetch_details on self must succeed");
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
