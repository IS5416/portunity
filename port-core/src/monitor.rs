//! Runtime monitors — periodic poller + (Phase 3) ETW change trigger, traffic
//! counter. Both publish to the shared [`crate::events::EventBus`] and are
//! frontend-agnostic. New-style layout: `monitor.rs` (root) + leaves
//! `monitor/{etw,poller,traffic}.rs`.
//!
//! Note: the ETW change-trigger (`monitor::etw`) is intentionally NOT wired yet
//! — it needs a doc-grounded control-code verification and a real Windows ETW
//! session to validate (Phase 3 manual-UAT surface, see STATE.md / REVIEW-phase3-readiness.md).
//! The poller below is the verified fallback and is what drives live refresh
//! until ETW lands.

pub mod poller;

use std::time::Duration;

use crate::events::EventBus;

/// Default 2s polling cadence (ROADMAP.md Phase 3: "2s polling fallback").
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Spawn the periodic poller, publishing `PollTick` every `interval`.
///
/// Returns a task handle; dropping it (or the runtime) stops the poller.
pub fn spawn_poller(bus: EventBus, interval: Duration) -> tokio::task::JoinHandle<()> {
    poller::spawn(bus, interval)
}

