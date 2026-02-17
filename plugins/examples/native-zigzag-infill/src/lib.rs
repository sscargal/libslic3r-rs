//! Native zigzag infill plugin for slicecore.
//!
//! This is an example plugin demonstrating how to create a native (cdylib)
//! infill pattern plugin. It generates a zigzag infill pattern by creating
//! vertical scan lines across the polygon boundary and connecting them
//! alternately at top and bottom to form a continuous zigzag path.
//!
//! # Building
//!
//! ```bash
//! cargo build --manifest-path plugins/examples/native-zigzag-infill/Cargo.toml
//! ```
//!
//! The resulting `.so` / `.dll` / `.dylib` can be placed alongside a `plugin.toml`
//! manifest and loaded by the slicecore `PluginRegistry`.

// Suppress the non_local_definitions lint triggered by abi_stable macro expansion.
#![allow(non_local_definitions)]

use abi_stable::export_root_module;
use abi_stable::prefix_type::PrefixTypeTrait;
use abi_stable::sabi_extern_fn;
use abi_stable::sabi_trait::TD_Opaque;
use abi_stable::std_types::{RBox, RErr, ROk, RResult, RString, RVec};

use slicecore_plugin_api::traits::{
    InfillPatternPlugin, InfillPatternPlugin_TO, InfillPluginMod, InfillPluginMod_Ref,
};
use slicecore_plugin_api::types::{FfiInfillLine, InfillRequest, InfillResult};

// ---------------------------------------------------------------------------
// Root module entry point
// ---------------------------------------------------------------------------

/// Export the root module so `abi_stable` can discover this plugin.
#[export_root_module]
fn instantiate_root_module() -> InfillPluginMod_Ref {
    InfillPluginMod { new: new_plugin }.leak_into_prefix()
}

/// Factory function called by the host to create a plugin instance.
#[sabi_extern_fn]
fn new_plugin() -> InfillPatternPlugin_TO<'static, RBox<()>> {
    InfillPatternPlugin_TO::from_value(ZigzagInfillPlugin, TD_Opaque)
}

// ---------------------------------------------------------------------------
// Plugin implementation
// ---------------------------------------------------------------------------

/// A zigzag infill pattern plugin.
///
/// Generates continuous diagonal lines that bounce between polygon boundaries,
/// forming a zigzag path with good structural connectivity.
#[derive(Debug, Clone)]
struct ZigzagInfillPlugin;

impl InfillPatternPlugin for ZigzagInfillPlugin {
    fn name(&self) -> RString {
        "zigzag".into()
    }

    fn description(&self) -> RString {
        "Zigzag infill: continuous diagonal lines that bounce between boundaries".into()
    }

    fn generate(&self, request: &InfillRequest) -> RResult<InfillResult, RString> {
        match generate_zigzag(request) {
            Ok(result) => ROk(result),
            Err(e) => RErr(RString::from(e)),
        }
    }
}

// ---------------------------------------------------------------------------
// Zigzag infill algorithm
// ---------------------------------------------------------------------------

/// A 2D point in integer coordinate space.
#[derive(Debug, Clone, Copy)]
struct Point {
    x: i64,
    y: i64,
}

/// An edge of the polygon boundary.
#[derive(Debug, Clone, Copy)]
struct Edge {
    p0: Point,
    p1: Point,
}

/// Generate zigzag infill for the given request.
///
/// Algorithm:
/// 1. Decode flattened boundary_points into polygon edges using boundary_lengths.
/// 2. Compute the bounding box of all boundary points.
/// 3. Calculate scan line spacing from line_width and density.
/// 4. For each vertical scan line, find intersections with polygon edges.
/// 5. Sort intersections by Y and pair them into segments (inside the polygon).
/// 6. Connect adjacent scan line segments alternately at top/bottom to form
///    a continuous zigzag path.
fn generate_zigzag(request: &InfillRequest) -> Result<InfillResult, String> {
    // Decode boundary into edges
    let edges = decode_boundary_edges(&request.boundary_points, &request.boundary_lengths)?;

    if edges.is_empty() {
        return Ok(InfillResult {
            lines: RVec::new(),
        });
    }

    // Collect all boundary points for bounding box computation
    let all_points: Vec<Point> = edges.iter().map(|e| e.p0).collect();

    // Compute bounding box
    let (min_x, max_x, _min_y, _max_y) = bounding_box(&all_points);

    // Calculate spacing between scan lines
    // line_width is in mm, but coordinates are in internal units (COORD_SCALE = 1_000_000)
    let coord_scale: f64 = 1_000_000.0;
    let density = request.density.clamp(0.01, 1.0);
    let spacing_mm = request.line_width / density;
    let spacing = (spacing_mm * coord_scale) as i64;

    if spacing <= 0 {
        return Ok(InfillResult {
            lines: RVec::new(),
        });
    }

    // Generate vertical scan lines and find intersections
    let mut scan_line_segments: Vec<Vec<(i64, i64)>> = Vec::new();
    let mut x = min_x + spacing;

    while x < max_x {
        let mut intersections = find_vertical_intersections(x, &edges);
        intersections.sort();
        intersections.dedup();

        // Pair intersections into segments (inside the polygon)
        let mut segments: Vec<(i64, i64)> = Vec::new();
        let mut i = 0;
        while i + 1 < intersections.len() {
            segments.push((intersections[i], intersections[i + 1]));
            i += 2;
        }

        scan_line_segments.push(segments);
        x += spacing;
    }

    // Build zigzag lines by connecting scan line segments
    let mut lines: Vec<FfiInfillLine> = Vec::new();
    let mut connect_at_top = false; // Alternate top/bottom connections

    let scan_x_start = min_x + spacing;

    for (scan_idx, segments) in scan_line_segments.iter().enumerate() {
        let scan_x = scan_x_start + (scan_idx as i64) * spacing;

        // Add the scan line segments themselves (vertical lines within the polygon)
        for &(y_start, y_end) in segments {
            lines.push(FfiInfillLine {
                start_x: scan_x,
                start_y: y_start,
                end_x: scan_x,
                end_y: y_end,
            });
        }

        // Connect to the next scan line if available
        if scan_idx + 1 < scan_line_segments.len() {
            let next_x = scan_x + spacing;
            let next_segments = &scan_line_segments[scan_idx + 1];

            if !segments.is_empty() && !next_segments.is_empty() {
                if connect_at_top {
                    // Connect at the topmost points (max Y)
                    let current_top = segments.last().unwrap().1;
                    let next_top = next_segments.last().unwrap().1;
                    lines.push(FfiInfillLine {
                        start_x: scan_x,
                        start_y: current_top,
                        end_x: next_x,
                        end_y: next_top,
                    });
                } else {
                    // Connect at the bottommost points (min Y)
                    let current_bottom = segments.first().unwrap().0;
                    let next_bottom = next_segments.first().unwrap().0;
                    lines.push(FfiInfillLine {
                        start_x: scan_x,
                        start_y: current_bottom,
                        end_x: next_x,
                        end_y: next_bottom,
                    });
                }
            }

            connect_at_top = !connect_at_top;
        }
    }

    Ok(InfillResult {
        lines: RVec::from(lines),
    })
}

/// Decode flattened boundary_points and boundary_lengths into polygon edges.
///
/// `boundary_points` contains `[x0, y0, x1, y1, ...]` coordinate pairs.
/// `boundary_lengths` contains the number of *points* per polygon.
/// Each polygon's edges are formed by connecting consecutive points, with
/// the last point connected back to the first.
fn decode_boundary_edges(
    boundary_points: &RVec<i64>,
    boundary_lengths: &RVec<u32>,
) -> Result<Vec<Edge>, String> {
    let mut edges = Vec::new();
    let mut offset = 0usize;

    for &num_points in boundary_lengths.iter() {
        let n = num_points as usize;
        if n < 3 {
            // Need at least 3 points for a valid polygon
            offset += n * 2;
            continue;
        }

        let coords_needed = n * 2;
        if offset + coords_needed > boundary_points.len() {
            return Err(format!(
                "boundary_points too short: need {} elements at offset {}, have {}",
                coords_needed,
                offset,
                boundary_points.len()
            ));
        }

        // Extract points for this polygon
        let mut points = Vec::with_capacity(n);
        for i in 0..n {
            let idx = offset + i * 2;
            points.push(Point {
                x: boundary_points[idx],
                y: boundary_points[idx + 1],
            });
        }

        // Create edges (including closing edge from last to first)
        for i in 0..n {
            let next = (i + 1) % n;
            edges.push(Edge {
                p0: points[i],
                p1: points[next],
            });
        }

        offset += coords_needed;
    }

    Ok(edges)
}

/// Compute the bounding box of a set of points.
/// Returns (min_x, max_x, min_y, max_y).
fn bounding_box(points: &[Point]) -> (i64, i64, i64, i64) {
    let mut min_x = i64::MAX;
    let mut max_x = i64::MIN;
    let mut min_y = i64::MAX;
    let mut max_y = i64::MIN;

    for p in points {
        min_x = min_x.min(p.x);
        max_x = max_x.max(p.x);
        min_y = min_y.min(p.y);
        max_y = max_y.max(p.y);
    }

    (min_x, max_x, min_y, max_y)
}

/// Find Y-coordinates where a vertical line at `x` intersects the polygon edges.
///
/// For each edge, if the vertical line at `x` crosses the edge's X range,
/// compute the Y intersection using linear interpolation.
fn find_vertical_intersections(x: i64, edges: &[Edge]) -> Vec<i64> {
    let mut intersections = Vec::new();

    for edge in edges {
        let (ex_min, ex_max) = if edge.p0.x <= edge.p1.x {
            (edge.p0.x, edge.p1.x)
        } else {
            (edge.p1.x, edge.p0.x)
        };

        // Skip edges that don't span this X value
        if x < ex_min || x > ex_max {
            continue;
        }

        // Handle vertical edges (same X for both endpoints)
        if edge.p0.x == edge.p1.x {
            // Vertical edge: add both endpoints as intersections
            intersections.push(edge.p0.y);
            intersections.push(edge.p1.y);
            continue;
        }

        // Linear interpolation: y = p0.y + (x - p0.x) * (p1.y - p0.y) / (p1.x - p0.x)
        // Use i128 to avoid overflow with large coordinates
        let dx = (edge.p1.x - edge.p0.x) as i128;
        let dy = (edge.p1.y - edge.p0.y) as i128;
        let t_num = (x - edge.p0.x) as i128;
        let y = edge.p0.y as i128 + (t_num * dy) / dx;
        intersections.push(y as i64);
    }

    intersections
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use abi_stable::std_types::RVec;

    /// Helper: create an InfillRequest for a simple axis-aligned rectangle.
    ///
    /// The rectangle spans from (x0, y0) to (x1, y1) in internal coordinates.
    fn rect_request(x0: i64, y0: i64, x1: i64, y1: i64, density: f64) -> InfillRequest {
        InfillRequest {
            boundary_points: RVec::from(vec![x0, y0, x1, y0, x1, y1, x0, y1]),
            boundary_lengths: RVec::from(vec![4u32]),
            density,
            layer_index: 0,
            layer_z: 0.2,
            line_width: 0.4,
        }
    }

    #[test]
    fn test_plugin_name() {
        let plugin = ZigzagInfillPlugin;
        assert_eq!(plugin.name().as_str(), "zigzag");
    }

    #[test]
    fn test_plugin_description() {
        let plugin = ZigzagInfillPlugin;
        assert!(plugin.description().as_str().contains("Zigzag"));
    }

    #[test]
    fn test_decode_boundary_edges_rectangle() {
        let points = RVec::from(vec![0i64, 0, 100, 0, 100, 100, 0, 100]);
        let lengths = RVec::from(vec![4u32]);
        let edges = decode_boundary_edges(&points, &lengths).unwrap();
        assert_eq!(edges.len(), 4);
    }

    #[test]
    fn test_decode_boundary_edges_degenerate() {
        let points = RVec::from(vec![0i64, 0, 100, 0]);
        let lengths = RVec::from(vec![2u32]);
        let edges = decode_boundary_edges(&points, &lengths).unwrap();
        // 2 points is too few for a polygon, should be skipped
        assert_eq!(edges.len(), 0);
    }

    #[test]
    fn test_bounding_box() {
        let points = vec![
            Point { x: 10, y: 20 },
            Point { x: 50, y: 5 },
            Point { x: 30, y: 70 },
        ];
        let (min_x, max_x, min_y, max_y) = bounding_box(&points);
        assert_eq!(min_x, 10);
        assert_eq!(max_x, 50);
        assert_eq!(min_y, 5);
        assert_eq!(max_y, 70);
    }

    #[test]
    fn test_vertical_intersections_simple_rect() {
        // Rectangle: (0,0) -> (100,0) -> (100,100) -> (0,100)
        let edges = vec![
            Edge {
                p0: Point { x: 0, y: 0 },
                p1: Point { x: 100, y: 0 },
            },
            Edge {
                p0: Point { x: 100, y: 0 },
                p1: Point { x: 100, y: 100 },
            },
            Edge {
                p0: Point { x: 100, y: 100 },
                p1: Point { x: 0, y: 100 },
            },
            Edge {
                p0: Point { x: 0, y: 100 },
                p1: Point { x: 0, y: 0 },
            },
        ];

        let ys = find_vertical_intersections(50, &edges);
        // Should intersect top and bottom edges
        assert!(ys.contains(&0));
        assert!(ys.contains(&100));
    }

    #[test]
    fn test_zigzag_generates_lines_for_rectangle() {
        // 10mm x 10mm rectangle in internal coordinates (COORD_SCALE = 1_000_000)
        let request = rect_request(0, 0, 10_000_000, 10_000_000, 0.2);
        let result = generate_zigzag(&request).unwrap();
        // With density 0.2 and line_width 0.4mm, spacing = 2mm = 2_000_000 units
        // Over 10mm width, should get several scan lines
        assert!(
            !result.lines.is_empty(),
            "Expected non-empty infill lines for rectangular boundary"
        );
    }

    #[test]
    fn test_zigzag_density_affects_line_count() {
        let low_density = rect_request(0, 0, 10_000_000, 10_000_000, 0.1);
        let high_density = rect_request(0, 0, 10_000_000, 10_000_000, 0.5);

        let low_result = generate_zigzag(&low_density).unwrap();
        let high_result = generate_zigzag(&high_density).unwrap();

        // Higher density should produce more lines
        assert!(
            high_result.lines.len() > low_result.lines.len(),
            "Higher density should produce more infill lines: low={}, high={}",
            low_result.lines.len(),
            high_result.lines.len()
        );
    }

    #[test]
    fn test_zigzag_empty_boundary() {
        let request = InfillRequest {
            boundary_points: RVec::new(),
            boundary_lengths: RVec::new(),
            density: 0.2,
            layer_index: 0,
            layer_z: 0.2,
            line_width: 0.4,
        };
        let result = generate_zigzag(&request).unwrap();
        assert!(result.lines.is_empty());
    }

    #[test]
    fn test_zigzag_full_density() {
        // Full density (1.0) should produce tightly-packed lines
        let request = rect_request(0, 0, 5_000_000, 5_000_000, 1.0);
        let result = generate_zigzag(&request).unwrap();
        assert!(
            !result.lines.is_empty(),
            "Full density should produce infill lines"
        );
    }

    #[test]
    fn test_plugin_trait_object() {
        let plugin = ZigzagInfillPlugin;
        let trait_obj = InfillPatternPlugin_TO::from_value(plugin, TD_Opaque);

        assert_eq!(trait_obj.name().as_str(), "zigzag");

        let request = rect_request(0, 0, 10_000_000, 10_000_000, 0.2);
        match trait_obj.generate(&request) {
            ROk(result) => assert!(!result.lines.is_empty()),
            RErr(e) => panic!("unexpected error: {}", e),
        }
    }
}
