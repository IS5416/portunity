//! TEA update function — Message → State mutation.
//!
//! Pure function that transforms application state in response to messages.
//! Returns `None` for this tracer (no chained messages needed).

use crate::app::App;
use crate::message::Message;
use std::time::Instant;

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
        }
        Message::ScanError(e) => {
            // D-03: keep last successful data in ports, only set error
            app.error = Some(e);
            app.scanning = false;
        }
    }
}
