//! Application state for the TEA architecture.
//!
//! The `App` struct holds all mutable state. It is mutated by `update()`
//! and read by component `render()` methods.

use std::collections::HashMap;
use std::time::Instant;

use port_core::models::{Connection, Filter};
use port_core::process::{KillOutcome, ProcessSnapshot};

use crate::message::{FilterField, SortColumn, SortOrder};

/// Kill status bar message with a semantic tone (D-04).
#[derive(Debug, Clone)]
pub struct KillStatus {
    /// Full text rendered in the status bar (already width-truncated).
    pub text: String,
    /// Visual tone: in-progress info, success, or error.
    pub tone: KillTone,
}

/// Tone for a kill status message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillTone {
    /// Kill in progress (graceful signal sent, timeout counting).
    InProgress,
    /// Kill succeeded.
    Success,
    /// Kill failed (access denied, hard block, already exited, other).
    Error,
}

/// Central application state.
pub struct App {
    /// Current port list from the last successful scan.
    pub ports: Vec<Connection>,

    /// Filtered port list (view model). Populated when search or filter is active.
    pub filtered_ports: Vec<Connection>,

    /// Whether a scan is currently in progress.
    pub scanning: bool,

    /// Last error message, if any.
    pub error: Option<String>,

    /// Timestamp of the last successful scan completion.
    pub last_scan_time: Option<Instant>,

    /// Whether the user has requested to quit.
    pub should_quit: bool,

    /// Currently sorted column.
    pub sort_column: SortColumn,

    /// Current sort order for the sorted column.
    pub sort_order: SortOrder,

    /// Index of the selected row (0-based).
    pub selected_index: usize,

    /// Timestamp of the last auto-refresh (for 5s interval).
    pub last_auto_refresh: Option<Instant>,

    // --- Search state ---

    /// Current fuzzy search query string.
    pub search_query: String,

    /// Whether the search bar overlay is active.
    pub search_active: bool,

    /// Cursor position within the search query (for arrow key navigation).
    pub search_cursor_pos: usize,

    // --- Filter state ---

    /// Current multi-dimensional filter criteria.
    pub active_filter: Filter,

    /// Whether the filter panel overlay is active.
    pub filter_active: bool,

    /// Whether a filter has been applied (via Enter). Keeps display_data()
    /// showing filtered_ports even after the panel is dismissed.
    pub filter_applied: bool,

    /// Currently focused field in the filter panel (tab cycles).
    pub filter_focused_field: FilterField,

    /// Raw text buffers for filter field input (character-by-character accumulation).
    /// Parsed into active_filter on Tab or Enter.
    pub filter_field_text: HashMap<FilterField, String>,

    // --- Admin state ---

    /// Whether the current process has administrator privileges.
    pub is_admin: bool,

    /// Whether the startup admin check has completed (gates status bar display).
    pub admin_check_done: bool,

    /// Whether an elevation request is in-flight (prevents double-elevation).
    pub elevating: bool,

    /// Currently active tab index (0=Overview, 1=Ports, 2=History, 3=Traffic, 4=Firewall).
    pub active_tab: usize,

    /// Whether the display needs a re-render (set on any state mutation).
    pub needs_render: bool,

    /// Instant of the last render, for minimum refresh interval.
    pub last_render: Option<Instant>,

    // --- Kill flow state (D-01..D-04, D-09) ---

    /// Last kill outcome / progress message shown in the status bar (D-04).
    pub kill_status: Option<KillStatus>,

    /// Whether a kill task is in flight (prevents double-kill).
    pub kill_in_flight: bool,

    /// PID awaiting confirmation in the dialog (Some = dialog open).
    pub confirm_pid: Option<u32>,

    /// Process name shown in the confirmation dialog.
    pub confirm_name: Option<String>,

    /// Port number of the selected row (dialog reason string).
    pub confirm_port: Option<u16>,

    /// Send-safe snapshot captured before the confirm gate — the kill identity
    /// verified against creation time at execution (PROC-07).
    pub pending_kill_snapshot: Option<ProcessSnapshot>,

    /// PID of the last successfully killed process (cleared on next scan;
    /// drives row strikethrough in plan 02-02).
    pub last_killed_pid: Option<u32>,

    /// Graceful-kill timeout read from settings at kill start (D-02/D-15).
    pub kill_timeout_secs: u64,

    /// Current terminal width — truncation budget for status/footer strings.
    pub term_width: u16,
}

impl App {
    /// Create a new application state.
    ///
    /// Starts in scanning mode so the first frame shows scanning indicator (D-15).
    pub fn new() -> Self {
        Self {
            ports: Vec::new(),
            filtered_ports: Vec::new(),
            scanning: true,
            error: None,
            last_scan_time: None,
            should_quit: false,
            sort_column: SortColumn::Port,
            sort_order: SortOrder::Ascending,
            selected_index: 0,
            last_auto_refresh: None,
            search_query: String::new(),
            search_active: false,
            search_cursor_pos: 0,
            active_filter: Filter::default(),
            filter_active: false,
            filter_applied: false,
            filter_focused_field: FilterField::PortMin,
            filter_field_text: HashMap::new(),
            is_admin: false,
            admin_check_done: false,
            elevating: false,
            active_tab: 0,
            needs_render: true, // first frame always renders
            last_render: None,
            kill_status: None,
            kill_in_flight: false,
            confirm_pid: None,
            confirm_name: None,
            confirm_port: None,
            pending_kill_snapshot: None,
            last_killed_pid: None,
            kill_timeout_secs: 5,
            term_width: 80,
        }
    }

    /// Return the data slice that should be displayed in the port table.
    ///
    /// When search or filter is active, returns filtered_ports.
    /// Otherwise returns the full port list.
    pub fn display_data(&self) -> &[Connection] {
        if self.search_active || self.filter_active || self.filter_applied {
            &self.filtered_ports
        } else {
            &self.ports
        }
    }
}

/// Map a kill outcome to the locked UI-SPEC status-bar string (Kill Flow Copy).
///
/// Returns `(text, tone)`. Every string renders within `term_width` columns.
/// Truncation rule (UI-SPEC Assumption A9 — declared here):
/// - General: when the full string exceeds `term_width`, `{name}` truncates
///   with U+2026 while the actionable tail ("Press a to elevate." / "Press w
///   to review the whitelist.") stays verbatim. Never wrap, never overflow.
/// - HardBlocked specifically: full form when it fits; else the compact form
///   `✗ {name} … Press w to review the whitelist.` with `{name}` budget
///   `term_width - 41` (the full 127+ char form never fits the 80-col gate).
///
/// `timeout_secs` is reserved for outcome strings that embed the graceful
/// timeout (the KillTimeout copy is handled directly in update.rs).
pub fn format_kill_status(
    name: &str,
    pid: u32,
    outcome: &KillOutcome,
    timeout_secs: u64,
    term_width: u16,
) -> (String, KillTone) {
    let _ = timeout_secs; // reserved — see doc comment
    let width = term_width as usize;

    match outcome {
        KillOutcome::HardBlocked(_) => {
            // Full form (UI-SPEC): "✗ {name} is protected — … Press w to review
            // the whitelist." — 127 fixed chars + name; never fits ≤80 cols.
            let full = format!(
                "✗ {} is protected — it is critical to Windows. Killing it would crash or destabilize your system. Press w to review the whitelist.",
                name
            );
            let text = if full.chars().count() <= width {
                full
            } else {
                // Compact form with {name} budget = term_width - 41
                let name_budget = width.saturating_sub(41);
                let compact_name = truncate_ellipsis(name, name_budget);
                format!(
                    "✗ {} \u{2026} Press w to review the whitelist.",
                    compact_name
                )
            };
            (text, KillTone::Error)
        }
        KillOutcome::Graceful => (
            truncate_name_preserving_tail(
                &format!("✓ {} (PID {}) terminated gracefully", name, pid),
                name,
                width,
            ),
            KillTone::Success,
        ),
        KillOutcome::ForceKilled => (
            truncate_name_preserving_tail(
                &format!("✓ {} (PID {}) force-killed", name, pid),
                name,
                width,
            ),
            KillTone::Success,
        ),
        KillOutcome::Direct => (
            truncate_name_preserving_tail(
                &format!("✓ {} (PID {}) terminated", name, pid),
                name,
                width,
            ),
            KillTone::Success,
        ),
        KillOutcome::AlreadyExited => (
            truncate_name_preserving_tail(
                &format!("✗ {} (PID {}) already exited", name, pid),
                name,
                width,
            ),
            KillTone::Error,
        ),
        KillOutcome::AccessDenied => (
            truncate_name_preserving_tail(
                &format!(
                    "✗ Cannot terminate {} (PID {}) — admin rights needed. Press a to elevate.",
                    name, pid
                ),
                name,
                width,
            ),
            KillTone::Error,
        ),
        KillOutcome::Failed(reason) => (
            truncate_name_preserving_tail(
                &format!("✗ Failed to terminate {} (PID {}): {}", name, pid, reason),
                name,
                width,
            ),
            KillTone::Error,
        ),
    }
}

/// Truncate `{name}` inside `template` with U+2026 when the string exceeds
/// `term_width` — the fixed chrome and the actionable tail stay verbatim.
fn truncate_name_preserving_tail(template: &str, name: &str, term_width: usize) -> String {
    if template.chars().count() <= term_width {
        return template.to_string();
    }

    let name_chars = name.chars().count();
    let fixed_chars = template.chars().count().saturating_sub(name_chars);
    let name_budget = term_width.saturating_sub(fixed_chars);
    let new_name = truncate_ellipsis(name, name_budget);

    let mut result = template.replacen(name, &new_name, 1);
    // Safety net: never overflow the status bar (e.g. very long failure reason).
    if result.chars().count() > term_width {
        result = result.chars().take(term_width.saturating_sub(1)).collect::<String>() + "\u{2026}";
    }
    result
}

/// Truncate a string to `max_len` chars with a trailing U+2026.
/// `max_len <= 1` yields a bare ellipsis.
fn truncate_ellipsis(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let truncated: String = s.chars().take(max_len.saturating_sub(1)).collect();
        format!("{}\u{2026}", truncated)
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: u16 = 80;

    fn fmt(name: &str, pid: u32, outcome: &KillOutcome) -> (String, KillTone) {
        format_kill_status(name, pid, outcome, 5, W)
    }

    #[test]
    fn graceful_mapping() {
        let (text, tone) = fmt("node.exe", 1234, &KillOutcome::Graceful);
        assert_eq!(text, "✓ node.exe (PID 1234) terminated gracefully");
        assert_eq!(tone, KillTone::Success);
    }

    #[test]
    fn force_killed_mapping() {
        let (text, tone) = fmt("node.exe", 1234, &KillOutcome::ForceKilled);
        assert_eq!(text, "✓ node.exe (PID 1234) force-killed");
        assert_eq!(tone, KillTone::Success);
    }

    #[test]
    fn direct_mapping() {
        let (text, tone) = fmt("node.exe", 1234, &KillOutcome::Direct);
        assert_eq!(text, "✓ node.exe (PID 1234) terminated");
        assert_eq!(tone, KillTone::Success);
    }

    #[test]
    fn already_exited_mapping() {
        let (text, tone) = fmt("node.exe", 1234, &KillOutcome::AlreadyExited);
        assert_eq!(text, "✗ node.exe (PID 1234) already exited");
        assert_eq!(tone, KillTone::Error);
    }

    #[test]
    fn access_denied_mapping() {
        // "a.exe" is short enough that the full 79-char string fits 80 cols.
        let (text, tone) = fmt("a.exe", 1234, &KillOutcome::AccessDenied);
        assert_eq!(
            text,
            "✗ Cannot terminate a.exe (PID 1234) — admin rights needed. Press a to elevate."
        );
        assert_eq!(tone, KillTone::Error);
    }

    #[test]
    fn hard_blocked_mapping() {
        let (text, tone) = fmt("svchost.exe", 5678, &KillOutcome::HardBlocked("reason"));
        // Full form is 127+ chars — never fits 80 cols, so the compact form
        // "✗ {name} … Press w to review the whitelist." applies (A9).
        assert!(text.starts_with("✗ svchost.exe"));
        assert!(text.ends_with("Press w to review the whitelist."));
        assert!(text.contains('\u{2026}'));
        assert_eq!(text, "✗ svchost.exe … Press w to review the whitelist.");
        assert_eq!(tone, KillTone::Error);
    }

    #[test]
    fn failed_mapping() {
        let (text, tone) = fmt(
            "node.exe",
            1234,
            &KillOutcome::Failed("process did not exit".into()),
        );
        assert_eq!(
            text,
            "✗ Failed to terminate node.exe (PID 1234): process did not exit"
        );
        assert_eq!(tone, KillTone::Error);
    }

    #[test]
    fn all_strings_fit_within_80_cols_with_short_names() {
        let outcomes = [
            KillOutcome::Graceful,
            KillOutcome::ForceKilled,
            KillOutcome::Direct,
            KillOutcome::AlreadyExited,
            KillOutcome::AccessDenied,
            KillOutcome::HardBlocked("x"),
            KillOutcome::Failed("boom".into()),
        ];
        for o in &outcomes {
            let (text, _) = fmt("app.exe", 9999, o);
            assert!(
                text.chars().count() <= 80,
                "'{}' is {} chars — must fit 80",
                text,
                text.chars().count()
            );
        }
    }

    #[test]
    fn hard_block_truncates_name_preserving_tail() {
        let long_name = "a_very_long_process_name_that_keeps_going.exe";
        let (text, _) = fmt(long_name, 9999, &KillOutcome::HardBlocked("x"));
        assert!(
            text.chars().count() <= 80,
            "truncated string '{}' = {} chars — must fit 80",
            text,
            text.chars().count()
        );
        assert!(
            text.ends_with("Press w to review the whitelist."),
            "actionable tail must survive truncation, got: {}",
            text
        );
        assert!(text.contains('\u{2026}'), "truncation marker missing: {}", text);
    }

    #[test]
    fn access_denied_truncates_name_preserving_tail() {
        let long_name = "another_really_long_process_name_that_goes_on.exe";
        let (text, _) = fmt(long_name, 9999, &KillOutcome::AccessDenied);
        assert!(text.chars().count() <= 80);
        assert!(
            text.ends_with("Press a to elevate."),
            "actionable tail must survive truncation, got: {}",
            text
        );
        assert!(text.contains('\u{2026}'));
    }

    #[test]
    fn short_names_are_not_truncated() {
        let (text, _) = fmt("node.exe", 1234, &KillOutcome::Graceful);
        assert!(!text.contains('\u{2026}'));
        assert_eq!(text, "✓ node.exe (PID 1234) terminated gracefully");
    }
}
