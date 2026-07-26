//! Windows TCP table enumeration — AF_INET GetExtendedTcpTable wrapper.
//!
//! Uses the two-call buffer pattern: first call gets required buffer size,
//! second call retrieves the actual data. Includes retry for table growth
//! between calls.

use std::collections::HashMap;

use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID,
    MIB_TCP_STATE_CLOSING, MIB_TCP_STATE_CLOSE_WAIT, MIB_TCP_STATE_ESTAB,
    MIB_TCP_STATE_FIN_WAIT1, MIB_TCP_STATE_FIN_WAIT2, MIB_TCP_STATE_LAST_ACK,
    MIB_TCP_STATE_LISTEN, MIB_TCP_STATE_SYN_RCVD, MIB_TCP_STATE_SYN_SENT,
    MIB_TCP_STATE_TIME_WAIT, TCP_TABLE_OWNER_PID_ALL,
};
use windows::Win32::Networking::WinSock::{ntohs, AF_INET};

use crate::models::{Connection, Port, PortState, ProcessInfo, Protocol};

/// Win32 error code for ERROR_INSUFFICIENT_BUFFER.
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;

/// Win32 error code for ERROR_SUCCESS / NO_ERROR.
const NO_ERROR: u32 = 0;

/// Maximum buffer retries for the two-call pattern.
const MAX_RETRIES: usize = 2;

/// Raw TCP table scan using GetExtendedTcpTable with two-call pattern.
///
/// Returns owned copies of MIB_TCPROW_OWNER_PID rows.
fn scan_tcp_table_raw(af: u32) -> crate::Result<Vec<MIB_TCPROW_OWNER_PID>> {
    let mut buffer_size: u32 = 0;
    let mut retries = 0;

    loop {
        // First call: get required buffer size
        let result = unsafe {
            GetExtendedTcpTable(
                None,
                &mut buffer_size,
                false, // sort by local port
                af,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };

        if result != ERROR_INSUFFICIENT_BUFFER {
            if result == NO_ERROR {
                // No data — empty table
                return Ok(Vec::new());
            }
            return Err(crate::Error::Platform(format!(
                "TCP scan size query failed: error code {}",
                result
            )));
        }

        // Allocate buffer with returned size
        let mut buffer: Vec<u8> = vec![0u8; buffer_size as usize];

        // Second call: retrieve actual data
        let result = unsafe {
            GetExtendedTcpTable(
                Some(buffer.as_mut_ptr() as *mut _),
                &mut buffer_size,
                false,
                af,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };

        if result == NO_ERROR {
            let table = buffer.as_ptr() as *const MIB_TCPTABLE_OWNER_PID;
            let num_entries = unsafe { (*table).dwNumEntries } as usize;

            let mut rows = Vec::with_capacity(num_entries);
            let first_row = unsafe { &(*table).table[0] as *const MIB_TCPROW_OWNER_PID };

            for i in 0..num_entries {
                let row = unsafe { first_row.add(i).read() };
                rows.push(row);
            }

            return Ok(rows);
        }

        // Check if table grew between calls
        if result == ERROR_INSUFFICIENT_BUFFER && retries < MAX_RETRIES {
            retries += 1;
            // Buffer size was already updated; double for retry
            buffer_size = buffer_size.saturating_mul(2);
            continue;
        }

        return Err(crate::Error::Platform(format!(
            "TCP scan data query failed after {} retries: error code {}",
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

/// Resolve process names from a set of PIDs using sysinfo.
fn resolve_process_names(pids: &[u32]) -> HashMap<u32, String> {
    use sysinfo::{Pid, System};

    let mut system = System::new_all();
    system.refresh_all();

    let mut names = HashMap::new();

    for &pid in pids {
        if pid == 0 {
            names.insert(pid, "<idle>".to_string());
            continue;
        }

        let name = system
            .process(Pid::from(pid as usize))
            .map(|p| p.name().to_string_lossy().to_string())
            .unwrap_or_else(|| "<access denied>".to_string());

        names.insert(pid, name);
    }

    names
}

/// Scan all active TCP ports on the local machine.
///
/// Calls `GetExtendedTcpTable` inside `tokio::task::spawn_blocking`
/// to avoid blocking the async runtime. Resolves process names
/// via `sysinfo::System` and caches by PID.
pub async fn scan_tcp() -> crate::Result<Vec<Connection>> {
    tokio::task::spawn_blocking(move || {
        let rows = scan_tcp_table_raw(AF_INET.0 as u32)?;

        // Collect unique PIDs for batch process name resolution
        let mut pid_set: Vec<u32> = rows.iter().map(|r| r.dwOwningPid).collect();
        pid_set.sort_unstable();
        pid_set.dedup();

        let process_names = resolve_process_names(&pid_set);

        let connections: Vec<Connection> = rows
            .iter()
            .map(|row| {
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
                let process_name = process_names
                    .get(&pid)
                    .cloned()
                    .unwrap_or_else(|| "<unknown>".to_string());

                Connection {
                    port: Port {
                        number: local_port,
                        protocol: Protocol::Tcp,
                        state: map_tcp_state(row.dwState),
                    },
                    process: ProcessInfo {
                        pid,
                        name: process_name,
                        executable_path: None,
                        command_line: None,
                        start_time: None,
                        is_signed: None,
                        is_system_critical: pid == 0 || pid == 4,
                        parent_pid: None,
                    },
                    remote_address: remote_addr,
                    remote_port,
                    bytes_sent: 0,
                    bytes_received: 0,
                }
            })
            .collect();

        Ok(connections)
    })
    .await
    .map_err(|e| crate::Error::Platform(format!("spawn_blocking failed: {}", e)))?
}
