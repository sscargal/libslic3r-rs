//! Error types for geometry operations.
//!
//! [`GeoError`] covers validation failures (degenerate polygons), boolean
//! operation failures, and offsetting failures. All variants include
//! human-readable messages suitable for logging and debugging.

use thiserror::Error;

/// Errors that can occur during geometry operations.
#[derive(Debug, Error)]
pub enum GeoError {
    /// Polygon has fewer than the minimum 3 points required.
    #[error("polygon has fewer than 3 points ({0} given)")]
    TooFewPoints(usize),

    /// Polygon has zero area (all points may be collinear or duplicated).
    #[error("polygon has zero area")]
    ZeroArea,

    /// After removing collinear and duplicate points, fewer than 3 effective
    /// vertices remain.
    #[error("polygon has collinear points that reduce it below 3 effective vertices")]
    AllCollinear,

    /// Polygon has self-intersections that prevent valid processing.
    #[error("polygon has self-intersections")]
    SelfIntersecting,

    /// A polygon boolean operation (union, intersection, difference, XOR) failed.
    #[error("boolean operation failed: {0}")]
    BooleanOpFailed(String),

    /// A polygon offset (inflate/deflate) operation failed.
    #[error("offset operation failed: {0}")]
    OffsetFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_are_descriptive() {
        let e = GeoError::TooFewPoints(2);
        assert_eq!(e.to_string(), "polygon has fewer than 3 points (2 given)");

        let e = GeoError::ZeroArea;
        assert_eq!(e.to_string(), "polygon has zero area");

        let e = GeoError::AllCollinear;
        assert_eq!(
            e.to_string(),
            "polygon has collinear points that reduce it below 3 effective vertices"
        );

        let e = GeoError::SelfIntersecting;
        assert_eq!(e.to_string(), "polygon has self-intersections");

        let e = GeoError::BooleanOpFailed("test reason".into());
        assert_eq!(e.to_string(), "boolean operation failed: test reason");

        let e = GeoError::OffsetFailed("test reason".into());
        assert_eq!(e.to_string(), "offset operation failed: test reason");
    }

    #[test]
    fn error_implements_std_error() {
        let e: Box<dyn std::error::Error> = Box::new(GeoError::ZeroArea);
        assert!(e.to_string().contains("zero area"));
    }
}
