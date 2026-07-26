//! Port occupation history — SQLite-backed.

pub trait HistoryStore {
    fn record(&self, entry: crate::models::HistoryEntry) -> crate::Result<()>;
    fn query(&self, filter: &crate::models::HistoryFilter) -> crate::Result<Vec<crate::models::HistoryEntry>>;
}
