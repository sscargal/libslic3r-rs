//! Plugin discovery via manifest scanning.
//!
//! Scans a directory for `plugin.toml` manifest files and parses them into
//! [`PluginManifest`] structs. Each plugin is expected to be in its own
//! subdirectory: `plugins_dir/plugin-name/plugin.toml`.

use std::path::{Path, PathBuf};

use slicecore_plugin_api::PluginManifest;

use crate::error::PluginSystemError;
use crate::status::{self, PluginStatus};

/// The current API version of the host, used for compatibility checks.
const HOST_API_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Scans a directory for plugin manifests (`plugin.toml` files).
///
/// Expects a directory structure like:
/// ```text
/// plugins_dir/
///   zigzag-infill/
///     plugin.toml
///     libzigzag_infill.so
///   spiral-infill/
///     plugin.toml
///     spiral_infill.wasm
/// ```
///
/// Returns a list of `(plugin_dir, manifest)` pairs for all valid plugins.
/// Invalid manifests are reported as errors.
pub fn discover_plugins(dir: &Path) -> Result<Vec<(PathBuf, PluginManifest)>, PluginSystemError> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut plugins = Vec::new();

    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let plugin_dir = entry.path();
        if !plugin_dir.is_dir() {
            continue;
        }

        let manifest_path = plugin_dir.join("plugin.toml");
        if !manifest_path.exists() {
            continue;
        }

        let manifest = parse_manifest(&manifest_path)?;
        validate_version_compatibility(&manifest, &manifest_path)?;
        plugins.push((plugin_dir, manifest));
    }

    Ok(plugins)
}

/// A plugin discovered during directory scanning, including status and error info.
///
/// Unlike [`discover_plugins`] which fails on the first error,
/// [`discover_all_with_status`] collects all plugins including broken ones
/// so they can be listed in CLI output.
#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    /// Path to the plugin directory.
    pub dir: PathBuf,
    /// Parsed manifest, if available. `None` when the manifest is corrupt.
    pub manifest: Option<PluginManifest>,
    /// Enable/disable status from the `.status` file.
    pub status: PluginStatus,
    /// Error message if the plugin is broken (bad manifest, version mismatch, etc.).
    pub error: Option<String>,
}

/// Discovers all plugins in a directory, collecting errors per-plugin instead
/// of failing on the first error.
///
/// Auto-creates `.status` files for plugins that lack them. Returns an empty
/// `Vec` if the directory does not exist.
///
/// # Errors
///
/// Returns `Err` only for top-level I/O failures (e.g., permission denied on
/// the directory itself). Per-plugin errors are captured in
/// [`DiscoveredPlugin::error`].
pub fn discover_all_with_status(dir: &Path) -> Result<Vec<DiscoveredPlugin>, PluginSystemError> {
    if !dir.exists() || !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut plugins = Vec::new();
    let entries = std::fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let plugin_dir = entry.path();
        if !plugin_dir.is_dir() {
            continue;
        }

        let manifest_path = plugin_dir.join("plugin.toml");
        if !manifest_path.exists() {
            continue;
        }

        // Read or auto-create .status file
        let plugin_status = status::read_status(&plugin_dir).unwrap_or_default();

        // Try to parse the manifest
        let manifest = match parse_manifest(&manifest_path) {
            Ok(m) => m,
            Err(e) => {
                plugins.push(DiscoveredPlugin {
                    dir: plugin_dir,
                    manifest: None,
                    status: plugin_status,
                    error: Some(e.to_string()),
                });
                continue;
            }
        };

        // Try to validate version compatibility
        if let Err(e) = validate_version_compatibility(&manifest, &manifest_path) {
            plugins.push(DiscoveredPlugin {
                dir: plugin_dir,
                manifest: Some(manifest),
                status: plugin_status,
                error: Some(e.to_string()),
            });
            continue;
        }

        plugins.push(DiscoveredPlugin {
            dir: plugin_dir,
            manifest: Some(manifest),
            status: plugin_status,
            error: None,
        });
    }

    Ok(plugins)
}

/// Parses a `plugin.toml` file into a [`PluginManifest`].
pub(crate) fn parse_manifest(path: &Path) -> Result<PluginManifest, PluginSystemError> {
    let contents = std::fs::read_to_string(path).map_err(|e| PluginSystemError::ManifestError {
        path: path.to_path_buf(),
        reason: format!("Failed to read: {}", e),
    })?;

    toml::from_str(&contents).map_err(|e| PluginSystemError::ManifestError {
        path: path.to_path_buf(),
        reason: format!("Failed to parse TOML: {}", e),
    })
}

/// Validates that a plugin's API version requirements are compatible with the host.
pub(crate) fn validate_version_compatibility(
    manifest: &PluginManifest,
    manifest_path: &Path,
) -> Result<(), PluginSystemError> {
    let host_version =
        semver::Version::parse(HOST_API_VERSION).map_err(|e| PluginSystemError::ManifestError {
            path: manifest_path.to_path_buf(),
            reason: format!("Host API version parse error: {}", e),
        })?;

    let min_version = semver::Version::parse(&manifest.metadata.min_api_version).map_err(|e| {
        PluginSystemError::ManifestError {
            path: manifest_path.to_path_buf(),
            reason: format!(
                "Invalid min_api_version '{}': {}",
                manifest.metadata.min_api_version, e
            ),
        }
    })?;

    let max_version = semver::Version::parse(&manifest.metadata.max_api_version).map_err(|e| {
        PluginSystemError::ManifestError {
            path: manifest_path.to_path_buf(),
            reason: format!(
                "Invalid max_api_version '{}': {}",
                manifest.metadata.max_api_version, e
            ),
        }
    })?;

    if host_version < min_version || host_version > max_version {
        return Err(PluginSystemError::VersionIncompatible {
            plugin: manifest.metadata.name.clone(),
            required: format!("{} - {}", min_version, max_version),
            available: host_version.to_string(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_valid_manifest_toml() -> String {
        format!(
            r#"
library_filename = "libtest_infill.so"
plugin_type = "native"
capabilities = ["infill_pattern"]

[metadata]
name = "test-infill"
version = "1.0.0"
description = "A test infill plugin"
author = "Test Author"
license = "MIT"
min_api_version = "0.0.0"
max_api_version = "99.99.99"
"#
        )
    }

    #[test]
    fn discover_plugins_empty_directory() {
        let dir = TempDir::new().unwrap();
        let plugins = discover_plugins(dir.path()).unwrap();
        assert!(plugins.is_empty());
    }

    #[test]
    fn discover_plugins_nonexistent_directory() {
        let plugins = discover_plugins(Path::new("/nonexistent/path")).unwrap();
        assert!(plugins.is_empty());
    }

    #[test]
    fn discover_plugins_finds_manifest() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join("test-infill");
        fs::create_dir(&plugin_dir).unwrap();
        fs::write(plugin_dir.join("plugin.toml"), create_valid_manifest_toml()).unwrap();

        let plugins = discover_plugins(dir.path()).unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].1.metadata.name, "test-infill");
    }

    #[test]
    fn discover_plugins_skips_dirs_without_manifest() {
        let dir = TempDir::new().unwrap();
        // Plugin dir without plugin.toml
        let plugin_dir = dir.path().join("no-manifest");
        fs::create_dir(&plugin_dir).unwrap();
        fs::write(plugin_dir.join("some_file.txt"), "not a manifest").unwrap();

        let plugins = discover_plugins(dir.path()).unwrap();
        assert!(plugins.is_empty());
    }

    #[test]
    fn discover_plugins_rejects_invalid_toml() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join("bad-manifest");
        fs::create_dir(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.toml"),
            "this is not valid toml {{{{",
        )
        .unwrap();

        let result = discover_plugins(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, PluginSystemError::ManifestError { .. }));
    }

    #[test]
    fn discover_plugins_rejects_incompatible_version() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join("old-plugin");
        fs::create_dir(&plugin_dir).unwrap();
        // Plugin requires API version 99.0.0 - 99.99.99 (way above our host version)
        let manifest = r#"
library_filename = "libold.so"
plugin_type = "native"
capabilities = ["infill_pattern"]

[metadata]
name = "old-plugin"
version = "1.0.0"
description = "Old plugin"
author = "Test"
license = "MIT"
min_api_version = "99.0.0"
max_api_version = "99.99.99"
"#;
        fs::write(plugin_dir.join("plugin.toml"), manifest).unwrap();

        let result = discover_plugins(dir.path());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PluginSystemError::VersionIncompatible { .. }
        ));
    }

    #[test]
    fn discover_all_with_status_returns_valid_plugins() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join("test-infill");
        fs::create_dir(&plugin_dir).unwrap();
        fs::write(plugin_dir.join("plugin.toml"), create_valid_manifest_toml()).unwrap();

        let plugins = discover_all_with_status(dir.path()).unwrap();
        assert_eq!(plugins.len(), 1);
        assert!(plugins[0].manifest.is_some());
        assert!(plugins[0].error.is_none());
        assert_eq!(
            plugins[0].manifest.as_ref().unwrap().metadata.name,
            "test-infill"
        );
    }

    #[test]
    fn discover_all_with_status_broken_manifest() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join("bad-plugin");
        fs::create_dir(&plugin_dir).unwrap();
        fs::write(plugin_dir.join("plugin.toml"), "not valid toml {{{{").unwrap();

        let plugins = discover_all_with_status(dir.path()).unwrap();
        assert_eq!(plugins.len(), 1);
        assert!(plugins[0].manifest.is_none());
        assert!(plugins[0].error.is_some());
    }

    #[test]
    fn discover_all_with_status_auto_creates_status_files() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join("test-infill");
        fs::create_dir(&plugin_dir).unwrap();
        fs::write(plugin_dir.join("plugin.toml"), create_valid_manifest_toml()).unwrap();

        // No .status file yet
        assert!(!plugin_dir.join(".status").exists());

        let plugins = discover_all_with_status(dir.path()).unwrap();
        assert_eq!(plugins.len(), 1);
        assert!(plugins[0].status.enabled);

        // .status file should now exist
        assert!(plugin_dir.join(".status").exists());
    }

    #[test]
    fn discover_all_with_status_nonexistent_dir() {
        let plugins = discover_all_with_status(Path::new("/nonexistent/path")).unwrap();
        assert!(plugins.is_empty());
    }

    #[test]
    fn discover_all_with_status_reads_existing_status() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join("test-infill");
        fs::create_dir(&plugin_dir).unwrap();
        fs::write(plugin_dir.join("plugin.toml"), create_valid_manifest_toml()).unwrap();
        fs::write(plugin_dir.join(".status"), r#"{"enabled": false}"#).unwrap();

        let plugins = discover_all_with_status(dir.path()).unwrap();
        assert_eq!(plugins.len(), 1);
        assert!(!plugins[0].status.enabled);
    }

    #[test]
    fn discover_plugins_finds_multiple() {
        let dir = TempDir::new().unwrap();

        for name in &["plugin-a", "plugin-b", "plugin-c"] {
            let plugin_dir = dir.path().join(name);
            fs::create_dir(&plugin_dir).unwrap();
            let toml_content = format!(
                r#"
library_filename = "lib{}.so"
plugin_type = "native"
capabilities = ["infill_pattern"]

[metadata]
name = "{}"
version = "1.0.0"
description = "Plugin {}"
author = "Test"
license = "MIT"
min_api_version = "0.0.0"
max_api_version = "99.99.99"
"#,
                name.replace('-', "_"),
                name,
                name,
            );
            fs::write(plugin_dir.join("plugin.toml"), toml_content).unwrap();
        }

        let plugins = discover_plugins(dir.path()).unwrap();
        assert_eq!(plugins.len(), 3);
    }
}
