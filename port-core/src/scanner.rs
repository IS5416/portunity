//! Port scanning — system API abstraction and combined scan orchestration.
//!
//! The `PortScanner` trait defines the platform-agnostic interface.
//! `scan_all()` is the primary entry point: concurrent TCP + UDP
//! via `tokio::join!`, batch PID resolution, unified result set.
//! Individual protocol scanners (tcp, udp) are also available for
//! targeted use.

pub mod tcp;
pub mod udp;
pub mod resolver;

use async_trait::async_trait;

pub use resolver::ProcessResolver;
pub use tcp::scan_tcp;
pub use udp::scan_udp;

use crate::models::Connection;

/// Trait for platform-specific port scanning.
///
/// Implementations use OS APIs (Windows IP Helper, Linux /proc/net, etc.)
/// All implementations are `Send + Sync` for use across async boundaries.
#[async_trait]
pub trait PortScanner: Send + Sync {
    /// Scan all active TCP/UDP ports and return connections with process info.
    async fn scan(&self) -> crate::Result<Vec<Connection>>;

    /// Scan ports owned by a specific process.
    async fn scan_process(&self, pid: u32) -> crate::Result<Vec<Connection>>;
}

/// Scan all active TCP and UDP ports concurrently, batch-resolve process names.
///
/// Per D-04: uses `tokio::join!` to run TCP and UDP scans simultaneously.
/// Per D-16: collects all unique PIDs, resolves process names in a single
/// batch via `ProcessResolver`. Wall-clock time = max(TCP_scan, UDP_scan).
pub async fn scan_all() -> crate::Result<Vec<Connection>> {
    // Concurrent TCP + UDP scan (D-04)
    let (tcp_result, udp_result) = tokio::join!(scan_tcp(), scan_udp());

    let (mut tcp_conns, tcp_pids) = tcp_result?;
    let (udp_conns, udp_pids) = udp_result?;

    // Merge results
    tcp_conns.extend(udp_conns);

    // Collect all unique PIDs (D-16: batch process name resolution)
    let mut all_pids: Vec<u32> = tcp_pids;
    all_pids.extend(udp_pids);
    all_pids.sort_unstable();
    all_pids.dedup();

    // Resolve process names for all PIDs
    let mut resolver = ProcessResolver::new();
    resolver.resolve_batch(&all_pids)?;

    // Apply resolved names to all connections
    for conn in &mut tcp_conns {
        let pid = conn.process.pid;
        if let Some(name) = resolver.get(pid) {
            conn.process.name = name.to_string();
        }
    }

    Ok(tcp_conns)
}
