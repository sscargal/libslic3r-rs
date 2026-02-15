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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mm_to_coord_one_mm() {
        assert_eq!(mm_to_coord(1.0), 1_000_000);
    }

    #[test]
    fn mm_to_coord_micrometer() {
        assert_eq!(mm_to_coord(0.001), 1_000);
    }

    #[test]
    fn mm_to_coord_nanometer() {
        assert_eq!(mm_to_coord(0.000001), 1);
    }

    #[test]
    fn mm_to_coord_negative() {
        assert_eq!(mm_to_coord(-1.0), -1_000_000);
    }

    #[test]
    fn coord_to_mm_basic() {
        assert!((coord_to_mm(1_000_000) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn round_trip_typical_values() {
        let values = [0.0, 0.001, 0.1, 1.0, 10.0, 100.0, 250.123456];
        for &v in &values {
            let result = coord_to_mm(mm_to_coord(v));
            assert!(
                (result - v).abs() < 1e-6,
                "round-trip failed for {}: got {}",
                v,
                result
            );
        }
    }

    #[test]
    fn large_value_no_overflow() {
        let coord = mm_to_coord(500.0);
        assert_eq!(coord, 500_000_000);
        let mm = coord_to_mm(coord);
        assert!((mm - 500.0).abs() < 1e-9);
    }

    #[test]
    fn points_to_ipoints_round_trip() {
        let pts = [(1.0, 2.0), (3.5, 4.5), (-0.5, 100.0)];
        let ipts = points_to_ipoints(&pts);
        let back = ipoints_to_points(&ipts);

        for (i, (&original, converted)) in pts.iter().zip(back.iter()).enumerate() {
            assert!(
                (original.0 - converted.0).abs() < 1e-6,
                "x mismatch at {}: {} vs {}",
                i,
                original.0,
                converted.0
            );
            assert!(
                (original.1 - converted.1).abs() < 1e-6,
                "y mismatch at {}: {} vs {}",
                i,
                original.1,
                converted.1
            );
        }
    }

    #[test]
    fn points_to_ipoints_empty() {
        let pts: &[(f64, f64)] = &[];
        let ipts = points_to_ipoints(pts);
        assert!(ipts.is_empty());
    }
}
