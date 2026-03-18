//! TPMS-D (Schwarz Diamond) infill pattern generation using implicit surface
//! evaluation and marching squares contour extraction.
//!
//! The Schwarz Diamond is a triply periodic minimal surface (TPMS) defined by:
//! ```text
//! f(x, y, z) = sin(x)*sin(y)*sin(z) + sin(x)*cos(y)*cos(z)
//!            + cos(x)*sin(y)*cos(z) + cos(x)*cos(y)*sin(z)
//! ```
//!
//! At a fixed Z height, this becomes a 2D implicit function whose zero
//! iso-contour gives smooth, curved infill lines. Marching squares is used
//! to extract these contours from a sampled grid.
//!
//! The Schwarz Diamond pattern provides a different stress distribution from
//! the Gyroid, with tetrahedral symmetry that makes it useful for functional
//! parts requiring specific mechanical properties.

use slicecore_geo::polygon::ValidPolygon;
use slicecore_geo::{point_in_polygon, PointLocation};
use slicecore_math::{coord_to_mm, IPoint2};

use super::{compute_bounding_box, InfillLine};

/// Evaluates the Schwarz Diamond TPMS implicit function at a point in 3D space.
///
/// Returns `sin(x)*sin(y)*sin(z) + sin(x)*cos(y)*cos(z)
///        + cos(x)*sin(y)*cos(z) + cos(x)*cos(y)*sin(z)`.
/// The zero iso-surface of this function is the Schwarz Diamond minimal surface.
#[inline]
fn schwarz_diamond(x: f64, y: f64, z: f64) -> f64 {
    x.sin() * y.sin() * z.sin()
        + x.sin() * y.cos() * z.cos()
        + x.cos() * y.sin() * z.cos()
        + x.cos() * y.cos() * z.sin()
}

/// A line segment in floating-point mm coordinates (used internally
/// before converting to integer coordinates).
#[derive(Clone, Debug)]
struct Segment {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

/// Linearly interpolates between two values to find where the function
/// crosses zero. Returns the interpolation parameter t in [0, 1].
#[inline]
fn lerp_t(v0: f64, v1: f64) -> f64 {
    let denom = v0 - v1;
    if denom.abs() < 1e-12 {
        0.5
    } else {
        v0 / denom
    }
}

/// Extracts iso-contour line segments from a 2D grid of scalar values using
/// marching squares.
///
/// The grid is `(nx+1) x (ny+1)` values sampled at positions
/// `(origin_x + i*step, origin_y + j*step)` for `i in 0..=nx, j in 0..=ny`.
///
/// Returns segments in the coordinate space defined by origin and step.
fn marching_squares(
    grid: &[f64],
    nx: usize,
    ny: usize,
    origin_x: f64,
    origin_y: f64,
    step: f64,
) -> Vec<Segment> {
    let cols = nx + 1;
    let mut segments = Vec::new();

    let val = |ix: usize, iy: usize| -> f64 { grid[iy * cols + ix] };
    let pos_x = |ix: usize| -> f64 { origin_x + ix as f64 * step };
    let pos_y = |iy: usize| -> f64 { origin_y + iy as f64 * step };

    for iy in 0..ny {
        for ix in 0..nx {
            let v_bl = val(ix, iy);
            let v_br = val(ix + 1, iy);
            let v_tr = val(ix + 1, iy + 1);
            let v_tl = val(ix, iy + 1);

            let case = ((v_bl > 0.0) as u8)
                | (((v_br > 0.0) as u8) << 1)
                | (((v_tr > 0.0) as u8) << 2)
                | (((v_tl > 0.0) as u8) << 3);

            if case == 0 || case == 15 {
                continue;
            }

            let t_bottom = lerp_t(v_bl, v_br);
            let bottom = (pos_x(ix) + t_bottom * step, pos_y(iy));

            let t_right = lerp_t(v_br, v_tr);
            let right = (pos_x(ix + 1), pos_y(iy) + t_right * step);

            let t_top = lerp_t(v_tl, v_tr);
            let top = (pos_x(ix) + t_top * step, pos_y(iy + 1));

            let t_left = lerp_t(v_bl, v_tl);
            let left = (pos_x(ix), pos_y(iy) + t_left * step);

            match case {
                1 | 14 => {
                    segments.push(Segment {
                        x0: bottom.0,
                        y0: bottom.1,
                        x1: left.0,
                        y1: left.1,
                    });
                }
                2 | 13 => {
                    segments.push(Segment {
                        x0: bottom.0,
                        y0: bottom.1,
                        x1: right.0,
                        y1: right.1,
                    });
                }
                3 | 12 => {
                    segments.push(Segment {
                        x0: left.0,
                        y0: left.1,
                        x1: right.0,
                        y1: right.1,
                    });
                }
                4 | 11 => {
                    segments.push(Segment {
                        x0: right.0,
                        y0: right.1,
                        x1: top.0,
                        y1: top.1,
                    });
                }
                6 | 9 => {
                    segments.push(Segment {
                        x0: bottom.0,
                        y0: bottom.1,
                        x1: top.0,
                        y1: top.1,
                    });
                }
                7 | 8 => {
                    segments.push(Segment {
                        x0: left.0,
                        y0: left.1,
                        x1: top.0,
                        y1: top.1,
                    });
                }
                5 => {
                    let center = (v_bl + v_br + v_tr + v_tl) / 4.0;
                    if center > 0.0 {
                        segments.push(Segment {
                            x0: bottom.0,
                            y0: bottom.1,
                            x1: right.0,
                            y1: right.1,
                        });
                        segments.push(Segment {
                            x0: left.0,
                            y0: left.1,
                            x1: top.0,
                            y1: top.1,
                        });
                    } else {
                        segments.push(Segment {
                            x0: bottom.0,
                            y0: bottom.1,
                            x1: left.0,
                            y1: left.1,
                        });
                        segments.push(Segment {
                            x0: right.0,
                            y0: right.1,
                            x1: top.0,
                            y1: top.1,
                        });
                    }
                }
                10 => {
                    let center = (v_bl + v_br + v_tr + v_tl) / 4.0;
                    if center > 0.0 {
                        segments.push(Segment {
                            x0: bottom.0,
                            y0: bottom.1,
                            x1: left.0,
                            y1: left.1,
                        });
                        segments.push(Segment {
                            x0: right.0,
                            y0: right.1,
                            x1: top.0,
                            y1: top.1,
                        });
                    } else {
                        segments.push(Segment {
                            x0: bottom.0,
                            y0: bottom.1,
                            x1: right.0,
                            y1: right.1,
                        });
                        segments.push(Segment {
                            x0: left.0,
                            y0: left.1,
                            x1: top.0,
                            y1: top.1,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    segments
}

/// Checks whether a point (in mm) is inside any polygon in the infill region.
fn point_inside_region(x_mm: f64, y_mm: f64, infill_region: &[ValidPolygon]) -> bool {
    let pt = IPoint2::from_mm(x_mm, y_mm);
    for poly in infill_region {
        let loc = point_in_polygon(&pt, poly.points());
        if loc == PointLocation::Inside || loc == PointLocation::OnBoundary {
            return true;
        }
    }
    false
}

/// Generates Schwarz Diamond (TPMS-D) infill lines for the given region using
/// implicit surface evaluation and marching squares contour extraction.
///
/// The Schwarz Diamond implicit surface is evaluated at the given layer Z height.
/// The resulting 2D iso-contour at f=0 gives smooth, curved infill lines with
/// tetrahedral symmetry.
///
/// # Parameters
/// - `infill_region`: The boundary polygons defining the infill area.
/// - `density`: Fill density as a fraction (0.0 = empty, 1.0 = solid).
/// - `_layer_index`: Current layer index (unused; Z height determines pattern).
/// - `layer_z`: Z height of the current layer in mm.
/// - `line_width`: Extrusion line width in mm.
///
/// # Returns
/// A vector of [`InfillLine`] segments representing the TPMS-D infill pattern.
/// Returns empty if density <= 0.0 or infill_region is empty.
pub fn generate(
    infill_region: &[ValidPolygon],
    density: f64,
    _layer_index: usize,
    layer_z: f64,
    line_width: f64,
) -> Vec<InfillLine> {
    if density <= 0.0 || infill_region.is_empty() || line_width <= 0.0 {
        return Vec::new();
    }

    let density = density.min(1.0);

    // Compute bounding box in integer coords, then convert to mm.
    let (min_x, min_y, max_x, max_y) = compute_bounding_box(infill_region);
    let bbox_min_x = coord_to_mm(min_x);
    let bbox_min_y = coord_to_mm(min_y);
    let bbox_max_x = coord_to_mm(max_x);
    let bbox_max_y = coord_to_mm(max_y);

    let bbox_w = bbox_max_x - bbox_min_x;
    let bbox_h = bbox_max_y - bbox_min_y;

    if bbox_w <= 0.0 || bbox_h <= 0.0 {
        return Vec::new();
    }

    // Compute frequency from density.
    // spacing = line_width / density (mm per period in output space)
    // freq = 2*PI / spacing (radians per mm)
    let spacing = line_width / density;
    let freq = std::f64::consts::TAU / spacing;

    // Grid step: use line_width for a good balance of detail vs. performance.
    let grid_step = line_width;

    // Number of grid cells in each direction.
    let nx = ((bbox_w / grid_step).ceil() as usize).max(1);
    let ny = ((bbox_h / grid_step).ceil() as usize).max(1);

    // Sample the Schwarz Diamond function on the grid.
    let cols = nx + 1;
    let rows = ny + 1;
    let z_scaled = layer_z * freq;

    let mut grid = vec![0.0_f64; cols * rows];
    for iy in 0..rows {
        let y_mm = bbox_min_y + iy as f64 * grid_step;
        let y_scaled = y_mm * freq;
        for ix in 0..cols {
            let x_mm = bbox_min_x + ix as f64 * grid_step;
            let x_scaled = x_mm * freq;
            grid[iy * cols + ix] = schwarz_diamond(x_scaled, y_scaled, z_scaled);
        }
    }

    // Extract iso-contours at threshold=0 using marching squares.
    let segments = marching_squares(&grid, nx, ny, bbox_min_x, bbox_min_y, grid_step);

    // Convert segments to InfillLine, filtering by point-in-polygon.
    // Keep segments where BOTH endpoints are inside the infill region.
    let mut lines = Vec::with_capacity(segments.len());
    for seg in &segments {
        // Skip degenerate (zero-length) segments.
        let dx = seg.x1 - seg.x0;
        let dy = seg.y1 - seg.y0;
        if dx * dx + dy * dy < 1e-12 {
            continue;
        }

        if point_inside_region(seg.x0, seg.y0, infill_region)
            && point_inside_region(seg.x1, seg.y1, infill_region)
        {
            lines.push(InfillLine {
                start: IPoint2::from_mm(seg.x0, seg.y0),
                end: IPoint2::from_mm(seg.x1, seg.y1),
            });
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
    use slicecore_math::mm_to_coord;

    /// Helper to create a validated CCW square at the origin with given size (mm).
    fn make_square(size: f64) -> ValidPolygon {
        Polygon::from_mm(&[(0.0, 0.0), (size, 0.0), (size, size), (0.0, size)])
            .validate()
            .unwrap()
    }

    #[test]
    fn schwarz_diamond_zero_crossing() {
        // The Schwarz Diamond function can be rewritten as:
        // f(x,y,z) = sin(x)*sin(y)*sin(z) + sin(x)*cos(y)*cos(z)
        //          + cos(x)*sin(y)*cos(z) + cos(x)*cos(y)*sin(z)
        // At (0, 0, 0): sin(0)*sin(0)*sin(0) + sin(0)*cos(0)*cos(0)
        //             + cos(0)*sin(0)*cos(0) + cos(0)*cos(0)*sin(0)
        //             = 0 + 0 + 0 + 0 = 0
        let val = schwarz_diamond(0.0, 0.0, 0.0);
        assert!(
            val.abs() < 1e-10,
            "Schwarz Diamond at (0,0,0) should be 0, got {}",
            val
        );
    }

    #[test]
    fn schwarz_diamond_nonzero_at_known_point() {
        // At (PI/2, PI/2, PI/2):
        // sin(PI/2)*sin(PI/2)*sin(PI/2) + sin(PI/2)*cos(PI/2)*cos(PI/2)
        // + cos(PI/2)*sin(PI/2)*cos(PI/2) + cos(PI/2)*cos(PI/2)*sin(PI/2)
        // = 1*1*1 + 1*0*0 + 0*1*0 + 0*0*1 = 1
        let val = schwarz_diamond(
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::FRAC_PI_2,
        );
        assert!(
            (val - 1.0).abs() < 1e-10,
            "Schwarz Diamond at (PI/2, PI/2, PI/2) should be 1, got {}",
            val
        );
    }

    #[test]
    fn tpms_d_20mm_square_produces_lines() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.2, 0, 0.3, 0.4);
        assert!(
            !lines.is_empty(),
            "20mm square at 20% density should produce TPMS-D infill lines"
        );
    }

    #[test]
    fn tpms_d_lines_within_bounding_box() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.3, 0, 0.3, 0.4);

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
    fn tpms_d_zero_density_returns_empty() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.0, 0, 0.3, 0.4);
        assert!(
            lines.is_empty(),
            "0% density should produce no TPMS-D lines"
        );
    }

    #[test]
    fn tpms_d_empty_region_returns_empty() {
        let lines = generate(&[], 0.2, 0, 0.3, 0.4);
        assert!(lines.is_empty(), "Empty region should return empty lines");
    }

    #[test]
    fn tpms_d_different_z_heights_produce_different_patterns() {
        let square = make_square(20.0);
        let lines_z1 = generate(&[square.clone()], 0.2, 0, 0.2, 0.4);
        let lines_z2 = generate(&[square.clone()], 0.2, 0, 0.4, 0.4);
        let lines_z3 = generate(&[square], 0.2, 0, 0.6, 0.4);

        assert!(!lines_z1.is_empty(), "Z=0.2 should produce lines");
        assert!(!lines_z2.is_empty(), "Z=0.4 should produce lines");
        assert!(!lines_z3.is_empty(), "Z=0.6 should produce lines");

        // At least two of the three should differ.
        let counts = [lines_z1.len(), lines_z2.len(), lines_z3.len()];
        let all_same = counts[0] == counts[1] && counts[1] == counts[2];
        if all_same {
            let starts_z1: Vec<_> = lines_z1.iter().map(|l| (l.start.x, l.start.y)).collect();
            let starts_z2: Vec<_> = lines_z2.iter().map(|l| (l.start.x, l.start.y)).collect();
            assert_ne!(
                starts_z1, starts_z2,
                "Different Z heights should produce different TPMS-D patterns"
            );
        }
    }

    #[test]
    fn tpms_d_deterministic() {
        let square = make_square(20.0);
        let lines1 = generate(&[square.clone()], 0.2, 0, 0.3, 0.4);
        let lines2 = generate(&[square], 0.2, 0, 0.3, 0.4);

        assert_eq!(
            lines1.len(),
            lines2.len(),
            "Same inputs should produce same number of lines"
        );
        for (a, b) in lines1.iter().zip(lines2.iter()) {
            assert_eq!(a.start, b.start, "Lines should be identical");
            assert_eq!(a.end, b.end, "Lines should be identical");
        }
    }
}
