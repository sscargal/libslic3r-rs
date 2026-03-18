//! Per-plugin status files for enable/disable state.
//!
//! Each plugin directory may contain a `.status` JSON file that tracks
//! whether the plugin is enabled or disabled. Missing `.status` files
//! are auto-created with `enabled: true` on first read.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::PluginSystemError;

/// Persistent status for a single plugin.
///
/// Serialized as JSON in a `.status` file within the plugin directory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginStatus {
    /// Whether the plugin is enabled. Disabled plugins are skipped during
    /// discovery-and-load but still appear in listing output.
    pub enabled: bool,
}

impl Default for PluginStatus {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Reads the `.status` file from a plugin directory.
///
/// If the file does not exist, creates a default status file (enabled)
/// and returns it. If the file exists but is corrupt, returns a
/// [`PluginSystemError::StatusFileError`].
///
/// # Errors
///
/// Returns [`PluginSystemError::StatusFileError`] if the file exists but
/// cannot be parsed, or [`PluginSystemError::Io`] if a filesystem error
/// occurs during auto-creation.
pub fn read_status(plugin_dir: &Path) -> Result<PluginStatus, PluginSystemError> {
    let status_path = plugin_dir.join(".status");

    if !status_path.exists() {
        let default_status = PluginStatus::default();
        write_status(plugin_dir, &default_status)?;
        return Ok(default_status);
    }

    let contents =
        std::fs::read_to_string(&status_path).map_err(|e| PluginSystemError::StatusFileError {
            path: status_path.clone(),
            reason: format!("Failed to read: {e}"),
        })?;

    serde_json::from_str(&contents).map_err(|e| PluginSystemError::StatusFileError {
        path: status_path,
        reason: format!("Failed to parse JSON: {e}"),
    })
}

/// Writes a [`PluginStatus`] to the `.status` file in a plugin directory.
///
/// Creates or overwrites the file with pretty-printed JSON.
///
/// # Errors
///
/// Returns [`PluginSystemError::StatusFileError`] if the file cannot be written.
pub fn write_status(plugin_dir: &Path, status: &PluginStatus) -> Result<(), PluginSystemError> {
    let status_path = plugin_dir.join(".status");
    let json =
        serde_json::to_string_pretty(status).map_err(|e| PluginSystemError::StatusFileError {
            path: status_path.clone(),
            reason: format!("Failed to serialize: {e}"),
        })?;

    std::fs::write(&status_path, json).map_err(|e| PluginSystemError::StatusFileError {
        path: status_path,
        reason: format!("Failed to write: {e}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn status_default_is_enabled() {
        let status = PluginStatus::default();
        assert!(status.enabled);
    }

    #[test]
    fn read_status_missing_file_auto_creates_enabled() {
        let dir = TempDir::new().unwrap();
        let status = read_status(dir.path()).unwrap();
        assert!(status.enabled);

        // File should now exist
        let status_path = dir.path().join(".status");
        assert!(status_path.exists());
    }

    #[test]
    fn read_status_existing_disabled() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".status"), r#"{"enabled": false}"#).unwrap();

        let status = read_status(dir.path()).unwrap();
        assert!(!status.enabled);
    }

    #[test]
    fn write_status_roundtrip() {
        let dir = TempDir::new().unwrap();
        let original = PluginStatus { enabled: false };
        write_status(dir.path(), &original).unwrap();
        let loaded = read_status(dir.path()).unwrap();
        assert_eq!(original, loaded);
    }

    #[test]
    fn read_status_corrupt_file_returns_error() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".status"), "not json at all {{{{").unwrap();

        let result = read_status(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, PluginSystemError::StatusFileError { .. }));
    }
}
