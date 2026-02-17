//! Rectilinear infill pattern generation.
//!
//! Generates parallel scan lines clipped to an infill region boundary.
//! Supports horizontal (0-degree) and vertical (90-degree) scan lines.
//! Per-layer angle alternation creates a cross-hatching pattern for
//! structural strength.

use slicecore_geo::polygon::ValidPolygon;
use slicecore_math::Coord;

use super::{compute_bounding_box, compute_spacing, InfillLine};

/// Generates rectilinear infill lines clipped to an infill region.
///
/// # Parameters
/// - `infill_region`: The boundary polygons defining the infill area.
/// - `density`: Fill density as a fraction (0.0 = empty, 1.0 = solid).
/// - `angle_degrees`: Angle of the scan lines (0 = horizontal, 90 = vertical).
/// - `line_width`: Extrusion line width in mm.
///
/// # Returns
/// A vector of [`InfillLine`] segments, each representing one infill extrusion.
/// Returns empty if density <= 0.0 or infill_region is empty.
pub fn generate(
    infill_region: &[ValidPolygon],
    density: f64,
    angle_degrees: f64,
    line_width: f64,
) -> Vec<InfillLine> {
    if density <= 0.0 || infill_region.is_empty() || line_width <= 0.0 {
        return Vec::new();
    }

    // Clamp density to 1.0 max.
    let density = density.min(1.0);

    let spacing = match compute_spacing(density, line_width) {
        Some(s) => s,
        None => return Vec::new(),
    };

    // Compute bounding box of all infill region polygons.
    let (min_x, min_y, max_x, max_y) = compute_bounding_box(infill_region);

    // Generate scan lines based on angle.
    generate_at_angle(
        infill_region,
        spacing,
        angle_degrees,
        min_x,
        min_y,
        max_x,
        max_y,
    )
}

/// Generates infill lines at a given angle.
///
/// For Phase 4: supports 0 degrees (horizontal) and 90 degrees (vertical).
/// This helper is reused by Grid infill.
pub(crate) fn generate_at_angle(
    infill_region: &[ValidPolygon],
    spacing: Coord,
    angle_degrees: f64,
    min_x: Coord,
    min_y: Coord,
    max_x: Coord,
    max_y: Coord,
) -> Vec<InfillLine> {
    let is_vertical = (angle_degrees - 90.0).abs() < 1.0;

    if is_vertical {
        generate_vertical_lines(infill_region, spacing, min_x, min_y, max_x, max_y)
    } else {
        // Default: horizontal lines (angle 0 degrees).
        generate_horizontal_lines(infill_region, spacing, min_x, min_y, max_x, max_y)
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Generates horizontal scan lines and clips them against the infill region.
fn generate_horizontal_lines(
    infill_region: &[ValidPolygon],
    spacing: Coord,
    min_x: Coord,
    min_y: Coord,
    _max_x: Coord,
    max_y: Coord,
) -> Vec<InfillLine> {
    let mut lines = Vec::new();

    // Start from min_y + spacing/2 to center the pattern within the region.
    let mut y = min_y + spacing / 2;

    while y <= max_y {
        // Find all intersections of this horizontal line with polygon edges.
        let mut intersections = find_horizontal_intersections(infill_region, y);

        // Sort intersections by x coordinate.
        intersections.sort_unstable();

        // Pair up intersections (even-odd rule).
        let mut i = 0;
        while i + 1 < intersections.len() {
            let x_enter = intersections[i];
            let x_exit = intersections[i + 1];

            // Only produce lines with nonzero length.
            if x_enter < x_exit {
                // Trim to bounding box.
                let x_start = x_enter.max(min_x);
                let x_end = x_exit;

                if x_start < x_end {
                    lines.push(InfillLine {
                        start: slicecore_math::IPoint2::new(x_start, y),
                        end: slicecore_math::IPoint2::new(x_end, y),
                    });
                }
            }
            i += 2;
        }

        y += spacing;
    }

    lines
}

/// Generates vertical scan lines and clips them against the infill region.
fn generate_vertical_lines(
    infill_region: &[ValidPolygon],
    spacing: Coord,
    min_x: Coord,
    min_y: Coord,
    max_x: Coord,
    _max_y: Coord,
) -> Vec<InfillLine> {
    let mut lines = Vec::new();

    // Start from min_x + spacing/2 to center the pattern.
    let mut x = min_x + spacing / 2;

    while x <= max_x {
        // Find all intersections of this vertical line with polygon edges.
        let mut intersections = find_vertical_intersections(infill_region, x);

        // Sort intersections by y coordinate.
        intersections.sort_unstable();

        // Pair up intersections (even-odd rule).
        let mut i = 0;
        while i + 1 < intersections.len() {
            let y_enter = intersections[i];
            let y_exit = intersections[i + 1];

            if y_enter < y_exit {
                let y_start = y_enter.max(min_y);
                let y_end = y_exit;

                if y_start < y_end {
                    lines.push(InfillLine {
                        start: slicecore_math::IPoint2::new(x, y_start),
                        end: slicecore_math::IPoint2::new(x, y_end),
                    });
                }
            }
            i += 2;
        }

        x += spacing;
    }

    lines
}

/// Finds X-coordinates where a horizontal line at `y` intersects polygon edges.
///
/// For each edge (p1, p2) where min(p1.y, p2.y) <= y <= max(p1.y, p2.y),
/// compute the x intersection via linear interpolation.
pub(crate) fn find_horizontal_intersections(
    polygons: &[ValidPolygon],
    y: Coord,
) -> Vec<Coord> {
    let mut intersections = Vec::new();

    for poly in polygons {
        let pts = poly.points();
        let n = pts.len();

        for i in 0..n {
            let p1 = pts[i];
            let p2 = pts[(i + 1) % n];

            let y_min = p1.y.min(p2.y);
            let y_max = p1.y.max(p2.y);

            // Skip edges that don't cross this Y.
            if y < y_min || y > y_max {
                continue;
            }

            // Skip horizontal edges (they don't produce a crossing).
            if p1.y == p2.y {
                continue;
            }

            // Compute x intersection via linear interpolation.
            // x = p1.x + (y - p1.y) * (p2.x - p1.x) / (p2.y - p1.y)
            let dx = p2.x as i128 - p1.x as i128;
            let dy = p2.y as i128 - p1.y as i128;
            let t_num = y as i128 - p1.y as i128;

            let x = p1.x as i128 + (t_num * dx) / dy;
            intersections.push(x as Coord);
        }
    }

    intersections
}

/// Finds Y-coordinates where a vertical line at `x` intersects polygon edges.
pub(crate) fn find_vertical_intersections(
    polygons: &[ValidPolygon],
    x: Coord,
) -> Vec<Coord> {
    let mut intersections = Vec::new();

    for poly in polygons {
        let pts = poly.points();
        let n = pts.len();

        for i in 0..n {
            let p1 = pts[i];
            let p2 = pts[(i + 1) % n];

            let x_min = p1.x.min(p2.x);
            let x_max = p1.x.max(p2.x);

            // Skip edges that don't cross this X.
            if x < x_min || x > x_max {
                continue;
            }

            // Skip vertical edges (they don't produce a crossing).
            if p1.x == p2.x {
                continue;
            }

            // Compute y intersection via linear interpolation.
            // y = p1.y + (x - p1.x) * (p2.y - p1.y) / (p2.x - p1.x)
            let dy = p2.y as i128 - p1.y as i128;
            let dx = p2.x as i128 - p1.x as i128;
            let t_num = x as i128 - p1.x as i128;

            let y = p1.y as i128 + (t_num * dy) / dx;
            intersections.push(y as Coord);
        }
    }

    intersections
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infill::{alternate_infill_angle, generate_rectilinear_infill};
    use slicecore_geo::polygon::Polygon;
    use slicecore_math::mm_to_coord;

    /// Helper to create a validated CCW square at the origin with given size (mm).
    fn make_square(size: f64) -> ValidPolygon {
        Polygon::from_mm(&[
            (0.0, 0.0),
            (size, 0.0),
            (size, size),
            (0.0, size),
        ])
        .validate()
        .unwrap()
    }

    #[test]
    fn infill_20mm_square_20_percent() {
        let square = make_square(20.0);
        let lines = generate_rectilinear_infill(&[square], 0.2, 0.0, 0.4);
        assert!(
            !lines.is_empty(),
            "20mm square at 20% density should produce infill lines"
        );
    }

    #[test]
    fn infill_zero_density_is_empty() {
        let square = make_square(20.0);
        let lines = generate_rectilinear_infill(&[square], 0.0, 0.0, 0.4);
        assert!(lines.is_empty(), "0% density should produce no infill lines");
    }

    #[test]
    fn infill_negative_density_is_empty() {
        let square = make_square(20.0);
        let lines = generate_rectilinear_infill(&[square], -0.1, 0.0, 0.4);
        assert!(
            lines.is_empty(),
            "Negative density should produce no infill lines"
        );
    }

    #[test]
    fn infill_100_percent_produces_many_lines() {
        let square = make_square(20.0);
        let lines_solid = generate_rectilinear_infill(&[square.clone()], 1.0, 0.0, 0.4);
        let lines_sparse = generate_rectilinear_infill(&[square], 0.2, 0.0, 0.4);

        assert!(
            !lines_solid.is_empty(),
            "100% density should produce infill lines"
        );
        assert!(
            lines_solid.len() > lines_sparse.len(),
            "100% density ({}) should produce more lines than 20% ({})",
            lines_solid.len(),
            lines_sparse.len()
        );
    }

    #[test]
    fn infill_lines_within_bounding_box() {
        let square = make_square(20.0);
        let lines = generate_rectilinear_infill(&[square], 0.3, 0.0, 0.4);

        let min = mm_to_coord(0.0);
        let max = mm_to_coord(20.0);

        for line in &lines {
            assert!(
                line.start.x >= min && line.start.x <= max,
                "Line start x ({}) outside bounds [{}, {}]",
                line.start.x,
                min,
                max
            );
            assert!(
                line.end.x >= min && line.end.x <= max,
                "Line end x ({}) outside bounds [{}, {}]",
                line.end.x,
                min,
                max
            );
            assert!(
                line.start.y >= min && line.start.y <= max,
                "Line start y ({}) outside bounds [{}, {}]",
                line.start.y,
                min,
                max
            );
            assert!(
                line.end.y >= min && line.end.y <= max,
                "Line end y ({}) outside bounds [{}, {}]",
                line.end.y,
                min,
                max
            );
        }
    }

    #[test]
    fn infill_horizontal_lines_have_same_y() {
        let square = make_square(20.0);
        let lines = generate_rectilinear_infill(&[square], 0.2, 0.0, 0.4);

        for line in &lines {
            assert_eq!(
                line.start.y, line.end.y,
                "Horizontal infill lines should have same y: start={}, end={}",
                line.start.y, line.end.y
            );
        }
    }

    #[test]
    fn infill_vertical_lines_have_same_x() {
        let square = make_square(20.0);
        let lines = generate_rectilinear_infill(&[square], 0.2, 90.0, 0.4);

        for line in &lines {
            assert_eq!(
                line.start.x, line.end.x,
                "Vertical infill lines should have same x: start={}, end={}",
                line.start.x, line.end.x
            );
        }
    }

    #[test]
    fn infill_empty_region_returns_empty() {
        let lines = generate_rectilinear_infill(&[], 0.5, 0.0, 0.4);
        assert!(lines.is_empty(), "Empty region should return empty lines");
    }

    #[test]
    fn alternate_infill_angle_pattern() {
        assert!(
            (alternate_infill_angle(0) - 0.0).abs() < f64::EPSILON,
            "Layer 0 should be 0 degrees"
        );
        assert!(
            (alternate_infill_angle(1) - 90.0).abs() < f64::EPSILON,
            "Layer 1 should be 90 degrees"
        );
        assert!(
            (alternate_infill_angle(2) - 0.0).abs() < f64::EPSILON,
            "Layer 2 should be 0 degrees"
        );
        assert!(
            (alternate_infill_angle(3) - 90.0).abs() < f64::EPSILON,
            "Layer 3 should be 90 degrees"
        );
    }

    #[test]
    fn infill_density_above_1_clamped() {
        let square = make_square(20.0);
        let lines_100 = generate_rectilinear_infill(&[square.clone()], 1.0, 0.0, 0.4);
        let lines_200 = generate_rectilinear_infill(&[square], 2.0, 0.0, 0.4);

        // Density > 1.0 should be clamped to 1.0, producing same result.
        assert_eq!(
            lines_100.len(),
            lines_200.len(),
            "Density > 1.0 should be clamped to 1.0"
        );
    }
}
