//! Process management — process identity, smart kill escalation, whitelist protection.
//!
//! Sub-modules:
//! - `handle`: ProcessSnapshot (Send-safe) + Win32 handle wrapper
//! - `whitelist`: Built-in + user-tier protection gating
//! - `kill`: Smart kill escalation pipeline (WmClose → Ctrl+C → Force)
//!
//! The `ProcessManager` trait defines the platform-agnostic interface.
//! `WindowsProcessManager` (Windows target) delegates to the free functions
//! exposed by the sub-modules.

pub mod handle;
pub mod whitelist;
pub mod kill;

use async_trait::async_trait;

pub use handle::{creation_matches, open_verified, snapshot_for, ProcessSnapshot};
pub use kill::{kill, route_strategy, KillOutcome, Strategy};
pub use whitelist::{
    builtin_match, protection_status, user_match, BuiltinEntry, Protection, BUILTIN,
};

/// Trait for platform-specific process management.
///
/// Implementations open, verify, and act on process handles.
/// All implementations are `Send + Sync` for use across async boundaries.
#[async_trait]
pub trait ProcessManager: Send + Sync {
    /// Get detailed information about a running process, identified by a snapshot.
    async fn details(&self, snapshot: &ProcessSnapshot) -> crate::Result<crate::models::ProcessInfo>;

    /// Terminate a process identified by a snapshot, with a configurable timeout.
    ///
    /// Returns a `KillOutcome` describing what happened — graceful shutdown,
    /// force-kill after timeout, access-denied, etc.
    async fn terminate(&self, snapshot: &ProcessSnapshot, timeout_secs: u64) -> KillOutcome;
}

/// Windows implementation of `ProcessManager`.
///
/// Thin wrapper delegating to the free functions in the `process` sub-modules.
/// Mirrors the `WindowsPortScanner` delegation pattern in `scanner/windows.rs`.
#[cfg(target_os = "windows")]
pub struct WindowsProcessManager;

#[cfg(target_os = "windows")]
#[async_trait]
impl ProcessManager for WindowsProcessManager {
    async fn details(
        &self,
        _snapshot: &ProcessSnapshot,
    ) -> crate::Result<crate::models::ProcessInfo> {
        // info.rs lands in plan 02-02; for now return a minimal success stub.
        // The trait reshape is the goal of this plan — no details() runtime call exists yet.
        Err(crate::Error::Platform(
            "Process details not yet implemented (plan 02-02)".into(),
        ))
    }

    async fn terminate(&self, snapshot: &ProcessSnapshot, timeout_secs: u64) -> KillOutcome {
        kill(snapshot.clone(), timeout_secs, || {}).await
    }
}
