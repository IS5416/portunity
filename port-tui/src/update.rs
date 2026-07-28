//! TEA update function — Message → State mutation.
//!
//! Pure function that transforms application state in response to messages.
//! Handles sort toggling, row navigation, scan lifecycle, auto-refresh tracking,
//! fuzzy search, and multi-dimensional filtering.

use std::time::Instant;

use port_core::models::Filter;
use port_core::filter;

use crate::app::App;
use crate::message::{FilterField, Message, SortColumn, SortOrder};

/// Process a message and mutate application state accordingly.
pub fn update(app: &mut App, msg: Message) {
    match msg {
        Message::Quit => {
            app.should_quit = true;
        }
        Message::Tick => {
            // No state change — time-driven re-render handled by main loop
        }
        Message::Refresh => {
            app.scanning = true;
            app.error = None;
        }
        Message::ScanComplete(connections) => {
            app.ports = connections;
            app.scanning = false;
            app.error = None;
            app.last_scan_time = Some(Instant::now());
            app.last_auto_refresh = Some(Instant::now());
            // Re-apply active search or filter to new data
            if app.search_active && !app.search_query.is_empty() {
                app.filtered_ports = filter::fuzzy_search(&app.ports, &app.search_query);
            } else if app.filter_active {
                app.filtered_ports = filter::apply_filters(&app.ports, &app.active_filter);
            } else {
                app.filtered_ports = app.ports.clone();
            }
            // Reset selection to top on new data, clamped to display data
            app.selected_index = 0;
            // Re-apply current sort
            sort_ports(app);
        }
        Message::ScanError(e) => {
            // D-03: keep last successful data in ports, only set error
            app.error = Some(e);
            app.scanning = false;
        }
        Message::Sort(col) => {
            if app.sort_column == col {
                // Same column: cycle sort order
                app.sort_order = app.sort_order.cycle();
            } else {
                // Different column: start ascending
                app.sort_column = col;
                app.sort_order = SortOrder::Ascending;
            }
            sort_ports(app);
        }
        Message::MoveUp => {
            let len = display_len(app);
            if app.selected_index > 0 {
                app.selected_index -= 1;
            }
            // Clamp in case data changed
            if len > 0 && app.selected_index >= len {
                app.selected_index = len - 1;
            }
        }
        Message::MoveDown => {
            let len = display_len(app);
            if app.selected_index + 1 < len {
                app.selected_index += 1;
            }
        }
        Message::ScrollTop => {
            app.selected_index = 0;
        }
        Message::ScrollBottom => {
            let len = display_len(app);
            app.selected_index = len.saturating_sub(1);
        }

        // --- Search handlers ---

        Message::SearchActivate => {
            app.search_active = true;
            app.search_query.clear();
            app.search_cursor_pos = 0;
            app.filtered_ports = app.ports.clone();
        }
        Message::SearchDeactivate => {
            app.search_active = false;
            app.search_query.clear();
            app.search_cursor_pos = 0;
            // If filter is also active, apply filter instead
            if app.filter_active {
                app.filtered_ports = filter::apply_filters(&app.ports, &app.active_filter);
            } else {
                app.filtered_ports = app.ports.clone();
            }
        }
        Message::SearchClear => {
            app.search_query.clear();
            app.search_cursor_pos = 0;
            app.search_active = false;
            if app.filter_active {
                app.filtered_ports = filter::apply_filters(&app.ports, &app.active_filter);
            } else {
                app.filtered_ports = app.ports.clone();
            }
        }
        Message::SearchInput(ch) => {
            if app.search_active {
                if app.search_cursor_pos <= app.search_query.len() {
                    app.search_query.insert(app.search_cursor_pos, ch);
                } else {
                    app.search_query.push(ch);
                }
                app.search_cursor_pos += 1;
                recalc_search(app);
            }
        }
        Message::SearchBackspace => {
            if app.search_active && !app.search_query.is_empty() {
                if app.search_cursor_pos > 0 {
                    app.search_query.remove(app.search_cursor_pos - 1);
                    app.search_cursor_pos -= 1;
                }
                recalc_search(app);
            }
        }
        Message::SearchCursorLeft => {
            if app.search_active && app.search_cursor_pos > 0 {
                app.search_cursor_pos -= 1;
            }
        }
        Message::SearchCursorRight => {
            if app.search_active && app.search_cursor_pos < app.search_query.len() {
                app.search_cursor_pos += 1;
            }
        }

        // --- Filter handlers ---

        Message::FilterActivate => {
            app.filter_active = true;
            app.active_filter = Filter::default();
        }
        Message::FilterDeactivate => {
            app.filter_active = false;
            app.active_filter = Filter::default();
            // If search is also active, apply search instead
            if app.search_active && !app.search_query.is_empty() {
                app.filtered_ports = filter::fuzzy_search(&app.ports, &app.search_query);
            } else {
                app.filtered_ports = app.ports.clone();
            }
        }
        Message::FilterUpdateField(field, value) => {
            match field {
                FilterField::PortMin => {
                    if let Ok(n) = value.parse::<u16>() {
                        let (_, max) = app.active_filter.port_range.unwrap_or((0, 65535));
                        app.active_filter.port_range = Some((n, max));
                    }
                    // On parse failure: ignore (plan: user can retry)
                }
                FilterField::PortMax => {
                    if let Ok(n) = value.parse::<u16>() {
                        let (min, _) = app.active_filter.port_range.unwrap_or((0, 65535));
                        app.active_filter.port_range = Some((min, n));
                    }
                }
                FilterField::ProcessName => {
                    if value.trim().is_empty() {
                        app.active_filter.process_names.clear();
                    } else {
                        app.active_filter.process_names = vec![value];
                    }
                }
                FilterField::Pid => {
                    if value.trim().is_empty() {
                        app.active_filter.pids.clear();
                    } else if let Ok(n) = value.parse::<u32>() {
                        app.active_filter.pids = vec![n];
                    }
                }
                FilterField::Protocol => {
                    app.active_filter.protocols = match value.to_lowercase().trim() {
                        "tcp" => vec![port_core::models::Protocol::Tcp, port_core::models::Protocol::Tcp6],
                        "udp" => vec![port_core::models::Protocol::Udp, port_core::models::Protocol::Udp6],
                        "tcp4" => vec![port_core::models::Protocol::Tcp],
                        "tcp6" => vec![port_core::models::Protocol::Tcp6],
                        "udp4" => vec![port_core::models::Protocol::Udp],
                        "udp6" => vec![port_core::models::Protocol::Udp6],
                        "" => vec![],
                        _ => vec![], // unknown: clear
                    };
                }
                FilterField::State => {
                    app.active_filter.states = match value.to_lowercase().trim() {
                        "listen" | "listening" => vec![port_core::models::PortState::Listen],
                        "established" | "estab" => vec![port_core::models::PortState::Established],
                        "time_wait" | "timewait" | "t_wait" => vec![port_core::models::PortState::TimeWait],
                        "close_wait" | "closewait" | "c_wait" => vec![port_core::models::PortState::CloseWait],
                        "syn_sent" | "synsent" => vec![port_core::models::PortState::SynSent],
                        "" => vec![],
                        _ => vec![], // unknown: clear
                    };
                }
            }
        }
        Message::FilterTabField => {
            app.filter_focused_field = app.filter_focused_field.next();
        }
        Message::FilterApply => {
            app.filtered_ports = filter::apply_filters(&app.ports, &app.active_filter);
            // Clamp selection
            let len = display_len(app);
            if len > 0 && app.selected_index >= len {
                app.selected_index = len.saturating_sub(1);
            }
        }

        // --- Admin / elevation handlers ---

        Message::AdminCheck(is_admin) => {
            app.is_admin = is_admin;
            app.admin_check_done = true;
        }
        Message::ElevateRequest => {
            // Handled by the main event loop (triggers spawn_blocking).
            // The update function just records the intent.
            // The main loop sets app.elevating to prevent double-elevation.
        }
        Message::ElevateDeclined => {
            // UAC was declined — app continues in non-admin mode (D-07)
            app.elevating = false;
        }

        Message::SwitchTab(index) => {
            if index < 5 {
                app.active_tab = index;
            }
        }
    }
}

/// Return the length of the currently visible data.
fn display_len(app: &App) -> usize {
    if app.search_active || app.filter_active {
        app.filtered_ports.len()
    } else {
        app.ports.len()
    }
}

/// Recompute filtered_ports from the current search query.
fn recalc_search(app: &mut App) {
    if app.search_query.is_empty() {
        app.filtered_ports = app.ports.clone();
    } else {
        app.filtered_ports = filter::fuzzy_search(&app.ports, &app.search_query);
    }
    // Clamp selection
    let len = display_len(app);
    if len > 0 && app.selected_index >= len {
        app.selected_index = len.saturating_sub(1);
    }
}

/// Sort the port list in-place according to current sort_column and sort_order.
///
/// When search or filter is active, sort operates on filtered_ports.
/// SortOrder::None preserves current order.
fn sort_ports(app: &mut App) {
    if matches!(app.sort_order, SortOrder::None) {
        return;
    }

    let col = app.sort_column;
    let ascending = matches!(app.sort_order, SortOrder::Ascending);

    // Sort the active data view
    let data = if app.search_active || app.filter_active {
        &mut app.filtered_ports
    } else {
        &mut app.ports
    };

    data.sort_by(|a, b| {
        let cmp = match col {
            SortColumn::Port => a.port.number.cmp(&b.port.number),
            SortColumn::Protocol => {
                let pa = protocol_order(a.port.protocol);
                let pb = protocol_order(b.port.protocol);
                pa.cmp(&pb)
            }
            SortColumn::State => {
                let sa = state_order(a.port.state);
                let sb = state_order(b.port.state);
                sa.cmp(&sb)
            }
            SortColumn::ProcessName => {
                a.process.name.to_lowercase().cmp(&b.process.name.to_lowercase())
            }
            SortColumn::Pid => a.process.pid.cmp(&b.process.pid),
        };

        if ascending {
            cmp
        } else {
            cmp.reverse()
        }
    });
}

/// Map Protocol to a sortable integer.
fn protocol_order(p: port_core::models::Protocol) -> u8 {
    match p {
        port_core::models::Protocol::Tcp => 0,
        port_core::models::Protocol::Udp => 1,
        port_core::models::Protocol::Tcp6 => 2,
        port_core::models::Protocol::Udp6 => 3,
    }
}

/// Map PortState to a sortable integer (roughly by severity/importance).
fn state_order(s: port_core::models::PortState) -> u8 {
    match s {
        port_core::models::PortState::Listen => 0,
        port_core::models::PortState::Established => 1,
        port_core::models::PortState::CloseWait => 2,
        port_core::models::PortState::TimeWait => 3,
        port_core::models::PortState::FinWait1 => 4,
        port_core::models::PortState::FinWait2 => 5,
        port_core::models::PortState::LastAck => 6,
        port_core::models::PortState::Closing => 7,
        port_core::models::PortState::SynSent => 8,
        port_core::models::PortState::SynReceived => 9,
        port_core::models::PortState::Unknown => 10,
    }
}
