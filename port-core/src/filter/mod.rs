//! Multi-dimensional port/process filtering engine.

pub trait FilterEngine {
    fn apply(&self, connections: Vec<crate::models::Connection>, filter: &crate::models::Filter) -> Vec<crate::models::Connection>;
}
