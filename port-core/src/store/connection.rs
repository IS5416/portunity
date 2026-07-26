//! SQLite connection management — WAL mode, settings table.
//!
//! Initializes the database with WAL journal mode for concurrent access
//! between TUI and GUI frontends. Creates the settings table on first run.

use std::path::PathBuf;

/// Initialize a SQLite database connection at the given path.
///
/// Enables WAL mode and busy timeout, and ensures the settings table exists.
pub fn init_db(db_path: &std::path::Path) -> crate::Result<rusqlite::Connection> {
    let conn = rusqlite::Connection::open(db_path).map_err(|e| {
        crate::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    })?;

    // Enable WAL mode for concurrent access (Pitfall #12)
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(|e| {
            crate::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        })?;

    // Verify WAL mode is active
    let journal_mode: String = conn
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .map_err(|e| {
            crate::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        })?;

    if journal_mode.to_lowercase() != "wal" {
        return Err(crate::Error::Platform(format!(
            "failed to enable WAL mode; got '{}'",
            journal_mode
        )));
    }

    // Set busy timeout to 5 seconds
    conn.execute_batch("PRAGMA busy_timeout=5000;")
        .map_err(|e| {
            crate::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        })?;

    // Create settings table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        );",
    )
    .map_err(|e| {
        crate::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    })?;

    // Insert default schema version
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('schema_version', '1');",
        [],
    )
    .map_err(|e| {
        crate::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    })?;

    Ok(conn)
}

/// Default database file path in the user's application data directory.
///
/// Returns `%APPDATA%/Portunity/portunity.db`.
pub fn default_db_path() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));

    let dir = base.join("Portunity");
    dir.join("portunity.db")
}

/// Ensure the parent directory for a path exists.
pub fn ensure_parent_dir(path: &std::path::Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}
