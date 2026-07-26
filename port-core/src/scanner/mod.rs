//! Port scanning — system API abstraction.

pub trait PortScanner {
    fn scan(&self) -> crate::Result<Vec<crate::models::Connection>>;
    fn scan_process(&self, pid: u32) -> crate::Result<Vec<crate::models::Connection>>;
}
