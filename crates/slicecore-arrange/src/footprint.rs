//! Footprint computation and collision detection for part arrangement.
//!
//! Projects 3D mesh vertices to 2D XY convex hulls, expands footprints
//! for spacing/brim/raft margins, and detects collisions between parts
//! via polygon intersection.

use slicecore_geo::{convex_hull, offset_polygon, polygon_intersection, JoinType, Polygon};
use slicecore_math::{mm_to_coord, IPoint2, Point3, COORD_SCALE};

/// Projects 3D vertices to XY and computes the convex hull footprint.
///
/// Each [`Point3`] is projected to [`IPoint2`] by discarding Z and scaling
/// X/Y via [`COORD_SCALE`]. The convex hull of the projected points is
/// returned.
///
/// For degenerate inputs (collinear or single-point), falls back to a
/// bounding box rectangle.
///
/// # Examples
///
/// ```
/// use slicecore_math::Point3;
/// use slicecore_arrange::footprint::compute_footprint;
///
/// let vertices = vec![
///     Point3::new(0.0, 0.0, 0.0),
///     Point3::new(10.0, 0.0, 0.0),
///     Point3::new(10.0, 10.0, 5.0),
///     Point3::new(0.0, 10.0, 5.0),
/// ];
/// let hull = compute_footprint(&vertices);
/// assert_eq!(hull.len(), 4);
/// ```
#[must_use]
pub fn compute_footprint(vertices: &[Point3]) -> Vec<IPoint2> {
    if vertices.is_empty() {
        return Vec::new();
    }

    let projected: Vec<IPoint2> = vertices
        .iter()
        .map(|v| IPoint2::from_mm(v.x, v.y))
        .collect();

    let hull = convex_hull(&projected);

    // Degenerate: fewer than 3 hull points -- fall back to bounding box
    if hull.len() < 3 {
        return bounding_box_fallback(&projected);
    }

    hull
}

/// Expands a convex hull footprint outward for spacing, brim, and raft.
///
/// The total expansion distance is `spacing_mm / 2.0 + brim_width_mm + raft_margin_mm`,
/// applied as an outward polygon offset with round join type.
///
/// If the expansion is zero or negative, the original hull is returned.
/// If the offset operation collapses (returns empty), the original hull
/// is returned as a fallback.
///
/// # Examples
///
/// ```
/// use slicecore_math::IPoint2;
/// use slicecore_arrange::footprint::expand_footprint;
///
/// let hull = vec![
///     IPoint2::from_mm(0.0, 0.0),
///     IPoint2::from_mm(10.0, 0.0),
///     IPoint2::from_mm(10.0, 10.0),
///     IPoint2::from_mm(0.0, 10.0),
/// ];
/// let expanded = expand_footprint(&hull, 2.0, 0.0, 0.0);
/// // Expanded hull should be larger than original
/// assert!(expanded.len() >= 4);
/// ```
#[must_use]
pub fn expand_footprint(
    hull: &[IPoint2],
    spacing_mm: f64,
    brim_width_mm: f64,
    raft_margin_mm: f64,
) -> Vec<IPoint2> {
    let total_mm = spacing_mm / 2.0 + brim_width_mm + raft_margin_mm;
    if total_mm <= 0.0 || hull.len() < 3 {
        return hull.to_vec();
    }

    let delta = mm_to_coord(total_mm);

    // Convert hull to ValidPolygon via Polygon::validate
    let polygon = Polygon::new(hull.to_vec());
    let Ok(valid) = polygon.validate() else {
        return hull.to_vec();
    };

    match offset_polygon(&valid, delta, JoinType::Round) {
        Ok(result) if !result.is_empty() => result[0].points().to_vec(),
        _ => hull.to_vec(),
    }
}

/// Tests whether two footprint polygons overlap.
///
/// Returns `true` if the polygon intersection of the two footprints
/// is non-empty (they share interior area).
///
/// # Examples
///
/// ```
/// use slicecore_math::IPoint2;
/// use slicecore_arrange::footprint::footprints_overlap;
///
/// let a = vec![
///     IPoint2::from_mm(0.0, 0.0),
///     IPoint2::from_mm(10.0, 0.0),
///     IPoint2::from_mm(10.0, 10.0),
///     IPoint2::from_mm(0.0, 10.0),
/// ];
/// let b = vec![
///     IPoint2::from_mm(5.0, 5.0),
///     IPoint2::from_mm(15.0, 5.0),
///     IPoint2::from_mm(15.0, 15.0),
///     IPoint2::from_mm(5.0, 15.0),
/// ];
/// assert!(footprints_overlap(&a, &b));
/// ```
#[must_use]
pub fn footprints_overlap(a: &[IPoint2], b: &[IPoint2]) -> bool {
    if a.len() < 3 || b.len() < 3 {
        return false;
    }

    let poly_a = Polygon::new(a.to_vec());
    let poly_b = Polygon::new(b.to_vec());

    let Ok(valid_a) = poly_a.validate() else {
        return false;
    };
    let Ok(valid_b) = poly_b.validate() else {
        return false;
    };

    match polygon_intersection(&[valid_a], &[valid_b]) {
        Ok(result) => !result.is_empty(),
        Err(_) => false,
    }
}

/// Rotates a footprint hull around its centroid by the given angle.
///
/// The rotation is always applied from the original hull (not accumulated).
/// The result is re-convex-hulled to clean up any floating-point precision
/// artifacts.
///
/// # Examples
///
/// ```
/// use slicecore_math::IPoint2;
/// use slicecore_arrange::footprint::rotate_footprint;
///
/// let hull = vec![
///     IPoint2::from_mm(0.0, 0.0),
///     IPoint2::from_mm(10.0, 0.0),
///     IPoint2::from_mm(10.0, 5.0),
///     IPoint2::from_mm(0.0, 5.0),
/// ];
/// let rotated = rotate_footprint(&hull, 90.0);
/// assert!(rotated.len() >= 3);
/// ```
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    reason = "i64 coord values are within f64 precision for printing dimensions"
)]
pub fn rotate_footprint(hull: &[IPoint2], angle_deg: f64) -> Vec<IPoint2> {
    if hull.is_empty() {
        return Vec::new();
    }

    let center = centroid(hull);
    let angle_rad = angle_deg.to_radians();
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();

    let cx = center.x as f64;
    let cy = center.y as f64;

    let rotated: Vec<IPoint2> = hull
        .iter()
        .map(|p| {
            let dx = p.x as f64 - cx;
            let dy = p.y as f64 - cy;
            let rx = dx * cos_a - dy * sin_a + cx;
            let ry = dx * sin_a + dy * cos_a + cy;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "coords are within i64 range for printing dimensions"
            )]
            IPoint2::new(rx.round() as i64, ry.round() as i64)
        })
        .collect();

    convex_hull(&rotated)
}

/// Computes the area of a footprint hull in square millimeters.
///
/// Uses the shoelace formula on integer coordinates, converting via
/// [`COORD_SCALE`].
///
/// # Examples
///
/// ```
/// use slicecore_math::IPoint2;
/// use slicecore_arrange::footprint::footprint_area;
///
/// let hull = vec![
///     IPoint2::from_mm(0.0, 0.0),
///     IPoint2::from_mm(10.0, 0.0),
///     IPoint2::from_mm(10.0, 10.0),
///     IPoint2::from_mm(0.0, 10.0),
/// ];
/// let area = footprint_area(&hull);
/// assert!((area - 100.0).abs() < 0.01);
/// ```
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    reason = "polygon area in coord^2 fits comfortably in f64 mantissa for printing dimensions"
)]
pub fn footprint_area(hull: &[IPoint2]) -> f64 {
    if hull.len() < 3 {
        return 0.0;
    }
    let mut sum: i128 = 0;
    let n = hull.len();
    for i in 0..n {
        let a = &hull[i];
        let b = &hull[(i + 1) % n];
        sum += i128::from(a.x) * i128::from(b.y) - i128::from(b.x) * i128::from(a.y);
    }
    (sum.unsigned_abs() as f64) / (COORD_SCALE * COORD_SCALE * 2.0)
}

/// Computes the centroid (average) of hull points.
///
/// # Examples
///
/// ```
/// use slicecore_math::IPoint2;
/// use slicecore_arrange::footprint::centroid;
///
/// let hull = vec![
///     IPoint2::from_mm(0.0, 0.0),
///     IPoint2::from_mm(10.0, 0.0),
///     IPoint2::from_mm(10.0, 10.0),
///     IPoint2::from_mm(0.0, 10.0),
/// ];
/// let c = centroid(&hull);
/// let (cx, cy) = c.to_mm();
/// assert!((cx - 5.0).abs() < 0.01);
/// assert!((cy - 5.0).abs() < 0.01);
/// ```
#[must_use]
#[expect(
    clippy::cast_possible_wrap,
    reason = "hull length is always small (polygon vertices) so usize->i64 is safe"
)]
pub fn centroid(hull: &[IPoint2]) -> IPoint2 {
    if hull.is_empty() {
        return IPoint2::zero();
    }
    let n = hull.len() as i64;
    let sum_x: i64 = hull.iter().map(|p| p.x).sum();
    let sum_y: i64 = hull.iter().map(|p| p.y).sum();
    IPoint2::new(sum_x / n, sum_y / n)
}

/// Falls back to a bounding box rectangle for degenerate hulls.
fn bounding_box_fallback(points: &[IPoint2]) -> Vec<IPoint2> {
    if points.is_empty() {
        return Vec::new();
    }

    let min_x = points.iter().map(|p| p.x).min().unwrap_or(0);
    let max_x = points.iter().map(|p| p.x).max().unwrap_or(0);
    let min_y = points.iter().map(|p| p.y).min().unwrap_or(0);
    let max_y = points.iter().map(|p| p.y).max().unwrap_or(0);

    // If all points are identical, create a tiny box around them
    let (min_x, max_x) = if min_x == max_x {
        (min_x - 1000, max_x + 1000) // +/- 1 micrometer
    } else {
        (min_x, max_x)
    };
    let (min_y, max_y) = if min_y == max_y {
        (min_y - 1000, max_y + 1000)
    } else {
        (min_y, max_y)
    };

    vec![
        IPoint2::new(min_x, min_y),
        IPoint2::new(max_x, min_y),
        IPoint2::new(max_x, max_y),
        IPoint2::new(min_x, max_y),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footprint_square_mesh() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, 0.0),
            Point3::new(10.0, 10.0, 5.0),
            Point3::new(0.0, 10.0, 5.0),
            Point3::new(5.0, 5.0, 10.0), // interior point, should be excluded
        ];
        let hull = compute_footprint(&vertices);
        assert_eq!(hull.len(), 4, "Square footprint should have 4 hull points");
    }

    #[test]
    fn footprint_degenerate_vertical_line() {
        // All points on a vertical line (same X/Y, different Z)
        let vertices = vec![
            Point3::new(5.0, 5.0, 0.0),
            Point3::new(5.0, 5.0, 10.0),
            Point3::new(5.0, 5.0, 20.0),
        ];
        let hull = compute_footprint(&vertices);
        // Should fall back to bounding box (4 corners)
        assert_eq!(
            hull.len(),
            4,
            "Degenerate single-point should produce bbox rectangle"
        );
    }

    #[test]
    fn footprint_degenerate_collinear_xy() {
        // Points collinear in XY (different Z)
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, 5.0),
            Point3::new(20.0, 0.0, 10.0),
        ];
        let hull = compute_footprint(&vertices);
        // Should fall back to bounding box rectangle
        assert_eq!(hull.len(), 4, "Collinear XY should produce bbox rectangle");
    }

    #[test]
    fn footprint_empty_vertices() {
        let hull = compute_footprint(&[]);
        assert!(hull.is_empty());
    }

    #[test]
    fn expand_with_positive_spacing() {
        let hull = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(10.0, 10.0),
            IPoint2::from_mm(0.0, 10.0),
        ];
        let expanded = expand_footprint(&hull, 2.0, 0.0, 0.0);
        let original_area = footprint_area(&hull);
        let expanded_area = footprint_area(&expanded);
        assert!(
            expanded_area > original_area,
            "Expanded area ({expanded_area}) should be > original ({original_area})"
        );
    }

    #[test]
    fn expand_with_brim_and_raft() {
        let hull = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(10.0, 10.0),
            IPoint2::from_mm(0.0, 10.0),
        ];
        let expanded = expand_footprint(&hull, 2.0, 3.0, 1.0);
        let original_area = footprint_area(&hull);
        let expanded_area = footprint_area(&expanded);
        // total expansion = 2/2 + 3 + 1 = 5mm
        assert!(
            expanded_area > original_area + 50.0,
            "Brim+raft expansion should significantly increase area"
        );
    }

    #[test]
    fn expand_zero_passthrough() {
        let hull = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(10.0, 10.0),
            IPoint2::from_mm(0.0, 10.0),
        ];
        let expanded = expand_footprint(&hull, 0.0, 0.0, 0.0);
        assert_eq!(expanded, hull, "Zero expansion should return original");
    }

    #[test]
    fn overlap_overlapping_squares() {
        let a = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(10.0, 10.0),
            IPoint2::from_mm(0.0, 10.0),
        ];
        let b = vec![
            IPoint2::from_mm(5.0, 5.0),
            IPoint2::from_mm(15.0, 5.0),
            IPoint2::from_mm(15.0, 15.0),
            IPoint2::from_mm(5.0, 15.0),
        ];
        assert!(footprints_overlap(&a, &b));
    }

    #[test]
    fn overlap_separated_squares() {
        let a = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(10.0, 10.0),
            IPoint2::from_mm(0.0, 10.0),
        ];
        let b = vec![
            IPoint2::from_mm(20.0, 20.0),
            IPoint2::from_mm(30.0, 20.0),
            IPoint2::from_mm(30.0, 30.0),
            IPoint2::from_mm(20.0, 30.0),
        ];
        assert!(!footprints_overlap(&a, &b));
    }

    #[test]
    fn rotate_90_degrees() {
        // Rectangle 20mm wide, 10mm tall
        let hull = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(20.0, 0.0),
            IPoint2::from_mm(20.0, 10.0),
            IPoint2::from_mm(0.0, 10.0),
        ];
        let rotated = rotate_footprint(&hull, 90.0);
        assert!(
            rotated.len() >= 3,
            "Rotated hull should have at least 3 points"
        );
        // After 90 degree rotation, the footprint area should be preserved
        let orig_area = footprint_area(&hull);
        let rot_area = footprint_area(&rotated);
        assert!(
            (orig_area - rot_area).abs() < 1.0,
            "Rotation should preserve area: {orig_area} vs {rot_area}"
        );
    }

    #[test]
    fn rotate_360_degrees_identity() {
        let hull = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(10.0, 10.0),
            IPoint2::from_mm(0.0, 10.0),
        ];
        let rotated = rotate_footprint(&hull, 360.0);
        let orig_area = footprint_area(&hull);
        let rot_area = footprint_area(&rotated);
        assert!(
            (orig_area - rot_area).abs() < 0.1,
            "360-degree rotation should preserve area: {orig_area} vs {rot_area}"
        );
    }

    #[test]
    fn area_computation() {
        let hull = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(10.0, 10.0),
            IPoint2::from_mm(0.0, 10.0),
        ];
        let area = footprint_area(&hull);
        assert!((area - 100.0).abs() < 0.01, "Expected 100 mm^2, got {area}");
    }

    #[test]
    fn area_triangle() {
        let hull = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(5.0, 10.0),
        ];
        let area = footprint_area(&hull);
        assert!((area - 50.0).abs() < 0.01, "Expected 50 mm^2, got {area}");
    }

    #[test]
    fn centroid_of_square() {
        let hull = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(10.0, 10.0),
            IPoint2::from_mm(0.0, 10.0),
        ];
        let c = centroid(&hull);
        let (cx, cy) = c.to_mm();
        assert!(
            (cx - 5.0).abs() < 0.01,
            "Centroid X: expected 5.0, got {cx}"
        );
        assert!(
            (cy - 5.0).abs() < 0.01,
            "Centroid Y: expected 5.0, got {cy}"
        );
    }
}
