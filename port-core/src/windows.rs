//! Windows platform implementation — IP Helper API, process resolution.
//!
//! Provides `WindowsPortScanner` implementing the `PortScanner` trait
//! via `GetExtendedTcpTable` and `sysinfo` for process name resolution.

use async_trait::async_trait;

use crate::models::Connection;
use crate::scanner::PortScanner;

/// Windows-specific port scanner using the IP Helper API.
///
/// Stateless — each `scan()` call is independent. All blocking
/// Win32 API calls run inside `tokio::task::spawn_blocking`.
pub struct WindowsPortScanner;

#[async_trait]
impl PortScanner for WindowsPortScanner {
    /// Scan all active TCP ports on the local machine.
    async fn scan(&self) -> crate::Result<Vec<Connection>> {
        crate::scanner::tcp::scan_tcp().await
    }

    /// Scan ports owned by a specific process.
    ///
    /// Returns `Error::NotFound` if no ports match the given PID.
    async fn scan_process(&self, pid: u32) -> crate::Result<Vec<Connection>> {
        let all = self.scan().await?;
        let filtered: Vec<Connection> = all
            .into_iter()
            .filter(|c| c.process.pid == pid)
            .collect();

        if filtered.is_empty() {
            return Err(crate::Error::NotFound(format!(
                "no ports found for PID {}",
                pid
            )));
        }

        Ok(filtered)
    }
}
