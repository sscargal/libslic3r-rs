//! Cubic infill pattern generation.
//!
//! Cubic infill creates a 3D interlocking cube structure by cycling through
//! three angles (0, 60, and 120 degrees) across layers with Z-dependent
//! phase offsets. This provides good omnidirectional strength and is a
//! popular production infill choice.
//!
//! The rotation approach transforms polygon coordinates to generate horizontal
//! scanlines, then rotates the results back, avoiding diagonal clipping from scratch.

use slicecore_geo::polygon::ValidPolygon;
use slicecore_math::{Coord, IPoint2};

use super::rectilinear::find_horizontal_intersections;
use super::{compute_bounding_box, compute_spacing, InfillLine};

/// The three angles (in degrees) that cubic infill cycles through.
const CUBIC_ANGLES: [f64; 3] = [0.0, 60.0, 120.0];

/// Frequency factor for Z-dependent phase offset.
/// Controls the vertical period of the 3D cube structure.
const Z_FREQUENCY: f64 = 1.0;

/// Generates cubic infill lines clipped to an infill region.
///
/// Cubic infill cycles through three angles (0, 60, 120 degrees) across
/// layers. Each layer's lines are shifted by a Z-dependent phase offset
/// to create interlocking 3D cube structures when viewed in cross-section.
///
/// # Parameters
/// - `infill_region`: The boundary polygons defining the infill area.
/// - `density`: Fill density as a fraction (0.0 = empty, 1.0 = solid).
/// - `layer_index`: Current layer index (selects angle: 0, 60, or 120 degrees).
/// - `layer_z`: Z height of the current layer in mm (used for phase offset).
/// - `line_width`: Extrusion line width in mm.
///
/// # Returns
/// A vector of [`InfillLine`] segments for the cubic pattern at this layer.
/// Returns empty if density <= 0.0 or infill_region is empty.
pub fn generate(
    infill_region: &[ValidPolygon],
    density: f64,
    layer_index: usize,
    layer_z: f64,
    line_width: f64,
) -> Vec<InfillLine> {
    if density <= 0.0 || infill_region.is_empty() || line_width <= 0.0 {
        return Vec::new();
    }

    let density = density.min(1.0);

    let spacing = match compute_spacing(density, line_width) {
        Some(s) => s,
        None => return Vec::new(),
    };

    // Select angle based on layer index cycling through 3 angles.
    let angle_deg = CUBIC_ANGLES[layer_index % 3];
    let angle_rad = angle_deg.to_radians();

    // Compute Z-dependent phase offset (in coordinate units).
    let spacing_mm = line_width / density;
    let offset_mm = (layer_z * Z_FREQUENCY) % spacing_mm;
    let offset = (offset_mm * slicecore_math::COORD_SCALE).round() as Coord;

    if angle_deg.abs() < 1.0 {
        // Angle 0: horizontal lines with Z-offset shift.
        generate_horizontal_with_offset(infill_region, spacing, offset)
    } else {
        // Angle 60 or 120: rotate polygon points by -angle, generate horizontal
        // lines with offset, then rotate line endpoints back by +angle.
        generate_rotated_lines(infill_region, spacing, angle_rad, offset)
    }
}

/// Generates horizontal lines with a Z-dependent phase offset.
fn generate_horizontal_with_offset(
    infill_region: &[ValidPolygon],
    spacing: Coord,
    offset: Coord,
) -> Vec<InfillLine> {
    let (min_x, min_y, _max_x, max_y) = compute_bounding_box(infill_region);

    let mut lines = Vec::new();

    // Start position includes offset for Z-dependent shift.
    let mut y = min_y + spacing / 2 + offset % spacing;

    // Ensure we start before or at the region boundary.
    while y > min_y + spacing {
        y -= spacing;
    }

    while y <= max_y {
        let mut intersections = find_horizontal_intersections(infill_region, y);
        intersections.sort_unstable();

        let mut i = 0;
        while i + 1 < intersections.len() {
            let x_enter = intersections[i];
            let x_exit = intersections[i + 1];

            if x_enter < x_exit {
                let x_start = x_enter.max(min_x);
                let x_end = x_exit;

                if x_start < x_end {
                    lines.push(InfillLine {
                        start: IPoint2::new(x_start, y),
                        end: IPoint2::new(x_end, y),
                    });
                }
            }
            i += 2;
        }

        y += spacing;
    }

    lines
}

/// Generates lines at an arbitrary angle using rotation.
///
/// 1. Rotate all polygon points by -angle (into horizontal frame).
/// 2. Generate horizontal lines with offset in the rotated frame.
/// 3. Rotate resulting line endpoints back by +angle.
fn generate_rotated_lines(
    infill_region: &[ValidPolygon],
    spacing: Coord,
    angle_rad: f64,
    offset: Coord,
) -> Vec<InfillLine> {
    let (min_x, min_y, max_x, max_y) = compute_bounding_box(infill_region);
    let center = IPoint2::new((min_x + max_x) / 2, (min_y + max_y) / 2);

    // Rotate all polygon points by -angle to align lines horizontally.
    let rotated_polygons = rotate_polygons(infill_region, -angle_rad, center);

    // Compute bounding box of rotated polygons.
    let (r_min_x, r_min_y, _r_max_x, r_max_y) = compute_bounding_box_raw(&rotated_polygons);

    let mut lines = Vec::new();

    // Generate horizontal scanlines in rotated frame.
    let mut y = r_min_y + spacing / 2 + offset % spacing;
    while y > r_min_y + spacing {
        y -= spacing;
    }

    while y <= r_max_y {
        let mut intersections = find_horizontal_intersections_raw(&rotated_polygons, y);
        intersections.sort_unstable();

        let mut i = 0;
        while i + 1 < intersections.len() {
            let x_enter = intersections[i];
            let x_exit = intersections[i + 1];

            if x_enter < x_exit {
                let x_start = x_enter.max(r_min_x);
                let x_end = x_exit;

                if x_start < x_end {
                    // Rotate endpoints back by +angle.
                    let start = rotate_point(IPoint2::new(x_start, y), angle_rad, center);
                    let end = rotate_point(IPoint2::new(x_end, y), angle_rad, center);

                    lines.push(InfillLine { start, end });
                }
            }
            i += 2;
        }

        y += spacing;
    }

    lines
}

/// Rotates a point around a center by the given angle (radians).
fn rotate_point(pt: IPoint2, angle_rad: f64, center: IPoint2) -> IPoint2 {
    let dx = pt.x as f64 - center.x as f64;
    let dy = pt.y as f64 - center.y as f64;
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();
    IPoint2::new(
        center.x + (dx * cos_a - dy * sin_a).round() as Coord,
        center.y + (dx * sin_a + dy * cos_a).round() as Coord,
    )
}

/// Rotates all polygon points by the given angle around a center.
///
/// Returns raw point vectors (not ValidPolygon, since rotation may change
/// winding order and we only need the geometry for scanline intersection).
fn rotate_polygons(
    polygons: &[ValidPolygon],
    angle_rad: f64,
    center: IPoint2,
) -> Vec<Vec<IPoint2>> {
    polygons
        .iter()
        .map(|poly| {
            poly.points()
                .iter()
                .map(|pt| rotate_point(*pt, angle_rad, center))
                .collect()
        })
        .collect()
}

/// Computes bounding box from raw point vectors.
fn compute_bounding_box_raw(polygons: &[Vec<IPoint2>]) -> (Coord, Coord, Coord, Coord) {
    let mut min_x = Coord::MAX;
    let mut min_y = Coord::MAX;
    let mut max_x = Coord::MIN;
    let mut max_y = Coord::MIN;

    for pts in polygons {
        for pt in pts {
            min_x = min_x.min(pt.x);
            min_y = min_y.min(pt.y);
            max_x = max_x.max(pt.x);
            max_y = max_y.max(pt.y);
        }
    }

    (min_x, min_y, max_x, max_y)
}

/// Finds horizontal intersections against raw point vectors.
///
/// Same algorithm as `rectilinear::find_horizontal_intersections` but
/// works on raw `Vec<IPoint2>` instead of `ValidPolygon`.
fn find_horizontal_intersections_raw(polygons: &[Vec<IPoint2>], y: Coord) -> Vec<Coord> {
    let mut intersections = Vec::new();

    for pts in polygons {
        let n = pts.len();
        for i in 0..n {
            let p1 = pts[i];
            let p2 = pts[(i + 1) % n];

            let y_min = p1.y.min(p2.y);
            let y_max = p1.y.max(p2.y);

            if y < y_min || y > y_max {
                continue;
            }

            if p1.y == p2.y {
                continue;
            }

            let dx = p2.x as i128 - p1.x as i128;
            let dy = p2.y as i128 - p1.y as i128;
            let t_num = y as i128 - p1.y as i128;

            let x = p1.x as i128 + (t_num * dx) / dy;
            intersections.push(x as Coord);
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
    use slicecore_geo::polygon::Polygon;
    use slicecore_math::mm_to_coord;

    /// Helper to create a validated CCW square at the origin with given size (mm).
    fn make_square(size: f64) -> ValidPolygon {
        Polygon::from_mm(&[(0.0, 0.0), (size, 0.0), (size, size), (0.0, size)])
            .validate()
            .unwrap()
    }

    #[test]
    fn cubic_20mm_square_produces_lines() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.2, 0, 0.0, 0.4);
        assert!(
            !lines.is_empty(),
            "20mm square at 20% density should produce cubic infill lines"
        );
    }

    #[test]
    fn cubic_angle_cycling_same_angle_every_3_layers() {
        let square = make_square(20.0);
        let lines_0 = generate(&[square.clone()], 0.2, 0, 0.0, 0.4);
        let lines_3 = generate(&[square], 0.2, 3, 0.0, 0.4);

        // Layer 0 and layer 3 should use the same angle (0 degrees).
        // With same Z (0.0), they should produce identical output.
        assert_eq!(
            lines_0.len(),
            lines_3.len(),
            "Layer 0 ({}) and layer 3 ({}) should produce same number of lines (same angle)",
            lines_0.len(),
            lines_3.len()
        );

        // Verify the positions match.
        for (a, b) in lines_0.iter().zip(lines_3.iter()) {
            assert_eq!(a.start, b.start, "Same-angle layers should have matching line starts");
            assert_eq!(a.end, b.end, "Same-angle layers should have matching line ends");
        }
    }

    #[test]
    fn cubic_different_angles_across_layers() {
        let square = make_square(20.0);
        let lines_0 = generate(&[square.clone()], 0.2, 0, 0.0, 0.4);
        let lines_1 = generate(&[square.clone()], 0.2, 1, 0.0, 0.4);
        let lines_2 = generate(&[square], 0.2, 2, 0.0, 0.4);

        // Layer 0 (0 deg), layer 1 (60 deg), layer 2 (120 deg) should differ.
        let pos_0: Vec<_> = lines_0.iter().map(|l| (l.start, l.end)).collect();
        let pos_1: Vec<_> = lines_1.iter().map(|l| (l.start, l.end)).collect();
        let pos_2: Vec<_> = lines_2.iter().map(|l| (l.start, l.end)).collect();

        assert_ne!(
            pos_0, pos_1,
            "Layer 0 (0 deg) and layer 1 (60 deg) should produce different line positions"
        );
        assert_ne!(
            pos_1, pos_2,
            "Layer 1 (60 deg) and layer 2 (120 deg) should produce different line positions"
        );
        assert_ne!(
            pos_0, pos_2,
            "Layer 0 (0 deg) and layer 2 (120 deg) should produce different line positions"
        );
    }

    #[test]
    fn cubic_lines_within_bounding_box() {
        let square = make_square(20.0);
        // Test all three angle layers.
        for layer in 0..3 {
            let lines = generate(&[square.clone()], 0.3, layer, 0.5, 0.4);

            let min = mm_to_coord(0.0);
            let max = mm_to_coord(20.0);
            // Allow generous tolerance for rotated lines.
            let tolerance = mm_to_coord(0.5);

            for (i, line) in lines.iter().enumerate() {
                assert!(
                    line.start.x >= min - tolerance && line.start.x <= max + tolerance,
                    "Layer {} line {} start x ({}) outside bounds [{}, {}]",
                    layer,
                    i,
                    line.start.x,
                    min - tolerance,
                    max + tolerance
                );
                assert!(
                    line.end.x >= min - tolerance && line.end.x <= max + tolerance,
                    "Layer {} line {} end x ({}) outside bounds [{}, {}]",
                    layer,
                    i,
                    line.end.x,
                    min - tolerance,
                    max + tolerance
                );
                assert!(
                    line.start.y >= min - tolerance && line.start.y <= max + tolerance,
                    "Layer {} line {} start y ({}) outside bounds [{}, {}]",
                    layer,
                    i,
                    line.start.y,
                    min - tolerance,
                    max + tolerance
                );
                assert!(
                    line.end.y >= min - tolerance && line.end.y <= max + tolerance,
                    "Layer {} line {} end y ({}) outside bounds [{}, {}]",
                    layer,
                    i,
                    line.end.y,
                    min - tolerance,
                    max + tolerance
                );
            }
        }
    }

    #[test]
    fn cubic_z_offset_shifts_lines() {
        let square = make_square(20.0);
        // Same layer index but different Z heights should produce shifted line positions.
        let lines_z0 = generate(&[square.clone()], 0.2, 0, 0.0, 0.4);
        let lines_z1 = generate(&[square], 0.2, 0, 1.0, 0.4);

        let pos_z0: Vec<_> = lines_z0.iter().map(|l| (l.start, l.end)).collect();
        let pos_z1: Vec<_> = lines_z1.iter().map(|l| (l.start, l.end)).collect();

        assert_ne!(
            pos_z0, pos_z1,
            "Same layer index at different Z heights should produce shifted line positions"
        );
    }

    #[test]
    fn cubic_empty_region_returns_empty() {
        let lines = generate(&[], 0.2, 0, 0.0, 0.4);
        assert!(
            lines.is_empty(),
            "Empty region should return empty cubic lines"
        );
    }

    #[test]
    fn cubic_zero_density_returns_empty() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.0, 0, 0.0, 0.4);
        assert!(
            lines.is_empty(),
            "Zero density should return empty cubic lines"
        );
    }

    #[test]
    fn cubic_60_degree_has_diagonal_lines() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.2, 1, 0.0, 0.4);

        // Layer 1 uses 60 degrees: lines should be diagonal (neither horizontal nor vertical).
        let has_diagonal = lines.iter().any(|l| l.start.x != l.end.x && l.start.y != l.end.y);
        assert!(
            has_diagonal,
            "60-degree cubic layer should produce diagonal lines"
        );
    }
}
