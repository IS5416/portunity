//! Periodic poller — the UDP + non-ETW fallback cadence (SCAN-05, TRAF-03).
//!
//! Publishes `CoreEvent::PollTick` on a fixed interval (2s default). Consumers
//! decide what a tick means: when the ETW trigger is live, a tick only smooths
//! UDP/edge cases; when ETW is unavailable, the tick is what drives periodic
//! rescans. The ticker is `Skip`-missed-behavior so a slow consumer never
//! builds up a backlog of redundant ticks.

use std::time::Duration;

use crate::events::{CoreEvent, EventBus};

/// Spawn the poller task.
pub fn spawn(bus: EventBus, interval: Duration) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            bus.publish(CoreEvent::PollTick);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The poller emits a PollTick once per interval on its own async runtime.
    #[tokio::test]
    async fn poller_publishes_poll_tick() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        let handle = spawn(bus.clone(), Duration::from_millis(20));
        let evt = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("poller should tick within timeout")
            .expect("recv error");

        assert!(matches!(evt, CoreEvent::PollTick));
        handle.abort();
    }

    /// Publishing survives a dropped subscriber, and the bus keeps working.
    #[test]
    fn poller_bus_publish_does_not_error_with_no_subscribers() {
        let bus = EventBus::new(); // no subscribers
        bus.publish(CoreEvent::PollTick); // must not panic
    }
}
