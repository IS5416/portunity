//! Process-related data models.

/// Detailed information about a running process.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub executable_path: Option<String>,
    pub command_line: Option<String>,
    pub start_time: Option<std::time::SystemTime>,
    pub is_signed: Option<bool>,
    pub is_system_critical: bool,
    /// Whether the owning process is on the user's whitelist (confirmation gate).
    pub user_protected: bool,
    pub parent_pid: Option<u32>,
}
