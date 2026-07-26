//! Port scanning — system API abstraction.

pub mod tcp;

use async_trait::async_trait;

/// Trait for platform-specific port scanning.
///
/// Implementations use OS APIs (Windows IP Helper, Linux /proc/net, etc.)
/// All implementations are `Send + Sync` for use across async boundaries.
#[async_trait]
pub trait PortScanner: Send + Sync {
    /// Scan all active TCP/UDP ports and return connections with process info.
    async fn scan(&self) -> crate::Result<Vec<crate::models::Connection>>;

    /// Scan ports owned by a specific process.
    async fn scan_process(&self, pid: u32) -> crate::Result<Vec<crate::models::Connection>>;
}
