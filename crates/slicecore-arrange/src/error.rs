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

    /// Sequential mode detected gantry clearance overlap between parts.
    #[error("sequential mode overlap between parts '{part_a}' and '{part_b}'")]
    SequentialOverlap {
        /// First overlapping part identifier.
        part_a: String,
        /// Second overlapping part identifier.
        part_b: String,
    },
}
