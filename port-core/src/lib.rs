//! Portunity core library.
//!
//! Shared logic for port scanning, process management, filtering,
//! and platform abstraction — consumed by both TUI and GUI frontends.

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("platform error: {0}")]
    Platform(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

// Platform abstraction — Windows first, Linux/macOS extension points reserved.
#[cfg(target_os = "windows")]
mod windows;
#[cfg(not(target_os = "windows"))]
compile_error!("Portunity currently only supports Windows. Linux/macOS support is planned.");

pub mod models;
pub mod scanner;
pub mod process;
pub mod firewall;
pub mod history;
pub mod traffic;
pub mod filter;
pub mod store;
pub mod config;

pub use store::*;
pub use config::*;
