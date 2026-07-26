//! Process management — enumeration, detail, termination.

pub trait ProcessManager {
    fn details(&self, pid: u32) -> crate::Result<crate::models::ProcessInfo>;
    fn terminate(&self, pid: u32, force: bool) -> crate::Result<()>;
}
