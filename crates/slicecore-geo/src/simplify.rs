//! Polyline/polygon simplification using the Ramer-Douglas-Peucker algorithm.
//!
//! Removes points that are within `epsilon` distance of the line between
//! their neighbors, reducing vertex count while preserving shape within
//! the specified tolerance.

use slicecore_math::IPoint2;

use crate::area::perpendicular_distance;

/// Simplifies a polyline using the Ramer-Douglas-Peucker algorithm.
///
/// `epsilon` is in internal coordinate units (use `mm_to_coord` to convert
/// from millimeters). Points closer than `epsilon` to the line between
/// their neighbors are removed.
///
/// Returns a new vector of points with redundant vertices removed.
/// The first and last points are always preserved.
///
/// # Panics
///
/// Does not panic for any input.
pub fn simplify(points: &[IPoint2], epsilon: i64) -> Vec<IPoint2> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let epsilon_f64 = epsilon as f64;
    let mut result = Vec::new();
    rdp_recursive(points, epsilon_f64, &mut result);
    result.push(*points.last().unwrap());
    result
}

/// Recursive Ramer-Douglas-Peucker implementation.
///
/// Finds the point with maximum distance from the line between first and last.
/// If that distance exceeds epsilon, recursively simplifies both halves.
/// Otherwise, only the first point is kept (last is added by the caller).
fn rdp_recursive(points: &[IPoint2], epsilon: f64, result: &mut Vec<IPoint2>) {
    if points.len() < 2 {
        if !points.is_empty() {
            result.push(points[0]);
        }
        return;
    }

    let first = &points[0];
    let last = &points[points.len() - 1];

    // Find the point with maximum distance from the line first->last
    let mut max_dist = 0.0f64;
    let mut max_idx = 0;

    for (i, pt) in points.iter().enumerate().skip(1).take(points.len() - 2) {
        let dist = perpendicular_distance(pt, first, last);
        if dist > max_dist {
            max_dist = dist;
            max_idx = i;
        }
    }

    if max_dist > epsilon {
        // Recursively simplify both halves
        rdp_recursive(&points[..=max_idx], epsilon, result);
        rdp_recursive(&points[max_idx..], epsilon, result);
    } else {
        // All intermediate points are within epsilon; keep only first
        result.push(*first);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_math::mm_to_coord;

    #[test]
    fn simplify_straight_line_to_endpoints() {
        // Points along a straight horizontal line
        let points = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(1.0, 0.0),
            IPoint2::from_mm(2.0, 0.0),
            IPoint2::from_mm(3.0, 0.0),
            IPoint2::from_mm(4.0, 0.0),
        ];
        let simplified = simplify(&points, mm_to_coord(0.1));
        assert_eq!(
            simplified.len(),
            2,
            "Straight line should simplify to 2 endpoints, got {}",
            simplified.len()
        );
        assert_eq!(simplified[0], points[0]);
        assert_eq!(simplified[1], *points.last().unwrap());
    }

    #[test]
    fn simplify_already_simple_triangle() {
        // Triangle -- no point can be removed without exceeding epsilon
        let points = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(5.0, 10.0),
        ];
        let simplified = simplify(&points, mm_to_coord(0.1));
        assert_eq!(simplified.len(), 3, "Triangle should remain unchanged");
    }

    #[test]
    fn simplify_removes_near_collinear() {
        // Square with extra point near the bottom edge
        let points = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(5.0, 0.001), // nearly on the line
            IPoint2::from_mm(10.0, 0.0),
        ];
        let simplified = simplify(&points, mm_to_coord(0.01));
        assert_eq!(
            simplified.len(),
            2,
            "Near-collinear point should be removed"
        );
    }

    #[test]
    fn simplify_preserves_significant_deviation() {
        let points = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(5.0, 5.0), // significant deviation
            IPoint2::from_mm(10.0, 0.0),
        ];
        let simplified = simplify(&points, mm_to_coord(0.1));
        assert_eq!(
            simplified.len(),
            3,
            "Significant deviation should be preserved"
        );
    }

    #[test]
    fn simplify_empty() {
        let points: Vec<IPoint2> = vec![];
        let simplified = simplify(&points, 100);
        assert!(simplified.is_empty());
    }

    #[test]
    fn simplify_single_point() {
        let points = vec![IPoint2::from_mm(5.0, 5.0)];
        let simplified = simplify(&points, 100);
        assert_eq!(simplified.len(), 1);
    }

    #[test]
    fn simplify_two_points() {
        let points = vec![IPoint2::from_mm(0.0, 0.0), IPoint2::from_mm(10.0, 0.0)];
        let simplified = simplify(&points, 100);
        assert_eq!(simplified.len(), 2);
    }

    #[test]
    fn simplify_with_zero_epsilon_preserves_all() {
        let points = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(5.0, 0.001),
            IPoint2::from_mm(10.0, 0.0),
        ];
        // epsilon = 0 means keep everything (nothing is exactly on the line)
        let simplified = simplify(&points, 0);
        assert_eq!(simplified.len(), 3);
    }
}
