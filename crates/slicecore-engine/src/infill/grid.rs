//! Grid infill pattern generation.
//!
//! Grid infill produces a crosshatch pattern by generating lines at both
//! 0 degrees (horizontal) and 90 degrees (vertical) on the same layer.
//! This provides strength in both directions simultaneously.

use slicecore_geo::polygon::ValidPolygon;

use super::rectilinear;
use super::InfillLine;

/// Generates grid infill lines for the given region.
///
/// Grid infill produces lines in both horizontal and vertical directions
/// on the same layer, creating a crosshatch pattern. Each direction uses
/// the full density -- the user should select a lower density for grid
/// since it provides strength in both directions.
///
/// # Parameters
/// - `infill_region`: The boundary polygons defining the infill area.
/// - `density`: Fill density as a fraction (0.0 = empty, 1.0 = solid).
/// - `_layer_index`: Current layer index (unused; grid is the same every layer).
/// - `line_width`: Extrusion line width in mm.
///
/// # Returns
/// A vector of [`InfillLine`] segments in both horizontal and vertical directions.
pub fn generate(
    infill_region: &[ValidPolygon],
    density: f64,
    _layer_index: usize,
    line_width: f64,
) -> Vec<InfillLine> {
    // Generate horizontal lines (0 degrees).
    let mut lines = rectilinear::generate(infill_region, density, 0.0, line_width);
    // Generate vertical lines (90 degrees) on the same layer.
    lines.extend(rectilinear::generate(
        infill_region,
        density,
        90.0,
        line_width,
    ));
    lines
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_geo::polygon::Polygon;

    /// Helper to create a validated CCW square at the origin with given size (mm).
    fn make_square(size: f64) -> ValidPolygon {
        Polygon::from_mm(&[(0.0, 0.0), (size, 0.0), (size, size), (0.0, size)])
            .validate()
            .unwrap()
    }

    #[test]
    fn grid_produces_both_directions() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.2, 0, 0.4);

        assert!(!lines.is_empty(), "Grid should produce infill lines");

        // Check that we have both horizontal (same y) and vertical (same x) lines.
        let has_horizontal = lines.iter().any(|l| l.start.y == l.end.y);
        let has_vertical = lines.iter().any(|l| l.start.x == l.end.x);

        assert!(
            has_horizontal,
            "Grid should produce horizontal lines (same y)"
        );
        assert!(
            has_vertical,
            "Grid should produce vertical lines (same x)"
        );
    }

    #[test]
    fn grid_produces_more_lines_than_rectilinear() {
        let square = make_square(20.0);
        let grid_lines = generate(&[square.clone()], 0.2, 0, 0.4);
        let rectilinear_lines = rectilinear::generate(&[square], 0.2, 0.0, 0.4);

        assert!(
            grid_lines.len() > rectilinear_lines.len(),
            "Grid ({}) should produce more lines than rectilinear ({}) at same density",
            grid_lines.len(),
            rectilinear_lines.len()
        );
    }

    #[test]
    fn grid_lines_include_same_y_and_same_x() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.2, 0, 0.4);

        let same_y_count = lines.iter().filter(|l| l.start.y == l.end.y).count();
        let same_x_count = lines.iter().filter(|l| l.start.x == l.end.x).count();

        assert!(
            same_y_count > 0,
            "Grid should have horizontal segments (same y)"
        );
        assert!(
            same_x_count > 0,
            "Grid should have vertical segments (same x)"
        );
    }

    #[test]
    fn grid_empty_region_returns_empty() {
        let lines = generate(&[], 0.2, 0, 0.4);
        assert!(lines.is_empty(), "Empty region should return empty lines");
    }

    #[test]
    fn grid_zero_density_returns_empty() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.0, 0, 0.4);
        assert!(lines.is_empty(), "Zero density should return empty lines");
    }
}
