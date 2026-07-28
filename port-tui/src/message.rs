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
    None,
    Ascending,
    Descending,
}

impl SortOrder {
    /// Cycle through sort orders: None → Ascending → Descending → None.
    pub fn cycle(self) -> Self {
        match self {
            Self::None => Self::Ascending,
            Self::Ascending => Self::Descending,
            Self::Descending => Self::None,
        }
    }
}

/// Fields in the filter panel that can be edited.
#[derive(Debug, Clone, PartialEq, Eq)]
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

    /// Cycle to the next field in the filter panel.
    FilterTabField,

    // --- Admin / elevation messages ---

    /// Result of the startup admin check.
    AdminCheck(bool),

    /// User pressed 'a' to request admin elevation.
    ElevateRequest,

    /// UAC prompt was declined; continue in non-admin mode.
    ElevateDeclined,
}
