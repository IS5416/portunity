//! Process-related data models.

/// Detailed information about a running process.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub executable_path: Option<String>,
    pub command_line: Option<String>,
    /// `SystemTime` has no serde impl — skipped on the wire (defaults to None
    /// on deserialize). Phase 3 can move this to `chrono::DateTime<Utc>` when
    /// history/GUI need start times serialized.
    #[serde(skip)]
    pub start_time: Option<std::time::SystemTime>,
    pub is_signed: Option<bool>,
    pub is_system_critical: bool,
    /// Whether the owning process is on the user's whitelist (confirmation gate).
    pub user_protected: bool,
    pub parent_pid: Option<u32>,
}
