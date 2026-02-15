//! Signed area and winding direction computation.
//!
//! Uses the shoelace formula with i128 intermediate arithmetic to prevent
//! overflow on polygons with large coordinates. The signed area determines
//! winding direction: positive for counter-clockwise (CCW), negative for
//! clockwise (CW).

use slicecore_math::{IPoint2, COORD_SCALE};

use crate::polygon::Winding;

/// Computes the signed area of a polygon using the shoelace formula.
///
/// Returns a value in internal coordinate units squared (nanometer^2).
/// - Positive: counter-clockwise winding
/// - Negative: clockwise winding
/// - Zero: degenerate polygon (collinear points or fewer than 3 points)
///
/// Uses i128 intermediate arithmetic to prevent overflow. With i64 coordinates
/// up to ~9.2e18, the cross product terms fit within i128 range.
///
/// Note: The result is 2x the actual area (avoids the division by 2 to stay
/// in integer arithmetic). Use [`signed_area_f64`] for the actual area in mm^2.
pub fn signed_area_2x(points: &[IPoint2]) -> i128 {
    let n = points.len();
    if n < 3 {
        return 0;
    }

    let mut area: i128 = 0;
    for i in 0..n {
        let j = (i + 1) % n;
        // Shoelace: sum of (x_i * y_{i+1} - x_{i+1} * y_i)
        area += points[i].x as i128 * points[j].y as i128
            - points[j].x as i128 * points[i].y as i128;
    }
    area
}

/// Computes the signed area of a polygon in internal coordinate units squared.
///
/// This is `signed_area_2x / 2` but returns i64. For most practical polygons
/// (build volumes up to 500mm), this will not overflow i64. For extremely
/// large polygons, use [`signed_area_2x`] which returns i128.
pub fn signed_area_i64(points: &[IPoint2]) -> i64 {
    (signed_area_2x(points) / 2) as i64
}

/// Computes the signed area of a polygon in square millimeters.
///
/// Converts from internal coordinate units by dividing by `COORD_SCALE^2`.
/// Positive for CCW, negative for CW.
pub fn signed_area_f64(points: &[IPoint2]) -> f64 {
    signed_area_2x(points) as f64 / (2.0 * COORD_SCALE * COORD_SCALE)
}

/// Determines the winding direction of a polygon from its signed area.
///
/// Returns `None` for degenerate polygons (zero area, fewer than 3 points).
pub fn winding_direction(points: &[IPoint2]) -> Option<Winding> {
    let area = signed_area_2x(points);
    if area > 0 {
        Some(Winding::CounterClockwise)
    } else if area < 0 {
        Some(Winding::Clockwise)
    } else {
        None
    }
}

/// Helper: computes the cross product of vectors (b-a) and (c-a).
///
/// Returns positive if the turn a->b->c is counter-clockwise,
/// negative if clockwise, zero if collinear.
pub(crate) fn cross_product_i128(a: &IPoint2, b: &IPoint2, c: &IPoint2) -> i128 {
    let abx = b.x as i128 - a.x as i128;
    let aby = b.y as i128 - a.y as i128;
    let acx = c.x as i128 - a.x as i128;
    let acy = c.y as i128 - a.y as i128;
    abx * acy - aby * acx
}

/// Computes the perpendicular distance squared from point `p` to the line
/// segment from `a` to `b`, scaled to avoid floating-point.
///
/// Returns (numerator^2, denominator) where distance^2 = numerator^2 / denominator.
/// Both values are in i128 to avoid overflow.
#[allow(dead_code)]
pub(crate) fn perpendicular_distance_sq(
    p: &IPoint2,
    a: &IPoint2,
    b: &IPoint2,
) -> (i128, i128) {
    let dx = b.x as i128 - a.x as i128;
    let dy = b.y as i128 - a.y as i128;
    let numerator = (dy * (p.x as i128 - a.x as i128) - dx * (p.y as i128 - a.y as i128)).abs();
    let denominator = dx * dx + dy * dy;
    (numerator * numerator, denominator)
}

/// Computes the perpendicular distance from point `p` to the infinite line
/// through `a` and `b` as a Coord-scale value (for comparison with epsilon).
///
/// Uses floating-point for the final sqrt. Returns 0 if a == b.
pub(crate) fn perpendicular_distance(p: &IPoint2, a: &IPoint2, b: &IPoint2) -> f64 {
    let dx = b.x as f64 - a.x as f64;
    let dy = b.y as f64 - a.y as f64;
    let len_sq = dx * dx + dy * dy;
    if len_sq == 0.0 {
        // a and b are the same point -- distance is just point-to-point
        let px = p.x as f64 - a.x as f64;
        let py = p.y as f64 - a.y as f64;
        return (px * px + py * py).sqrt();
    }
    let numerator =
        (dy * (p.x as f64 - a.x as f64) - dx * (p.y as f64 - a.y as f64)).abs();
    numerator / len_sq.sqrt()
}

/// Helper: squared distance between two IPoint2 values (in Coord^2 units).
#[allow(dead_code)]
pub(crate) fn distance_sq(a: &IPoint2, b: &IPoint2) -> i128 {
    let dx = b.x as i128 - a.x as i128;
    let dy = b.y as i128 - a.y as i128;
    dx * dx + dy * dy
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a CCW unit square (10mm x 10mm).
    fn ccw_square_10mm() -> Vec<IPoint2> {
        vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(10.0, 10.0),
            IPoint2::from_mm(0.0, 10.0),
        ]
    }

    /// Helper to create a CW unit square (reversed winding).
    fn cw_square_10mm() -> Vec<IPoint2> {
        let mut sq = ccw_square_10mm();
        sq.reverse();
        sq
    }

    #[test]
    fn signed_area_ccw_square() {
        let sq = ccw_square_10mm();
        let area = signed_area_f64(&sq);
        assert!(
            (area - 100.0).abs() < 1e-6,
            "Expected 100 mm^2, got {}",
            area
        );
    }

    #[test]
    fn signed_area_cw_square() {
        let sq = cw_square_10mm();
        let area = signed_area_f64(&sq);
        assert!(
            (area - (-100.0)).abs() < 1e-6,
            "Expected -100 mm^2, got {}",
            area
        );
    }

    #[test]
    fn signed_area_triangle() {
        // Right triangle: (0,0), (10,0), (0,10) -> area = 50 mm^2
        let tri = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(0.0, 10.0),
        ];
        let area = signed_area_f64(&tri);
        assert!(
            (area - 50.0).abs() < 1e-6,
            "Expected 50 mm^2, got {}",
            area
        );
    }

    #[test]
    fn signed_area_collinear_is_zero() {
        // Three collinear points: all on x-axis
        let pts = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(5.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
        ];
        let area = signed_area_2x(&pts);
        assert_eq!(area, 0, "Collinear points should have zero area");
    }

    #[test]
    fn signed_area_single_point() {
        let pts = vec![IPoint2::from_mm(5.0, 5.0)];
        assert_eq!(signed_area_2x(&pts), 0);
    }

    #[test]
    fn signed_area_two_points() {
        let pts = vec![IPoint2::from_mm(0.0, 0.0), IPoint2::from_mm(10.0, 10.0)];
        assert_eq!(signed_area_2x(&pts), 0);
    }

    #[test]
    fn signed_area_empty() {
        let pts: Vec<IPoint2> = vec![];
        assert_eq!(signed_area_2x(&pts), 0);
    }

    #[test]
    fn signed_area_i64_matches_f64() {
        let sq = ccw_square_10mm();
        let area_i64 = signed_area_i64(&sq);
        let area_f64 = signed_area_f64(&sq);
        // area_i64 is in coord^2 units; area_f64 is in mm^2
        let area_i64_mm2 = area_i64 as f64 / (COORD_SCALE * COORD_SCALE);
        assert!(
            (area_i64_mm2 - area_f64).abs() < 1e-6,
            "i64 ({}) and f64 ({}) area should match",
            area_i64_mm2,
            area_f64
        );
    }

    #[test]
    fn winding_direction_ccw() {
        let sq = ccw_square_10mm();
        assert_eq!(winding_direction(&sq), Some(Winding::CounterClockwise));
    }

    #[test]
    fn winding_direction_cw() {
        let sq = cw_square_10mm();
        assert_eq!(winding_direction(&sq), Some(Winding::Clockwise));
    }

    #[test]
    fn winding_direction_degenerate() {
        let pts = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(5.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
        ];
        assert_eq!(winding_direction(&pts), None);
    }

    #[test]
    fn winding_direction_too_few_points() {
        let pts = vec![IPoint2::from_mm(0.0, 0.0), IPoint2::from_mm(5.0, 5.0)];
        assert_eq!(winding_direction(&pts), None);
    }

    #[test]
    fn large_coordinates_no_overflow() {
        // Large polygon: 1000mm x 1000mm
        let sq = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(1000.0, 0.0),
            IPoint2::from_mm(1000.0, 1000.0),
            IPoint2::from_mm(0.0, 1000.0),
        ];
        let area = signed_area_f64(&sq);
        assert!(
            (area - 1_000_000.0).abs() < 1.0,
            "Expected 1,000,000 mm^2, got {}",
            area
        );
    }
}
