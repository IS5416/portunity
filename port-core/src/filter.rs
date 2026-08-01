//! Multi-dimensional port/process filtering engine.
//!
//! Provides free functions for combined dimension filtering (AND across dimensions,
//! OR within Vec dimensions) and fuzzy text search across all connection fields.
//!
//! No trait needed — the filter module has no platform-specific variants.

use crate::models::{Connection, Filter};

/// Apply multi-dimensional filters to a connection list.
///
/// Filter dimensions are combined with AND logic (a connection must satisfy
/// ALL active dimensions). Within each Vec-based dimension, OR logic applies
/// (matching any entry qualifies). Empty Vecs and `None` fields are treated
/// as "no filter" (pass-all).
///
/// Dimensions:
/// - `port_range`: port.number must be within [min, max] inclusive
/// - `protocols`: port.protocol must equal one of the listed values
/// - `process_names`: process.name must contain one of the listed values (case-insensitive substring)
/// - `pids`: process.pid must equal one of the listed values
/// - `states`: port.state must equal one of the listed values
/// - `search_text`: fuzzy search across concatenated fields (case-insensitive substring)
/// - `system_only`: if Some(true), only system-critical processes; if Some(false), only non-system
/// - `remote_only`: if Some(true), only connections with remote address; if Some(false), only without
/// - `favorite_only`: reserved for Phase 6 (currently pass-all)
pub fn apply_filters(connections: &[Connection], filter: &Filter) -> Vec<Connection> {
    let mut result: Vec<Connection> = connections.to_vec();

    // --- Port range filter ---
    if let Some((min, max)) = filter.port_range {
        result.retain(|c| c.port.number >= min && c.port.number <= max);
    }

    // --- Protocol filter (OR within Vec) ---
    if !filter.protocols.is_empty() {
        result.retain(|c| filter.protocols.iter().any(|p| c.port.protocol == *p));
    }

    // --- Process name filter: case-insensitive substring match (OR within Vec) ---
    if !filter.process_names.is_empty() {
        result.retain(|c| {
            let name_lower = c.process.name.to_lowercase();
            filter
                .process_names
                .iter()
                .any(|n| name_lower.contains(&n.to_lowercase()))
        });
    }

    // --- PID filter (OR within Vec) ---
    if !filter.pids.is_empty() {
        result.retain(|c| filter.pids.iter().any(|pid| c.process.pid == *pid));
    }

    // --- State filter (OR within Vec) ---
    if !filter.states.is_empty() {
        result.retain(|c| filter.states.iter().any(|s| c.port.state == *s));
    }

    // --- Fuzzy search text ---
    if let Some(ref query) = filter.search_text {
        if !query.trim().is_empty() {
            result = fuzzy_search(&result, query);
        }
    }

    // --- System-only filter ---
    if let Some(system_only) = filter.system_only {
        result.retain(|c| c.process.is_system_critical == system_only);
    }

    // --- Remote-only filter ---
    if let Some(remote_only) = filter.remote_only {
        if remote_only {
            result.retain(|c| c.remote_address.is_some());
        } else {
            result.retain(|c| c.remote_address.is_none());
        }
    }

    // --- Favorite-only filter (reserved, currently pass-all) ---
    // Phase 6 will integrate with the favorites database.

    result
}

/// Fuzzy search across all connection fields.
///
/// Builds a searchable string from concatenated port number, process name,
/// PID, protocol, and state, then performs a case-insensitive substring match.
/// Empty or whitespace query returns all connections unchanged.
///
/// This is intentionally simple (substring across concatenated fields).
/// Full fuzzy matching (typo-tolerant / Levenshtein) is a Phase 6 enhancement
/// when search history and ranking are added.
pub fn fuzzy_search(connections: &[Connection], query: &str) -> Vec<Connection> {
    let query = query.trim().to_lowercase();

    if query.is_empty() {
        return connections.to_vec();
    }

    connections
        .iter()
        .filter(|c| {
            let searchable = format!(
                "{} {} {} {:?} {:?}",
                c.port.number,
                c.process.name,
                c.process.pid,
                c.port.protocol,
                c.port.state
            )
            .to_lowercase();

            searchable.contains(&query)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Port, PortState, Protocol};
    use crate::models::process::ProcessInfo;

    fn make_conn(port_num: u16, name: &str, pid: u32, proto: Protocol, state: PortState) -> Connection {
        Connection {
            port: Port {
                number: port_num,
                protocol: proto,
                state,
            },
            process: ProcessInfo {
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

    #[test]
    fn port_range_filter() {
        let conns = vec![
            make_conn(80, "http", 1, Protocol::Tcp, PortState::Listen),
            make_conn(3000, "node", 2, Protocol::Tcp, PortState::Listen),
            make_conn(8080, "nginx", 3, Protocol::Tcp, PortState::Listen),
        ];
        let filter = Filter {
            port_range: Some((1000, 9000)),
            ..Default::default()
        };
        let result = apply_filters(&conns, &filter);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn protocol_filter() {
        let conns = vec![
            make_conn(80, "http", 1, Protocol::Tcp, PortState::Listen),
            make_conn(53, "dns", 2, Protocol::Udp, PortState::Unknown),
        ];
        let filter = Filter {
            protocols: vec![Protocol::Udp],
            ..Default::default()
        };
        let result = apply_filters(&conns, &filter);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].port.number, 53);
    }

    #[test]
    fn process_name_substring_filter() {
        let conns = vec![
            make_conn(80, "nginx.exe", 1, Protocol::Tcp, PortState::Listen),
            make_conn(3000, "node.exe", 2, Protocol::Tcp, PortState::Listen),
            make_conn(8080, "python.exe", 3, Protocol::Tcp, PortState::Listen),
        ];
        let filter = Filter {
            process_names: vec!["gin".to_string()],
            ..Default::default()
        };
        let result = apply_filters(&conns, &filter);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].process.name, "nginx.exe");
    }

    #[test]
    fn combined_and_filter() {
        let conns = vec![
            make_conn(80, "nginx.exe", 1, Protocol::Tcp, PortState::Listen),
            make_conn(3000, "node.exe", 2, Protocol::Tcp, PortState::Listen),
            make_conn(8080, "nginx.exe", 3, Protocol::Tcp, PortState::Established),
        ];
        let filter = Filter {
            process_names: vec!["nginx".to_string()],
            states: vec![PortState::Listen],
            ..Default::default()
        };
        let result = apply_filters(&conns, &filter);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].port.number, 80);
    }

    #[test]
    fn or_within_vec_and_across_dimensions() {
        // SRCH-02 combination documented explicitly: OR applies WITHIN a
        // Vec dimension (protocols matches Tcp OR Udp), AND applies ACROSS
        // dimensions (must also match the process-name dimension).
        let conns = vec![
            make_conn(80, "nginx.exe", 1, Protocol::Tcp, PortState::Listen),
            make_conn(53, "dns.exe", 2, Protocol::Udp, PortState::Unknown),
            make_conn(443, "dns.exe", 3, Protocol::Tcp, PortState::Listen),
        ];
        let filter = Filter {
            // OR within: either protocol qualifies
            protocols: vec![Protocol::Tcp, Protocol::Udp],
            // AND across: the name dimension must also match
            process_names: vec!["dns".to_string()],
            ..Default::default()
        };
        let result = apply_filters(&conns, &filter);
        // dns.exe on UDP 53 (Udp arm of the OR) AND dns.exe on TCP 443
        // (Tcp arm of the OR); nginx.exe fails the name AND.
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|c| c.port.number == 53));
        assert!(result.iter().any(|c| c.port.number == 443));
    }

    #[test]
    fn fuzzy_search_matches_multiple_fields() {
        let conns = vec![
            make_conn(8080, "nginx.exe", 1234, Protocol::Tcp, PortState::Listen),
            make_conn(3000, "node.exe", 5678, Protocol::Tcp, PortState::Listen),
        ];
        // Search for "8080" matches port number
        let result = fuzzy_search(&conns, "8080");
        assert_eq!(result.len(), 1);

        // Search for "nginx" matches process name
        let result = fuzzy_search(&conns, "nginx");
        assert_eq!(result.len(), 1);

        // Search for "5678" matches PID
        let result = fuzzy_search(&conns, "5678");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn fuzzy_search_case_insensitive() {
        let conns = vec![make_conn(80, "Nginx.Exe", 1, Protocol::Tcp, PortState::Listen)];
        let result = fuzzy_search(&conns, "nginx");
        assert_eq!(result.len(), 1);
        let result = fuzzy_search(&conns, "NGINX");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn empty_search_returns_all() {
        let conns = vec![make_conn(80, "http", 1, Protocol::Tcp, PortState::Listen)];
        let result = fuzzy_search(&conns, "");
        assert_eq!(result.len(), 1);
        let result = fuzzy_search(&conns, "   ");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn empty_filter_returns_all() {
        let conns = vec![make_conn(80, "http", 1, Protocol::Tcp, PortState::Listen)];
        let result = apply_filters(&conns, &Filter::default());
        assert_eq!(result.len(), 1);
    }
}
