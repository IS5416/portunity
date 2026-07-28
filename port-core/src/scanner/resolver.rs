//! Batch PID-to-process-name resolver with cache.
//!
//! Per D-16: collects all unique PIDs from port scans, batch-resolves
//! process names via sysinfo, and caches results by PID. Same PID
//! appearing on multiple ports hits the cache — no per-connection
//! OpenProcess calls.

use std::collections::HashMap;

/// Cached PID-to-process-name resolver.
///
/// Collects PIDs across all protocol scans, resolves names in a single
/// batch via sysinfo, and serves lookups from an in-memory cache.
pub struct ProcessResolver {
    /// PID → process name cache.
    cache: HashMap<u32, String>,
}

impl ProcessResolver {
    /// Create a new resolver with an empty cache.
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Batch-resolve process names for the given PIDs.
    ///
    /// Only queries sysinfo for PIDs not already in the cache.
    /// Special-cases PID 0 (System Idle Process) and PID 4 (System).
    /// Failed resolution caches as "<unknown>" — some processes are protected.
    pub fn resolve_batch(&mut self, pids: &[u32]) -> crate::Result<()> {
        // Collect PIDs not yet in cache
        let uncached: Vec<u32> = pids
            .iter()
            .copied()
            .filter(|pid| !self.cache.contains_key(pid))
            .collect();

        if uncached.is_empty() {
            return Ok(());
        }

        // Handle special PIDs without sysinfo lookup
        for &pid in &uncached {
            if pid == 0 {
                self.cache.insert(pid, "System Idle Process".to_string());
            } else if pid == 4 {
                self.cache.insert(pid, "System".to_string());
            }
        }

        // Filter to PIDs that need sysinfo lookup
        let need_lookup: Vec<u32> = uncached
            .iter()
            .copied()
            .filter(|pid| *pid != 0 && *pid != 4)
            .collect();

        if need_lookup.is_empty() {
            return Ok(());
        }

        // Initialize sysinfo once, then refresh individual processes
        use sysinfo::{Pid, System};

        let mut system = System::new_all();
        system.refresh_all();

        for pid in &need_lookup {
            let name = system
                .process(Pid::from(*pid as usize))
                .map(|p| p.name().to_string_lossy().to_string())
                .unwrap_or_else(|| "<unknown>".to_string());

            self.cache.insert(*pid, name);
        }

        Ok(())
    }

    /// Get the cached process name for a PID.
    ///
    /// Returns `None` if the PID was never resolved. Call `resolve_batch`
    /// before calling `get` for best results.
    pub fn get(&self, pid: u32) -> Option<&str> {
        self.cache.get(&pid).map(|s| s.as_str())
    }

    /// Insert a name into the cache directly (for testing or manual overrides).
    #[allow(dead_code)]
    pub fn insert(&mut self, pid: u32, name: String) {
        self.cache.insert(pid, name);
    }

    /// Return the number of cached entries.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Return true if the cache is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

impl Default for ProcessResolver {
    fn default() -> Self {
        Self::new()
    }
}
