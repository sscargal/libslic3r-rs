//! Gyroid infill pattern generation using TPMS implicit surface evaluation
//! and marching squares contour extraction.
//!
//! The gyroid is a triply periodic minimal surface (TPMS) defined by:
//! ```text
//! f(x, y, z) = cos(x)*sin(y) + cos(y)*sin(z) + cos(z)*sin(x)
//! ```
//!
//! At a fixed Z height, this becomes a 2D implicit function whose zero
//! iso-contour gives smooth, curved infill lines. Marching squares is used
//! to extract these contours from a sampled grid.
//!
//! The gyroid pattern provides the highest strength-to-weight ratio of any
//! infill pattern due to its isotropic stress distribution.

use slicecore_geo::polygon::ValidPolygon;
use slicecore_geo::{point_in_polygon, PointLocation};
use slicecore_math::{coord_to_mm, IPoint2};

use super::{compute_bounding_box, InfillLine};

/// Evaluates the gyroid TPMS implicit function at a point in 3D space.
///
/// Returns `cos(x)*sin(y) + cos(y)*sin(z) + cos(z)*sin(x)`.
/// The zero iso-surface of this function is the gyroid minimal surface.
#[inline]
fn gyroid_value(x: f64, y: f64, z: f64) -> f64 {
    x.cos() * y.sin() + y.cos() * z.sin() + z.cos() * x.sin()
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
    // Avoid division by zero for identical values.
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

    // Helper: sample the grid value at (ix, iy).
    let val = |ix: usize, iy: usize| -> f64 { grid[iy * cols + ix] };

    // Helper: position of grid node (ix, iy) in mm coordinates.
    let pos_x = |ix: usize| -> f64 { origin_x + ix as f64 * step };
    let pos_y = |iy: usize| -> f64 { origin_y + iy as f64 * step };

    for iy in 0..ny {
        for ix in 0..nx {
            // Corner values (bottom-left, bottom-right, top-right, top-left).
            let v_bl = val(ix, iy);
            let v_br = val(ix + 1, iy);
            let v_tr = val(ix + 1, iy + 1);
            let v_tl = val(ix, iy + 1);

            // Build the 4-bit case index: bit set means value > 0.
            let case = ((v_bl > 0.0) as u8)
                | (((v_br > 0.0) as u8) << 1)
                | (((v_tr > 0.0) as u8) << 2)
                | (((v_tl > 0.0) as u8) << 3);

            // Skip cases with no contour.
            if case == 0 || case == 15 {
                continue;
            }

            // Edge midpoint coordinates via linear interpolation.
            // Bottom edge (bl -> br)
            let t_bottom = lerp_t(v_bl, v_br);
            let bottom = (pos_x(ix) + t_bottom * step, pos_y(iy));

            // Right edge (br -> tr)
            let t_right = lerp_t(v_br, v_tr);
            let right = (pos_x(ix + 1), pos_y(iy) + t_right * step);

            // Top edge (tl -> tr)
            let t_top = lerp_t(v_tl, v_tr);
            let top = (pos_x(ix) + t_top * step, pos_y(iy + 1));

            // Left edge (bl -> tl)
            let t_left = lerp_t(v_bl, v_tl);
            let left = (pos_x(ix), pos_y(iy) + t_left * step);

            match case {
                // Single segment cases
                1 | 14 => {
                    segments.push(Segment {
                        x0: bottom.0, y0: bottom.1,
                        x1: left.0, y1: left.1,
                    });
                }
                2 | 13 => {
                    segments.push(Segment {
                        x0: bottom.0, y0: bottom.1,
                        x1: right.0, y1: right.1,
                    });
                }
                3 | 12 => {
                    segments.push(Segment {
                        x0: left.0, y0: left.1,
                        x1: right.0, y1: right.1,
                    });
                }
                4 | 11 => {
                    segments.push(Segment {
                        x0: right.0, y0: right.1,
                        x1: top.0, y1: top.1,
                    });
                }
                6 | 9 => {
                    segments.push(Segment {
                        x0: bottom.0, y0: bottom.1,
                        x1: top.0, y1: top.1,
                    });
                }
                7 | 8 => {
                    segments.push(Segment {
                        x0: left.0, y0: left.1,
                        x1: top.0, y1: top.1,
                    });
                }
                // Ambiguous saddle cases -- resolve using center value.
                5 => {
                    let center = (v_bl + v_br + v_tr + v_tl) / 4.0;
                    if center > 0.0 {
                        // Connect bottom-right and top-left
                        segments.push(Segment {
                            x0: bottom.0, y0: bottom.1,
                            x1: right.0, y1: right.1,
                        });
                        segments.push(Segment {
                            x0: left.0, y0: left.1,
                            x1: top.0, y1: top.1,
                        });
                    } else {
                        // Connect bottom-left and top-right
                        segments.push(Segment {
                            x0: bottom.0, y0: bottom.1,
                            x1: left.0, y1: left.1,
                        });
                        segments.push(Segment {
                            x0: right.0, y0: right.1,
                            x1: top.0, y1: top.1,
                        });
                    }
                }
                10 => {
                    let center = (v_bl + v_br + v_tr + v_tl) / 4.0;
                    if center > 0.0 {
                        // Connect bottom-left and top-right
                        segments.push(Segment {
                            x0: bottom.0, y0: bottom.1,
                            x1: left.0, y1: left.1,
                        });
                        segments.push(Segment {
                            x0: right.0, y0: right.1,
                            x1: top.0, y1: top.1,
                        });
                    } else {
                        // Connect bottom-right and top-left
                        segments.push(Segment {
                            x0: bottom.0, y0: bottom.1,
                            x1: right.0, y1: right.1,
                        });
                        segments.push(Segment {
                            x0: left.0, y0: left.1,
                            x1: top.0, y1: top.1,
                        });
                    }
                }
                _ => {} // Cases 0 and 15 already handled above.
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

/// Generates gyroid infill lines for the given region using TPMS evaluation
/// and marching squares contour extraction.
///
/// The gyroid implicit surface `f(x,y,z) = cos(x)*sin(y) + cos(y)*sin(z) + cos(z)*sin(x)`
/// is evaluated at the given layer Z height. The resulting 2D iso-contour at f=0
/// gives smooth, curved infill lines that distribute stress evenly.
///
/// # Parameters
/// - `infill_region`: The boundary polygons defining the infill area.
/// - `density`: Fill density as a fraction (0.0 = empty, 1.0 = solid).
/// - `_layer_index`: Current layer index (unused; Z height determines pattern).
/// - `layer_z`: Z height of the current layer in mm.
/// - `line_width`: Extrusion line width in mm.
///
/// # Returns
/// A vector of [`InfillLine`] segments representing the gyroid infill pattern.
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

    // Compute gyroid frequency from density.
    // spacing = line_width / density (mm per period in output space)
    // freq = 2*PI / spacing (radians per mm)
    let spacing = line_width / density;
    let freq = std::f64::consts::TAU / spacing;

    // Grid step: use line_width for a good balance of detail vs. performance.
    // Finer grids capture more detail but are slower.
    let grid_step = line_width;

    // Number of grid cells in each direction.
    let nx = ((bbox_w / grid_step).ceil() as usize).max(1);
    let ny = ((bbox_h / grid_step).ceil() as usize).max(1);

    // Sample the gyroid function on the grid.
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
            grid[iy * cols + ix] = gyroid_value(x_scaled, y_scaled, z_scaled);
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
    fn gyroid_20mm_square_produces_lines() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.2, 0, 0.3, 0.4);
        assert!(
            !lines.is_empty(),
            "20mm square at 20% density should produce gyroid infill lines"
        );
    }

    #[test]
    fn gyroid_different_z_heights_produce_different_patterns() {
        let square = make_square(20.0);
        let lines_z1 = generate(&[square.clone()], 0.2, 0, 0.2, 0.4);
        let lines_z2 = generate(&[square.clone()], 0.2, 0, 0.4, 0.4);
        let lines_z3 = generate(&[square], 0.2, 0, 0.6, 0.4);

        assert!(!lines_z1.is_empty(), "Z=0.2 should produce lines");
        assert!(!lines_z2.is_empty(), "Z=0.4 should produce lines");
        assert!(!lines_z3.is_empty(), "Z=0.6 should produce lines");

        // At least two of the three should differ in line count
        // (exact coordinates will certainly differ since Z changes the
        // gyroid cross-section).
        let counts = [lines_z1.len(), lines_z2.len(), lines_z3.len()];
        let all_same = counts[0] == counts[1] && counts[1] == counts[2];
        // Even if counts happen to match, the actual positions differ.
        // But as a basic check, verify we get non-trivial output.
        if all_same {
            // Compare actual start positions to detect variation.
            let starts_z1: Vec<_> = lines_z1.iter().map(|l| (l.start.x, l.start.y)).collect();
            let starts_z2: Vec<_> = lines_z2.iter().map(|l| (l.start.x, l.start.y)).collect();
            assert_ne!(
                starts_z1, starts_z2,
                "Different Z heights should produce different gyroid patterns"
            );
        }
    }

    #[test]
    fn gyroid_lines_within_bounding_box() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.3, 0, 0.3, 0.4);

        let min = mm_to_coord(0.0);
        let max = mm_to_coord(20.0);

        for line in &lines {
            assert!(
                line.start.x >= min && line.start.x <= max,
                "Line start x ({}) outside bounds [{}, {}]",
                line.start.x, min, max
            );
            assert!(
                line.end.x >= min && line.end.x <= max,
                "Line end x ({}) outside bounds [{}, {}]",
                line.end.x, min, max
            );
            assert!(
                line.start.y >= min && line.start.y <= max,
                "Line start y ({}) outside bounds [{}, {}]",
                line.start.y, min, max
            );
            assert!(
                line.end.y >= min && line.end.y <= max,
                "Line end y ({}) outside bounds [{}, {}]",
                line.end.y, min, max
            );
        }
    }

    #[test]
    fn gyroid_zero_density_returns_empty() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.0, 0, 0.3, 0.4);
        assert!(lines.is_empty(), "0% density should produce no gyroid lines");
    }

    #[test]
    fn gyroid_empty_region_returns_empty() {
        let lines = generate(&[], 0.2, 0, 0.3, 0.4);
        assert!(lines.is_empty(), "Empty region should return empty lines");
    }

    #[test]
    fn gyroid_contains_diagonal_segments() {
        // Gyroid should produce curves (non-axis-aligned segments),
        // unlike rectilinear which only has horizontal/vertical lines.
        let square = make_square(20.0);
        let lines = generate(&[square], 0.2, 0, 0.3, 0.4);

        let has_diagonal = lines.iter().any(|l| {
            l.start.x != l.end.x && l.start.y != l.end.y
        });
        assert!(
            has_diagonal,
            "Gyroid infill should contain diagonal/curved segments"
        );
    }

    #[test]
    fn gyroid_deterministic() {
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

    #[test]
    fn marching_squares_known_grid() {
        // Create a simple 2x2 grid (3x3 nodes) with known values
        // that produces predictable segments.
        //
        // Grid layout (3x3):
        //   (-1)  (+1)  (-1)
        //   (+1)  (-1)  (+1)
        //   (-1)  (+1)  (-1)
        //
        // This checkerboard pattern should produce segments in all cells.
        let grid = vec![
            -1.0, 1.0, -1.0,  // row 0 (bottom)
            1.0, -1.0, 1.0,   // row 1 (middle)
            -1.0, 1.0, -1.0,  // row 2 (top)
        ];
        let segments = marching_squares(&grid, 2, 2, 0.0, 0.0, 1.0);
        assert!(
            !segments.is_empty(),
            "Checkerboard grid should produce marching squares segments"
        );
        // Each of the 4 cells has a saddle point (case 5 or 10),
        // producing 2 segments each = 8 total.
        assert_eq!(
            segments.len(), 8,
            "2x2 checkerboard should produce 8 segments (2 per saddle cell)"
        );
    }

    #[test]
    fn marching_squares_all_positive_no_segments() {
        // All positive values -> case 15 for all cells -> no segments.
        let grid = vec![1.0; 9]; // 3x3 grid, 2x2 cells
        let segments = marching_squares(&grid, 2, 2, 0.0, 0.0, 1.0);
        assert!(
            segments.is_empty(),
            "All-positive grid should produce no segments"
        );
    }

    #[test]
    fn marching_squares_all_negative_no_segments() {
        // All negative values -> case 0 for all cells -> no segments.
        let grid = vec![-1.0; 9];
        let segments = marching_squares(&grid, 2, 2, 0.0, 0.0, 1.0);
        assert!(
            segments.is_empty(),
            "All-negative grid should produce no segments"
        );
    }

    #[test]
    fn marching_squares_single_corner_positive() {
        // 1x1 cell with only bottom-left corner positive -> case 1.
        let grid = vec![
            1.0, -1.0, // bottom row
            -1.0, -1.0, // top row
        ];
        let segments = marching_squares(&grid, 1, 1, 0.0, 0.0, 1.0);
        assert_eq!(
            segments.len(), 1,
            "Single positive corner should produce 1 segment"
        );
    }

    // -----------------------------------------------------------------------
    // Integration tests
    // -----------------------------------------------------------------------

    #[test]
    fn integration_end_to_end_multiple_layers() {
        // Generate gyroid on a 20mm square at density=0.3 for 3 different Z heights.
        let square = make_square(20.0);
        let layers = [
            generate(&[square.clone()], 0.3, 0, 0.2, 0.4),
            generate(&[square.clone()], 0.3, 1, 0.4, 0.4),
            generate(&[square], 0.3, 2, 0.6, 0.4),
        ];

        let min = mm_to_coord(0.0);
        let max = mm_to_coord(20.0);

        for (i, lines) in layers.iter().enumerate() {
            assert!(
                !lines.is_empty(),
                "Layer {} (z={}) should produce non-empty infill",
                i,
                0.2 + i as f64 * 0.2
            );
            // Verify all endpoints are within bbox.
            for line in lines {
                assert!(
                    line.start.x >= min && line.start.x <= max
                        && line.start.y >= min && line.start.y <= max
                        && line.end.x >= min && line.end.x <= max
                        && line.end.y >= min && line.end.y <= max,
                    "Layer {} has line outside bounding box",
                    i
                );
            }
        }

        // Verify Z-variation: at least 2 of 3 layers differ in line count or positions.
        let counts: Vec<usize> = layers.iter().map(|l| l.len()).collect();
        let starts_0: Vec<_> = layers[0].iter().map(|l| (l.start.x, l.start.y)).collect();
        let starts_1: Vec<_> = layers[1].iter().map(|l| (l.start.x, l.start.y)).collect();
        let starts_2: Vec<_> = layers[2].iter().map(|l| (l.start.x, l.start.y)).collect();

        let differ_01 = counts[0] != counts[1] || starts_0 != starts_1;
        let differ_12 = counts[1] != counts[2] || starts_1 != starts_2;
        let differ_02 = counts[0] != counts[2] || starts_0 != starts_2;

        assert!(
            differ_01 || differ_12 || differ_02,
            "At least two layers should produce different patterns (Z variation)"
        );
    }

    #[test]
    fn integration_density_variation() {
        // 10% density should produce fewer/wider-spaced lines than 50% density.
        let square = make_square(20.0);
        let lines_10 = generate(&[square.clone()], 0.1, 0, 0.3, 0.4);
        let lines_50 = generate(&[square], 0.5, 0, 0.3, 0.4);

        assert!(
            !lines_10.is_empty(),
            "10% density should produce some lines"
        );
        assert!(
            !lines_50.is_empty(),
            "50% density should produce some lines"
        );
        assert!(
            lines_50.len() > lines_10.len(),
            "50% density ({}) should produce more lines than 10% ({})",
            lines_50.len(),
            lines_10.len()
        );
    }

    #[test]
    fn integration_stress_test_large_region() {
        // 100mm x 100mm region at 15% density -- should complete quickly
        // and produce many lines without panicking.
        let square = make_square(100.0);
        let start = std::time::Instant::now();
        let lines = generate(&[square], 0.15, 0, 0.3, 0.4);
        let elapsed = start.elapsed();

        assert!(
            !lines.is_empty(),
            "100mm region at 15% density should produce infill lines"
        );
        assert!(
            lines.len() > 100,
            "100mm region should produce many lines, got {}",
            lines.len()
        );
        assert!(
            elapsed.as_secs_f64() < 1.0,
            "100mm region should complete in < 1 second, took {:.3}s",
            elapsed.as_secs_f64()
        );
    }

    #[test]
    fn integration_marching_squares_all_16_cases() {
        // Craft a grid that exercises all 16 marching squares cases.
        // We build a 4x4 grid (5x5 nodes) with carefully chosen values.
        //
        // Case index is: BL | (BR << 1) | (TR << 2) | (TL << 3)
        // where each bit indicates value > 0.
        //
        // We need cells producing cases 0-15. We'll create them in a
        // 4x4 arrangement of cells.
        //
        // For each case, we set the four corners appropriately.
        // Rather than constructing a continuous grid (which constrains
        // neighboring cells), we test each case in isolation.

        let p = 1.0_f64;
        let n = -1.0_f64;

        // Test each case individually with a 1x1 cell (2x2 grid).
        let cases: [(u8, [f64; 4]); 16] = [
            (0, [n, n, n, n]),   // all negative
            (1, [p, n, n, n]),   // BL only
            (2, [n, p, n, n]),   // BR only
            (3, [p, p, n, n]),   // BL + BR
            (4, [n, n, p, n]),   // TR only
            (5, [p, n, p, n]),   // BL + TR (saddle)
            (6, [n, p, p, n]),   // BR + TR
            (7, [p, p, p, n]),   // BL + BR + TR
            (8, [n, n, n, p]),   // TL only
            (9, [p, n, n, p]),   // BL + TL
            (10, [n, p, n, p]),  // BR + TL (saddle)
            (11, [p, p, n, p]),  // BL + BR + TL
            (12, [n, n, p, p]),  // TR + TL
            (13, [p, n, p, p]),  // BL + TR + TL
            (14, [n, p, p, p]),  // BR + TR + TL
            (15, [p, p, p, p]), // all positive
        ];

        for (case_idx, corners) in &cases {
            // Grid layout: BL=grid[0], BR=grid[1], TL=grid[2], TR=grid[3]
            // In row-major: row 0 = [BL, BR], row 1 = [TL, TR]
            let grid = vec![corners[0], corners[1], corners[3], corners[2]];
            let segments = marching_squares(&grid, 1, 1, 0.0, 0.0, 1.0);

            match case_idx {
                0 | 15 => assert!(
                    segments.is_empty(),
                    "Case {} should produce 0 segments",
                    case_idx
                ),
                5 | 10 => assert_eq!(
                    segments.len(), 2,
                    "Saddle case {} should produce 2 segments, got {}",
                    case_idx, segments.len()
                ),
                _ => assert_eq!(
                    segments.len(), 1,
                    "Case {} should produce 1 segment, got {}",
                    case_idx, segments.len()
                ),
            }
        }
    }
}
