//! Settings struct + TOML load/save.
//!
//! Reads and writes `%APPDATA%/Portunity/settings.toml`.
//! Creates default settings on first run.

use std::path::PathBuf;

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

/// Load settings from the TOML file, creating defaults if the file is missing.
///
/// If the file exists but cannot be parsed, logs a warning and returns defaults
/// so the application can still run.
pub fn load_settings() -> crate::Result<AppSettings> {
    let path = settings_path();

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

    let content = std::fs::read_to_string(&path)?;

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

/// Save settings to the TOML file.
///
/// Creates the parent directory if it does not exist.
pub fn save_settings(settings: &AppSettings) -> crate::Result<()> {
    let path = settings_path();

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let toml_str = toml::to_string_pretty(settings).map_err(|e| {
        crate::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    })?;

    std::fs::write(&path, toml_str)?;

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
}
