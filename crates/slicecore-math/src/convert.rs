//! Conversion utilities between millimeter (f64) and integer coordinate spaces.
//!
//! These functions are the single source of truth for converting between the
//! floating-point world (mesh vertices, user-facing dimensions) and the integer
//! coordinate world (polygon operations, path planning).

use crate::coord::{Coord, IPoint2, COORD_SCALE};

/// Converts a millimeter value to an integer coordinate.
///
/// Multiplies by [`COORD_SCALE`] (1,000,000) and rounds to nearest integer.
///
/// # Examples
///
/// ```
/// use slicecore_math::convert::mm_to_coord;
///
/// assert_eq!(mm_to_coord(1.0), 1_000_000);
/// assert_eq!(mm_to_coord(0.001), 1_000); // micrometer
/// assert_eq!(mm_to_coord(0.000001), 1);  // nanometer
/// ```
#[inline]
pub fn mm_to_coord(mm: f64) -> Coord {
    (mm * COORD_SCALE).round() as Coord
}

/// Converts an integer coordinate back to millimeters.
///
/// Divides by [`COORD_SCALE`] (1,000,000).
///
/// # Examples
///
/// ```
/// use slicecore_math::convert::coord_to_mm;
///
/// assert!((coord_to_mm(1_000_000) - 1.0).abs() < 1e-9);
/// ```
#[inline]
pub fn coord_to_mm(coord: Coord) -> f64 {
    coord as f64 / COORD_SCALE
}

/// Converts a slice of (f64, f64) millimeter pairs to integer points.
///
/// Each pair (x_mm, y_mm) is converted via [`mm_to_coord`].
pub fn points_to_ipoints(points: &[(f64, f64)]) -> Vec<IPoint2> {
    points
        .iter()
        .map(|&(x, y)| IPoint2::from_mm(x, y))
        .collect()
}

/// Converts a slice of integer points to (f64, f64) millimeter pairs.
///
/// Each point is converted via [`coord_to_mm`].
pub fn ipoints_to_points(points: &[IPoint2]) -> Vec<(f64, f64)> {
    points.iter().map(|p| p.to_mm()).collect()
}
