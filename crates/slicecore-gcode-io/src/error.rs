//! Error types for G-code I/O operations.

use thiserror::Error;

/// Errors that can occur during G-code generation and validation.
#[derive(Debug, Error)]
pub enum GcodeError {
    /// I/O error writing G-code output.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Feedrate must be positive.
    #[error("invalid feedrate: {0} (must be positive)")]
    InvalidFeedrate(f64),

    /// Temperature out of valid range.
    #[error("invalid temperature: {0} (must be 0-400)")]
    InvalidTemperature(f64),

    /// Non-finite coordinate value (NaN or infinity).
    #[error("invalid coordinate: {0}")]
    InvalidCoordinate(String),

    /// General formatting error.
    #[error("format error: {0}")]
    FormatError(String),
}
