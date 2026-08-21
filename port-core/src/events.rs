//! EventBus — weakly-coupled producer/consumer broadcast (CORE-03).
//!
//! Decouples producers (scanner, ETW monitor, poller, traffic monitor) from
//! consumers (history recorder, TUI adapter, GUI forwarding). A multi-producer
//! `tokio::sync::broadcast` channel: any number of subscribers get every
//! published event. Edition rule: `EventBus` is `Clone` and `Send + Sync`, so
//! one handle can be spawned into many tasks.
//!
//! Consumers that must NOT drop events (the history recorder) use their own
//! bounded task channel rather than relying on broadcast's laggy-subscriber
//! drop policy (bounded-mpsc recommendation from `.planning/review/REVIEW-phase3-readiness.md`).

use tokio::sync::broadcast;

use crate::models::{Connection, TrafficStats};

/// Core application events published on the bus.
#[derive(Debug, Clone)]
pub enum CoreEvent {
    /// A scan completed with fresh port data (published after `scan_all`).
    PortsScanned(Vec<Connection>),
    /// ETW observed a network/traffic change — a rescan should be triggered.
    ///
    /// Trigger-only by design (STATE.md:43, PITFALLS #15): the event payload
    /// is never trusted for attribution; `scan_all` via `GetExtendedTcpTable`
    /// is the ground truth.
    NetworkChanged,
    /// The poller fired (periodic tick, ~2s). Covers UDP and the non-ETW fallback.
    PollTick,
    /// Live-mode status changed (ETW available vs polling-only).
    LiveMode { etw: bool },
    /// Per-second traffic snapshot (published by the traffic monitor, Wave 3.3).
    TrafficUpdate(Vec<TrafficStats>),
}

/// A multi-producer broadcast bus for `CoreEvent`s.
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<CoreEvent>,
}

impl EventBus {
    /// Capacity 256 — enough headroom for per-second snapshots and scan
    /// completions; consumers must drain promptly (Pitfall #6 discipline).
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(256);
        Self { tx }
    }

    /// Subscribe to all future events.
    pub fn subscribe(&self) -> broadcast::Receiver<CoreEvent> {
        self.tx.subscribe()
    }

    /// Broadcast an event to every subscriber.
    pub fn publish(&self, event: CoreEvent) {
        // A send only fails if there are no receivers left — fine to ignore.
        let _ = self.tx.send(event);
    }

    /// Number of current subscribers (test/observability helper).
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_reaches_all_subscribers() {
        let bus = EventBus::new();
        let mut a = bus.subscribe();
        let mut b = bus.subscribe();

        bus.publish(CoreEvent::NetworkChanged);

        assert!(
            matches!(a.blocking_recv(), Ok(CoreEvent::NetworkChanged)),
            "subscriber A missed the event"
        );
        assert!(
            matches!(b.blocking_recv(), Ok(CoreEvent::NetworkChanged)),
            "subscriber B missed the event"
        );
    }

    #[test]
    fn subscriber_counts() {
        let bus = EventBus::new();
        assert_eq!(bus.subscriber_count(), 0);
        let rx = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);
        drop(rx);
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn default_is_empty_bus() {
        let bus = EventBus::default();
        let mut rx = bus.subscribe();
        bus.publish(CoreEvent::LiveMode { etw: true });
        assert!(matches!(rx.blocking_recv(), Ok(CoreEvent::LiveMode { etw: true })));
    }
}
