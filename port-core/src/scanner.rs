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

use std::collections::HashSet;

use async_trait::async_trait;

pub use resolver::ProcessResolver;
pub use tcp::scan_tcp;
pub use udp::scan_udp;

use crate::config::AppSettings;
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

    // Protection marker post-pass (RESEARCH Pattern 4, plan 02-02 Task 1):
    // built-in basename match -> is_system_critical; user-entry basename
    // match -> QueryFullProcessImageNameW -> user_match -> user_protected.
    // One spawn_blocking scope (Pitfall #9) with fresh settings (D-15).
    // This keeps filter.rs's `system_only` filter truthful and feeds the
    // ◆ table markers at render time (O(1) per row — no per-row OpenProcess).
    let conns = tokio::task::spawn_blocking(move || {
        let settings =
            crate::config::load_settings().unwrap_or_else(|_| crate::config::default_settings());
        apply_protection_postpass(&mut tcp_conns, &settings, crate::process::info::query_full_path);
        tcp_conns
    })
    .await
    .map_err(|e| crate::Error::Platform(format!("spawn_blocking join error: {}", e)))?;

    Ok(conns)
}

/// Apply scan-time protection markers (RESEARCH Pattern 4).
///
/// For each connection, the built-in tier is checked FIRST (Pitfall #6): a
/// built-in basename sets `is_system_critical` and the user tier is skipped.
/// Otherwise, when the process basename matches a user-whitelist entry's
/// basename, the full executable path is resolved (via `path_for`) and
/// `user_match` decides `user_protected`. Only same-basename processes pay
/// the ~1ms path query (D-10: user entries match full paths).
///
/// The `path_for` closure abstracts QueryFullProcessImageNameW so the marker
/// logic is unit-testable without a live Windows process.
pub(crate) fn apply_protection_postpass<F>(
    conns: &mut [Connection],
    settings: &AppSettings,
    path_for: F,
) where
    F: Fn(u32) -> Option<String>,
{
    let user_entry_basenames: HashSet<String> = settings
        .whitelist
        .iter()
        .filter_map(|p| std::path::Path::new(p).file_name().and_then(|f| f.to_str()))
        .map(|s| s.to_lowercase())
        .collect();

    for conn in conns {
        let basename = conn.process.name.as_str();
        if crate::process::builtin_match(conn.process.pid, basename).is_some() {
            conn.process.is_system_critical = true;
        } else if user_entry_basenames.contains(&basename.to_lowercase()) {
            if let Some(path) = path_for(conn.process.pid) {
                conn.process.user_protected =
                    crate::process::user_match(&path, &settings.whitelist);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppSettings;
    use crate::models::{Port, PortState, Protocol};

    fn conn(pid: u32, name: &str) -> Connection {
        Connection {
            port: Port {
                number: 80,
                protocol: Protocol::Tcp,
                state: PortState::Listen,
            },
            process: crate::models::ProcessInfo {
                pid,
                name: name.to_string(),
                executable_path: None,
                command_line: None,
                start_time: None,
                is_signed: None,
                is_system_critical: false,
                user_protected: false,
                parent_pid: None,
            },
            remote_address: None,
            remote_port: None,
            bytes_sent: 0,
            bytes_received: 0,
        }
    }

    fn settings(whitelist: Vec<String>) -> AppSettings {
        AppSettings {
            admin_detected: false,
            schema_version: 1,
            whitelist,
            kill_timeout_secs: 5,
        }
    }

    // ── protection marker post-pass (RESEARCH Pattern 4) ──

    /// A built-in basename (csrss.exe) sets is_system_critical via
    /// builtin_match without any path query.
    #[test]
    fn postpass_marks_builtin_basename_system_critical() {
        let mut conns = vec![conn(100, "csrss.exe")];
        apply_protection_postpass(&mut conns, &settings(vec![]), |_| None);
        assert!(conns[0].process.is_system_critical);
        assert!(!conns[0].process.user_protected);
    }

    /// A basename matching a user-whitelist entry's basename triggers the
    /// full-path query; a matching path sets user_protected.
    #[test]
    fn postpass_marks_user_whitelist_path_protected() {
        let s = settings(vec![r"C:\apps\node.exe".to_string()]);
        let mut conns = vec![conn(123, "node.exe")];
        apply_protection_postpass(&mut conns, &s, |_| Some(r"C:\apps\node.exe".to_string()));
        assert!(!conns[0].process.is_system_critical);
        assert!(conns[0].process.user_protected);
    }

    /// Built-in tier wins over the user tier (Pitfall #6) — a process that is
    /// both built-in and user-listed is marked system-critical only.
    #[test]
    fn postpass_builtin_wins_over_user_tier() {
        let s = settings(vec![r"C:\Windows\System32\csrss.exe".to_string()]);
        let mut conns = vec![conn(100, "csrss.exe")];
        apply_protection_postpass(&mut conns, &s, |_| {
            Some(r"C:\Windows\System32\csrss.exe".to_string())
        });
        assert!(conns[0].process.is_system_critical);
        assert!(!conns[0].process.user_protected);
    }

    /// PID 4 (System) is always system-critical regardless of name.
    #[test]
    fn postpass_pid_4_always_critical() {
        let mut conns = vec![conn(4, "System")];
        apply_protection_postpass(&mut conns, &settings(vec![]), |_| None);
        assert!(conns[0].process.is_system_critical);
    }

    /// A non-matching basename with a non-matching resolved path stays
    /// unprotected — the markers remain false.
    #[test]
    fn postpass_unmatched_unchanged() {
        let s = settings(vec![r"C:\apps\node.exe".to_string()]);
        let mut conns = vec![conn(999, "myapp.exe")];
        apply_protection_postpass(&mut conns, &s, |_| {
            Some(r"C:\other\myapp.exe".to_string())
        });
        assert!(!conns[0].process.is_system_critical);
        assert!(!conns[0].process.user_protected);
    }

    /// A user-entry basename whose resolved path does NOT match the whitelist
    /// entry stays unprotected (same name, different path — D-10 semantics).
    #[test]
    fn postpass_user_basename_with_unmatched_path_stays_unprotected() {
        let s = settings(vec![r"C:\apps\node.exe".to_string()]);
        let mut conns = vec![conn(123, "node.exe")];
        apply_protection_postpass(&mut conns, &s, |_| {
            Some(r"D:\tools\node.exe".to_string())
        });
        assert!(!conns[0].process.is_system_critical);
        assert!(!conns[0].process.user_protected);
    }
}
