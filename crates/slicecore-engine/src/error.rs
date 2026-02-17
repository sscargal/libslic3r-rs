//! Error types for the slicing engine.

use std::path::PathBuf;

/// Errors that can occur during engine operations.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// Failed to read a config file.
    #[error("failed to read config file {0}: {1}")]
    ConfigIo(PathBuf, std::io::Error),

    /// Failed to parse TOML config.
    #[error("failed to parse config: {0}")]
    ConfigParse(#[from] toml::de::Error),

    /// Mesh has no geometry (no vertices or triangles).
    #[error("Mesh has no geometry")]
    EmptyMesh,

    /// Slicing produced no layers (mesh may be too thin).
    #[error("Slicing produced no layers")]
    NoLayers,

    /// Configuration error.
    #[error("Config error: {0}")]
    ConfigError(String),

    /// G-code write error.
    #[error("G-code write error: {0}")]
    GcodeError(#[from] slicecore_gcode_io::GcodeError),

    /// I/O error.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Plugin error during infill generation or plugin system operation.
    #[error("Plugin error in '{plugin}': {message}")]
    Plugin {
        /// The name of the plugin that failed.
        plugin: String,
        /// Human-readable error description.
        message: String,
    },
}
