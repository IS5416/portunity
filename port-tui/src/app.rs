//! Application state for the TEA architecture.
//!
//! The `App` struct holds all mutable state. It is mutated by `update()`
//! and read by component `render()` methods.

use std::time::Instant;

use port_core::models::Connection;

use crate::message::{SortColumn, SortOrder};

/// Central application state.
pub struct App {
    /// Current port list from the last successful scan.
    pub ports: Vec<Connection>,

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
}

impl App {
    /// Create a new application state.
    ///
    /// Starts in scanning mode so the first frame shows scanning indicator (D-15).
    pub fn new() -> Self {
        Self {
            ports: Vec::new(),
            scanning: true,
            error: None,
            last_scan_time: None,
            should_quit: false,
            sort_column: SortColumn::Port,
            sort_order: SortOrder::None,
            selected_index: 0,
            last_auto_refresh: None,
        }
    }
}
