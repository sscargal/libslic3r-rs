//! Point-in-polygon test using the winding number algorithm.
//!
//! The winding number algorithm counts how many times the polygon winds
//! around the test point. A non-zero winding number means the point is
//! inside. This method is more robust than ray casting for edge cases
//! (points on edges, collinear vertices).

use slicecore_math::IPoint2;

/// Result of a point-in-polygon test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointLocation {
    /// The point is strictly inside the polygon.
    Inside,
    /// The point is strictly outside the polygon.
    Outside,
    /// The point lies on the polygon boundary (edge or vertex).
    OnBoundary,
}

/// Tests whether a point is inside, outside, or on the boundary of a polygon.
///
/// Uses the winding number algorithm: for each edge, determine if the point
/// is to the left or right. Accumulate the winding number. Non-zero means inside.
///
/// The polygon is specified as a slice of points (implicitly closed: last
/// point connects to first).
pub fn point_in_polygon(point: &IPoint2, polygon: &[IPoint2]) -> PointLocation {
    let n = polygon.len();
    if n < 3 {
        return PointLocation::Outside;
    }

    let mut winding: i32 = 0;

    for i in 0..n {
        let a = &polygon[i];
        let b = &polygon[(i + 1) % n];

        // Check if point is on this edge segment
        if is_on_segment(point, a, b) {
            return PointLocation::OnBoundary;
        }

        if a.y <= point.y {
            if b.y > point.y {
                // Upward crossing
                if is_left(a, b, point) > 0 {
                    winding += 1;
                }
            }
        } else if b.y <= point.y {
            // Downward crossing
            if is_left(a, b, point) < 0 {
                winding -= 1;
            }
        }
    }

    if winding != 0 {
        PointLocation::Inside
    } else {
        PointLocation::Outside
    }
}

/// Computes the cross product (b-a) x (p-a) using i128 to avoid overflow.
///
/// Returns:
/// - Positive if p is to the left of the line a->b
/// - Negative if p is to the right
/// - Zero if p is on the line
fn is_left(a: &IPoint2, b: &IPoint2, p: &IPoint2) -> i128 {
    (b.x as i128 - a.x as i128) * (p.y as i128 - a.y as i128)
        - (p.x as i128 - a.x as i128) * (b.y as i128 - a.y as i128)
}

/// Checks if point `p` lies on the line segment from `a` to `b`.
///
/// First checks collinearity (cross product == 0), then checks that p's
/// coordinates are within the bounding box of the segment.
fn is_on_segment(p: &IPoint2, a: &IPoint2, b: &IPoint2) -> bool {
    let cross = is_left(a, b, p);
    if cross != 0 {
        return false;
    }
    // p is collinear with a and b; check if it's within the segment bounds
    let min_x = a.x.min(b.x);
    let max_x = a.x.max(b.x);
    let min_y = a.y.min(b.y);
    let max_y = a.y.max(b.y);
    p.x >= min_x && p.x <= max_x && p.y >= min_y && p.y <= max_y
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square_polygon() -> Vec<IPoint2> {
        vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(10.0, 10.0),
            IPoint2::from_mm(0.0, 10.0),
        ]
    }

    #[test]
    fn point_clearly_inside() {
        let poly = square_polygon();
        assert_eq!(
            point_in_polygon(&IPoint2::from_mm(5.0, 5.0), &poly),
            PointLocation::Inside
        );
    }

    #[test]
    fn point_clearly_outside() {
        let poly = square_polygon();
        assert_eq!(
            point_in_polygon(&IPoint2::from_mm(15.0, 5.0), &poly),
            PointLocation::Outside
        );
    }

    #[test]
    fn point_on_vertex() {
        let poly = square_polygon();
        assert_eq!(
            point_in_polygon(&IPoint2::from_mm(0.0, 0.0), &poly),
            PointLocation::OnBoundary
        );
    }

    #[test]
    fn point_on_edge() {
        let poly = square_polygon();
        assert_eq!(
            point_in_polygon(&IPoint2::from_mm(5.0, 0.0), &poly),
            PointLocation::OnBoundary
        );
    }

    #[test]
    fn point_at_center_of_origin_polygon() {
        // Polygon centered at origin
        let poly = vec![
            IPoint2::from_mm(-5.0, -5.0),
            IPoint2::from_mm(5.0, -5.0),
            IPoint2::from_mm(5.0, 5.0),
            IPoint2::from_mm(-5.0, 5.0),
        ];
        assert_eq!(
            point_in_polygon(&IPoint2::from_mm(0.0, 0.0), &poly),
            PointLocation::Inside
        );
    }

    #[test]
    fn point_outside_negative_coords() {
        let poly = square_polygon();
        assert_eq!(
            point_in_polygon(&IPoint2::from_mm(-1.0, -1.0), &poly),
            PointLocation::Outside
        );
    }

    #[test]
    fn triangle_inside() {
        let tri = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(5.0, 10.0),
        ];
        assert_eq!(
            point_in_polygon(&IPoint2::from_mm(5.0, 3.0), &tri),
            PointLocation::Inside
        );
    }

    #[test]
    fn triangle_outside() {
        let tri = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(5.0, 10.0),
        ];
        assert_eq!(
            point_in_polygon(&IPoint2::from_mm(0.0, 10.0), &tri),
            PointLocation::Outside
        );
    }

    #[test]
    fn degenerate_too_few_points() {
        let pts = vec![IPoint2::from_mm(0.0, 0.0), IPoint2::from_mm(1.0, 0.0)];
        assert_eq!(
            point_in_polygon(&IPoint2::from_mm(0.5, 0.0), &pts),
            PointLocation::Outside
        );
    }

    #[test]
    fn point_on_top_edge() {
        let poly = square_polygon();
        assert_eq!(
            point_in_polygon(&IPoint2::from_mm(5.0, 10.0), &poly),
            PointLocation::OnBoundary
        );
    }
}
