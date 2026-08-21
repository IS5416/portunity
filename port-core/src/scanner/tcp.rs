//! Windows TCP table enumeration — dual-stack (AF_INET + AF_INET6)
//! GetExtendedTcpTable wrapper with exponential buffer retry.
//!
//! Per D-01: start buffer at 16KB, double on ERROR_INSUFFICIENT_BUFFER,
//! max 3 retries. Per D-02: enumerate both AF_INET and AF_INET6 tables,
//! merge results, deduplicate IPv4-mapped IPv6 entries.

use std::collections::HashSet;

use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCP6TABLE_OWNER_PID,
    MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, MIB_TCP_STATE_CLOSING,
    MIB_TCP_STATE_CLOSE_WAIT, MIB_TCP_STATE_ESTAB, MIB_TCP_STATE_FIN_WAIT1,
    MIB_TCP_STATE_FIN_WAIT2, MIB_TCP_STATE_LAST_ACK, MIB_TCP_STATE_LISTEN,
    MIB_TCP_STATE_SYN_RCVD, MIB_TCP_STATE_SYN_SENT, MIB_TCP_STATE_TIME_WAIT,
    TCP_TABLE_OWNER_PID_ALL,
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

/// Scan the IPv4 TCP table (AF_INET) using exponential buffer retry.
/// Returns owned copies of MIB_TCPROW_OWNER_PID rows.
fn scan_tcp_table_raw() -> crate::Result<Vec<MIB_TCPROW_OWNER_PID>> {
    let mut buffer_size: u32 = INITIAL_BUFFER_SIZE;
    let mut retries = 0;

    loop {
        let mut buffer: Vec<u8> = vec![0u8; buffer_size as usize];

        let result = unsafe {
            GetExtendedTcpTable(
                Some(buffer.as_mut_ptr() as *mut _),
                &mut buffer_size,
                false,
                AF_INET.0 as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };

        if result == NO_ERROR {
            let table = buffer.as_ptr() as *const MIB_TCPTABLE_OWNER_PID;
            let num_entries = unsafe { (*table).dwNumEntries } as usize;

            let mut rows = Vec::with_capacity(num_entries);
            if num_entries > 0 {
                let first_row =
                    unsafe { &(*table).table[0] as *const MIB_TCPROW_OWNER_PID };
                for i in 0..num_entries {
                    let row = unsafe { first_row.add(i).read() };
                    rows.push(row);
                }
            }
            return Ok(rows);
        }

        if result == ERROR_INSUFFICIENT_BUFFER && retries < MAX_RETRIES {
            retries += 1;
            // The OS updated buffer_size to the required value.
            // Ensure we allocate at least double previous to converge (D-01).
            buffer_size = buffer_size.max(buffer_size.saturating_mul(2));
            continue;
        }

        return Err(crate::Error::Platform(format!(
            "TCP IPv4 scan failed after {} retries: error code {}",
            retries, result
        )));
    }
}

/// Scan the IPv6 TCP table (AF_INET6) using exponential buffer retry.
/// Returns owned copies of MIB_TCP6ROW_OWNER_PID rows.
fn scan_tcp6_table_raw() -> crate::Result<Vec<MIB_TCP6ROW_OWNER_PID>> {
    let mut buffer_size: u32 = INITIAL_BUFFER_SIZE;
    let mut retries = 0;

    loop {
        let mut buffer: Vec<u8> = vec![0u8; buffer_size as usize];

        let result = unsafe {
            GetExtendedTcpTable(
                Some(buffer.as_mut_ptr() as *mut _),
                &mut buffer_size,
                false,
                AF_INET6.0 as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };

        if result == NO_ERROR {
            let table = buffer.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID;
            let num_entries = unsafe { (*table).dwNumEntries } as usize;

            let mut rows = Vec::with_capacity(num_entries);
            if num_entries > 0 {
                let first_row =
                    unsafe { &(*table).table[0] as *const MIB_TCP6ROW_OWNER_PID };
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
            "TCP IPv6 scan failed after {} retries: error code {}",
            retries, result
        )));
    }
}

/// Map a Windows MIB_TCP_STATE constant (stored as u32 in dwState) to PortState.
fn map_tcp_state(dw_state: u32) -> PortState {
    let state_val = dw_state as i32;
    if state_val == MIB_TCP_STATE_LISTEN.0 {
        PortState::Listen
    } else if state_val == MIB_TCP_STATE_ESTAB.0 {
        PortState::Established
    } else if state_val == MIB_TCP_STATE_TIME_WAIT.0 {
        PortState::TimeWait
    } else if state_val == MIB_TCP_STATE_CLOSE_WAIT.0 {
        PortState::CloseWait
    } else if state_val == MIB_TCP_STATE_SYN_SENT.0 {
        PortState::SynSent
    } else if state_val == MIB_TCP_STATE_SYN_RCVD.0 {
        PortState::SynReceived
    } else if state_val == MIB_TCP_STATE_FIN_WAIT1.0 {
        PortState::FinWait1
    } else if state_val == MIB_TCP_STATE_FIN_WAIT2.0 {
        PortState::FinWait2
    } else if state_val == MIB_TCP_STATE_LAST_ACK.0 {
        PortState::LastAck
    } else if state_val == MIB_TCP_STATE_CLOSING.0 {
        PortState::Closing
    } else {
        PortState::Unknown
    }
}

/// Format a u32 IPv4 address from network byte order to dotted-decimal.
fn format_ipv4(addr: u32) -> String {
    let octets = u32::from_be(addr).to_be_bytes();
    format!("{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3])
}

/// Format a 16-byte IPv6 address as an RFC 5952 string.
///
/// If the address is an IPv4-mapped IPv6 address (`::ffff:a.b.c.d`),
/// the `is_ipv4_mapped` flag is set to true so the caller can deduplicate.
/// `pub(crate)` — reused by `udp.rs` for UDP6 local addresses (will move to
/// `std::net` when the formatter is consolidated).
pub(crate) fn format_ipv6(addr: &[u8; 16]) -> String {
    // Check for IPv4-mapped IPv6: first 10 bytes are 0, next 2 are 0xFF
    let is_mapped = addr[0..10].iter().all(|&b| b == 0)
        && addr[10] == 0xFF
        && addr[11] == 0xFF;

    if is_mapped {
        // Format as ::ffff:a.b.c.d
        format!(
            "::ffff:{}.{}.{}.{}",
            addr[12], addr[13], addr[14], addr[15]
        )
    } else {
        // RFC 5952 canonical representation
        let groups: Vec<String> = (0..8)
            .map(|i| {
                let hi = addr[i * 2] as u16;
                let lo = addr[i * 2 + 1] as u16;
                hi << 8 | lo
            })
            .map(|g| format!("{:x}", g))
            .collect();

        // Find longest run of zeros for :: compression
        let mut best_start = 0;
        let mut best_len = 0;
        let mut cur_start = 0;
        let mut cur_len = 0;

        for (i, g) in groups.iter().enumerate() {
            if g == "0" {
                if cur_len == 0 {
                    cur_start = i;
                }
                cur_len += 1;
            } else {
                if cur_len > best_len {
                    best_start = cur_start;
                    best_len = cur_len;
                }
                cur_len = 0;
            }
        }
        if cur_len > best_len {
            best_start = cur_start;
            best_len = cur_len;
        }

        if best_len > 1 {
            let mut result = String::new();
            for i in 0..best_start {
                if i > 0 {
                    result.push(':');
                }
                result.push_str(&groups[i]);
            }
            result.push_str("::");
            for i in (best_start + best_len)..8 {
                result.push_str(&groups[i]);
                if i < 7 {
                    result.push(':');
                }
            }
            // Handle edge case: all zeros
            if best_start == 0 && best_len == 8 {
                return "::".to_string();
            }
            // A run starting at group 0 already yields a leading "::"
            // (the compression push above) — prepending another colon
            // would produce an invalid ":::…" (IN-02). No fix-up needed.
            result
        } else {
            groups.join(":")
        }
    }
}

/// Check if an IPv6 address (16 bytes) is an IPv4-mapped IPv6 address.
fn is_ipv4_mapped(addr: &[u8; 16]) -> bool {
    addr[0..10].iter().all(|&b| b == 0) && addr[10] == 0xFF && addr[11] == 0xFF
}

/// Build a Connection from MIB_TCPROW_OWNER_PID (IPv4 row).
///
/// Process name is left empty — `scan_all` batch-resolves names via
/// `ProcessResolver` and fills them in (single sysinfo refresh, D-16).
fn connection_from_tcp_row(row: &MIB_TCPROW_OWNER_PID) -> Connection {
    let local_port = unsafe { ntohs(row.dwLocalPort as u16) };
    let remote_port_raw = unsafe { ntohs(row.dwRemotePort as u16) };
    let remote_port = if remote_port_raw == 0 {
        None
    } else {
        Some(remote_port_raw)
    };
    let remote_addr = if row.dwRemoteAddr == 0 {
        None
    } else {
        Some(format_ipv4(row.dwRemoteAddr))
    };

    let pid = row.dwOwningPid;
    let protocol = Protocol::Tcp;

    Connection {
        port: Port {
            number: local_port,
            protocol,
            state: map_tcp_state(row.dwState),
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
        remote_address: remote_addr,
        remote_port,
        bytes_sent: 0,
        bytes_received: 0,
    }
}

/// Build a Connection from MIB_TCP6ROW_OWNER_PID (IPv6 row).
///
/// Process name is left empty — resolved by `scan_all` (see above).
fn connection_from_tcp6_row(row: &MIB_TCP6ROW_OWNER_PID) -> Connection {
    let local_port = unsafe { ntohs(row.dwLocalPort as u16) };
    let remote_port_raw = unsafe { ntohs(row.dwRemotePort as u16) };
    let remote_port = if remote_port_raw == 0 {
        None
    } else {
        Some(remote_port_raw)
    };
    // WR-06: populate the remote address like the IPv4 row does — None
    // when there is no remote endpoint (listen rows), RFC 5952 string
    // otherwise. Previously hardcoded None left TCP6 rows showing "—"
    // while TCP4 rows showed the remote address.
    let remote_addr = if remote_port_raw == 0 {
        None
    } else {
        Some(format_ipv6(&row.ucRemoteAddr))
    };

    let pid = row.dwOwningPid;
    let protocol = Protocol::Tcp6;

    Connection {
        port: Port {
            number: local_port,
            protocol,
            state: map_tcp_state(row.dwState),
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
        local_address: Some(format_ipv6(&row.ucLocalAddr)),
        remote_address: remote_addr,
        remote_port,
        bytes_sent: 0,
        bytes_received: 0,
    }
}

/// Scan all active TCP ports on the local machine (dual-stack).
///
/// Per D-02: calls both AF_INET and AF_INET6 tables, merges results,
/// and deduplicates IPv4-mapped IPv6 entries.
///
/// Returns raw connections (process names EMPTY) + unique PIDs for batch
/// process resolution. Name resolution is the single responsibility of
/// `scanner::scan_all` — doing it here as well meant two full sysinfo
/// refreshes per combined scan, both discarded when scan_all re-resolved.
#[doc = "Per Pitfall #9 all blocking Win32 calls run in `spawn_blocking`."]
pub async fn scan_tcp() -> crate::Result<(Vec<Connection>, Vec<u32>)> {
    tokio::task::spawn_blocking(move || {
        // Scan both address families
        let v4_rows = scan_tcp_table_raw()?;
        let v6_rows = scan_tcp6_table_raw()?;

        // Collect unique PIDs from both tables
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
            .map(connection_from_tcp_row)
            .collect();

        // Build IPv6 connections, deduplicating IPv4-mapped entries (D-02)
        let mut seen: HashSet<(u16, Protocol, u32)> = connections
            .iter()
            .map(|c| (c.port.number, c.port.protocol, c.process.pid))
            .collect();

        for row in &v6_rows {
            let local_port = unsafe { ntohs(row.dwLocalPort as u16) };

            // D-02: an IPv4-mapped IPv6 address represents the same endpoint.
            // Keep the AF_INET entry (canonical), drop the IPv4-mapped Tcp6
            // duplicate. Key on (port, protocol, PID): a PID-0 TIME_WAIT v4 row
            // must NOT suppress a different process's mapped-v6 listener on the
            // same port (review: dedup key ignores process identity).
            if is_ipv4_mapped(&row.ucLocalAddr) {
                let key = (local_port, Protocol::Tcp, row.dwOwningPid);
                if seen.contains(&key) {
                    // Already have the AF_INET version for this PID — skip.
                    continue;
                }
            }

            let conn = connection_from_tcp6_row(row);
            seen.insert((conn.port.number, conn.port.protocol, conn.process.pid));
            connections.push(conn);
        }

        Ok((connections, pid_set))
    })
    .await
    .map_err(|e| crate::Error::Platform(format!("spawn_blocking failed: {}", e)))?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a 16-byte IPv6 address from 8 big-endian groups.
    fn ipv6(groups: [u16; 8]) -> [u8; 16] {
        let mut out = [0u8; 16];
        for (i, g) in groups.iter().enumerate() {
            out[i * 2] = (g >> 8) as u8;
            out[i * 2 + 1] = (g & 0xff) as u8;
        }
        out
    }

    #[test]
    fn format_ipv6_loopback_no_extra_colon() {
        // Regression for IN-02: the leading zero run must produce exactly
        // one "::" — the old prepend branch emitted ":::1".
        assert_eq!(format_ipv6(&ipv6([0, 0, 0, 0, 0, 0, 0, 1])), "::1");
    }

    #[test]
    fn format_ipv6_all_zeros() {
        assert_eq!(format_ipv6(&ipv6([0; 8])), "::");
    }

    #[test]
    fn format_ipv6_leading_run_compressed() {
        assert_eq!(format_ipv6(&ipv6([0, 0, 0, 0, 0, 1, 2, 3])), "::1:2:3");
    }

    #[test]
    fn format_ipv6_trailing_run_compressed() {
        assert_eq!(
            format_ipv6(&ipv6([0x2001, 0xdb8, 1, 2, 0, 0, 0, 0])),
            "2001:db8:1:2::"
        );
    }

    #[test]
    fn format_ipv6_middle_run_compressed() {
        assert_eq!(
            format_ipv6(&ipv6([0x2001, 0xdb8, 0, 0, 1, 0, 0, 1])),
            "2001:db8::1:0:0:1"
        );
    }

    #[test]
    fn format_ipv6_no_compression() {
        assert_eq!(
            format_ipv6(&ipv6([
                0x2001, 0xdb8, 0x85a3, 0x8d3, 0x1319, 0x8a2e, 0x370, 0x7334
            ])),
            "2001:db8:85a3:8d3:1319:8a2e:370:7334"
        );
    }

    #[test]
    fn format_ipv6_mapped_ipv4() {
        let mut addr = [0u8; 16];
        addr[10] = 0xff;
        addr[11] = 0xff;
        addr[12] = 192;
        addr[13] = 0;
        addr[14] = 2;
        addr[15] = 1;
        assert_eq!(format_ipv6(&addr), "::ffff:192.0.2.1");
    }

    /// Build a MIB_TCP6ROW_OWNER_PID row for tests.
    ///
    /// Ports are stored network-order by the OS: `ntohs(dwPort as u16)`
    /// must recover the logical port, so the test writes the byte-swapped
    /// u16 (`port.to_be()`) widened to u32.
    #[cfg(target_os = "windows")]
    fn tcp6_row(remote_port: u16, remote_addr: [u8; 16], dw_state: u32) -> MIB_TCP6ROW_OWNER_PID {
        MIB_TCP6ROW_OWNER_PID {
            ucLocalAddr: [0; 16],
            dwLocalScopeId: 0,
            dwLocalPort: u32::from(80u16.to_be()),
            ucRemoteAddr: remote_addr,
            dwRemoteScopeId: 0,
            dwRemotePort: u32::from(remote_port.to_be()),
            dwState: dw_state,
            dwOwningPid: 1234,
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn tcp6_remote_address_populated_when_port_set() {
        // Regression for WR-06: TCP6 rows must show the remote address
        // (previously hardcoded None)...
        let row = tcp6_row(443, ipv6([0, 0, 0, 0, 0, 0, 0, 1]), 5);
        let conn = connection_from_tcp6_row(&row);
        assert_eq!(conn.remote_address.as_deref(), Some("::1"));
        assert_eq!(conn.remote_port, Some(443));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn tcp6_remote_address_none_when_port_zero() {
        // ...and stay None for listen rows (dwRemotePort == 0), mirroring
        // the IPv4 row's logic.
        let row = tcp6_row(0, [0; 16], 2); // MIB_TCP_STATE_LISTEN = 2
        let conn = connection_from_tcp6_row(&row);
        assert_eq!(conn.remote_address, None);
        assert_eq!(conn.remote_port, None);
    }
}
