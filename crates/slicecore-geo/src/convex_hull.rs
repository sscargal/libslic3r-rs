//! 2D convex hull computation using the Graham scan algorithm.
//!
//! Given a set of 2D integer points, computes the convex hull --
//! the smallest convex polygon containing all points. The result
//! is returned in counter-clockwise (CCW) order.

use slicecore_math::IPoint2;

/// Computes the convex hull of a set of 2D points using Graham scan.
///
/// Returns the hull vertices in counter-clockwise (CCW) order.
///
/// - If fewer than 3 points are given, returns them as-is (with duplicates removed).
/// - Collinear points on the hull boundary are excluded (only corner points kept).
pub fn convex_hull(points: &[IPoint2]) -> Vec<IPoint2> {
    if points.len() <= 1 {
        return points.to_vec();
    }

    // Find the lowest point (smallest y, then smallest x for tie-breaking)
    let mut sorted = points.to_vec();
    // Remove duplicates first
    sorted.sort();
    sorted.dedup();

    if sorted.len() <= 2 {
        return sorted;
    }

    // Find pivot: lowest y, then leftmost x
    let pivot_idx = sorted
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.y.cmp(&b.y).then_with(|| a.x.cmp(&b.x)))
        .map(|(i, _)| i)
        .unwrap();

    sorted.swap(0, pivot_idx);
    let pivot = sorted[0];

    // Sort by polar angle from pivot
    sorted[1..].sort_by(|a, b| {
        let cross = cross_2d(&pivot, a, b);
        if cross == 0 {
            // Collinear: sort by distance (closer first)
            let dist_a = dist_sq(&pivot, a);
            let dist_b = dist_sq(&pivot, b);
            dist_a.cmp(&dist_b)
        } else if cross > 0 {
            std::cmp::Ordering::Less // a is counter-clockwise from b
        } else {
            std::cmp::Ordering::Greater
        }
    });

    // Graham scan
    let mut hull: Vec<IPoint2> = Vec::with_capacity(sorted.len());

    for &p in &sorted {
        while hull.len() >= 2 {
            let top = hull[hull.len() - 1];
            let next_to_top = hull[hull.len() - 2];
            // If not a left turn (counter-clockwise), pop
            if cross_2d(&next_to_top, &top, &p) <= 0 {
                hull.pop();
            } else {
                break;
            }
        }
        hull.push(p);
    }

    hull
}

/// Cross product of vectors (b-a) x (c-a).
///
/// Returns positive for CCW turn, negative for CW, zero for collinear.
fn cross_2d(a: &IPoint2, b: &IPoint2, c: &IPoint2) -> i128 {
    let abx = b.x as i128 - a.x as i128;
    let aby = b.y as i128 - a.y as i128;
    let acx = c.x as i128 - a.x as i128;
    let acy = c.y as i128 - a.y as i128;
    abx * acy - aby * acx
}

/// Squared distance between two points (avoids sqrt, just for comparison).
fn dist_sq(a: &IPoint2, b: &IPoint2) -> i128 {
    let dx = b.x as i128 - a.x as i128;
    let dy = b.y as i128 - a.y as i128;
    dx * dx + dy * dy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hull_of_triangle() {
        let pts = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(5.0, 10.0),
        ];
        let hull = convex_hull(&pts);
        assert_eq!(hull.len(), 3, "Triangle hull should have 3 points");
    }

    #[test]
    fn hull_of_square() {
        let pts = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(10.0, 10.0),
            IPoint2::from_mm(0.0, 10.0),
        ];
        let hull = convex_hull(&pts);
        assert_eq!(hull.len(), 4, "Square hull should have 4 points");
    }

    #[test]
    fn hull_removes_interior_point() {
        let pts = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(10.0, 10.0),
            IPoint2::from_mm(0.0, 10.0),
            IPoint2::from_mm(5.0, 5.0), // interior point
        ];
        let hull = convex_hull(&pts);
        assert_eq!(hull.len(), 4, "Interior point should be excluded from hull");
    }

    #[test]
    fn hull_collinear_points() {
        // All points on a line
        let pts = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(5.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
        ];
        let hull = convex_hull(&pts);
        // Collinear: hull degenerates to endpoints
        assert!(
            hull.len() == 2,
            "Collinear hull should have 2 endpoints, got {}",
            hull.len()
        );
    }

    #[test]
    fn hull_duplicate_points() {
        let pts = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(5.0, 10.0),
        ];
        let hull = convex_hull(&pts);
        assert_eq!(hull.len(), 3, "Duplicates should be removed");
    }

    #[test]
    fn hull_single_point() {
        let pts = vec![IPoint2::from_mm(5.0, 5.0)];
        let hull = convex_hull(&pts);
        assert_eq!(hull.len(), 1);
    }

    #[test]
    fn hull_two_points() {
        let pts = vec![IPoint2::from_mm(0.0, 0.0), IPoint2::from_mm(10.0, 0.0)];
        let hull = convex_hull(&pts);
        assert_eq!(hull.len(), 2);
    }

    #[test]
    fn hull_empty() {
        let pts: Vec<IPoint2> = vec![];
        let hull = convex_hull(&pts);
        assert!(hull.is_empty());
    }

    #[test]
    fn hull_ccw_order() {
        // Verify result is CCW by checking signed area is positive
        let pts = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(10.0, 10.0),
            IPoint2::from_mm(0.0, 10.0),
            IPoint2::from_mm(5.0, 5.0),
        ];
        let hull = convex_hull(&pts);
        assert!(hull.len() >= 3);

        // Compute signed area to verify CCW
        use crate::area::signed_area_2x;
        let area = signed_area_2x(&hull);
        assert!(area > 0, "Hull should be CCW (positive area), got {}", area);
    }

    #[test]
    fn hull_many_random_looking_points() {
        let pts = vec![
            IPoint2::from_mm(1.0, 2.0),
            IPoint2::from_mm(3.0, 1.0),
            IPoint2::from_mm(5.0, 4.0),
            IPoint2::from_mm(2.0, 5.0),
            IPoint2::from_mm(4.0, 3.0),
            IPoint2::from_mm(0.0, 3.0),
            IPoint2::from_mm(6.0, 2.0),
        ];
        let hull = convex_hull(&pts);
        // All original points should be inside or on the hull
        assert!(hull.len() >= 3);
        assert!(hull.len() <= pts.len());
    }
}
