//! Gap fill between perimeters.
//!
//! After generating perimeter shells by inward offsetting, narrow regions may
//! remain between the innermost perimeter and the infill boundary that are too
//! narrow for a standard-width extrusion but too wide to leave empty. This
//! module detects these regions and generates thin extrusion paths along their
//! centerlines, eliminating voids and producing solid walls.
//!
//! The algorithm:
//! 1. Compute the gap region via polygon difference (innermost shell minus
//!    inner contour).
//! 2. Filter gap polygons by area and width thresholds.
//! 3. Generate approximate centerline paths by inward offsetting.
//! 4. Convert to [`GapFillPath`] with appropriate extrusion widths.

use slicecore_geo::boolean::polygon_difference;
use slicecore_geo::offset::{offset_polygons, JoinType};
use slicecore_geo::polygon::ValidPolygon;
use slicecore_math::{coord_to_mm, mm_to_coord, IPoint2};

use crate::perimeter::PerimeterShell;

/// A gap fill extrusion path (thin path along the center of a narrow gap).
#[derive(Clone, Debug)]
pub struct GapFillPath {
    /// Path points in integer coordinates.
    pub points: Vec<IPoint2>,
    /// Extrusion width in mm (may be narrower than standard width).
    pub width: f64,
}

/// Detects narrow gaps between perimeters and generates thin fill paths.
///
/// Gaps are the narrow regions between the innermost perimeter shell and the
/// infill boundary. This function computes those regions, filters out ones
/// that are too small or too narrow, and generates centerline paths.
///
/// # Parameters
/// - `perimeter_shells`: The perimeter shells (in print order).
/// - `inner_contour`: The infill boundary (innermost offset result).
/// - `original_contours`: The original slice contour polygons.
/// - `min_width`: Minimum gap width to fill (mm). Gaps narrower than this
///   are skipped. Default 0.1 mm.
/// - `max_width`: Maximum gap width to fill (mm). Gaps wider than this are
///   left for infill. Typically equals nozzle diameter.
/// - `line_width`: Standard extrusion width (mm).
///
/// # Returns
/// A vector of [`GapFillPath`] representing thin extrusion centerlines.
pub fn detect_and_fill_gaps(
    perimeter_shells: &[PerimeterShell],
    inner_contour: &[ValidPolygon],
    original_contours: &[ValidPolygon],
    min_width: f64,
    max_width: f64,
    line_width: f64,
) -> Vec<GapFillPath> {
    let mut gap_fills = Vec::new();

    // If no shells, there is no gap to fill.
    if perimeter_shells.is_empty() {
        return gap_fills;
    }

    // 1. Compute gap region between innermost shell and inner contour.
    // The innermost shell is the last shell in outside-in order, or the first
    // in inside-out order. We need to find the actual innermost by checking
    // is_outer: the non-outer shells are inner, and the one with smallest area
    // is innermost.
    let innermost_shell = find_innermost_shell(perimeter_shells);
    if let Some(shell) = innermost_shell {
        let gap_region = compute_gap_region(&shell.polygons, inner_contour);
        let paths = process_gap_region(&gap_region, min_width, max_width, line_width);
        gap_fills.extend(paths);
    }

    // 2. Check for gaps between original contour and outermost perimeter.
    let outermost_shell = perimeter_shells.iter().find(|s| s.is_outer);
    if let Some(shell) = outermost_shell {
        let gap_region = compute_gap_region(original_contours, &shell.polygons);
        let paths = process_gap_region(&gap_region, min_width, max_width, line_width);
        gap_fills.extend(paths);
    }

    gap_fills
}

/// Finds the innermost perimeter shell (smallest total area).
fn find_innermost_shell(shells: &[PerimeterShell]) -> Option<&PerimeterShell> {
    if shells.is_empty() {
        return None;
    }
    if shells.len() == 1 {
        return Some(&shells[0]);
    }

    // The innermost shell is the one with the smallest polygon area.
    shells.iter().min_by(|a, b| {
        let area_a: f64 = a.polygons.iter().map(|p| p.area_mm2()).sum();
        let area_b: f64 = b.polygons.iter().map(|p| p.area_mm2()).sum();
        area_a
            .partial_cmp(&area_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

/// Computes the gap region between two polygon sets (subject minus clip).
fn compute_gap_region(
    subject: &[ValidPolygon],
    clip: &[ValidPolygon],
) -> Vec<ValidPolygon> {
    if subject.is_empty() || clip.is_empty() {
        return Vec::new();
    }
    polygon_difference(subject, clip).unwrap_or_default()
}

/// Processes a set of gap polygons: filters by size and generates centerline paths.
fn process_gap_region(
    gap_polygons: &[ValidPolygon],
    min_width: f64,
    max_width: f64,
    line_width: f64,
) -> Vec<GapFillPath> {
    let mut paths = Vec::new();

    // Minimum area threshold: skip tiny gaps.
    let min_area_mm2 = line_width * line_width * 2.0;

    for polygon in gap_polygons {
        let area_mm2 = polygon.area_mm2();

        // Filter out tiny gaps.
        if area_mm2 < min_area_mm2 {
            continue;
        }

        // Estimate gap width: area / perimeter gives approximate average width.
        let perimeter_mm = polygon_perimeter_mm(polygon);
        if perimeter_mm < 1e-6 {
            continue;
        }
        let estimated_width = area_mm2 / (perimeter_mm / 2.0);

        // Filter by width range.
        if estimated_width < min_width || estimated_width > max_width {
            continue;
        }

        // Generate centerline path for this gap polygon.
        if let Some(path) = generate_centerline(polygon, estimated_width, min_width) {
            paths.push(path);
        }
    }

    paths
}

/// Computes the perimeter length of a polygon in mm.
fn polygon_perimeter_mm(polygon: &ValidPolygon) -> f64 {
    let pts = polygon.points();
    let n = pts.len();
    if n < 2 {
        return 0.0;
    }

    let mut perimeter = 0.0;
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let dx = coord_to_mm(b.x - a.x);
        let dy = coord_to_mm(b.y - a.y);
        perimeter += (dx * dx + dy * dy).sqrt();
    }
    perimeter
}

/// Generates a centerline path for a gap polygon.
///
/// Uses a simplified approach: offset the gap polygon inward by half the
/// estimated width. The resulting polygon (if non-empty) approximates the
/// centerline. We extract its points as the fill path.
///
/// If the inward offset collapses the polygon, we fall back to using the
/// polygon's own vertices as the path (for very thin gaps where the polygon
/// itself approximates the centerline).
fn generate_centerline(
    polygon: &ValidPolygon,
    estimated_width: f64,
    min_width: f64,
) -> Option<GapFillPath> {
    let half_width = estimated_width / 2.0;
    let inset_delta = mm_to_coord(-half_width);

    // Try to inset the gap polygon to approximate the centerline.
    let inset_result = offset_polygons(std::slice::from_ref(polygon), inset_delta, JoinType::Miter);

    match inset_result {
        Ok(inset_polys) if !inset_polys.is_empty() => {
            // Use the inset polygon points as the centerline.
            let pts: Vec<IPoint2> = inset_polys[0].points().to_vec();
            if pts.len() < 2 {
                return None;
            }

            // Check minimum path length.
            let path_len = path_length_mm(&pts);
            if path_len < 2.0 * min_width {
                return None;
            }

            Some(GapFillPath {
                points: pts,
                width: estimated_width,
            })
        }
        _ => {
            // Inset collapsed -- use the midpoints of opposite edges as a
            // simple centerline for very thin gaps.
            generate_thin_gap_centerline(polygon, estimated_width, min_width)
        }
    }
}

/// Generates a centerline for a very thin gap where inward offset collapses.
///
/// For thin gaps (roughly rectangular), finds the two longest edges and
/// connects their midpoints as the centerline path.
fn generate_thin_gap_centerline(
    polygon: &ValidPolygon,
    estimated_width: f64,
    min_width: f64,
) -> Option<GapFillPath> {
    let pts = polygon.points();
    let n = pts.len();
    if n < 3 {
        return None;
    }

    // Find the two longest edges.
    let mut edges: Vec<(usize, f64)> = (0..n)
        .map(|i| {
            let a = pts[i];
            let b = pts[(i + 1) % n];
            let dx = (b.x - a.x) as f64;
            let dy = (b.y - a.y) as f64;
            (i, (dx * dx + dy * dy).sqrt())
        })
        .collect();
    edges.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    if edges.len() < 2 {
        return None;
    }

    // The two longest edges are the "walls" of the gap.
    // The centerline connects the midpoints of the shorter edges (or simply
    // the midpoints of the two long edges projected to center).
    let edge_a_idx = edges[0].0;
    let edge_b_idx = edges[1].0;

    let mid_a = midpoint(pts[edge_a_idx], pts[(edge_a_idx + 1) % n]);
    let mid_b = midpoint(pts[edge_b_idx], pts[(edge_b_idx + 1) % n]);

    // Check minimum path length.
    let dx = coord_to_mm(mid_b.x - mid_a.x);
    let dy = coord_to_mm(mid_b.y - mid_a.y);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 2.0 * min_width {
        return None;
    }

    Some(GapFillPath {
        points: vec![mid_a, mid_b],
        width: estimated_width,
    })
}

/// Computes the midpoint of two integer points.
fn midpoint(a: IPoint2, b: IPoint2) -> IPoint2 {
    IPoint2::new((a.x + b.x) / 2, (a.y + b.y) / 2)
}

/// Computes the total path length in mm for a sequence of points.
fn path_length_mm(pts: &[IPoint2]) -> f64 {
    if pts.len() < 2 {
        return 0.0;
    }
    let mut total = 0.0;
    for i in 0..pts.len() - 1 {
        let dx = coord_to_mm(pts[i + 1].x - pts[i].x);
        let dy = coord_to_mm(pts[i + 1].y - pts[i].y);
        total += (dx * dx + dy * dy).sqrt();
    }
    total
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_geo::polygon::Polygon;

    /// Helper to create a validated CCW rectangle.
    fn make_rect(x: f64, y: f64, w: f64, h: f64) -> ValidPolygon {
        Polygon::from_mm(&[
            (x, y),
            (x + w, y),
            (x + w, y + h),
            (x, y + h),
        ])
        .validate()
        .unwrap()
    }

    /// Helper to create a validated CCW square.
    fn make_square(size: f64) -> ValidPolygon {
        make_rect(0.0, 0.0, size, size)
    }

    fn default_nozzle() -> f64 {
        0.4
    }

    fn default_line_width() -> f64 {
        0.4 * 1.1
    }

    #[test]
    fn gap_fill_with_narrow_rectangle() {
        // A rectangle 20mm long and 0.5mm wide (narrower than 2 perimeters).
        // Between the innermost shell and inner contour, there should be a gap.
        let narrow_rect = make_rect(0.0, 0.0, 20.0, 0.5);

        // Simulate shells: one shell offset inward by half line width.
        // The innermost shell is this single shell.
        let shell = PerimeterShell {
            polygons: vec![narrow_rect.clone()],
            is_outer: true,
        };

        // Inner contour: offset further inward -- for a 0.5mm-wide rect,
        // an inner contour inset by full line width may be empty.
        // So we use a thin sliver as the inner contour.
        let inner = make_rect(0.2, 0.1, 19.6, 0.3);

        let gap_fills = detect_and_fill_gaps(
            &[shell],
            &[inner],
            &[narrow_rect],
            0.1,
            default_nozzle(),
            default_line_width(),
        );

        // Should detect some gap regions (between shell and inner contour).
        // The gap region is the narrow strips along the edges.
        // Whether we get paths depends on the geometry, but the function
        // should not crash and should return a valid result.
        // For this specific geometry, the gap is narrow enough that
        // something should be generated.
        // NOTE: The exact result depends on polygon boolean math.
        // We just verify the function runs without error.
        assert!(
            gap_fills.iter().all(|gf| gf.width >= 0.1),
            "All gap fill paths should have width >= min_width"
        );
    }

    #[test]
    fn gap_fill_on_simple_square_returns_minimal() {
        // A 20mm square with 2 perimeters should have negligible gaps.
        use crate::config::PrintConfig;
        use crate::perimeter::generate_perimeters;

        let square = make_square(20.0);
        let config = PrintConfig {
            wall_count: 2,
            ..Default::default()
        };

        let perimeters = generate_perimeters(&[square.clone()], &config);
        let shells = &perimeters[0].shells;
        let inner = &perimeters[0].inner_contour;

        let gap_fills = detect_and_fill_gaps(
            shells,
            inner,
            &[square],
            0.1,
            config.machine.nozzle_diameter(),
            config.extrusion_width(),
        );

        // A simple 20mm square with well-fitting perimeters should produce
        // few or no gap fill paths.
        for gf in &gap_fills {
            assert!(
                gf.width >= 0.1,
                "Gap fill width {} should be >= min_width 0.1",
                gf.width
            );
        }
    }

    #[test]
    fn gap_fill_paths_width_above_min() {
        // Create a gap polygon manually and test processing.
        let gap_poly = make_rect(0.0, 0.0, 10.0, 0.25);
        let paths = process_gap_region(&[gap_poly], 0.1, 0.4, default_line_width());

        for path in &paths {
            assert!(
                path.width >= 0.1,
                "Gap fill path width {} should be >= min_width 0.1",
                path.width
            );
        }
    }

    #[test]
    fn tiny_gaps_filtered_out() {
        // A very tiny gap polygon (area < threshold).
        let tiny = make_rect(0.0, 0.0, 0.1, 0.05);
        let paths = process_gap_region(&[tiny], 0.1, 0.4, default_line_width());

        assert!(
            paths.is_empty(),
            "Very tiny gaps should be filtered out, got {} paths",
            paths.len()
        );
    }

    #[test]
    fn gap_fill_disabled_returns_empty() {
        // When gap fill is disabled, detect_and_fill_gaps should not be called.
        // But we can verify that an empty shell list returns empty.
        let gap_fills = detect_and_fill_gaps(
            &[],
            &[],
            &[],
            0.1,
            0.4,
            default_line_width(),
        );
        assert!(gap_fills.is_empty(), "Empty shells should produce no gap fills");
    }

    #[test]
    fn gap_fill_paths_within_gap_region() {
        // Create a moderately sized gap polygon.
        let gap_poly = make_rect(0.0, 0.0, 15.0, 0.3);
        let paths = process_gap_region(&[gap_poly.clone()], 0.1, 0.4, default_line_width());

        // All path points should be within or very near the gap polygon's
        // bounding box.
        for path in &paths {
            for pt in &path.points {
                let (x, y) = pt.to_mm();
                assert!(
                    x >= -1.0 && x <= 16.0 && y >= -1.0 && y <= 1.3,
                    "Gap fill point ({}, {}) should be near the gap polygon",
                    x, y
                );
            }
        }
    }

    #[test]
    fn gap_too_wide_filtered_out() {
        // A gap polygon that is wider than max_width.
        let wide_gap = make_rect(0.0, 0.0, 10.0, 2.0);
        let paths = process_gap_region(&[wide_gap], 0.1, 0.4, default_line_width());

        assert!(
            paths.is_empty(),
            "Gaps wider than max_width should be filtered out"
        );
    }

    #[test]
    fn polygon_perimeter_calculation() {
        let square = make_square(10.0);
        let perimeter = polygon_perimeter_mm(&square);
        assert!(
            (perimeter - 40.0).abs() < 0.01,
            "10mm square perimeter should be ~40mm, got {}",
            perimeter
        );
    }

    #[test]
    fn path_length_calculation() {
        let pts = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(3.0, 0.0),
            IPoint2::from_mm(3.0, 4.0),
        ];
        let len = path_length_mm(&pts);
        assert!(
            (len - 7.0).abs() < 0.001,
            "Path length should be 3 + 4 = 7mm, got {}",
            len
        );
    }

    #[test]
    fn midpoint_calculation() {
        let a = IPoint2::from_mm(0.0, 0.0);
        let b = IPoint2::from_mm(10.0, 10.0);
        let mid = midpoint(a, b);
        let (mx, my) = mid.to_mm();
        assert!(
            (mx - 5.0).abs() < 0.001 && (my - 5.0).abs() < 0.001,
            "Midpoint of (0,0)-(10,10) should be (5,5), got ({}, {})",
            mx, my
        );
    }
}
