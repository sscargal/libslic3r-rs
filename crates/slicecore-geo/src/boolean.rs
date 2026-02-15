//! Polygon boolean operations via clipper2-rust.
//!
//! Wraps the clipper2-rust library to provide union, intersection, difference,
//! and XOR operations on validated polygons. All operations use the NonZero
//! fill rule, which is standard for slicing operations.
//!
//! The conversion pipeline:
//! 1. `ValidPolygon` -> clipper2 `Path64` (point-by-point copy)
//! 2. Perform boolean operation via clipper2-rust
//! 3. clipper2 `Paths64` -> `Vec<ValidPolygon>` (validate each result path)

use clipper2_rust::{self, FillRule, Path64, Paths64, Point64};
use slicecore_math::IPoint2;

use crate::area::{signed_area_2x, signed_area_i64};
use crate::error::GeoError;
use crate::polygon::{ValidPolygon, Winding};

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

/// Converts clipper2 `Paths64` results back to validated polygons.
///
/// Each result path is checked for minimum vertex count and non-zero area.
/// Degenerate result paths (zero area, too few points) are silently skipped
/// -- this is expected behavior for boolean operations that produce thin
/// slivers or collapse regions.
fn paths_to_valid_polygons(paths: &Paths64) -> Result<Vec<ValidPolygon>, GeoError> {
    let mut result = Vec::with_capacity(paths.len());

    for path in paths {
        if path.len() < 3 {
            continue; // Skip degenerate paths
        }

        let points: Vec<IPoint2> = path.iter().map(|p| IPoint2::new(p.x, p.y)).collect();

        let area_2x = signed_area_2x(&points);
        if area_2x == 0 {
            continue; // Skip zero-area slivers
        }

        let area = signed_area_i64(&points);
        let winding = if area_2x > 0 {
            Winding::CounterClockwise
        } else {
            Winding::Clockwise
        };

        result.push(ValidPolygon::from_raw_parts(points, area, winding));
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Public boolean operations
// ---------------------------------------------------------------------------

/// Computes the union (logical OR) of subject and clip polygons.
///
/// The result contains all regions that are inside either the subjects or
/// the clips (or both). Non-overlapping polygons are returned as separate
/// polygons.
///
/// Uses the NonZero fill rule.
pub fn polygon_union(
    subjects: &[ValidPolygon],
    clips: &[ValidPolygon],
) -> Result<Vec<ValidPolygon>, GeoError> {
    let subject_paths = valid_polygons_to_paths(subjects);
    let clip_paths = valid_polygons_to_paths(clips);
    let result = clipper2_rust::union_64(&subject_paths, &clip_paths, FillRule::NonZero);
    paths_to_valid_polygons(&result)
}

/// Computes the intersection (logical AND) of subject and clip polygons.
///
/// The result contains only regions that are inside both the subjects and
/// the clips. Non-overlapping inputs produce an empty result.
///
/// Uses the NonZero fill rule.
pub fn polygon_intersection(
    subjects: &[ValidPolygon],
    clips: &[ValidPolygon],
) -> Result<Vec<ValidPolygon>, GeoError> {
    let subject_paths = valid_polygons_to_paths(subjects);
    let clip_paths = valid_polygons_to_paths(clips);
    let result = clipper2_rust::intersect_64(&subject_paths, &clip_paths, FillRule::NonZero);
    paths_to_valid_polygons(&result)
}

/// Computes the difference (subjects minus clips) of polygons.
///
/// The result contains regions that are inside the subjects but not inside
/// the clips. If clips fully contain the subjects, the result is empty.
///
/// Uses the NonZero fill rule.
pub fn polygon_difference(
    subjects: &[ValidPolygon],
    clips: &[ValidPolygon],
) -> Result<Vec<ValidPolygon>, GeoError> {
    let subject_paths = valid_polygons_to_paths(subjects);
    let clip_paths = valid_polygons_to_paths(clips);
    let result = clipper2_rust::difference_64(&subject_paths, &clip_paths, FillRule::NonZero);
    paths_to_valid_polygons(&result)
}

/// Computes the symmetric difference (XOR) of subject and clip polygons.
///
/// The result contains regions that are inside the subjects or the clips
/// but not both. Overlapping regions are removed.
///
/// Uses the NonZero fill rule.
pub fn polygon_xor(
    subjects: &[ValidPolygon],
    clips: &[ValidPolygon],
) -> Result<Vec<ValidPolygon>, GeoError> {
    let subject_paths = valid_polygons_to_paths(subjects);
    let clip_paths = valid_polygons_to_paths(clips);
    let result = clipper2_rust::xor_64(&subject_paths, &clip_paths, FillRule::NonZero);
    paths_to_valid_polygons(&result)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::polygon::Polygon;
    /// Helper to create a validated CCW square at a given position and size.
    fn make_square(x: f64, y: f64, size: f64) -> ValidPolygon {
        Polygon::from_mm(&[
            (x, y),
            (x + size, y),
            (x + size, y + size),
            (x, y + size),
        ])
        .validate()
        .unwrap()
    }

    /// Helper to get net area in mm^2 of a list of polygons.
    ///
    /// Uses signed area so that holes (CW, negative area) subtract from
    /// outer boundaries (CCW, positive area). This gives the true net area
    /// of a polygon-with-holes result.
    fn total_area_mm2(polys: &[ValidPolygon]) -> f64 {
        use slicecore_math::COORD_SCALE;
        polys
            .iter()
            .map(|p| p.area_i64() as f64 / (COORD_SCALE * COORD_SCALE))
            .sum::<f64>()
            .abs()
    }

    // ---- Basic correctness (5 tests) ----

    #[test]
    fn union_overlapping_squares() {
        let a = make_square(0.0, 0.0, 10.0); // (0,0)-(10,10)
        let b = make_square(5.0, 0.0, 10.0); // (5,0)-(15,10)
        let result = polygon_union(&[a], &[b]).unwrap();
        assert!(!result.is_empty(), "Union should produce at least one polygon");
        let area = total_area_mm2(&result);
        // Two 10x10 squares overlapping by 5x10 = 200 - 50 = 150 mm^2
        assert!(
            (area - 150.0).abs() < 1.0,
            "Expected ~150 mm^2, got {}",
            area
        );
    }

    #[test]
    fn intersection_overlapping_squares() {
        let a = make_square(0.0, 0.0, 10.0);
        let b = make_square(5.0, 0.0, 10.0);
        let result = polygon_intersection(&[a], &[b]).unwrap();
        assert!(!result.is_empty());
        let area = total_area_mm2(&result);
        // Overlap is 5x10 = 50 mm^2
        assert!(
            (area - 50.0).abs() < 1.0,
            "Expected ~50 mm^2, got {}",
            area
        );
    }

    #[test]
    fn difference_removes_overlap() {
        let a = make_square(0.0, 0.0, 10.0);
        let b = make_square(5.0, 0.0, 10.0);
        let result = polygon_difference(&[a], &[b]).unwrap();
        assert!(!result.is_empty());
        let area = total_area_mm2(&result);
        // A - B = 100 - 50 = 50 mm^2
        assert!(
            (area - 50.0).abs() < 1.0,
            "Expected ~50 mm^2, got {}",
            area
        );
    }

    #[test]
    fn xor_overlapping_squares() {
        let a = make_square(0.0, 0.0, 10.0);
        let b = make_square(5.0, 0.0, 10.0);
        let result = polygon_xor(&[a], &[b]).unwrap();
        assert!(!result.is_empty());
        let area = total_area_mm2(&result);
        // XOR = (A union B) - (A intersect B) = 150 - 50 = 100 mm^2
        assert!(
            (area - 100.0).abs() < 1.0,
            "Expected ~100 mm^2, got {}",
            area
        );
    }

    #[test]
    fn union_identical_squares() {
        let a = make_square(0.0, 0.0, 10.0);
        let b = make_square(0.0, 0.0, 10.0);
        let result = polygon_union(&[a], &[b]).unwrap();
        assert_eq!(result.len(), 1, "Union of identical squares = 1 polygon");
        let area = total_area_mm2(&result);
        assert!(
            (area - 100.0).abs() < 1.0,
            "Expected ~100 mm^2, got {}",
            area
        );
    }

    // ---- Edge cases (5 tests) ----

    #[test]
    fn union_non_overlapping_produces_two() {
        let a = make_square(0.0, 0.0, 10.0);
        let b = make_square(20.0, 0.0, 10.0);
        let result = polygon_union(&[a], &[b]).unwrap();
        assert_eq!(
            result.len(),
            2,
            "Non-overlapping union should produce 2 polygons"
        );
        let area = total_area_mm2(&result);
        assert!(
            (area - 200.0).abs() < 1.0,
            "Expected ~200 mm^2, got {}",
            area
        );
    }

    #[test]
    fn intersection_non_overlapping_empty() {
        let a = make_square(0.0, 0.0, 10.0);
        let b = make_square(20.0, 0.0, 10.0);
        let result = polygon_intersection(&[a], &[b]).unwrap();
        assert!(
            result.is_empty(),
            "Intersection of non-overlapping should be empty"
        );
    }

    #[test]
    fn difference_b_contains_a() {
        let a = make_square(2.0, 2.0, 6.0); // inner
        let b = make_square(0.0, 0.0, 10.0); // outer
        let result = polygon_difference(&[a], &[b]).unwrap();
        assert!(
            result.is_empty(),
            "A fully inside B: A - B should be empty"
        );
    }

    #[test]
    fn difference_identical_is_empty() {
        let a = make_square(0.0, 0.0, 10.0);
        let b = make_square(0.0, 0.0, 10.0);
        let result = polygon_difference(&[a], &[b]).unwrap();
        assert!(
            result.is_empty(),
            "Identical: A - A should be empty"
        );
    }

    #[test]
    fn union_with_no_clips() {
        let a = make_square(0.0, 0.0, 10.0);
        let result = polygon_union(&[a], &[]).unwrap();
        assert_eq!(result.len(), 1);
        let area = total_area_mm2(&result);
        assert!(
            (area - 100.0).abs() < 1.0,
            "Single polygon union should return itself"
        );
    }

    // ---- Degenerate geometry (10+ tests) ----

    #[test]
    fn degenerate_zero_area_spike() {
        // Triangle with a nearly-collinear vertex forming a spike.
        // The spike has near-zero area. Boolean ops should not crash.
        let spike = Polygon::from_mm(&[
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 0.001), // very thin spike
        ]);
        if let Ok(valid_spike) = spike.validate() {
            let square = make_square(0.0, 0.0, 10.0);
            let _result = polygon_union(&[square], &[valid_spike]);
            // Just verify it doesn't crash
        }
    }

    #[test]
    fn degenerate_collinear_vertices() {
        // Square with extra collinear points on edges.
        // After validation, collinear points are removed, so boolean ops see a clean square.
        let sq_with_collinear = Polygon::from_mm(&[
            (0.0, 0.0),
            (5.0, 0.0), // collinear
            (10.0, 0.0),
            (10.0, 5.0), // collinear
            (10.0, 10.0),
            (0.0, 10.0),
        ]);
        let valid = sq_with_collinear.validate().unwrap();
        let other = make_square(5.0, 0.0, 10.0);
        let result = polygon_union(&[valid], &[other]).unwrap();
        assert!(!result.is_empty());
        let area = total_area_mm2(&result);
        // Same as overlapping squares test
        assert!(
            (area - 150.0).abs() < 1.0,
            "Collinear: expected ~150 mm^2, got {}",
            area
        );
    }

    #[test]
    fn degenerate_very_thin_polygon() {
        // Very thin polygon: width of 1 internal unit = 1 nanometer
        let thin = Polygon::new(vec![
            IPoint2::new(0, 0),
            IPoint2::new(10_000_000, 0), // 10mm long
            IPoint2::new(10_000_000, 1), // 1 nanometer wide
            IPoint2::new(0, 1),
        ]);
        if let Ok(valid_thin) = thin.validate() {
            let square = make_square(0.0, 0.0, 10.0);
            let _result = polygon_union(&[square], &[valid_thin]);
            // Just verify it doesn't crash
        }
    }

    #[test]
    fn degenerate_duplicate_consecutive_vertices() {
        // Polygon with duplicate consecutive vertices -- validation removes them
        let poly = Polygon::from_mm(&[
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 0.0), // duplicate
            (10.0, 10.0),
            (0.0, 10.0),
        ]);
        let valid = poly.validate().unwrap();
        let other = make_square(5.0, 0.0, 10.0);
        let result = polygon_intersection(&[valid], &[other]).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn degenerate_coincident_edges() {
        // Two polygons sharing an edge exactly -- union should merge cleanly
        let a = make_square(0.0, 0.0, 10.0);
        let b = make_square(10.0, 0.0, 10.0); // shares edge at x=10
        let result = polygon_union(&[a], &[b]).unwrap();
        assert!(!result.is_empty());
        let area = total_area_mm2(&result);
        assert!(
            (area - 200.0).abs() < 1.0,
            "Adjacent squares union: expected ~200 mm^2, got {}",
            area
        );
    }

    #[test]
    fn degenerate_large_coordinates() {
        // Large polygon: coordinates near 1_000_000 mm (1km) -- should not overflow
        let large = Polygon::from_mm(&[
            (0.0, 0.0),
            (1_000_000.0, 0.0),
            (1_000_000.0, 1_000_000.0),
            (0.0, 1_000_000.0),
        ]);
        let valid = large.validate().unwrap();
        let other = Polygon::from_mm(&[
            (500_000.0, 0.0),
            (1_500_000.0, 0.0),
            (1_500_000.0, 1_000_000.0),
            (500_000.0, 1_000_000.0),
        ])
        .validate()
        .unwrap();

        let result = polygon_union(&[valid], &[other]).unwrap();
        assert!(!result.is_empty(), "Large coordinate union should work");
    }

    #[test]
    fn degenerate_very_small_polygon() {
        // Very small polygon: 1 micrometer triangle (1e-3 mm)
        let small = Polygon::from_mm(&[
            (0.0, 0.0),
            (0.001, 0.0),
            (0.0, 0.001),
        ]);
        if let Ok(valid_small) = small.validate() {
            let other = make_square(0.0, 0.0, 10.0);
            let result = polygon_union(&[other], &[valid_small]).unwrap();
            assert!(!result.is_empty());
        }
    }

    #[test]
    fn degenerate_polygon_with_hole() {
        // Outer CCW + inner CW hole
        let outer = make_square(0.0, 0.0, 20.0); // CCW outer
        let hole = make_square(5.0, 5.0, 10.0).ensure_cw(); // CW hole

        // Union of outer with hole treated as subject -- the hole should persist
        let result = polygon_difference(&[outer], &[hole.ensure_ccw()]).unwrap();
        assert!(!result.is_empty());
        let area = total_area_mm2(&result);
        // 20x20 - 10x10 = 400 - 100 = 300 mm^2
        assert!(
            (area - 300.0).abs() < 2.0,
            "Polygon with hole: expected ~300 mm^2, got {}",
            area
        );
    }

    #[test]
    fn degenerate_many_vertices_circle() {
        // Circle approximation with 100+ vertices
        let n = 200;
        let radius = 10.0; // 10mm radius
        let center = (50.0, 50.0);
        let points: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let angle = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
                (
                    center.0 + radius * angle.cos(),
                    center.1 + radius * angle.sin(),
                )
            })
            .collect();
        let circle = Polygon::from_mm(&points).validate().unwrap();
        let square = make_square(45.0, 45.0, 10.0);
        let result = polygon_intersection(&[circle], &[square]).unwrap();
        assert!(!result.is_empty(), "Circle-square intersection should produce result");
    }

    #[test]
    fn degenerate_self_intersecting_figure_eight() {
        // Figure-8: ValidPolygon may reject this (self-intersection),
        // but we test that the system handles it gracefully.
        let fig8 = Polygon::from_mm(&[
            (0.0, 0.0),
            (10.0, 10.0),
            (10.0, 0.0),
            (0.0, 10.0), // crosses the first edge
        ]);
        // ValidPolygon might accept or reject depending on whether
        // the simple cross-product check catches it. Either way, no crash.
        match fig8.validate() {
            Ok(valid) => {
                let square = make_square(0.0, 0.0, 10.0);
                let _result = polygon_union(&[square], &[valid]);
                // Just verify no crash
            }
            Err(_) => {
                // Expected: self-intersecting polygon rejected by validation
            }
        }
    }

    #[test]
    fn degenerate_touching_at_single_point() {
        // Two squares touching at a single corner point
        let a = make_square(0.0, 0.0, 10.0);
        let b = make_square(10.0, 10.0, 10.0); // touches at (10,10)
        let result = polygon_union(&[a], &[b]).unwrap();
        let area = total_area_mm2(&result);
        assert!(
            (area - 200.0).abs() < 1.0,
            "Corner-touching union: expected ~200 mm^2, got {}",
            area
        );
    }

    #[test]
    fn degenerate_one_polygon_inside_other() {
        // Small square fully inside large square
        let outer = make_square(0.0, 0.0, 20.0);
        let inner = make_square(5.0, 5.0, 10.0);
        let result = polygon_union(&[outer], &[inner]).unwrap();
        assert_eq!(result.len(), 1, "Inner polygon absorbed by outer");
        let area = total_area_mm2(&result);
        assert!(
            (area - 400.0).abs() < 1.0,
            "Nested union: expected 400 mm^2, got {}",
            area
        );
    }

    // ---- Additional correctness tests ----

    #[test]
    fn intersection_partial_overlap() {
        // Two squares overlapping in a corner
        let a = make_square(0.0, 0.0, 10.0);
        let b = make_square(5.0, 5.0, 10.0);
        let result = polygon_intersection(&[a], &[b]).unwrap();
        assert!(!result.is_empty());
        let area = total_area_mm2(&result);
        // Overlap is 5x5 = 25 mm^2
        assert!(
            (area - 25.0).abs() < 1.0,
            "Corner overlap: expected ~25 mm^2, got {}",
            area
        );
    }

    #[test]
    fn difference_partial() {
        // A large square minus a small inner square
        let a = make_square(0.0, 0.0, 20.0);
        let b = make_square(5.0, 5.0, 10.0);
        let result = polygon_difference(&[a], &[b]).unwrap();
        assert!(!result.is_empty());
        let area = total_area_mm2(&result);
        // 400 - 100 = 300 mm^2
        assert!(
            (area - 300.0).abs() < 2.0,
            "Partial diff: expected ~300 mm^2, got {}",
            area
        );
    }

    #[test]
    fn union_preserves_winding() {
        let a = make_square(0.0, 0.0, 10.0);
        let result = polygon_union(&[a], &[]).unwrap();
        assert_eq!(result.len(), 1);
        // Result should be CCW (outer boundary)
        assert_eq!(
            result[0].winding(),
            Winding::CounterClockwise,
            "Union result should be CCW"
        );
    }

    #[test]
    fn multiple_subjects_union() {
        let a = make_square(0.0, 0.0, 10.0);
        let b = make_square(5.0, 0.0, 10.0);
        let c = make_square(10.0, 0.0, 10.0);
        let result = polygon_union(&[a, b, c], &[]).unwrap();
        let area = total_area_mm2(&result);
        // Three overlapping squares from x=0..20, y=0..10 = 200 mm^2
        assert!(
            (area - 200.0).abs() < 1.0,
            "Three overlapping: expected ~200 mm^2, got {}",
            area
        );
    }
}
