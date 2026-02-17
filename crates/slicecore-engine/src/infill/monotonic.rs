//! Monotonic infill pattern generation.
//!
//! Monotonic infill is similar to rectilinear but all lines are printed
//! in a single direction (left-to-right for horizontal, bottom-to-top
//! for vertical). This eliminates ridges from bidirectional printing,
//! producing smoother top surfaces.

use slicecore_geo::polygon::ValidPolygon;

use super::rectilinear;
use super::{alternate_infill_angle, InfillLine};

/// Generates monotonic infill lines for the given region.
///
/// Monotonic infill produces the same scanlines as rectilinear, but ensures
/// all lines are printed in a consistent direction (left-to-right for
/// horizontal lines, bottom-to-top for vertical lines). This eliminates
/// the serpentine pattern that creates visible ridges on top surfaces.
///
/// # Parameters
/// - `infill_region`: The boundary polygons defining the infill area.
/// - `density`: Fill density as a fraction (0.0 = empty, 1.0 = solid).
/// - `layer_index`: Current layer index (used for angle alternation).
/// - `line_width`: Extrusion line width in mm.
///
/// # Returns
/// A vector of [`InfillLine`] segments, all oriented in the same direction.
pub fn generate(
    infill_region: &[ValidPolygon],
    density: f64,
    layer_index: usize,
    line_width: f64,
) -> Vec<InfillLine> {
    let angle = alternate_infill_angle(layer_index);
    let mut lines = rectilinear::generate(infill_region, density, angle, line_width);

    // Ensure all lines go in the same direction (monotonic).
    for line in &mut lines {
        if angle < 45.0 {
            // Horizontal: ensure left-to-right (start.x <= end.x).
            if line.start.x > line.end.x {
                std::mem::swap(&mut line.start, &mut line.end);
            }
        } else {
            // Vertical: ensure bottom-to-top (start.y <= end.y).
            if line.start.y > line.end.y {
                std::mem::swap(&mut line.start, &mut line.end);
            }
        }
    }

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
    fn monotonic_horizontal_all_left_to_right() {
        let square = make_square(20.0);
        // Layer 0 => angle = 0 (horizontal).
        let lines = generate(&[square], 0.2, 0, 0.4);

        assert!(!lines.is_empty(), "Should produce infill lines");

        for (i, line) in lines.iter().enumerate() {
            assert!(
                line.start.x <= line.end.x,
                "Line {} should go left-to-right: start.x={} end.x={}",
                i,
                line.start.x,
                line.end.x
            );
        }
    }

    #[test]
    fn monotonic_vertical_all_bottom_to_top() {
        let square = make_square(20.0);
        // Layer 1 => angle = 90 (vertical).
        let lines = generate(&[square], 0.2, 1, 0.4);

        assert!(!lines.is_empty(), "Should produce infill lines");

        for (i, line) in lines.iter().enumerate() {
            assert!(
                line.start.y <= line.end.y,
                "Line {} should go bottom-to-top: start.y={} end.y={}",
                i,
                line.start.y,
                line.end.y
            );
        }
    }

    #[test]
    fn monotonic_same_line_count_as_rectilinear() {
        let square = make_square(20.0);
        let mono_lines = generate(&[square.clone()], 0.2, 0, 0.4);
        let rect_lines = rectilinear::generate(&[square], 0.2, 0.0, 0.4);

        assert_eq!(
            mono_lines.len(),
            rect_lines.len(),
            "Monotonic ({}) should produce same number of lines as rectilinear ({})",
            mono_lines.len(),
            rect_lines.len()
        );
    }

    #[test]
    fn monotonic_empty_region_returns_empty() {
        let lines = generate(&[], 0.2, 0, 0.4);
        assert!(lines.is_empty(), "Empty region should return empty lines");
    }

    #[test]
    fn monotonic_zero_density_returns_empty() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.0, 0, 0.4);
        assert!(lines.is_empty(), "Zero density should return empty lines");
    }
}
