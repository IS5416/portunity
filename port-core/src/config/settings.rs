//! Settings struct + TOML load/save.
//!
//! Reads and writes `%APPDATA%/Portunity/settings.toml`.
//! Creates default settings on first run.
//!
//! `load_settings` is called on every scan and every kill, so it keeps a
//! process-local cache invalidated by file mtime/size — unchanged files are
//! served from memory (no per-scan disk read + TOML parse) while D-15's
//! "changes take effect immediately" is preserved because every atomic save
//! bumps the mtime, forcing one fresh re-read.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

/// Application settings persisted as TOML.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppSettings {
    /// Whether the current session detected administrator privileges.
    #[serde(default)]
    pub admin_detected: bool,

    /// Schema version for forward-compatibility.
    #[serde(default = "default_schema_version")]
    pub schema_version: i32,

    /// User-customizable whitelist of executable paths (D-13).
    /// Processes matching one of these paths require confirmation before kill.
    #[serde(default)]
    pub whitelist: Vec<String>,

    /// Graceful shutdown timeout in seconds before force-kill (D-02).
    #[serde(default = "default_kill_timeout_secs")]
    pub kill_timeout_secs: u64,
}

fn default_schema_version() -> i32 {
    1
}

/// Default graceful-kill timeout: 5 seconds (D-02).
pub fn default_kill_timeout_secs() -> u64 {
    5
}

/// Default settings for first-run initialization.
pub fn default_settings() -> AppSettings {
    AppSettings {
        admin_detected: false,
        schema_version: 1,
        whitelist: Vec::new(),
        kill_timeout_secs: 5,
    }
}

/// Path to the settings TOML file in the user's app data directory.
///
/// Returns `%APPDATA%/Portunity/settings.toml`.
pub fn settings_path() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));

    base.join("Portunity").join("settings.toml")
}

/// A cached copy of the last parsed settings plus the filesystem fingerprint
/// that identifies it. When the on-disk file's (mtime, size) still match, the
/// cached copy is returned without re-reading.
struct CachedSettings {
    modified: Option<SystemTime>,
    len: Option<u64>,
    settings: AppSettings,
}

impl CachedSettings {
    fn matches(&self, meta: &std::fs::Metadata) -> bool {
        meta.modified().ok() == self.modified && Some(meta.len()) == self.len
    }
}

/// Process-local settings cache. Holds at most one entry; guard handles
/// concurrent access (TUI event loop + the callers that live in spawn_blocking).
fn settings_cache() -> &'static Mutex<Option<CachedSettings>> {
    static CACHE: OnceLock<Mutex<Option<CachedSettings>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Load settings from the TOML file, creating defaults if the file is missing.
///
/// If the file exists but cannot be parsed, logs a warning and returns defaults
/// so the application can still run. Results are cached by (mtime, size) — an
/// unchanged file is served from memory instead of re-read + re-parsed on every
/// scan/kill; a save (which bumps the mtime) forces a fresh re-read, preserving
/// D-15's "whitelist changes take effect immediately".
pub fn load_settings() -> crate::Result<AppSettings> {
    let path = settings_path();

    // Fast path: cached copy still matches the file on disk.
    {
        let guard = settings_cache()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(cached) = guard.as_ref()
            && let Ok(meta) = std::fs::metadata(&path)
            && cached.matches(&meta)
        {
            return Ok(cached.settings.clone());
        }
    }

    // Slow path: read (or create) + parse, then refresh the cache with the
    // just-stat'd fingerprint so the cached entry is immediately valid.
    let settings = load_from_disk(&path)?;
    if let Ok(meta) = std::fs::metadata(&path) {
        let cached = CachedSettings {
            modified: meta.modified().ok(),
            len: Some(meta.len()),
            settings: settings.clone(),
        };
        let mut guard = settings_cache()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Some(cached);
    }
    Ok(settings)
}

/// Read + parse (or create-on-first-run) a settings file.
fn load_from_disk(path: &Path) -> crate::Result<AppSettings> {
    if !path.exists() {
        // Create parent directory and write defaults
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let defaults = default_settings();
        let toml_str = toml::to_string_pretty(&defaults).map_err(|e| {
            crate::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        })?;

        std::fs::write(&path, toml_str)?;

        return Ok(defaults);
    }

    let content = std::fs::read_to_string(path)?;

    match toml::from_str::<AppSettings>(&content) {
        Ok(settings) => Ok(settings),
        Err(e) => {
            // Log warning and return defaults — app should still run
            tracing::warn!(
                "Failed to parse settings file '{}': {}. Using defaults.",
                path.display(),
                e
            );
            Ok(default_settings())
        }
    }
}

/// Save settings to the TOML file atomically.
///
/// Writes to a same-directory temp file then renames over the target, so a
/// crash mid-write can never leave a truncated/corrupt `settings.toml` (one
/// that would silently reset the whitelist to defaults on next load — the
/// review's non-atomic `fs::write` window). Creates the parent directory if
/// it does not exist. The rename bumps the mtime, invalidating the cache.
pub fn save_settings(settings: &AppSettings) -> crate::Result<()> {
    let path = settings_path();

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let toml_str = toml::to_string_pretty(settings).map_err(|e| {
        crate::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    })?;

    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    // Unique temp name (pid-suffixed) avoids clobbering a concurrent frontend's
    // in-flight write while we prepare ours.
    let tmp = dir.join(format!(".{}.tmp{}", name, std::process::id()));

    std::fs::write(&tmp, &toml_str)?;
    std::fs::rename(&tmp, &path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phase-1-era TOML: only admin_detected + schema_version.
    /// Prove new fields (whitelist, kill_timeout_secs) default correctly.
    #[test]
    fn serde_defaults_on_phase1_era_toml() {
        let toml_str = r#"admin_detected = false
schema_version = 1
"#;

        let settings: AppSettings = toml::from_str(toml_str).expect("parse Phase-1 TOML");
        assert!(!settings.admin_detected);
        assert_eq!(settings.schema_version, 1);
        assert!(settings.whitelist.is_empty(), "whitelist should default to empty");
        assert_eq!(
            settings.kill_timeout_secs, 5,
            "kill_timeout_secs should default to 5"
        );
    }

    /// Round-trip with populated whitelist — preserves entries and order.
    #[test]
    fn serde_round_trip_with_whitelist() {
        let settings = AppSettings {
            admin_detected: true,
            schema_version: 1,
            whitelist: vec![
                r"C:\apps\node.exe".to_string(),
                r"D:\tools\python.exe".to_string(),
            ],
            kill_timeout_secs: 10,
        };

        let toml_str = toml::to_string_pretty(&settings).expect("serialize");
        let round_tripped: AppSettings = toml::from_str(&toml_str).expect("deserialize");

        assert_eq!(round_tripped.admin_detected, settings.admin_detected);
        assert_eq!(round_tripped.schema_version, settings.schema_version);
        assert_eq!(round_tripped.whitelist, settings.whitelist);
        assert_eq!(round_tripped.kill_timeout_secs, settings.kill_timeout_secs);
    }

    /// Verify duplicate entries are preserved as-written (not deduplicated).
    /// The matching logic handles duplicates correctly; TOML must not alter them.
    #[test]
    fn serde_preserves_duplicates() {
        let settings = AppSettings {
            admin_detected: false,
            schema_version: 1,
            whitelist: vec![
                r"C:\foo.exe".to_string(),
                r"C:\foo.exe".to_string(),
            ],
            kill_timeout_secs: 5,
        };

        let toml_str = toml::to_string_pretty(&settings).expect("serialize");
        let round_tripped: AppSettings = toml::from_str(&toml_str).expect("deserialize");

        assert_eq!(round_tripped.whitelist.len(), 2);
        assert_eq!(round_tripped.whitelist[0], "C:\\foo.exe");
        assert_eq!(round_tripped.whitelist[1], "C:\\foo.exe");
    }

    /// Save is atomic (no temp file left behind) and `save_settings` is
    /// immediately visible to `load_settings` (D-15 instant effect, now via
    /// the mtime-invalidated cache). Redirects APPDATA to a private temp dir.
    #[test]
    fn save_is_atomic_and_load_sees_it_immediately() {
        let dir = std::env::temp_dir().join(format!(
            "portunity-settings-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // edition-2024: env mutation is `unsafe`.
        unsafe { std::env::set_var("APPDATA", &dir) };

        // Create-on-first-run + write.
        save_settings(&default_settings()).unwrap();

        let entries = std::fs::read_dir(dir.join("Portunity"))
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(
            entries.iter().any(|n| n == "settings.toml"),
            "settings.toml missing: {entries:?}"
        );
        assert!(
            entries.iter().all(|n| !n.contains(".tmp")),
            "temp file leaked by atomic save: {entries:?}"
        );

        // A change is visible to the next load — the cache is invalidated by
        // the save's fresh mtime.
        let mut changed = default_settings();
        changed.kill_timeout_secs = 42;
        changed.whitelist.push(r"C:\apps\node.exe".to_string());
        save_settings(&changed).unwrap();
        let reloaded = load_settings().unwrap();
        assert_eq!(reloaded.kill_timeout_secs, 42);
        assert_eq!(reloaded.whitelist, vec![r"C:\apps\node.exe".to_string()]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
