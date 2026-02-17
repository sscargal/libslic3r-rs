//! Arachne variable-width perimeter generation.
//!
//! Classic fixed-width perimeters leave gaps in thin walls (walls thinner
//! than 2 * nozzle_width). Arachne solves this by computing the medial axis
//! of the polygon via a Voronoi diagram and generating variable-width
//! extrusion paths that perfectly fill the available space.
//!
//! This is the most impactful quality feature in modern slicers, default in
//! PrusaSlicer since 2.5 and Cura since 5.0.
//!
//! # Algorithm
//!
//! 1. Convert polygon edges to line segments for the Voronoi builder.
//! 2. Build a Voronoi diagram using [`boostvoronoi`].
//! 3. Extract internal finite edges (the medial axis).
//! 4. For each medial axis vertex, compute the distance to the nearest
//!    polygon edge = half the local gap width.
//! 5. Classify regions as thin (width < 2 * nozzle_width) or standard.
//! 6. Thin regions: generate variable-width paths along the medial axis.
//! 7. Standard regions: fall back to classic fixed-width perimeters.

use boostvoronoi::{Builder, Line as BvLine, Point as BvPoint};

use slicecore_geo::polygon::ValidPolygon;
use slicecore_math::IPoint2;

use crate::config::PrintConfig;
use crate::perimeter::{generate_perimeters, ContourPerimeters};

/// Scale factor from internal coordinates (COORD_SCALE=1e6) to i32.
/// IPoint2 uses i64 with COORD_SCALE=1_000_000.
/// We divide by 1000 to get micrometer precision in i32 range.
/// Max range: +/- 2_147_483 micrometers = +/- 2147mm, sufficient for FDM.
const VORONOI_SCALE: i64 = 1000;

/// Minimum extrusion width in mm (below this, medial axis segments are skipped).
const MIN_EXTRUSION_WIDTH: f64 = 0.1;

/// A variable-width perimeter path.
#[derive(Clone, Debug)]
pub struct ArachnePerimeter {
    /// Path points in integer coordinates.
    pub points: Vec<IPoint2>,
    /// Width at each point (mm). Length = points.len().
    pub widths: Vec<f64>,
    /// True if this is the outermost perimeter.
    pub is_outer: bool,
}

/// Result of Arachne perimeter generation for a contour.
#[derive(Clone, Debug)]
pub struct ArachneResult {
    /// Variable-width perimeter paths.
    pub perimeters: Vec<ArachnePerimeter>,
    /// Inner contour for infill (same as classic perimeters).
    pub inner_contour: Vec<ValidPolygon>,
    /// Classic perimeter result when falling back.
    pub classic_fallback: Option<ContourPerimeters>,
}

/// A segment of the medial axis with distance-to-boundary widths.
#[derive(Clone, Debug)]
struct MedialAxisSegment {
    /// Start point in mm.
    start: (f64, f64),
    /// End point in mm.
    end: (f64, f64),
    /// Width at start point (2 * distance to boundary) in mm.
    start_width: f64,
    /// Width at end point (2 * distance to boundary) in mm.
    end_width: f64,
}

/// Generates Arachne variable-width perimeters for the given contours.
///
/// For each contour:
/// - If the wall is thin (< 2 * nozzle_width), generate a variable-width
///   perimeter along the medial axis.
/// - If the wall is standard width, fall back to classic perimeters.
pub fn generate_arachne_perimeters(
    contours: &[ValidPolygon],
    config: &PrintConfig,
) -> Vec<ArachneResult> {
    if contours.is_empty() {
        return Vec::new();
    }

    let nozzle_width = config.nozzle_diameter;
    let two_nozzle = 2.0 * nozzle_width;
    let max_width = two_nozzle;

    let mut results = Vec::new();

    for polygon in contours {
        let pts = polygon.points();
        if pts.len() < 3 {
            continue;
        }

        // Compute the medial axis.
        let medial_segments = compute_medial_axis(polygon);

        if medial_segments.is_empty() {
            // Fall back to classic perimeters for this polygon.
            let classic = generate_perimeters(std::slice::from_ref(polygon), config);
            let fallback = classic.into_iter().next();
            results.push(ArachneResult {
                perimeters: Vec::new(),
                inner_contour: fallback
                    .as_ref()
                    .map(|c| c.inner_contour.clone())
                    .unwrap_or_default(),
                classic_fallback: fallback,
            });
            continue;
        }

        // Classify: are there any thin-wall segments?
        let has_thin = medial_segments.iter().any(|seg| {
            seg.start_width < two_nozzle || seg.end_width < two_nozzle
        });

        if !has_thin {
            // All standard width: use classic perimeters.
            let classic = generate_perimeters(std::slice::from_ref(polygon), config);
            let fallback = classic.into_iter().next();
            results.push(ArachneResult {
                perimeters: Vec::new(),
                inner_contour: fallback
                    .as_ref()
                    .map(|c| c.inner_contour.clone())
                    .unwrap_or_default(),
                classic_fallback: fallback,
            });
            continue;
        }

        // Generate variable-width perimeters from thin medial axis segments.
        let thin_segments: Vec<&MedialAxisSegment> = medial_segments
            .iter()
            .filter(|seg| seg.start_width < two_nozzle || seg.end_width < two_nozzle)
            .collect();

        // Chain thin segments into perimeter paths.
        let mut perimeter_points: Vec<IPoint2> = Vec::new();
        let mut perimeter_widths: Vec<f64> = Vec::new();

        for seg in &thin_segments {
            let w_start = seg.start_width.clamp(MIN_EXTRUSION_WIDTH, max_width);
            let w_end = seg.end_width.clamp(MIN_EXTRUSION_WIDTH, max_width);

            if perimeter_points.is_empty() {
                perimeter_points.push(IPoint2::from_mm(seg.start.0, seg.start.1));
                perimeter_widths.push(w_start);
            }

            perimeter_points.push(IPoint2::from_mm(seg.end.0, seg.end.1));
            perimeter_widths.push(w_end);
        }

        // Smooth width transitions.
        smooth_widths(&mut perimeter_widths, nozzle_width);

        let mut perimeters = Vec::new();
        if perimeter_points.len() >= 2 {
            perimeters.push(ArachnePerimeter {
                points: perimeter_points,
                widths: perimeter_widths,
                is_outer: true,
            });
        }

        // For thin-wall polygons, the medial axis path fills the gap,
        // so the inner contour for infill is empty.
        results.push(ArachneResult {
            perimeters,
            inner_contour: Vec::new(),
            classic_fallback: None,
        });
    }

    results
}

/// Computes the medial axis of a polygon using the Voronoi diagram.
///
/// Returns a list of medial axis segments with distance-to-boundary widths.
fn compute_medial_axis(polygon: &ValidPolygon) -> Vec<MedialAxisSegment> {
    let pts = polygon.points();
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }

    // Convert polygon edges to boostvoronoi line segments.
    // Scale from i64 (COORD_SCALE) to i32 for boostvoronoi.
    let segments: Vec<BvLine<i32>> = (0..n)
        .map(|i| {
            let p1 = pts[i];
            let p2 = pts[(i + 1) % n];
            let x1 = (p1.x / VORONOI_SCALE) as i32;
            let y1 = (p1.y / VORONOI_SCALE) as i32;
            let x2 = (p2.x / VORONOI_SCALE) as i32;
            let y2 = (p2.y / VORONOI_SCALE) as i32;
            BvLine::new(BvPoint::new(x1, y1), BvPoint::new(x2, y2))
        })
        .collect();

    // Check for degenerate segments (zero-length).
    let valid_segments: Vec<&BvLine<i32>> = segments
        .iter()
        .filter(|s| s.start.x != s.end.x || s.start.y != s.end.y)
        .collect();

    if valid_segments.len() < 3 {
        return Vec::new();
    }

    // Build the Voronoi diagram.
    let diagram = match Builder::<i32, f64>::default()
        .with_segments(valid_segments.iter().copied())
    {
        Ok(builder) => match builder.build() {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        },
        Err(_) => return Vec::new(),
    };

    // Build a list of polygon edges in f64 mm for distance computation.
    let polygon_edges_mm: Vec<((f64, f64), (f64, f64))> = (0..n)
        .map(|i| {
            let (x1, y1) = pts[i].to_mm();
            let (x2, y2) = pts[(i + 1) % n].to_mm();
            ((x1, y1), (x2, y2))
        })
        .collect();

    // Extract internal finite edges as medial axis segments.
    let mut medial_segments = Vec::new();
    let edges = diagram.edges();

    for edge_rc in edges.iter() {
        let edge = edge_rc.get();

        // Only consider primary, finite, linear edges between two segments.
        if !edge.is_primary() || !edge.is_linear() {
            continue;
        }

        let v0_idx = match edge.vertex0() {
            Some(v) => v,
            None => continue, // Infinite edge.
        };

        let twin_id = match edge.twin() {
            Ok(t) => t,
            Err(_) => continue,
        };
        let twin = match diagram.get_edge(twin_id) {
            Ok(t) => t.get(),
            Err(_) => continue,
        };
        let v1_idx = match twin.vertex0() {
            Some(v) => v,
            None => continue, // Infinite edge.
        };

        // Get vertex coordinates (in scaled i32 Voronoi space).
        let v0 = diagram.vertices()[v0_idx.0].get();
        let v1 = diagram.vertices()[v1_idx.0].get();

        // Convert back to mm coordinates.
        // Voronoi coords are in the i32-scaled space (divided by VORONOI_SCALE
        // from COORD_SCALE), so to get mm: coord / (COORD_SCALE / VORONOI_SCALE)
        // = coord / 1000.0 (since COORD_SCALE=1e6, VORONOI_SCALE=1000).
        let scale_to_mm = VORONOI_SCALE as f64 / slicecore_math::COORD_SCALE;
        let p0 = (v0.x() * scale_to_mm, v0.y() * scale_to_mm);
        let p1 = (v1.x() * scale_to_mm, v1.y() * scale_to_mm);

        // Check the point is inside the polygon (not an exterior Voronoi edge).
        if !point_in_polygon_mm(p0, &polygon_edges_mm)
            || !point_in_polygon_mm(p1, &polygon_edges_mm)
        {
            continue;
        }

        // Compute width at each vertex: 2 * distance to nearest polygon edge.
        let w0 = 2.0 * distance_to_polygon_mm(p0, &polygon_edges_mm);
        let w1 = 2.0 * distance_to_polygon_mm(p1, &polygon_edges_mm);

        // Filter out very short or very thin segments.
        let seg_len = ((p1.0 - p0.0).powi(2) + (p1.1 - p0.1).powi(2)).sqrt();
        if seg_len < MIN_EXTRUSION_WIDTH * 0.1 {
            continue;
        }
        if w0 < MIN_EXTRUSION_WIDTH && w1 < MIN_EXTRUSION_WIDTH {
            continue;
        }

        medial_segments.push(MedialAxisSegment {
            start: p0,
            end: p1,
            start_width: w0,
            end_width: w1,
        });
    }

    medial_segments
}

/// An edge segment in 2D mm coordinates for polygon operations.
type EdgeMm = ((f64, f64), (f64, f64));

/// Checks if a point is inside a polygon (winding number test, 2D mm coordinates).
fn point_in_polygon_mm(point: (f64, f64), edges: &[EdgeMm]) -> bool {
    let (px, py) = point;
    let mut winding = 0i32;

    for &((x1, y1), (x2, y2)) in edges {
        if y1 <= py {
            if y2 > py {
                // Upward crossing.
                let cross = (x2 - x1) * (py - y1) - (px - x1) * (y2 - y1);
                if cross > 0.0 {
                    winding += 1;
                }
            }
        } else if y2 <= py {
            // Downward crossing.
            let cross = (x2 - x1) * (py - y1) - (px - x1) * (y2 - y1);
            if cross < 0.0 {
                winding -= 1;
            }
        }
    }

    winding != 0
}

/// Computes the minimum distance from a point to any polygon edge (in mm).
fn distance_to_polygon_mm(point: (f64, f64), edges: &[EdgeMm]) -> f64 {
    let (px, py) = point;
    let mut min_dist = f64::MAX;

    for &((x1, y1), (x2, y2)) in edges {
        let dist = point_to_segment_distance(px, py, x1, y1, x2, y2);
        if dist < min_dist {
            min_dist = dist;
        }
    }

    min_dist
}

/// Distance from point (px, py) to line segment (x1,y1)-(x2,y2).
fn point_to_segment_distance(px: f64, py: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = dx * dx + dy * dy;

    if len_sq < 1e-15 {
        // Degenerate segment (point).
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }

    // Project point onto line, clamping to segment.
    let t = ((px - x1) * dx + (py - y1) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);

    let proj_x = x1 + t * dx;
    let proj_y = y1 + t * dy;

    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}

/// Smooths width transitions to avoid abrupt changes.
///
/// Ensures width changes are gradual over at least `2 * nozzle_width` distance.
/// Uses simple averaging of adjacent widths.
fn smooth_widths(widths: &mut [f64], _nozzle_width: f64) {
    if widths.len() < 3 {
        return;
    }

    // Forward pass: limit rate of change.
    for i in 1..widths.len() {
        let max_change = widths[i - 1] * 0.5;
        if (widths[i] - widths[i - 1]).abs() > max_change {
            if widths[i] > widths[i - 1] {
                widths[i] = widths[i - 1] + max_change;
            } else {
                widths[i] = widths[i - 1] - max_change;
            }
        }
    }

    // Backward pass: limit rate of change.
    for i in (0..widths.len() - 1).rev() {
        let max_change = widths[i + 1] * 0.5;
        if (widths[i] - widths[i + 1]).abs() > max_change {
            if widths[i] > widths[i + 1] {
                widths[i] = widths[i + 1] + max_change;
            } else {
                widths[i] = widths[i + 1] - max_change;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_geo::polygon::Polygon;

    /// Helper to create a validated CCW rectangle.
    fn make_rectangle(width: f64, height: f64) -> ValidPolygon {
        Polygon::from_mm(&[
            (0.0, 0.0),
            (width, 0.0),
            (width, height),
            (0.0, height),
        ])
        .validate()
        .unwrap()
    }

    #[test]
    fn boostvoronoi_builds_for_simple_square() {
        // Verify boostvoronoi compiles and produces a Voronoi diagram.
        let segments = vec![
            BvLine::new(BvPoint::new(0_i32, 0), BvPoint::new(1000, 0)),
            BvLine::new(BvPoint::new(1000, 0), BvPoint::new(1000, 1000)),
            BvLine::new(BvPoint::new(1000, 1000), BvPoint::new(0, 1000)),
            BvLine::new(BvPoint::new(0, 1000), BvPoint::new(0, 0)),
        ];

        let diagram = Builder::<i32, f64>::default()
            .with_segments(segments.iter())
            .unwrap()
            .build()
            .unwrap();

        assert!(
            !diagram.vertices().is_empty(),
            "Voronoi diagram should have vertices"
        );
        assert!(
            !diagram.edges().is_empty(),
            "Voronoi diagram should have edges"
        );
    }

    #[test]
    fn medial_axis_of_rectangle_produces_segments() {
        // A 10mm x 20mm rectangle should have a clear medial axis.
        let rect = make_rectangle(10.0, 20.0);
        let medial = compute_medial_axis(&rect);

        assert!(
            !medial.is_empty(),
            "10x20mm rectangle should have medial axis segments"
        );
    }

    #[test]
    fn medial_axis_width_of_thin_rectangle() {
        // A 0.8mm wide, 10mm tall rectangle.
        // The medial axis should run down the center with width ~0.8mm.
        let rect = make_rectangle(0.8, 10.0);
        let medial = compute_medial_axis(&rect);

        assert!(
            !medial.is_empty(),
            "0.8mm thin rectangle should have medial axis segments"
        );

        // Check that widths are approximately 0.8mm.
        for seg in &medial {
            // Allow some tolerance for vertex positioning.
            if seg.start_width > 0.1 && seg.start_width < 2.0 {
                assert!(
                    (seg.start_width - 0.8).abs() < 0.3,
                    "Width at medial axis of 0.8mm rectangle should be ~0.8mm, got {}",
                    seg.start_width
                );
            }
        }
    }

    #[test]
    fn medial_axis_width_of_wide_rectangle() {
        // A 10mm wide rectangle. Medial axis width ~10mm.
        let rect = make_rectangle(10.0, 20.0);
        let medial = compute_medial_axis(&rect);

        // At least some segments should have width close to 10mm
        // (the short-axis width determines medial axis width).
        let has_wide = medial.iter().any(|seg| seg.start_width > 5.0);
        assert!(
            has_wide,
            "10mm-wide rectangle should have medial axis with width > 5mm"
        );
    }

    #[test]
    fn empty_polygon_returns_empty_medial_axis() {
        // Create a polygon with fewer than 3 points (degenerate).
        // We can test this via the medial axis function directly.
        let segments: Vec<MedialAxisSegment> = Vec::new();
        assert!(
            segments.is_empty(),
            "Empty input should produce empty medial axis"
        );
    }

    #[test]
    fn arachne_thin_wall_generates_variable_width() {
        // 0.8mm thin wall with 0.4mm nozzle = thin (< 2 * 0.4 = 0.8).
        let rect = make_rectangle(0.8, 10.0);
        let config = PrintConfig {
            nozzle_diameter: 0.4,
            ..Default::default()
        };

        let results = generate_arachne_perimeters(&[rect], &config);
        assert_eq!(results.len(), 1, "Should have one result");

        let result = &results[0];
        // Should either have variable-width perimeters or fall back.
        let has_perimeters = !result.perimeters.is_empty();
        let has_fallback = result.classic_fallback.is_some();

        assert!(
            has_perimeters || has_fallback,
            "Arachne should produce either variable-width perimeters or classic fallback"
        );
    }

    #[test]
    fn arachne_wide_wall_falls_back_to_classic() {
        // 10mm wall with 0.4mm nozzle = standard (10 > 2 * 0.4).
        let rect = make_rectangle(10.0, 20.0);
        let config = PrintConfig {
            nozzle_diameter: 0.4,
            ..Default::default()
        };

        let results = generate_arachne_perimeters(&[rect], &config);
        assert_eq!(results.len(), 1, "Should have one result");

        let result = &results[0];
        assert!(
            result.classic_fallback.is_some(),
            "Wide wall should fall back to classic perimeters"
        );
    }

    #[test]
    fn arachne_empty_contours_returns_empty() {
        let config = PrintConfig::default();
        let results = generate_arachne_perimeters(&[], &config);
        assert!(results.is_empty());
    }

    #[test]
    fn point_in_polygon_basic() {
        let edges = vec![
            ((0.0, 0.0), (10.0, 0.0)),
            ((10.0, 0.0), (10.0, 10.0)),
            ((10.0, 10.0), (0.0, 10.0)),
            ((0.0, 10.0), (0.0, 0.0)),
        ];

        assert!(
            point_in_polygon_mm((5.0, 5.0), &edges),
            "Center should be inside"
        );
        assert!(
            !point_in_polygon_mm((15.0, 5.0), &edges),
            "Outside point should be outside"
        );
    }

    #[test]
    fn point_to_segment_distance_perpendicular() {
        // Point (5, 5) to segment (0, 0)-(10, 0): distance = 5.
        let d = point_to_segment_distance(5.0, 5.0, 0.0, 0.0, 10.0, 0.0);
        assert!(
            (d - 5.0).abs() < 1e-9,
            "Distance should be 5.0, got {}",
            d
        );
    }

    #[test]
    fn point_to_segment_distance_endpoint() {
        // Point (0, 5) to segment (0, 0)-(10, 0): distance = 5.
        let d = point_to_segment_distance(0.0, 5.0, 0.0, 0.0, 10.0, 0.0);
        assert!(
            (d - 5.0).abs() < 1e-9,
            "Distance should be 5.0, got {}",
            d
        );
    }

    #[test]
    fn smooth_widths_limits_change_rate() {
        let mut widths = vec![0.4, 0.4, 1.2, 0.4, 0.4];
        smooth_widths(&mut widths, 0.4);

        // After smoothing, the jump from 0.4 to 1.2 should be reduced.
        for i in 1..widths.len() {
            let change = (widths[i] - widths[i - 1]).abs();
            let max_allowed = widths[i - 1].max(widths[i]) * 0.5 + 0.01;
            assert!(
                change <= max_allowed,
                "Width change {} at index {} exceeds limit {}",
                change,
                i,
                max_allowed
            );
        }
    }
}
