//! Message enum for the TEA event loop.
//!
//! All user actions and system events are represented as `Message` variants.
//! The main loop maps crossterm events to Messages and the `update()` function
//! transforms application state.

use port_core::models::Connection;

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
    /// Return the next column in the cycle: Port → Protocol → State → ProcessName → Pid → Port.
    pub fn next(self) -> Self {
        match self {
            Self::Port => Self::Protocol,
            Self::Protocol => Self::State,
            Self::State => Self::ProcessName,
            Self::ProcessName => Self::Pid,
            Self::Pid => Self::Port,
        }
    }
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

    /// Periodic tick for time-driven updates.
    Tick,

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

    /// Clear search query and close the search bar.
    SearchClear,

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

    /// Switch to a tab by index (0=Overview, 1=Ports, 2=History, 3=Traffic, 4=Firewall).
    SwitchTab(usize),
}
