//! Per-slice contour resolution via Clipper2 polygon union.
//!
//! When a mesh has self-intersecting triangles, the sliced contours at
//! affected Z heights may overlap or self-intersect. This module provides
//! a contour resolution function that uses Clipper2's polygon union
//! (self-union) to clean up overlapping contours.
//!
//! The key insight is that unioning all contours with an empty clip set
//! causes Clipper2 to merge overlapping regions and resolve boundary
//! self-intersections in the subject set.

use slicecore_geo::{polygon_union, ValidPolygon};

/// Resolves overlapping and self-intersecting contours via polygon self-union.
///
/// Given a set of contours from a single slice layer, this function merges
/// overlapping regions and resolves self-intersecting boundaries by performing
/// a Clipper2 polygon union of all contours with an empty clip set.
///
/// # Behavior
///
/// - **0 contours**: Returns empty.
/// - **1+ contours**: Performs self-union to merge overlapping regions and
///   resolve self-intersecting edges. Even a single contour may have
///   self-intersecting edges (e.g., figure-8 shapes from overlapping mesh
///   bodies), which the NonZero fill rule resolves.
/// - **Error fallback**: If polygon_union fails (rare), returns the original
///   contours unchanged.
///
/// # Examples
///
/// Two overlapping squares become a single merged polygon:
///
/// ```ignore
/// let merged = resolve_contour_intersections(&overlapping_contours);
/// assert!(merged.len() <= overlapping_contours.len());
/// ```
pub fn resolve_contour_intersections(contours: &[ValidPolygon]) -> Vec<ValidPolygon> {
    if contours.is_empty() {
        return Vec::new();
    }

    // Self-union: union all contours with empty clip set to merge overlaps.
    // Even a single contour may have self-intersecting edges (e.g., from
    // overlapping mesh bodies producing a figure-8 polygon). The NonZero fill
    // rule resolves such self-intersections into proper non-overlapping polygons.
    match polygon_union(contours, &[]) {
        Ok(resolved) => resolved,
        Err(_) => {
            // Fallback: return original contours if union fails.
            contours.to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_geo::Polygon;

    /// Helper to create a validated CCW square at a given position and size.
    fn make_square(x: f64, y: f64, size: f64) -> ValidPolygon {
        Polygon::from_mm(&[(x, y), (x + size, y), (x + size, y + size), (x, y + size)])
            .validate()
            .unwrap()
    }

    /// Helper to get net area in mm^2 of a list of polygons.
    fn total_area_mm2(polys: &[ValidPolygon]) -> f64 {
        use slicecore_math::COORD_SCALE;
        polys
            .iter()
            .map(|p| p.area_i64() as f64 / (COORD_SCALE * COORD_SCALE))
            .sum::<f64>()
            .abs()
    }

    #[test]
    fn empty_input_returns_empty() {
        let result = resolve_contour_intersections(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn single_clean_contour_preserved() {
        let square = make_square(0.0, 0.0, 10.0);
        let result = resolve_contour_intersections(&[square.clone()]);
        // A clean (non-self-intersecting) single contour passes through union unchanged.
        assert_eq!(result.len(), 1);
        let area = total_area_mm2(&result);
        assert!(
            (area - 100.0).abs() < 1.0,
            "Expected ~100 mm^2, got {}",
            area
        );
    }

    #[test]
    fn overlapping_squares_merged_into_one() {
        let a = make_square(0.0, 0.0, 10.0); // (0,0)-(10,10)
        let b = make_square(5.0, 0.0, 10.0); // (5,0)-(15,10)
        let result = resolve_contour_intersections(&[a, b]);
        // Two overlapping squares should merge into a single polygon.
        assert_eq!(
            result.len(),
            1,
            "Overlapping squares should merge into one polygon, got {}",
            result.len()
        );
        let area = total_area_mm2(&result);
        // Two 10x10 squares overlapping by 5x10 = 200 - 50 = 150 mm^2
        assert!(
            (area - 150.0).abs() < 1.0,
            "Expected ~150 mm^2, got {}",
            area
        );
    }

    #[test]
    fn non_overlapping_contours_preserved() {
        let a = make_square(0.0, 0.0, 10.0);
        let b = make_square(20.0, 0.0, 10.0); // far apart
        let result = resolve_contour_intersections(&[a, b]);
        assert_eq!(
            result.len(),
            2,
            "Non-overlapping contours should be preserved, got {}",
            result.len()
        );
        let area = total_area_mm2(&result);
        assert!(
            (area - 200.0).abs() < 1.0,
            "Expected ~200 mm^2, got {}",
            area
        );
    }
}
