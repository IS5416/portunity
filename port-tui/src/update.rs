//! TEA update function — Message → State mutation.
//!
//! Pure function that transforms application state in response to messages.
//! Handles sort toggling, row navigation, scan lifecycle, auto-refresh tracking,
//! fuzzy search, and multi-dimensional filtering.

use std::time::Instant;

use port_core::filter;
use port_core::models::Filter;
use port_core::process::Protection;

use crate::app::{format_kill_status, App, KillStatus, KillTone, WhitelistFocus};
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
        Message::ElevateFailed(e) => {
            // Hard elevation failure (not a decline). Reset the elevating guard
            // (only ElevateDeclined did before — this latched-once bug left 'a'
            // dead for the session) and surface the error in the status bar.
            app.elevating = false;
            app.error = Some(e);
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

        // --- Whitelist overlay handlers (D-13..D-15, PROC-05) ---

        Message::ToggleWhitelistOverlay => {
            // Toggle the overlay. On OPEN: fresh settings read (D-15 working-
            // copy contract). Synchronous load — the UI-SPEC backstop declares
            // "no loading state exists by design" (settings.toml read is
            // <1ms); this is the one allowed sync file read on the runtime.
            app.whitelist_active = !app.whitelist_active;
            if app.whitelist_active {
                app.whitelist_settings = port_core::config::load_settings()
                    .unwrap_or_else(|_| port_core::config::default_settings());
                app.whitelist_focus = WhitelistFocus::List;
                app.whitelist_selected = 0;
                app.whitelist_input.clear();
                app.whitelist_input_cursor = 0;
            }
        }
        Message::WhitelistFocusNext => {
            app.whitelist_focus = app.whitelist_focus.next();
        }
        Message::WhitelistFocusPrev => {
            app.whitelist_focus = app.whitelist_focus.prev();
        }
        Message::WhitelistSelectMove { dir } => {
            // Move selection within the user list bounds (0..len, clamp).
            let len = app.whitelist_settings.whitelist.len();
            if len == 0 {
                return;
            }
            let target = app.whitelist_selected as i64 + dir as i64;
            app.whitelist_selected = target.clamp(0, len as i64 - 1) as usize;
        }
        Message::WhitelistDeleteSelected => {
            // No-op here — intercept-owned (spawn_blocking save in main.rs);
            // the removal lands when WhitelistSaved drains.
        }
        Message::WhitelistInput(ch) => {
            // Insert at the cursor (search-bar pattern).
            if app.whitelist_input_cursor <= app.whitelist_input.len() {
                app.whitelist_input.insert(app.whitelist_input_cursor, ch);
            } else {
                app.whitelist_input.push(ch);
            }
            app.whitelist_input_cursor += 1;
        }
        Message::WhitelistBackspace => {
            if app.whitelist_input_cursor > 0 && !app.whitelist_input.is_empty() {
                app.whitelist_input.remove(app.whitelist_input_cursor - 1);
                app.whitelist_input_cursor -= 1;
            }
        }
        Message::WhitelistCursorMove { dir } => {
            let len = app.whitelist_input.len();
            let target = app.whitelist_input_cursor as i64 + dir as i64;
            app.whitelist_input_cursor = target.clamp(0, len as i64) as usize;
        }
        Message::WhitelistAdd { .. } => {
            // No-op here — intercept-owned (validate + save in main.rs).
        }
        Message::WhitelistSaved { path, added } => {
            if added {
                // Add: push to the working copy (dedupe handled in the
                // intercept closure — a duplicate never reaches save).
                app.whitelist_settings.whitelist.push(path.clone());
                app.kill_status = Some(KillStatus {
                    text: whitelist_added_string(&path, app.term_width),
                    tone: KillTone::Info,
                });
            } else if app
                .whitelist_settings
                .whitelist
                .iter()
                .any(|e| e.eq_ignore_ascii_case(&path))
            {
                // Duplicate add — entry already present; no-op (D-13).
                app.kill_status = Some(KillStatus {
                    text: whitelist_duplicate_string(&path, app.term_width),
                    tone: KillTone::Info,
                });
            } else {
                // Removal — the delete intercept already applied it to the
                // working copy before saving; the message completes the
                // status string (D-15 instant effect).
                app.kill_status = Some(KillStatus {
                    text: whitelist_removed_string(&path, app.term_width),
                    tone: KillTone::Info,
                });
            }
        }
        Message::WhitelistError { path, reason } => {
            // UI-SPEC backstop: invalid path -> error, not added.
            app.kill_status = Some(KillStatus {
                text: whitelist_error_string(&path, &reason, app.term_width),
                tone: KillTone::Error,
            });
        }
        Message::ToggleHelp => {
            app.help_active = !app.help_active;
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

// ── Whitelist status-bar strings (D-13..D-15, UI-SPEC Copywriting) ──
//
// All strings fit `term_width` columns (Assumption A9): `{path}` truncates
// with U+2026 keeping the TAIL — the executable file name is the actionable
// part (UI-SPEC overflow rule).

/// "Added {path} — kills now require confirmation" (status.info).
fn whitelist_added_string(path: &str, term_width: u16) -> String {
    // Fixed chrome: "Added " (6) + " — kills now require confirmation" (33)
    let fixed = 39usize;
    let budget = term_width as usize - fixed;
    format!(
        "Added {} — kills now require confirmation",
        truncate_path_tail(path, budget)
    )
}

/// "Removed {path} — kills are instant again" (status.info).
fn whitelist_removed_string(path: &str, term_width: u16) -> String {
    // Fixed chrome: "Removed " (8) + " — kills are instant again" (27)
    let fixed = 35usize;
    let budget = term_width as usize - fixed;
    format!(
        "Removed {} — kills are instant again",
        truncate_path_tail(path, budget)
    )
}

/// "{path} is already on your protection list" (status.info — duplicate no-op).
fn whitelist_duplicate_string(path: &str, term_width: u16) -> String {
    // Fixed chrome: " is already on your protection list" (36)
    let fixed = 36usize;
    let budget = term_width as usize - fixed;
    format!(
        "{} is already on your protection list",
        truncate_path_tail(path, budget)
    )
}

/// "Cannot add {path}: {reason}" (status.error — UI-SPEC backstop).
fn whitelist_error_string(path: &str, reason: &str, term_width: u16) -> String {
    // Fixed chrome: "Cannot add " (11) + ": " (2) = 13
    let fixed = 13usize;
    let reason_short = truncate_ellipsis(reason, 48);
    let path_budget = (term_width as usize).saturating_sub(fixed + reason_short.chars().count());
    format!(
        "Cannot add {}: {}",
        truncate_path_tail(path, path_budget),
        reason_short
    )
}

/// Truncate a path keeping the tail segment (`…\dir\name.exe`) — the file
/// name is the actionable part (UI-SPEC overflow rule).
fn truncate_path_tail(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        return s.to_string();
    }
    let keep = max_len.saturating_sub(1);
    let tail: String = s.chars().skip(s.chars().count().saturating_sub(keep)).collect();
    format!("\u{2026}{}", tail)
}

/// Truncate a string to max_len chars, appending U+2026 if truncated.
fn truncate_ellipsis(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let truncated: String = s.chars().take(max_len.saturating_sub(1)).collect();
        format!("{}\u{2026}", truncated)
    } else {
        s.to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    /// All whitelist status strings fit the 80-col gate with any path length
    /// (A9 truncation keeps the path tail with U+2026).
    #[test]
    fn whitelist_status_strings_fit_80_cols() {
        let long_path = format!(
            "C:\\very\\long\\directory\\path\\that\\would\\overflow\\{}",
            "a".repeat(100)
        );
        let cases = [
            whitelist_added_string(&long_path, 80),
            whitelist_removed_string(&long_path, 80),
            whitelist_duplicate_string(&long_path, 80),
            whitelist_error_string(&long_path, "Path does not exist", 80),
            whitelist_error_string(&long_path, "could not save settings: disk full", 80),
        ];
        for s in cases {
            assert!(
                s.chars().count() <= 80,
                "'{}' is {} chars — must fit 80",
                s,
                s.chars().count()
            );
        }
    }

    #[test]
    fn whitelist_strings_keep_path_tail() {
        let long_path = format!(
            "C:\\very\\long\\directory\\path\\that\\would\\overflow\\{}",
            "a".repeat(100)
        );
        let s = whitelist_added_string(&long_path, 80);
        // The actionable tail (the file name region) survives with U+2026.
        assert!(s.contains('\u{2026}'));
        assert!(s.ends_with("— kills now require confirmation"));
        assert!(s.starts_with("Added "));
    }

    #[test]
    fn whitelist_strings_short_paths_unchanged() {
        let path = "C:\\apps\\node.exe";
        assert_eq!(
            whitelist_added_string(path, 80),
            "Added C:\\apps\\node.exe — kills now require confirmation"
        );
        assert_eq!(
            whitelist_removed_string(path, 80),
            "Removed C:\\apps\\node.exe — kills are instant again"
        );
        assert_eq!(
            whitelist_duplicate_string(path, 80),
            "C:\\apps\\node.exe is already on your protection list"
        );
    }
}
