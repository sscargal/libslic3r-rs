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
}
