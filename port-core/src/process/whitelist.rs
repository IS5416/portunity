//! Process whitelist — built-in system-critical list + user-tier confirmation gate.
//!
//! ## Two-Tier Protection (D-09)
//! 1. **Built-in tier** (`HardBlocked`): ~30 system-critical processes matched by basename
//!    or PID (0/4). No kill path exists — pressing `x` shows the reason in the status bar.
//! 2. **User tier** (`UserConfirm`): processes whose full executable path matches a user
//!    entry in `settings.toml`. Kill is gated behind a confirmation dialog.
//!
//! ## Match Semantics (D-10)
//! - Built-in: lowercase basename compare + PID 0/4 special case.
//! - User: normalized case-insensitive full-path compare.
//! - Built-in tier is checked FIRST (Pitfall #6) — a process matching both tiers
//!   is `HardBlocked` (built-in always wins).
//!
//! ## References
//! - Microsoft Restart Manager Critical System Services (learn.microsoft.com)
//! - RESEARCH Assumption A1: 26–30 entries, human-verified

use crate::config::AppSettings;

/// A single entry in the built-in system-critical whitelist.
#[derive(Debug, Clone)]
pub struct BuiltinEntry {
    /// Process basename (e.g. `smss.exe`), lowercase.
    pub name: &'static str,
    /// Plain-language reason (e.g. "Session manager — crashes all user sessions if killed").
    pub reason: &'static str,
}

/// Built-in system-critical process whitelist.
///
/// Grounded in Microsoft Restart Manager's Critical System Services list,
/// extended with session infrastructure and security processes.
/// Every entry carries a plain-language explanation.
///
/// **Tier 1**: Restart Manager canonical critical services (14 entries).
/// **Tier 2**: Session infrastructure + security processes (11 entries).
/// Total: 25 entries, satisfying the >=25 contract (PROC-04).
/// explorer.exe excluded by design — it restarts itself, and killing it
/// is disruptive but not system-fatal (RESEARCH decision).
pub const BUILTIN: &[BuiltinEntry] = &[
    // ── Tier 1: Restart Manager Critical System Services ──
    BuiltinEntry { name: "smss.exe",      reason: "Session Manager — starts and kills user sessions; crashes all sessions if killed" },
    BuiltinEntry { name: "csrss.exe",     reason: "Client/Server Runtime — core Win32 subsystem; system becomes unusable without it" },
    BuiltinEntry { name: "wininit.exe",   reason: "Windows Initialization — starts services.exe, lsass.exe, and lsaiso.exe at boot" },
    BuiltinEntry { name: "winlogon.exe",  reason: "Windows Logon — manages user login, logout, and the secure attention sequence (Ctrl+Alt+Del)" },
    BuiltinEntry { name: "services.exe",  reason: "Service Control Manager — starts, stops, and manages all Windows services" },
    BuiltinEntry { name: "lsass.exe",     reason: "Local Security Authority — enforces security policy, handles authentication tokens" },
    BuiltinEntry { name: "svchost.exe",   reason: "Service Host — hosts multiple Windows services (RPCSS, DcomLaunch, BITS, etc.)" },
    BuiltinEntry { name: "logonui.exe",   reason: "Logon UI — displays the login screen; killing it drops all users to a black screen" },
    BuiltinEntry { name: "dwm.exe",       reason: "Desktop Window Manager — compositing, transparency, taskbar thumbnails; kills the display" },
    BuiltinEntry { name: "fontdrvhost.exe", reason: "Font Driver Host — handles font rendering; killing it breaks text display system-wide" },
    BuiltinEntry { name: "spoolsv.exe",   reason: "Print Spooler — manages print jobs; killing it disables printing" },
    BuiltinEntry { name: "audiodg.exe",   reason: "Windows Audio Device Graph — processes audio; killing it mutes all system sound" },
    BuiltinEntry { name: "msmpeng.exe",   reason: "Microsoft Defender Antimalware — real-time threat protection and scanning" },
    BuiltinEntry { name: "sgrmbroker.exe", reason: "System Guard Runtime Monitor — attestation and runtime integrity checks" },

    // ── Tier 2: Session infrastructure + security processes ──
    BuiltinEntry { name: "lsaiso.exe",    reason: "Credential Guard — isolates LSA secrets in a virtualized container; killing it breaks auth" },
    BuiltinEntry { name: "conhost.exe",   reason: "Console Window Host — hosts console windows; killing it closes all open command prompts" },
    BuiltinEntry { name: "taskhostw.exe", reason: "Task Host — runs scheduled tasks; killing it cancels running system maintenance" },
    BuiltinEntry { name: "searchindexer.exe", reason: "Windows Search Indexer — indexes files for search; killing it disables Start menu search" },
    BuiltinEntry { name: "searchhost.exe", reason: "Search Host — powers the search UI in the taskbar and Start menu" },
    BuiltinEntry { name: "startmenuexperiencehost.exe", reason: "Start Menu Experience Host — renders the Start menu; killing it disables Start" },
    BuiltinEntry { name: "shellexperiencehost.exe", reason: "Shell Experience Host — taskbar visuals, Action Center, clock; killing it breaks the shell" },
    BuiltinEntry { name: "runtimebroker.exe", reason: "Runtime Broker — enforces app permissions (camera, mic, location); killing it may block app launches" },
    BuiltinEntry { name: "smartscreen.exe", reason: "Windows SmartScreen — reputation-based URL and download protection" },
    BuiltinEntry { name: "securityhealthservice.exe", reason: "Windows Security Health — monitors antivirus, firewall, and device security status" },
    BuiltinEntry { name: "securesystem.exe", reason: "Secure System — kernel enclave enforcement for Credential Guard and HVCI" },
];

// PID 4 is "System"
// PID 0 is "Idle" (or "System Idle Process")
// Both are handled by builtin_match() via PID, not name.

/// Match a process against the built-in whitelist by basename + PID.
///
/// Returns the plain-language `reason` string if the process is protected.
/// PID 0 and PID 4 are always protected regardless of name.
pub fn builtin_match(pid: u32, basename: &str) -> Option<&'static str> {
    // PID 0: System Idle Process (kernel pseudo-process)
    if pid == 0 {
        return Some("System Idle Process — not a real process; handles idle CPU time");
    }
    // PID 4: System (kernel threads)
    if pid == 4 {
        return Some("System (PID 4) — Windows kernel; terminating it would crash the system");
    }

    let lower = basename.to_lowercase();
    BUILTIN
        .iter()
        .find(|e| e.name == lower)
        .map(|e| e.reason)
}

/// Match a process against the user whitelist by full executable path.
///
/// Normalizes paths: trims whitespace, strips surrounding quotes,
/// strips trailing `\` or `/`, and compares case-insensitively.
pub fn user_match(path: &str, entries: &[String]) -> bool {
    let normalized = normalize_path(path);
    if normalized.is_empty() {
        return false;
    }

    entries
        .iter()
        .any(|entry| normalized.eq_ignore_ascii_case(&normalize_path(entry)))
}

/// Normalize a path for comparison: trim, strip quotes, strip trailing separators.
fn normalize_path(s: &str) -> String {
    let s = s.trim();
    // Strip surrounding quotes (single or double)
    let s = if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\''))
    {
        &s[1..s.len().saturating_sub(1)]
    } else {
        s
    };
    // Strip trailing backslash or forward slash
    let s = s
        .trim_end_matches('\\')
        .trim_end_matches('/');
    s.to_string()
}

/// Protection level for a process (derived from whitelist tiers).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Protection {
    /// No protection — instant kill is allowed.
    None,
    /// User-tier protection — kill requires confirmation dialog.
    UserConfirm,
    /// Built-in system-critical — no kill path exists (hard block).
    HardBlocked(&'static str),
}

/// Determine the protection level for a process.
///
/// Built-in tier is checked FIRST (Pitfall #6) — if a basename matches the built-in
/// list, the result is always `HardBlocked`, even if the path also appears in the user
/// whitelist. User-tier is only consulted when built-in does not match.
pub fn protection_status(
    pid: u32,
    basename: &str,
    path: Option<&str>,
    settings: &AppSettings,
) -> Protection {
    // Built-in tier first (Pitfall #6)
    if let Some(reason) = builtin_match(pid, basename) {
        return Protection::HardBlocked(reason);
    }

    // User tier — needs a path for full-path matching
    if let Some(p) = path {
        if !settings.whitelist.is_empty() && user_match(p, &settings.whitelist) {
            return Protection::UserConfirm;
        }
    }

    Protection::None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── BUILTIN contract tests (PROC-04) ──

    #[test]
    fn builtin_at_least_25_entries() {
        assert!(
            BUILTIN.len() >= 25,
            "BUILTIN must have >=25 entries; found {}",
            BUILTIN.len()
        );
    }

    #[test]
    fn builtin_names_are_lowercase() {
        for entry in BUILTIN {
            assert_eq!(
                entry.name,
                entry.name.to_lowercase(),
                "BUILTIN name '{}' must be lowercase",
                entry.name
            );
        }
    }

    #[test]
    fn builtin_names_are_unique() {
        let mut names: Vec<&str> = BUILTIN.iter().map(|e| e.name).collect();
        let orig_len = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(
            names.len(),
            orig_len,
            "BUILTIN names must be unique — duplicates found"
        );
    }

    #[test]
    fn builtin_reasons_non_empty() {
        for entry in BUILTIN {
            assert!(
                !entry.reason.is_empty(),
                "BUILTIN entry '{}' must have a non-empty reason",
                entry.name
            );
        }
    }

    // ── builtin_match tests ──

    #[test]
    fn builtin_match_pid_0() {
        let reason = builtin_match(0, "");
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("Idle"));
    }

    #[test]
    fn builtin_match_pid_4() {
        let reason = builtin_match(4, "");
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("System"));
    }

    #[test]
    fn builtin_match_case_insensitive_basename() {
        // csrss.exe is in BUILTIN
        let reason = builtin_match(100, "CSRSS.EXE");
        assert!(reason.is_some());

        let reason_lower = builtin_match(100, "csrss.exe");
        assert_eq!(reason, reason_lower);
    }

    #[test]
    fn builtin_match_not_found() {
        let reason = builtin_match(99999, "myapp.exe");
        assert!(reason.is_none());
    }

    // ── user_match + normalization tests (PROC-05) ──

    #[test]
    fn user_match_exact() {
        let entries = vec!["C:\\Program Files\\MyApp\\app.exe".to_string()];
        assert!(user_match("C:\\Program Files\\MyApp\\app.exe", &entries));
    }

    #[test]
    fn user_match_case_insensitive() {
        let entries = vec!["C:\\Program Files\\MyApp\\app.exe".to_string()];
        assert!(user_match("c:\\program files\\myapp\\app.exe", &entries));
    }

    #[test]
    fn user_match_quoted_path() {
        let entries = vec!["C:\\Program Files\\MyApp\\app.exe".to_string()];
        assert!(user_match("\"C:\\Program Files\\MyApp\\app.exe\"", &entries));
    }

    #[test]
    fn user_match_trailing_backslash() {
        let entries = vec!["C:\\Program Files\\".to_string()];
        assert!(user_match("C:\\Program Files", &entries));
        assert!(user_match("C:\\Program Files\\", &entries));
    }

    #[test]
    fn user_match_trailing_forward_slash() {
        let entries = vec!["C:/Users/foo".to_string()];
        assert!(user_match("C:/Users/foo/", &entries));
    }

    #[test]
    fn user_match_not_found() {
        let entries = vec!["C:\\Program Files\\Other\\app.exe".to_string()];
        assert!(!user_match("C:\\Program Files\\MyApp\\app.exe", &entries));
    }

    #[test]
    fn user_match_empty_entry_list() {
        let entries: Vec<String> = vec![];
        assert!(!user_match("anything.exe", &entries));
    }

    #[test]
    fn user_match_empty_path() {
        let entries = vec!["C:\\foo.exe".to_string()];
        assert!(!user_match("", &entries));
    }

    // ── protection_status matrix (PROC-03) ──

    fn test_settings(whitelist: Vec<String>) -> AppSettings {
        AppSettings {
            admin_detected: false,
            schema_version: 1,
            whitelist,
            kill_timeout_secs: 5,
        }
    }

    #[test]
    fn protection_none_for_unmatched_process() {
        let settings = test_settings(vec![]);
        let result = protection_status(99999, "myapp.exe", None, &settings);
        assert_eq!(result, Protection::None);
    }

    #[test]
    fn protection_hardblocked_for_csrss() {
        let settings = test_settings(vec![]);
        let result = protection_status(100, "csrss.exe", None, &settings);
        assert!(matches!(result, Protection::HardBlocked(_)));
    }

    #[test]
    fn protection_hardblocked_for_pid_4() {
        let settings = test_settings(vec![]);
        let result = protection_status(4, "System", None, &settings);
        assert!(matches!(result, Protection::HardBlocked(_)));
    }

    #[test]
    fn protection_userconfirm_for_whitelisted_path() {
        let settings = test_settings(vec!["C:\\Program Files\\MyApp\\app.exe".to_string()]);
        let result = protection_status(
            99999,
            "app.exe",
            Some("C:\\Program Files\\MyApp\\app.exe"),
            &settings,
        );
        assert_eq!(result, Protection::UserConfirm);
    }

    #[test]
    fn protection_none_for_user_unmatched_path() {
        let settings = test_settings(vec!["C:\\Program Files\\Other\\app.exe".to_string()]);
        let result = protection_status(
            99999,
            "app.exe",
            Some("C:\\Program Files\\MyApp\\app.exe"),
            &settings,
        );
        assert_eq!(result, Protection::None);
    }

    #[test]
    fn builtin_wins_over_user() {
        // csrss.exe is built-in; even if user-listed, HardBlocked wins (Pitfall #6)
        let settings = test_settings(vec!["C:\\Windows\\System32\\csrss.exe".to_string()]);
        let result = protection_status(
            700,
            "csrss.exe",
            Some("C:\\Windows\\System32\\csrss.exe"),
            &settings,
        );
        assert!(matches!(result, Protection::HardBlocked(_)));
    }

    #[test]
    fn user_tier_skipped_when_no_path() {
        // User whitelist entry exists but no path → can't do full-path match
        let settings = test_settings(vec!["C:\\Windows\\System32\\csrss.exe".to_string()]);
        let result = protection_status(99999, "not_in_builtin.exe", None, &settings);
        assert_eq!(result, Protection::None);
    }
}
