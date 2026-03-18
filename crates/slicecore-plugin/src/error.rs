//! Error types for the plugin system.
//!
//! These are host-side errors (not FFI-safe) used by the plugin registry,
//! loader, and discovery subsystems.

use std::path::PathBuf;

/// Errors that can occur in the plugin system.
#[derive(Debug, thiserror::Error)]
pub enum PluginSystemError {
    /// Plugin loading failed (native or WASM).
    #[error("Plugin load failed for {path}: {reason}")]
    LoadFailed {
        /// Path to the plugin that failed to load.
        path: PathBuf,
        /// Human-readable reason for the failure.
        reason: String,
    },

    /// Plugin API version is incompatible with the host.
    #[error("Plugin version incompatible: {plugin} requires API {required}, host has {available}")]
    VersionIncompatible {
        /// Name of the incompatible plugin.
        plugin: String,
        /// Version range the plugin requires.
        required: String,
        /// Version the host provides.
        available: String,
    },

    /// Plugin manifest (plugin.toml) could not be parsed or is invalid.
    #[error("Plugin manifest error at {path}: {reason}")]
    ManifestError {
        /// Path to the manifest file.
        path: PathBuf,
        /// Human-readable reason for the error.
        reason: String,
    },

    /// Plugin execution (generate call) failed.
    #[error("Plugin execution failed in {plugin}: {message}")]
    ExecutionFailed {
        /// Name of the plugin that failed.
        plugin: String,
        /// Error message from the plugin.
        message: String,
    },

    /// Plugin not found by name in the registry.
    #[error("Plugin not found: {name}")]
    NotFound {
        /// Name of the plugin that was not found.
        name: String,
    },

    /// Plugin status file could not be read or written.
    #[error("Plugin status file error at {path}: {reason}")]
    StatusFileError {
        /// Path to the status file that caused the error.
        path: PathBuf,
        /// Human-readable reason for the failure.
        reason: String,
    },

    /// Plugin is disabled but was explicitly referenced by name during slicing.
    #[error("Plugin '{name}' is disabled. Enable with `slicecore plugins enable {name}`")]
    PluginDisabled {
        /// Name of the disabled plugin.
        name: String,
    },

    /// IO error during plugin discovery or loading.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
