//! Filter/search data models.

/// Multi-dimensional filter for port/connection queries.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Filter {
    pub port_range: Option<(u16, u16)>,
    pub protocols: Vec<super::port::Protocol>,
    pub process_names: Vec<String>,
    pub pids: Vec<u32>,
    pub states: Vec<super::port::PortState>,
    pub search_text: Option<String>,    // fuzzy match across name/port/PID
    pub system_only: Option<bool>,      // filter by is_system_critical
    pub remote_only: Option<bool>,      // only established connections
    pub favorite_only: Option<bool>,    // only favorited ports
}
