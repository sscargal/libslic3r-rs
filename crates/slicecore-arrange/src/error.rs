//! Error types for the arrangement module.

use thiserror::Error;

/// Errors that can occur during build plate arrangement.
#[derive(Debug, Error)]
pub enum ArrangeError {
    /// The bed shape string could not be parsed.
    #[error("invalid bed shape: {0}")]
    InvalidBedShape(String),

    /// No parts were provided for arrangement.
    #[error("no parts provided for arrangement")]
    NoPartsProvided,

    /// A single part exceeds the bed dimensions.
    #[error("part '{part_id}' is too large for the bed")]
    PartTooLargeForBed {
        /// The identifier of the oversized part.
        part_id: String,
    },

    /// The operation was cancelled via a cancellation token.
    #[error("arrangement cancelled")]
    Cancelled,

    /// An error occurred during footprint computation (convex hull or offset).
    #[error("footprint error: {0}")]
    FootprintError(String),
}
