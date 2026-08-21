//! Windows UDP table enumeration — dual-stack (AF_INET + AF_INET6)
//! GetExtendedUdpTable wrapper with exponential buffer retry.
//!
//! Per SCAN-02: enumerates both AF_INET and AF_INET6 UDP endpoints.
//! UDP has no connection states — all entries get PortState::Unknown.
//! Per D-01: exponential buffer retry (16KB start, double, max 3 retries).
//! Per D-02: dual-stack merge with IPv4-mapped IPv6 deduplication.

use std::collections::HashSet;

use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedUdpTable, MIB_UDP6ROW_OWNER_PID, MIB_UDP6TABLE_OWNER_PID,
    MIB_UDPROW_OWNER_PID, MIB_UDPTABLE_OWNER_PID, UDP_TABLE_OWNER_PID,
};
use windows::Win32::Networking::WinSock::{ntohs, AF_INET, AF_INET6};

use crate::models::{Connection, Port, PortState, ProcessInfo, Protocol};

/// Win32 error code for ERROR_INSUFFICIENT_BUFFER.
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;

/// Win32 error code for ERROR_SUCCESS / NO_ERROR.
const NO_ERROR: u32 = 0;

/// Initial buffer size for the scan attempt (16KB per D-01).
const INITIAL_BUFFER_SIZE: u32 = 16384;

/// Maximum retries for exponential buffer growth (per D-01).
const MAX_RETRIES: usize = 3;

/// Scan the IPv4 UDP table (AF_INET) using exponential buffer retry.
fn scan_udp_table_raw() -> crate::Result<Vec<MIB_UDPROW_OWNER_PID>> {
    let mut buffer_size: u32 = INITIAL_BUFFER_SIZE;
    let mut retries = 0;

    loop {
        let mut buffer: Vec<u8> = vec![0u8; buffer_size as usize];

        let result = unsafe {
            GetExtendedUdpTable(
                Some(buffer.as_mut_ptr() as *mut _),
                &mut buffer_size,
                false,
                AF_INET.0 as u32,
                UDP_TABLE_OWNER_PID,
                0,
            )
        };

        if result == NO_ERROR {
            let table = buffer.as_ptr() as *const MIB_UDPTABLE_OWNER_PID;
            let num_entries = unsafe { (*table).dwNumEntries } as usize;

            let mut rows = Vec::with_capacity(num_entries);
            if num_entries > 0 {
                let first_row =
                    unsafe { &(*table).table[0] as *const MIB_UDPROW_OWNER_PID };
                for i in 0..num_entries {
                    let row = unsafe { first_row.add(i).read() };
                    rows.push(row);
                }
            }
            return Ok(rows);
        }

        if result == ERROR_INSUFFICIENT_BUFFER && retries < MAX_RETRIES {
            retries += 1;
            buffer_size = buffer_size.max(buffer_size.saturating_mul(2));
            continue;
        }

        return Err(crate::Error::Platform(format!(
            "UDP IPv4 scan failed after {} retries: error code {}",
            retries, result
        )));
    }
}

/// Scan the IPv6 UDP table (AF_INET6) using exponential buffer retry.
fn scan_udp6_table_raw() -> crate::Result<Vec<MIB_UDP6ROW_OWNER_PID>> {
    let mut buffer_size: u32 = INITIAL_BUFFER_SIZE;
    let mut retries = 0;

    loop {
        let mut buffer: Vec<u8> = vec![0u8; buffer_size as usize];

        let result = unsafe {
            GetExtendedUdpTable(
                Some(buffer.as_mut_ptr() as *mut _),
                &mut buffer_size,
                false,
                AF_INET6.0 as u32,
                UDP_TABLE_OWNER_PID,
                0,
            )
        };

        if result == NO_ERROR {
            let table = buffer.as_ptr() as *const MIB_UDP6TABLE_OWNER_PID;
            let num_entries = unsafe { (*table).dwNumEntries } as usize;

            let mut rows = Vec::with_capacity(num_entries);
            if num_entries > 0 {
                let first_row =
                    unsafe { &(*table).table[0] as *const MIB_UDP6ROW_OWNER_PID };
                for i in 0..num_entries {
                    let row = unsafe { first_row.add(i).read() };
                    rows.push(row);
                }
            }
            return Ok(rows);
        }

        if result == ERROR_INSUFFICIENT_BUFFER && retries < MAX_RETRIES {
            retries += 1;
            buffer_size = buffer_size.max(buffer_size.saturating_mul(2));
            continue;
        }

        return Err(crate::Error::Platform(format!(
            "UDP IPv6 scan failed after {} retries: error code {}",
            retries, result
        )));
    }
}

/// Check if an IPv6 address (16 bytes) is an IPv4-mapped IPv6 address.
fn is_ipv4_mapped(addr: &[u8; 16]) -> bool {
    addr[0..10].iter().all(|&b| b == 0) && addr[10] == 0xFF && addr[11] == 0xFF
}

/// Format a u32 IPv4 address from network byte order to dotted-decimal.
fn format_ipv4(addr: u32) -> String {
    let octets = u32::from_be(addr).to_be_bytes();
    format!("{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3])
}

/// Build a Connection from MIB_UDPROW_OWNER_PID (IPv4 row, no remote info).
///
/// Process name is left empty — `scan_all` batch-resolves names via
/// `ProcessResolver` (single sysinfo refresh, D-16).
fn connection_from_udp_row(row: &MIB_UDPROW_OWNER_PID) -> Connection {
    let local_port = unsafe { ntohs(row.dwLocalPort as u16) };
    let pid = row.dwOwningPid;

    Connection {
        port: Port {
            number: local_port,
            protocol: Protocol::Udp,
            state: PortState::Unknown, // UDP has no connection states
        },
        process: ProcessInfo {
            pid,
            name: String::new(),
            executable_path: None,
            command_line: None,
            start_time: None,
            is_signed: None,
            is_system_critical: pid == 0 || pid == 4,
            user_protected: false,
            parent_pid: None,
        },
        local_address: Some(format_ipv4(row.dwLocalAddr)),
        remote_address: None,
        remote_port: None,
        bytes_sent: 0,
        bytes_received: 0,
    }
}

/// Build a Connection from MIB_UDP6ROW_OWNER_PID (IPv6 row, no remote info).
///
/// Process name is left empty — resolved by `scan_all` (see above).
fn connection_from_udp6_row(row: &MIB_UDP6ROW_OWNER_PID) -> Connection {
    let local_port = unsafe { ntohs(row.dwLocalPort as u16) };
    let pid = row.dwOwningPid;

    Connection {
        port: Port {
            number: local_port,
            protocol: Protocol::Udp6,
            state: PortState::Unknown, // UDP has no connection states
        },
        process: ProcessInfo {
            pid,
            name: String::new(),
            executable_path: None,
            command_line: None,
            start_time: None,
            is_signed: None,
            is_system_critical: pid == 0 || pid == 4,
            user_protected: false,
            parent_pid: None,
        },
        local_address: Some(super::tcp::format_ipv6(&row.ucLocalAddr)),
        remote_address: None,
        remote_port: None,
        bytes_sent: 0,
        bytes_received: 0,
    }
}

/// Scan all active UDP ports on the local machine (dual-stack).
///
/// Per SCAN-02: calls GetExtendedUdpTable for both AF_INET and AF_INET6,
/// merges results with IPv4-mapped IPv6 deduplication.
///
/// Returns raw connections (process names EMPTY) + unique PIDs for batch
/// process resolution. Name resolution is the single responsibility of
/// `scanner::scan_all` (single sysinfo refresh, D-16) — resolving here as
/// well cost a second full refresh that scan_all then discarded.
#[doc = "Per Pitfall #9 all blocking Win32 calls run in `spawn_blocking`."]
pub async fn scan_udp() -> crate::Result<(Vec<Connection>, Vec<u32>)> {
    tokio::task::spawn_blocking(move || {
        let v4_rows = scan_udp_table_raw()?;
        let v6_rows = scan_udp6_table_raw()?;

        // Collect unique PIDs
        let mut pid_set: Vec<u32> = v4_rows
            .iter()
            .map(|r| r.dwOwningPid)
            .chain(v6_rows.iter().map(|r| r.dwOwningPid))
            .collect();
        pid_set.sort_unstable();
        pid_set.dedup();

        // Build IPv4 connections (names filled by scan_all)
        let mut connections: Vec<Connection> = v4_rows
            .iter()
            .map(connection_from_udp_row)
            .collect();

        // Deduplicate: track seen (port, protocol, pid) triples
        let mut seen: HashSet<(u16, Protocol, u32)> = connections
            .iter()
            .map(|c| (c.port.number, c.port.protocol, c.process.pid))
            .collect();

        // Build IPv6 connections, deduplicating IPv4-mapped entries (D-02).
        // Key on (port, protocol, PID): a PID-0 row for a port must not
        // suppress a different process's mapped-v6 endpoint on the same port.
        for row in &v6_rows {
            let local_port = unsafe { ntohs(row.dwLocalPort as u16) };

            if is_ipv4_mapped(&row.ucLocalAddr) {
                let key = (local_port, Protocol::Udp, row.dwOwningPid);
                if seen.contains(&key) {
                    continue;
                }
            }

            let conn = connection_from_udp6_row(row);
            seen.insert((conn.port.number, conn.port.protocol, conn.process.pid));
            connections.push(conn);
        }

        Ok((connections, pid_set))
    })
    .await
    .map_err(|e| crate::Error::Platform(format!("spawn_blocking failed: {}", e)))?
}
