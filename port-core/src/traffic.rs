//! Network traffic statistics per port/process.

pub trait TrafficMonitor {
    fn stats(&self) -> crate::Result<Vec<crate::models::TrafficStats>>;
    fn start_monitoring(&self) -> crate::Result<()>;
    fn stop_monitoring(&self);
}
