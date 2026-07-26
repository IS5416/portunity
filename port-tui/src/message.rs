//! Message enum for the TEA event loop.
//!
//! All user actions and system events are represented as `Message` variants.
//! The main loop maps crossterm events to Messages and the `update()` function
//! transforms application state.

use port_core::models::Connection;

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
}
