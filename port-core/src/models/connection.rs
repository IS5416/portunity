//! Connection data models.

use super::port::Port;

/// A network connection tying a process to a local port and optional remote endpoint.
///
/// `local_address` is populated at scan time (Phase 3 history/traffic seam) —
/// the OS reports it for every row but it was previously discarded.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Connection {
    pub port: Port,
    pub process: super::process::ProcessInfo,
    /// Local address as a string ("" not used; `None` only when unavailable).
    pub local_address: Option<String>,
    pub remote_address: Option<String>,
    pub remote_port: Option<u16>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// A recorded change in port occupation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub event: HistoryEvent,
    pub port: Port,
    pub process: Option<super::process::ProcessInfo>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum HistoryEvent {
    Occupied,
    Released,
    Changed,
}

/// Filter for querying history.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct HistoryFilter {
    pub port: Option<u16>,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
    pub since: Option<chrono::DateTime<chrono::Utc>>,
    pub limit: Option<usize>,
}

/// Real-time traffic statistics for a connection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrafficStats {
    pub pid: u32,
    pub process_name: String,
    pub port: u16,
    pub protocol: super::port::Protocol,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub send_rate: f64,     // bytes per second (recent window)
    pub receive_rate: f64,  // bytes per second (recent window)
}

/// A Windows Firewall rule.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FirewallRule {
    pub name: String,
    pub direction: FirewallDirection,
    pub action: FirewallAction,
    pub protocol: Option<super::port::Protocol>,
    pub local_port: Option<u16>,
    pub remote_port: Option<u16>,
    pub program_path: Option<String>,
    pub enabled: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FirewallDirection {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FirewallAction {
    Allow,
    Block,
}

/// User-facing favorites and labels.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Favorite {
    pub port: u16,
    pub label: Option<String>,
    pub tags: Vec<String>,
}
