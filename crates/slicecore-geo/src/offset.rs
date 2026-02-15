//! Polygon offsetting (inflate/deflate) via clipper2-rust.
//!
//! Offsets polygon boundaries inward (negative delta) or outward (positive
//! delta). This is used throughout the slicing pipeline for perimeter
//! generation, infill region computation, and support margin calculation.
//!
//! The offset distance is specified in internal coordinate units (use
//! `mm_to_coord` to convert from millimeters).

use clipper2_rust::{self, EndType, Path64, Paths64, Point64};
use slicecore_math::{Coord, IPoint2};

use crate::area::{signed_area_2x, signed_area_i64};
use crate::error::GeoError;
use crate::polygon::{ValidPolygon, Winding};

/// Join type for polygon offset corners.
///
/// Controls how corners are handled when offsetting:
/// - `Round`: Corners are rounded (arc approximation). Best for organic shapes.
/// - `Square`: Corners are squared at exactly the offset distance.
/// - `Miter`: Corners extend to a point, limited by a miter limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinType {
    /// Rounded corners (arc approximation).
    Round,
    /// Square corners at offset distance.
    Square,
    /// Mitered (pointed) corners, limited by miter limit.
    Miter,
}

impl JoinType {
    /// Converts to the clipper2-rust `JoinType`.
    fn to_clipper(self) -> clipper2_rust::JoinType {
        match self {
            JoinType::Round => clipper2_rust::JoinType::Round,
            JoinType::Square => clipper2_rust::JoinType::Square,
            JoinType::Miter => clipper2_rust::JoinType::Miter,
        }
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

/// Converts a `ValidPolygon` to a clipper2 `Path64`.
fn valid_polygon_to_path(poly: &ValidPolygon) -> Path64 {
    poly.points()
        .iter()
        .map(|p| Point64::new(p.x, p.y))
        .collect()
}

/// Converts a slice of `ValidPolygon` to clipper2 `Paths64`.
fn valid_polygons_to_paths(polys: &[ValidPolygon]) -> Paths64 {
    polys.iter().map(valid_polygon_to_path).collect()
}

/// Converts clipper2 result paths to validated polygons.
///
/// Degenerate paths (zero area, fewer than 3 points) are silently skipped.
/// This handles the case where inward offset causes polygon collapse.
fn paths_to_valid_polygons(paths: &Paths64) -> Vec<ValidPolygon> {
    let mut result = Vec::with_capacity(paths.len());

    for path in paths {
        if path.len() < 3 {
            continue;
        }

        let points: Vec<IPoint2> = path.iter().map(|p| IPoint2::new(p.x, p.y)).collect();

        let area_2x = signed_area_2x(&points);
        if area_2x == 0 {
            continue;
        }

        let area = signed_area_i64(&points);
        let winding = if area_2x > 0 {
            Winding::CounterClockwise
        } else {
            Winding::Clockwise
        };

        result.push(ValidPolygon::from_raw_parts(points, area, winding));
    }

    result
}

// ---------------------------------------------------------------------------
// Public offset API
// ---------------------------------------------------------------------------

/// Offsets a single polygon by the specified delta.
///
/// - Positive `delta`: outward offset (polygon grows)
/// - Negative `delta`: inward offset (polygon shrinks)
///
/// `delta` is in internal coordinate units (use `mm_to_coord` to convert
/// from millimeters). `join_type` controls corner treatment.
///
/// Returns a `Vec<ValidPolygon>` because offsetting can produce multiple
/// polygons (e.g., when an inward offset splits a concave polygon).
///
/// Returns an empty `Vec` if the polygon collapses (inward offset past
/// center). This is expected behavior, not an error.
pub fn offset_polygon(
    polygon: &ValidPolygon,
    delta: Coord,
    join_type: JoinType,
) -> Result<Vec<ValidPolygon>, GeoError> {
    let paths = vec![valid_polygon_to_path(polygon)];
    let result = clipper2_rust::inflate_paths_64(
        &paths,
        delta as f64,
        join_type.to_clipper(),
        EndType::Polygon,
        2.0, // miter_limit
        0.0, // arc_tolerance (0 = auto)
    );
    Ok(paths_to_valid_polygons(&result))
}

/// Offsets multiple polygons by the specified delta.
///
/// All polygons are offset by the same amount. This is more efficient
/// than calling [`offset_polygon`] in a loop when the polygons should
/// be processed together (e.g., for perimeter generation with proper
/// handling of adjacent boundaries).
///
/// See [`offset_polygon`] for parameter documentation.
pub fn offset_polygons(
    polygons: &[ValidPolygon],
    delta: Coord,
    join_type: JoinType,
) -> Result<Vec<ValidPolygon>, GeoError> {
    if polygons.is_empty() {
        return Ok(Vec::new());
    }
    let paths = valid_polygons_to_paths(polygons);
    let result = clipper2_rust::inflate_paths_64(
        &paths,
        delta as f64,
        join_type.to_clipper(),
        EndType::Polygon,
        2.0, // miter_limit
        0.0, // arc_tolerance (0 = auto)
    );
    Ok(paths_to_valid_polygons(&result))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::polygon::Polygon;
    use slicecore_math::mm_to_coord;

    /// Helper to create a validated CCW square.
    fn make_square(x: f64, y: f64, size: f64) -> ValidPolygon {
        Polygon::from_mm(&[(x, y), (x + size, y), (x + size, y + size), (x, y + size)])
            .validate()
            .unwrap()
    }

    fn total_area_mm2(polys: &[ValidPolygon]) -> f64 {
        polys.iter().map(|p| p.area_mm2()).sum()
    }

    #[test]
    fn outward_offset_increases_area() {
        let sq = make_square(0.0, 0.0, 10.0);
        let original_area = sq.area_mm2();
        let result = offset_polygon(&sq, mm_to_coord(1.0), JoinType::Miter).unwrap();
        assert!(!result.is_empty(), "Outward offset should produce result");
        let new_area = total_area_mm2(&result);
        assert!(
            new_area > original_area,
            "Outward offset area ({}) should be > original ({})",
            new_area,
            original_area
        );
        // 10mm square offset by 1mm with Miter -> ~12mm x 12mm = ~144 mm^2
        assert!(
            (new_area - 144.0).abs() < 2.0,
            "Expected ~144 mm^2, got {}",
            new_area
        );
    }

    #[test]
    fn inward_offset_decreases_area() {
        let sq = make_square(0.0, 0.0, 10.0);
        let original_area = sq.area_mm2();
        let result = offset_polygon(&sq, mm_to_coord(-1.0), JoinType::Miter).unwrap();
        assert!(
            !result.is_empty(),
            "Small inward offset should produce result"
        );
        let new_area = total_area_mm2(&result);
        assert!(
            new_area < original_area,
            "Inward offset area ({}) should be < original ({})",
            new_area,
            original_area
        );
        // 10mm square offset by -1mm with Miter -> ~8mm x 8mm = ~64 mm^2
        assert!(
            (new_area - 64.0).abs() < 2.0,
            "Expected ~64 mm^2, got {}",
            new_area
        );
    }

    #[test]
    fn inward_offset_past_center_collapses() {
        let sq = make_square(0.0, 0.0, 10.0);
        // Offset inward by 6mm -- for a 10mm square, this exceeds the 5mm half-width
        let result = offset_polygon(&sq, mm_to_coord(-6.0), JoinType::Miter).unwrap();
        assert!(
            result.is_empty(),
            "Inward offset past center should produce empty result, got {} polygons",
            result.len()
        );
    }

    #[test]
    fn offset_triangle_round() {
        // Triangle offset with round join -- should approximate rounded triangle
        let tri = Polygon::from_mm(&[(0.0, 0.0), (10.0, 0.0), (5.0, 10.0)])
            .validate()
            .unwrap();
        let original_area = tri.area_mm2();
        let result = offset_polygon(&tri, mm_to_coord(1.0), JoinType::Round).unwrap();
        assert!(!result.is_empty());
        let new_area = total_area_mm2(&result);
        assert!(
            new_area > original_area,
            "Rounded offset should increase area"
        );
        // Round join on triangle should produce more vertices than the original
        assert!(
            result[0].len() > 3,
            "Rounded offset triangle should have more than 3 vertices, got {}",
            result[0].len()
        );
    }

    #[test]
    fn offset_concave_polygon() {
        // L-shaped (concave) polygon
        let l_shape = Polygon::from_mm(&[
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 5.0),
            (5.0, 5.0),
            (5.0, 10.0),
            (0.0, 10.0),
        ])
        .validate()
        .unwrap();
        let original_area = l_shape.area_mm2();

        // Small outward offset
        let result = offset_polygon(&l_shape, mm_to_coord(0.5), JoinType::Miter).unwrap();
        assert!(!result.is_empty());
        let new_area = total_area_mm2(&result);
        assert!(
            new_area > original_area,
            "Outward offset of concave polygon should increase area"
        );
    }

    #[test]
    fn offset_polygons_batch() {
        let a = make_square(0.0, 0.0, 10.0);
        let b = make_square(20.0, 0.0, 10.0);
        let result = offset_polygons(&[a, b], mm_to_coord(1.0), JoinType::Miter).unwrap();
        assert!(
            result.len() >= 2,
            "Batch offset of 2 separate squares should produce >= 2 results"
        );
    }

    #[test]
    fn offset_empty_input() {
        let result = offset_polygons(&[], mm_to_coord(1.0), JoinType::Miter).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn offset_square_join_type() {
        let sq = make_square(0.0, 0.0, 10.0);
        let result = offset_polygon(&sq, mm_to_coord(1.0), JoinType::Square).unwrap();
        assert!(!result.is_empty());
        let area = total_area_mm2(&result);
        // Square join on a square is similar to miter
        assert!(
            area > 100.0,
            "Square join offset should produce larger area"
        );
    }

    #[test]
    fn offset_zero_delta_unchanged() {
        let sq = make_square(0.0, 0.0, 10.0);
        let result = offset_polygon(&sq, 0, JoinType::Miter).unwrap();
        // With delta=0, clipper2 returns a clone
        let area = total_area_mm2(&result);
        assert!(
            (area - 100.0).abs() < 1.0,
            "Zero delta should preserve area, got {}",
            area
        );
    }
}
