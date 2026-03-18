//! Honeycomb (hexagonal grid) infill pattern generation.
//!
//! Generates a honeycomb pattern by producing zigzag polylines that form
//! hexagonal cell boundaries when stacked. The pattern alternates between
//! two row phases on even/odd layers to create proper interlocking.
//!
//! Honeycomb provides excellent strength-to-weight ratio with its hexagonal
//! structure and is a popular production infill choice.

use slicecore_geo::polygon::ValidPolygon;
use slicecore_math::{Coord, IPoint2};

use super::rectilinear::find_horizontal_intersections;
use super::{compute_bounding_box, compute_spacing, InfillLine};

/// Generates honeycomb infill lines clipped to an infill region.
///
/// The honeycomb is built from zigzag polylines at +/-60 degrees that form
/// hexagonal cell boundaries. Even and odd layers shift the phase by half
/// a period to create proper interlocking between layers.
///
/// # Parameters
/// - `infill_region`: The boundary polygons defining the infill area.
/// - `density`: Fill density as a fraction (0.0 = empty, 1.0 = solid).
/// - `layer_index`: Current layer index (used for even/odd phase shift).
/// - `line_width`: Extrusion line width in mm.
///
/// # Returns
/// A vector of [`InfillLine`] segments forming the honeycomb pattern.
/// Returns empty if density <= 0.0 or infill_region is empty.
pub fn generate(
    infill_region: &[ValidPolygon],
    density: f64,
    layer_index: usize,
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

    let (min_x, min_y, max_x, max_y) = compute_bounding_box(infill_region);

    // Hex cell geometry (in coordinate units):
    // hex_height = spacing * sqrt(3) -- vertical distance of one hex cell
    // hex_half_width = spacing -- half the horizontal period
    // Row spacing = hex_height / 2 (zigzag rows at half-cell vertical intervals)
    let sqrt3 = 3.0_f64.sqrt();
    let hex_height_f = spacing as f64 * sqrt3;
    let hex_height = hex_height_f as Coord;
    let hex_half_width = spacing;
    let row_spacing = hex_height / 2;

    if row_spacing <= 0 || hex_half_width <= 0 {
        return Vec::new();
    }

    // Phase shift: odd layers shift by half a period horizontally.
    let phase_shift = if layer_index % 2 == 1 {
        hex_half_width
    } else {
        0
    };

    let mut lines = Vec::new();

    // Generate zigzag rows across the bounding box.
    // Each row is a horizontal zigzag: segments alternate between two y levels
    // separated by row_spacing, with x advancing by hex_half_width.
    let mut row_idx = 0i64;
    let mut y_base = min_y - row_spacing; // Start before region to catch edges

    while y_base <= max_y + row_spacing {
        let is_even_row = row_idx % 2 == 0;

        // For this zigzag row, generate segments.
        // Even rows: go from (x, y_base) to (x + half, y_base + row_spacing) to (x + full, y_base)
        // Odd rows: go from (x + half, y_base) to (x + full, y_base + row_spacing) to (x + 3half, y_base)
        let period = hex_half_width * 2; // Full horizontal period
        if period <= 0 {
            break;
        }

        let x_start = min_x - period + phase_shift;
        let mut x = x_start;

        while x <= max_x + period {
            let (seg_start, seg_end) = if is_even_row {
                // Rising segment: (x, y_base) -> (x + half, y_base + row_spacing)
                (
                    IPoint2::new(x, y_base),
                    IPoint2::new(x + hex_half_width, y_base + row_spacing),
                )
            } else {
                // Falling segment: (x, y_base + row_spacing) -> (x + half, y_base)
                (
                    IPoint2::new(x, y_base + row_spacing),
                    IPoint2::new(x + hex_half_width, y_base),
                )
            };

            // Clip this segment against the infill region.
            clip_segment_to_region(&seg_start, &seg_end, infill_region, &mut lines);

            x += hex_half_width;
        }

        y_base += row_spacing;
        row_idx += 1;
    }

    // Also add horizontal connecting segments at each row boundary.
    // These form the flat tops/bottoms of the hexagons.
    let mut y = min_y - row_spacing;
    let period = hex_half_width * 2;
    if period > 0 {
        let mut h_row = 0i64;
        while y <= max_y + row_spacing {
            // Horizontal segments connect every other zigzag peak.
            // On even row boundaries, connect pairs starting at offset 0.
            // On odd row boundaries, connect pairs starting at offset hex_half_width.
            let x_offset = if h_row % 2 == 0 {
                phase_shift
            } else {
                hex_half_width + phase_shift
            };

            let mut x = min_x - period + x_offset;
            while x + hex_half_width * 2 <= max_x + period {
                // Horizontal segment from x to x + period
                let seg_start = IPoint2::new(x, y);
                let seg_end = IPoint2::new(x + period, y);

                // Clip horizontal segment using scanline approach.
                clip_horizontal_to_region(y, x, x + period, infill_region, &mut lines);

                let _ = (seg_start, seg_end); // suppress unused warning
                x += period * 2;
            }

            y += row_spacing;
            h_row += 1;
        }
    }

    lines
}

/// Clips a line segment against the infill region using parametric intersection.
///
/// For diagonal segments, we find t-parameters where the segment crosses polygon
/// edges, then emit the portions that are inside the region (using even-odd rule).
fn clip_segment_to_region(
    start: &IPoint2,
    end: &IPoint2,
    infill_region: &[ValidPolygon],
    lines: &mut Vec<InfillLine>,
) {
    // Use a sampling approach: subdivide the segment and test if midpoints
    // are inside the region. More robust than parametric clipping for
    // arbitrary polygon regions.
    //
    // For efficiency, we find intersections of the segment with all polygon
    // edges and use even-odd pairing.

    let dx = end.x as i128 - start.x as i128;
    let dy = end.y as i128 - start.y as i128;
    let seg_len_sq = dx * dx + dy * dy;

    if seg_len_sq == 0 {
        return;
    }

    // Collect t-parameters (as fraction of segment length) where the segment
    // intersects polygon edges.
    let mut t_values: Vec<f64> = Vec::new();

    for poly in infill_region {
        let pts = poly.points();
        let n = pts.len();

        for i in 0..n {
            let p1 = pts[i];
            let p2 = pts[(i + 1) % n];

            // Solve: start + t * (end - start) = p1 + u * (p2 - p1)
            // Using 2D cross product method.
            let ex = p2.x as i128 - p1.x as i128;
            let ey = p2.y as i128 - p1.y as i128;

            let denom = dx * ey - dy * ex;
            if denom == 0 {
                continue; // Parallel segments
            }

            let qx = p1.x as i128 - start.x as i128;
            let qy = p1.y as i128 - start.y as i128;

            let t_num = qx * ey - qy * ex;
            let u_num = qx * dy - qy * dx;

            let t = t_num as f64 / denom as f64;
            let u = u_num as f64 / denom as f64;

            // Both parameters must be in [0, 1] for a valid intersection.
            if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                t_values.push(t);
            }
        }
    }

    // Sort t-values and pair them (even-odd rule).
    t_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Deduplicate very close t-values (edge intersections at polygon vertices).
    t_values.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

    // Emit line segments for paired inside regions.
    let mut i = 0;
    while i + 1 < t_values.len() {
        let t_enter = t_values[i];
        let t_exit = t_values[i + 1];

        if t_exit > t_enter + 1e-12 {
            let sx = start.x as f64 + t_enter * dx as f64;
            let sy = start.y as f64 + t_enter * dy as f64;
            let ex = start.x as f64 + t_exit * dx as f64;
            let ey = start.y as f64 + t_exit * dy as f64;

            let s = IPoint2::new(sx.round() as Coord, sy.round() as Coord);
            let e = IPoint2::new(ex.round() as Coord, ey.round() as Coord);

            if s != e {
                lines.push(InfillLine { start: s, end: e });
            }
        }

        i += 2;
    }
}

/// Clips a horizontal line segment against the infill region.
///
/// Uses the existing `find_horizontal_intersections` for efficient scanline clipping.
fn clip_horizontal_to_region(
    y: Coord,
    x_start: Coord,
    x_end: Coord,
    infill_region: &[ValidPolygon],
    lines: &mut Vec<InfillLine>,
) {
    let mut intersections = find_horizontal_intersections(infill_region, y);
    intersections.sort_unstable();

    let mut i = 0;
    while i + 1 < intersections.len() {
        let x_enter = intersections[i];
        let x_exit = intersections[i + 1];

        // Intersect with our segment range [x_start, x_end].
        let clipped_start = x_enter.max(x_start);
        let clipped_end = x_exit.min(x_end);

        if clipped_start < clipped_end {
            lines.push(InfillLine {
                start: IPoint2::new(clipped_start, y),
                end: IPoint2::new(clipped_end, y),
            });
        }

        i += 2;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infill::rectilinear;
    use slicecore_geo::polygon::Polygon;
    use slicecore_math::mm_to_coord;

    /// Helper to create a validated CCW square at the origin with given size (mm).
    fn make_square(size: f64) -> ValidPolygon {
        Polygon::from_mm(&[(0.0, 0.0), (size, 0.0), (size, size), (0.0, size)])
            .validate()
            .unwrap()
    }

    #[test]
    fn honeycomb_20mm_square_20_percent() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.2, 0, 0.4);
        assert!(
            !lines.is_empty(),
            "20mm square at 20% density should produce honeycomb infill lines"
        );
    }

    #[test]
    fn honeycomb_has_diagonal_segments() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.2, 0, 0.4);

        // Honeycomb should include segments that are neither horizontal nor vertical.
        let has_diagonal = lines
            .iter()
            .any(|l| l.start.x != l.end.x && l.start.y != l.end.y);

        assert!(
            has_diagonal,
            "Honeycomb should have diagonal segments (zigzag lines), found {} lines all axis-aligned",
            lines.len()
        );
    }

    #[test]
    fn honeycomb_differs_from_rectilinear() {
        let square = make_square(20.0);
        let honeycomb_lines = generate(&[square.clone()], 0.2, 0, 0.4);
        let rect_lines = rectilinear::generate(&[square], 0.2, 0.0, 0.4);

        // Honeycomb should produce a different number of lines than rectilinear.
        assert_ne!(
            honeycomb_lines.len(),
            rect_lines.len(),
            "Honeycomb ({}) should differ from rectilinear ({}) at same density",
            honeycomb_lines.len(),
            rect_lines.len()
        );
    }

    #[test]
    fn honeycomb_lines_within_bounding_box() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.3, 0, 0.4);

        let min = mm_to_coord(0.0);
        let max = mm_to_coord(20.0);

        // Allow small tolerance for rounding at boundaries.
        let tolerance = mm_to_coord(0.01);

        for (i, line) in lines.iter().enumerate() {
            assert!(
                line.start.x >= min - tolerance && line.start.x <= max + tolerance,
                "Line {} start x ({}) outside bounds [{}, {}]",
                i,
                line.start.x,
                min,
                max
            );
            assert!(
                line.end.x >= min - tolerance && line.end.x <= max + tolerance,
                "Line {} end x ({}) outside bounds [{}, {}]",
                i,
                line.end.x,
                min,
                max
            );
            assert!(
                line.start.y >= min - tolerance && line.start.y <= max + tolerance,
                "Line {} start y ({}) outside bounds [{}, {}]",
                i,
                line.start.y,
                min,
                max
            );
            assert!(
                line.end.y >= min - tolerance && line.end.y <= max + tolerance,
                "Line {} end y ({}) outside bounds [{}, {}]",
                i,
                line.end.y,
                min,
                max
            );
        }
    }

    #[test]
    fn honeycomb_empty_region_returns_empty() {
        let lines = generate(&[], 0.2, 0, 0.4);
        assert!(
            lines.is_empty(),
            "Empty region should return empty honeycomb lines"
        );
    }

    #[test]
    fn honeycomb_zero_density_returns_empty() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.0, 0, 0.4);
        assert!(
            lines.is_empty(),
            "Zero density should return empty honeycomb lines"
        );
    }

    #[test]
    fn honeycomb_layer_shift() {
        let square = make_square(20.0);
        let lines_even = generate(&[square.clone()], 0.2, 0, 0.4);
        let lines_odd = generate(&[square], 0.2, 1, 0.4);

        // Even and odd layers should differ (phase shift).
        // They may have same count but different positions.
        let even_positions: Vec<_> = lines_even.iter().map(|l| (l.start, l.end)).collect();
        let odd_positions: Vec<_> = lines_odd.iter().map(|l| (l.start, l.end)).collect();

        assert_ne!(
            even_positions, odd_positions,
            "Even and odd layers should have different line positions due to phase shift"
        );
    }

    #[test]
    fn honeycomb_higher_density_more_lines() {
        let square = make_square(20.0);
        let lines_low = generate(&[square.clone()], 0.15, 0, 0.4);
        let lines_high = generate(&[square], 0.5, 0, 0.4);

        assert!(
            lines_high.len() > lines_low.len(),
            "Higher density ({}) should produce more lines than lower density ({})",
            lines_high.len(),
            lines_low.len()
        );
    }
}
