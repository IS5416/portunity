//! Application state for the TEA architecture.
//!
//! The `App` struct holds all mutable state. It is mutated by `update()`
//! and read by component `render()` methods.

use std::collections::HashMap;
use std::time::Instant;

use port_core::models::{Connection, Filter};

use crate::message::{FilterField, SortColumn, SortOrder};

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
