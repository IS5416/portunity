//! Message enum for the TEA event loop.
//!
//! All user actions and system events are represented as `Message` variants.
//! The main loop maps crossterm events to Messages and the `update()` function
//! transforms application state.

use port_core::models::{Connection, ProcessInfo};
use port_core::process::{KillOutcome, ProcessSnapshot, Protection};

/// Sortable columns in the port table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Port,
    Protocol,
    State,
    ProcessName,
    Pid,
}

impl SortColumn {
    // Sort cycling is handled inline in update() (same column → toggle direction,
    // different column → start ascending). No cycle helper needed.
}

/// Sort direction for the currently sorted column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl SortOrder {
    /// Toggle sort direction: Ascending ↔ Descending.
    pub fn cycle(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

/// Fields in the filter panel that can be edited.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FilterField {
    PortMin,
    PortMax,
    ProcessName,
    Pid,
    Protocol,
    State,
}

impl FilterField {
    /// Return the next field in the tab cycle.
    pub fn next(&self) -> Self {
        match self {
            Self::PortMin => Self::PortMax,
            Self::PortMax => Self::ProcessName,
            Self::ProcessName => Self::Pid,
            Self::Pid => Self::Protocol,
            Self::Protocol => Self::State,
            Self::State => Self::PortMin,
        }
    }

    /// Return the previous field in the tab cycle.
    pub fn prev(&self) -> Self {
        match self {
            Self::PortMin => Self::State,
            Self::PortMax => Self::PortMin,
            Self::ProcessName => Self::PortMax,
            Self::Pid => Self::ProcessName,
            Self::Protocol => Self::Pid,
            Self::State => Self::Protocol,
        }
    }
}

#[derive(Debug)]
pub enum Message {
    /// Quit the application.
    Quit,

    /// Manual refresh triggered by user (r key).
    Refresh,

    /// Scan completed successfully with port data.
    ScanComplete(Vec<Connection>),

    /// Scan failed with an error message.
    ScanError(String),

    /// Toggle sort on a column / cycle sort order.
    Sort(SortColumn),

    /// Move selection up (j or Up).
    MoveUp,

    /// Move selection down (k or Down).
    MoveDown,

    /// Jump to top of port list (g).
    ScrollTop,

    /// Jump to bottom of port list (G).
    ScrollBottom,

    // --- Search messages ---

    /// Append a character to the search query.
    SearchInput(char),

    /// Remove the last character from the search query.
    SearchBackspace,

    /// Open the search bar overlay.
    SearchActivate,

    /// Close the search bar overlay (Enter confirms, Esc cancels).
    SearchDeactivate,

    /// Move search cursor left.
    SearchCursorLeft,

    /// Move search cursor right.
    SearchCursorRight,

    // --- Filter messages ---

    /// Open the filter panel overlay.
    FilterActivate,

    /// Close the filter panel overlay and clear filter.
    FilterDeactivate,

    /// Update a specific field in the filter panel.
    FilterUpdateField(FilterField, String),

    /// Apply the current filter criteria.
    FilterApply,

    /// Remove last character from focused filter field buffer.
    FilterFieldBackspace,

    /// Cycle to the next field in the filter panel (parses current buffer first).
    FilterTabField,

    /// Cycle to the previous field in the filter panel (Shift+Tab).
    FilterTabBackward,

    // --- Admin / elevation messages ---

    /// Result of the startup admin check.
    AdminCheck(bool),

    /// User pressed 'a' to request admin elevation.
    ElevateRequest,

    /// UAC prompt was declined; continue in non-admin mode.
    ElevateDeclined,

    /// Elevation attempt failed with a non-decline error (ShellExecuteExW
    /// failure while re-launching). Resets `app.elevating` so the 'a' key can
    /// be retried later in the session (previously only ElevateDeclined
    /// cleared the guard, leaving 'a' dead after a hard failure).
    ElevateFailed(String),

    /// Switch to a tab by index (0=Overview, 1=Ports, 2=History, 3=Traffic, 4=Firewall).
    SwitchTab(usize),

    // --- Kill flow messages (D-01..D-04, D-09) ---
    //
    // KillPrepared / KillExecute are internal flow messages beyond the UI-SPEC
    // list (the UI-SPEC list covers user-facing messages; the two-stage gate
    // needs the extra hops). KillPrepared carries the Send-safe snapshot +
    // protection verdict so the drain loop can route: None -> instant kill,
    // UserConfirm -> dialog, HardBlocked -> status message only.

    /// User pressed 'x' on a selected row — request kill of its owning process.
    Kill { pid: u32 },

    /// Snapshot + protection verdict computed off the async runtime.
    KillPrepared {
        snapshot: ProcessSnapshot,
        protection: Protection,
        name: String,
        pid: u32,
    },

    /// User confirmed the kill in the confirmation dialog (y / Enter).
    KillConfirmed { pid: u32 },

    /// User cancelled the confirmation dialog (n / Esc).
    KillCancelled,

    /// Kill execution started — graceful signal dispatched.
    KillStart { name: String, pid: u32 },

    /// Graceful timeout hit — force kill in progress.
    KillTimeout {
        name: String,
        pid: u32,
        timeout_secs: u64,
    },

    /// (Internal) execute the kill with an already-prepared snapshot.
    ///
    /// Declared per the plan's message contract (02-01 Task 2 step 1) for the
    /// two-stage gate; the current flow intercepts KillPrepared directly in the
    /// drain loop, so this variant is constructed by no producer yet — plan 02-03
    /// (whitelist overlay) and future kill paths may emit it.
    #[allow(dead_code)]
    KillExecute { snapshot: ProcessSnapshot },

    /// Final kill outcome for the status bar (D-04).
    KillOutcome {
        outcome: KillOutcome,
        name: String,
        pid: u32,
    },

    // --- Detail panel messages (D-05..D-08, PROC-06) ---
    //
    // The panel stores display data only — the kill path re-captures the
    // identity FRESH via snapshot_for(pid) at kill time (plan 02-01 Task 2),
    // so no creation-time FILETIME rides along here.

    /// User pressed 'd' — toggle the detail panel overlay (D-06).
    ToggleDetailPanel,

    /// Detail fetch completed — panel displays the process info (D-08).
    /// On fetch failure the intercept sends this with name/PID from the row
    /// and every other field None (renders "—" per UI-SPEC error state).
    DetailDataLoaded { process_info: ProcessInfo },

    /// Signature verification completed for a PID (D-07 on-demand + cache).
    SignatureVerified {
        pid: u32,
        is_signed: Option<bool>,
    },

    /// The process the panel is showing left the scan list — render the
    /// name strikethrough and Status "Exited" (UI-SPEC Detail Panel States).
    ProcessExited { pid: u32 },

    // --- Whitelist overlay messages (D-13..D-15, PROC-05) ---

    /// User pressed 'w' — toggle the whitelist overlay (D-14, any tab).
    ToggleWhitelistOverlay,

    /// Tab — move focus from user list to input (or back).
    WhitelistFocusNext,

    /// Shift+Tab — move focus from input to user list (or back).
    WhitelistFocusPrev,

    /// Move the user-list selection (j/k/up/down when the list is focused).
    WhitelistSelectMove { dir: i8 },

    /// Delete the selected user entry (d) — instant, no confirmation (D-15).
    WhitelistDeleteSelected,

    /// Append a character to the path input (input focus).
    WhitelistInput(char),

    /// Remove the character before the input cursor.
    WhitelistBackspace,

    /// Move the input cursor (left/right arrows).
    WhitelistCursorMove { dir: i8 },

    /// Enter in the input — validate + persist the path (intercept-owned).
    WhitelistAdd { path: String },

    /// Add/remove persisted — update() applies the working copy mutation and
    /// the status-bar string. `added: false` is either a duplicate-add no-op
    /// (path still present in the working copy) or a completed removal.
    WhitelistSaved { path: String, added: bool },

    /// Validation or save failure — status-bar error, entry not added.
    WhitelistError { path: String, reason: String },

    /// User pressed '?' — toggle the Help overlay.
    ToggleHelp,
}
