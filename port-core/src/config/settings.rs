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
}

fn default_schema_version() -> i32 {
    1
}

/// Default settings for first-run initialization.
pub fn default_settings() -> AppSettings {
    AppSettings {
        admin_detected: false,
        schema_version: 1,
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
