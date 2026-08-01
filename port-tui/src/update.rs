//! TEA update function — Message → State mutation.
//!
//! Pure function that transforms application state in response to messages.
//! Handles sort toggling, row navigation, scan lifecycle, auto-refresh tracking,
//! fuzzy search, and multi-dimensional filtering.

use std::time::Instant;

use port_core::filter;
use port_core::models::Filter;
use port_core::process::Protection;

use crate::app::{format_kill_status, App, KillStatus, KillTone};
use crate::message::{FilterField, Message, SortColumn, SortOrder};

/// Process a message and mutate application state accordingly.
pub fn update(app: &mut App, msg: Message) {
    match msg {
        Message::Quit => {
            app.should_quit = true;
        }
        Message::Refresh => {
            app.scanning = true;
            app.error = None;
        }
        Message::ScanComplete(connections) => {
            // A completed scan invalidates the transient kill marker
            // (drives row strikethrough until the freed port disappears).
            app.last_killed_pid = None;
            // Merge new scan data into existing list, preserving row order
            // for ports that persist between scans. New ports appear at end.
            app.ports = merge_scan_results(&app.ports, connections);
            app.scanning = false;
            app.error = None;
            app.last_scan_time = Some(Instant::now());
            app.last_auto_refresh = Some(Instant::now());
            // Re-apply active search or filter to new data
            if app.search_active && !app.search_query.is_empty() {
                app.filtered_ports = filter::fuzzy_search(&app.ports, &app.search_query);
            } else if app.filter_active || app.filter_applied {
                app.filtered_ports = filter::apply_filters(&app.ports, &app.active_filter);
            } else {
                app.filtered_ports = app.ports.clone();
            }
            // Preserve selection across refreshes; clamp if data shrank
            let len = display_len(app);
            if app.selected_index >= len {
                app.selected_index = len.saturating_sub(1);
            }
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
                // Same column: toggle sort direction (Asc ↔ Desc)
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
            app.filter_applied = false;
            app.active_filter = Filter::default();
            app.filter_field_text.clear();
            app.filtered_ports = app.ports.clone();
        }
        Message::FilterDeactivate => {
            app.filter_active = false;
            app.filter_applied = false;
            app.active_filter = Filter::default();
            app.filter_field_text.clear();
            // If search is also active, apply search instead
            if app.search_active && !app.search_query.is_empty() {
                app.filtered_ports = filter::fuzzy_search(&app.ports, &app.search_query);
            } else {
                app.filtered_ports = app.ports.clone();
            }
        }
        Message::FilterUpdateField(field, value) => {
            // Accumulate character into raw text buffer (no parsing)
            let buf = app.filter_field_text.entry(field.clone()).or_default();
            buf.push_str(&value);
        }
        Message::FilterFieldBackspace => {
            let buf = app.filter_field_text.entry(app.filter_focused_field.clone()).or_default();
            buf.pop();
        }
        Message::FilterTabField => {
            // Parse current field buffer into active_filter before advancing
            parse_filter_buffer(app);
            app.filter_focused_field = app.filter_focused_field.next();
        }
        Message::FilterTabBackward => {
            // Parse current field buffer into active_filter before reversing
            parse_filter_buffer(app);
            app.filter_focused_field = app.filter_focused_field.prev();
        }
        Message::FilterApply => {
            // Parse current field buffer, then apply all filters
            parse_filter_buffer(app);
            app.filtered_ports = filter::apply_filters(&app.ports, &app.active_filter);
            app.filter_active = false;
            app.filter_applied = true;
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

        // --- Kill flow handlers (D-01..D-04, D-09) ---

        Message::Kill { .. } => {
            // Handled by the main event loop (spawn_blocking: snapshot_for +
            // protection_status → KillPrepared). The update function records
            // no state — the row is the kill target, captured at keypress.
        }
        Message::KillPrepared {
            snapshot,
            protection,
            name,
            pid,
        } => {
            match protection {
                Protection::UserConfirm => {
                    // Gate the kill behind the confirmation dialog (D-09).
                    app.confirm_pid = Some(pid);
                    app.confirm_name = Some(name);
                    app.confirm_port = app
                        .display_data()
                        .get(app.selected_index)
                        .filter(|c| c.process.pid == pid)
                        .map(|c| c.port.number);
                    app.pending_kill_snapshot = Some(snapshot);
                }
                Protection::HardBlocked(_) => {
                    // Hard block — no kill path, no dialog (D-09).
                    // The reason string is the built-in entry's plain-language
                    // explanation; the status copy is the UI-SPEC locked string.
                    app.kill_status = Some(KillStatus {
                        text: format!(
                            "✗ {} is protected — it is critical to Windows. Killing it would crash or destabilize your system. Press w to review the whitelist.",
                            name
                        ),
                        tone: KillTone::Error,
                    });
                }
                Protection::None => {
                    // Handled by the drain-loop intercept (instant kill path) —
                    // never reaches update().
                }
            }
        }
        Message::KillConfirmed { .. } => {
            // Handled by the main event loop (spawn_blocking kill execution
            // using the pending snapshot). No state mutation here.
        }
        Message::KillCancelled => {
            // Dialog dismissed — process untouched (UI-SPEC L2-confirm).
            app.confirm_pid = None;
            app.confirm_name = None;
            app.confirm_port = None;
            app.pending_kill_snapshot = None;
        }
        Message::KillStart { name, pid } => {
            // Graceful signal dispatched — in-progress status (D-04).
            app.kill_status = Some(KillStatus {
                text: format!(
                    "Terminating {} (PID {}) — sending graceful close\u{2026}",
                    name, pid
                ),
                tone: KillTone::InProgress,
            });
        }
        Message::KillTimeout {
            name,
            pid,
            timeout_secs,
        } => {
            // Graceful timeout hit — force kill in progress.
            app.kill_status = Some(KillStatus {
                text: format!(
                    "Graceful close timed out ({}s) — force killing {} (PID {})\u{2026}",
                    timeout_secs, name, pid
                ),
                tone: KillTone::InProgress,
            });
        }
        Message::KillExecute { .. } => {
            // Handled by the main event loop (intercept-owned).
        }
        // --- Detail panel handlers (D-05..D-08, PROC-06) ---

        Message::ToggleDetailPanel => {
            // Toggle the panel. The fetch fires from the event-loop intercept
            // (spawn_blocking) — this handler only mutates state. Opening
            // records the selected row as the panel's PID; closing clears the
            // transient loading flag.
            app.detail_active = !app.detail_active;
            if app.detail_active {
                // detail_pid is set by the intercept before update() runs;
                // fall back to the selected row if no intercept ran.
                if app.detail_pid.is_none() {
                    app.detail_pid = app.selected_connection().map(|c| c.process.pid);
                }
            } else {
                app.detail_loading = false;
            }
        }
        Message::DetailDataLoaded { process_info } => {
            // Store the fetched detail data (the drain-loop special case
            // spawns the signature verification when the cache misses).
            app.detail_data = Some(process_info);
            app.detail_loading = false;
        }
        Message::SignatureVerified { pid, is_signed } => {
            // D-07 cache: store the verdict and, when it belongs to the
            // shown row, refresh the panel copy immediately.
            app.signature_cache.insert(pid, is_signed);
            if app.detail_pid == Some(pid) {
                if let Some(ref mut info) = app.detail_data {
                    if info.pid == pid {
                        info.is_signed = is_signed;
                    }
                }
            }
        }
        Message::ProcessExited { pid } => {
            // The shown process left the scan list — strikethrough + "Exited".
            if app.detail_pid == Some(pid) {
                app.detail_exited = true;
            }
        }

        Message::KillOutcome {
            outcome,
            name,
            pid,
        } => {
            // Final outcome → status bar (D-04); clear the dialog + in-flight
            // guard; successful kills trigger one immediate scan so the freed
            // port disappears (D-04 auto-refresh).
            let (text, tone) = format_kill_status(
                &name,
                pid,
                &outcome,
                app.kill_timeout_secs,
                app.term_width,
            );
            app.kill_status = Some(KillStatus { text, tone });

            app.kill_in_flight = false;
            app.confirm_pid = None;
            app.confirm_name = None;
            app.confirm_port = None;
            app.pending_kill_snapshot = None;

            match outcome {
                port_core::process::KillOutcome::Graceful
                | port_core::process::KillOutcome::ForceKilled
                | port_core::process::KillOutcome::Direct => {
                    app.last_killed_pid = Some(pid);
                }
                _ => {}
            }

            app.scanning = true;
        }
    }
}

/// Return the length of the currently visible data.
fn display_len(app: &App) -> usize {
    if app.search_active || app.filter_active || app.filter_applied {
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

/// Merge new scan results into the existing port list.
///
/// Ports that exist in both old and new lists keep their position.
/// Ports only in new appear at the end. Ports only in old are dropped.
/// This prevents the entire table from shifting on every auto-refresh.
fn merge_scan_results(
    old: &[port_core::models::Connection],
    new: Vec<port_core::models::Connection>,
) -> Vec<port_core::models::Connection> {
    use port_core::models::Connection;
    use std::collections::HashMap;

    // Index new connections by (port_number, protocol)
    let mut new_map: HashMap<(u16, port_core::models::Protocol), Connection> = HashMap::new();
    for c in new {
        new_map.insert((c.port.number, c.port.protocol), c);
    }

    let mut result: Vec<Connection> = Vec::with_capacity(new_map.len());

    // First pass: keep existing order for ports that still exist
    for old_conn in old {
        let key = (old_conn.port.number, old_conn.port.protocol);
        if let Some(updated) = new_map.remove(&key) {
            result.push(updated);
        }
    }

    // Second pass: append brand-new ports (sorted by port number for stability)
    let mut newcomers: Vec<Connection> = new_map.into_values().collect();
    newcomers.sort_by_key(|c| c.port.number);
    result.extend(newcomers);

    result
}

/// Parse the current focused field's text buffer into active_filter.
fn parse_filter_buffer(app: &mut App) {
    let field = app.filter_focused_field.clone();
    let value = app
        .filter_field_text
        .get(&field)
        .cloned()
        .unwrap_or_default();
    if value.is_empty() {
        return;
    }

    match field {
        FilterField::PortMin => {
            if let Ok(n) = value.parse::<u16>() {
                let (_, max) = app.active_filter.port_range.unwrap_or((0, 65535));
                app.active_filter.port_range = Some((n, max));
            }
        }
        FilterField::PortMax => {
            if let Ok(n) = value.parse::<u16>() {
                let (min, _) = app.active_filter.port_range.unwrap_or((0, 65535));
                app.active_filter.port_range = Some((min, n));
            }
        }
        FilterField::ProcessName => {
            app.active_filter.process_names = vec![value];
        }
        FilterField::Pid => {
            if let Ok(n) = value.parse::<u32>() {
                app.active_filter.pids = vec![n];
            }
        }
        FilterField::Protocol => {
            app.active_filter.protocols = match value.to_lowercase().trim() {
                "tcp" => vec![
                    port_core::models::Protocol::Tcp,
                    port_core::models::Protocol::Tcp6,
                ],
                "udp" => vec![
                    port_core::models::Protocol::Udp,
                    port_core::models::Protocol::Udp6,
                ],
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
                "established" | "estab" => {
                    vec![port_core::models::PortState::Established]
                }
                "time_wait" | "timewait" | "t_wait" => {
                    vec![port_core::models::PortState::TimeWait]
                }
                "close_wait" | "closewait" | "c_wait" => {
                    vec![port_core::models::PortState::CloseWait]
                }
                "syn_sent" | "synsent" => vec![port_core::models::PortState::SynSent],
                "" => vec![],
                _ => vec![], // unknown: clear
            };
        }
    }
}

/// Sort the port list in-place according to current sort_column and sort_order.
///
/// When search or filter is active, sort operates on filtered_ports.
/// Always applies sort — Ascending or Descending.
fn sort_ports(app: &mut App) {

    let col = app.sort_column;
    let ascending = matches!(app.sort_order, SortOrder::Ascending);

    // Sort the active data view
    let data = if app.search_active || app.filter_active || app.filter_applied {
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
