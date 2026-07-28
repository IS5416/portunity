//! TEA update function — Message → State mutation.
//!
//! Pure function that transforms application state in response to messages.
//! Handles sort toggling, row navigation, scan lifecycle, and auto-refresh tracking.

use std::time::Instant;

use crate::app::App;
use crate::message::{Message, SortColumn, SortOrder};

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
            // Reset selection to top on new data
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
            if app.selected_index > 0 {
                app.selected_index -= 1;
            }
        }
        Message::MoveDown => {
            if app.selected_index + 1 < app.ports.len() {
                app.selected_index += 1;
            }
        }
        Message::ScrollTop => {
            app.selected_index = 0;
        }
        Message::ScrollBottom => {
            app.selected_index = app.ports.len().saturating_sub(1);
        }
    }
}

/// Sort the port list in-place according to current sort_column and sort_order.
///
/// SortOrder::None preserves insertion order (scanner default).
fn sort_ports(app: &mut App) {
    if matches!(app.sort_order, SortOrder::None) {
        return;
    }

    let col = app.sort_column;
    let ascending = matches!(app.sort_order, SortOrder::Ascending);

    app.ports.sort_by(|a, b| {
        let cmp = match col {
            SortColumn::Port => a.port.number.cmp(&b.port.number),
            SortColumn::Protocol => {
                // Sort by protocol discriminant (Tcp=0, Udp=1, Tcp6=2, Udp6=3)
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
