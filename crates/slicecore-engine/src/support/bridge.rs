//! Bridge detection and classification for unsupported horizontal spans.
//!
//! Bridges are horizontal regions where material must span an unsupported gap
//! between two supported sides. Bridge detection uses a combined three-criteria
//! approach (per user decision):
//!
//! 1. **Angle threshold**: The region is near-horizontal (overhang angle >= 80 degrees
//!    from vertical, i.e., within 10 degrees of horizontal).
//! 2. **Endpoint support**: The region has supported material on at least two opposing
//!    sides (both ends of the span are anchored).
//! 3. **Minimum span length**: The unsupported gap is at least `min_span_mm` wide
//!    (default 5mm) to avoid classifying short overhangs as bridges.
//!
//! All three criteria must be met to classify a region as a bridge. This avoids
//! false positives from tiny overhangs, single-sided overhangs, and steep angles.

use serde::{Deserialize, Serialize};
use slicecore_geo::polygon::ValidPolygon;
use slicecore_geo::{offset_polygons, polygon_intersection, JoinType};
use slicecore_math::{coord_to_mm, mm_to_coord, IBBox2, IPoint2};

/// A detected bridge region with span metadata.
///
/// Bridge regions receive special treatment during printing: slower speed,
/// higher fan, reduced flow, and infill lines perpendicular to the span.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BridgeRegion {
    /// The bridge region polygon.
    pub contour: ValidPolygon,
    /// Angle in radians of the primary bridge span direction.
    ///
    /// This is the direction across the unsupported gap (perpendicular to
    /// the supported edges). Bridge infill lines should run perpendicular
    /// to this direction.
    pub span_direction: f64,
    /// Estimated length of the longest unsupported span in mm.
    pub span_length: f64,
    /// Layer index where this bridge was detected.
    pub layer_index: usize,
    /// Z height in mm.
    pub z: f64,
}

/// Checks whether a single overhang region qualifies as a bridge.
///
/// A region is a bridge candidate if all three criteria are met:
/// 1. It exists as an overhang (implicitly near-horizontal in a layer-diff context).
/// 2. The layer below has support on at least two opposing sides of the region.
/// 3. The unsupported span is at least `min_span_mm` wide.
///
/// # Parameters
///
/// - `region`: The overhang region polygon to evaluate.
/// - `below_contours`: Polygons from the layer below (potential support).
/// - `min_span_mm`: Minimum span length in mm to qualify as a bridge.
///
/// # Returns
///
/// `Some(BridgeRegion)` if all criteria are met, `None` otherwise.
/// The returned `BridgeRegion` has `layer_index` and `z` set to 0 -- the
/// caller should fill in the correct values.
pub fn is_bridge_candidate(
    region: &ValidPolygon,
    below_contours: &[ValidPolygon],
    min_span_mm: f64,
) -> Option<BridgeRegion> {
    if below_contours.is_empty() {
        return None;
    }

    let pts = region.points();
    if pts.len() < 3 {
        return None;
    }

    // Compute bounding box of the region.
    let bbox = IBBox2::from_points(pts)?;

    let bbox_width = coord_to_mm(bbox.max.x - bbox.min.x);
    let bbox_height = coord_to_mm(bbox.max.y - bbox.min.y);

    // Criterion 1 -- Angle threshold:
    // Since we are working with 2D slices, overhang regions passed to us are
    // already detected as overhangs (near-horizontal surfaces). The layer-diff
    // algorithm only produces overhangs beyond the overhang angle threshold.
    // For bridge classification within already-detected overhangs, we check
    // that the region is plausibly a horizontal span: it must have a meaningful
    // 2D extent (not a degenerate sliver). This is implicitly satisfied if the
    // region passes the min span and endpoint criteria.

    // Criterion 3 -- Minimum span length:
    // The span must be at least min_span_mm. We estimate span as the shorter
    // dimension of the bounding box (the gap width, not the length along the
    // supported edges).
    let (span_length, span_direction) = if bbox_width <= bbox_height {
        // Narrower in X -> span crosses in X direction, supported edges run along Y.
        (bbox_width, 0.0_f64) // 0 radians = horizontal (X direction)
    } else {
        // Narrower in Y -> span crosses in Y direction, supported edges run along X.
        (bbox_height, std::f64::consts::FRAC_PI_2) // PI/2 radians = vertical (Y direction)
    };

    if span_length < min_span_mm {
        return None;
    }

    // Criterion 2 -- Endpoint support:
    // Check that the below_contours support the region on at least two opposing
    // sides. We do this by expanding the below_contours slightly and checking
    // intersection with thin strips along the region's opposing edges.
    let expand_delta = mm_to_coord(0.5); // 0.5mm expansion tolerance
    let expanded_below = match offset_polygons(below_contours, expand_delta, JoinType::Miter) {
        Ok(expanded) if !expanded.is_empty() => expanded,
        _ => return None,
    };

    // Create thin probe strips on opposing sides of the bounding box.
    let strip_thickness = mm_to_coord(0.3); // thin probe strip

    let (has_side_a, has_side_b) = if span_direction == 0.0 {
        // Span is in X direction. Check for support on left (min_x) and right (max_x).
        let left_strip = make_strip(
            bbox.min.x - strip_thickness,
            bbox.min.y,
            bbox.min.x + strip_thickness,
            bbox.max.y,
        );
        let right_strip = make_strip(
            bbox.max.x - strip_thickness,
            bbox.min.y,
            bbox.max.x + strip_thickness,
            bbox.max.y,
        );
        let left_hit = check_intersection(&expanded_below, &left_strip);
        let right_hit = check_intersection(&expanded_below, &right_strip);
        (left_hit, right_hit)
    } else {
        // Span is in Y direction. Check for support on bottom (min_y) and top (max_y).
        let bottom_strip = make_strip(
            bbox.min.x,
            bbox.min.y - strip_thickness,
            bbox.max.x,
            bbox.min.y + strip_thickness,
        );
        let top_strip = make_strip(
            bbox.min.x,
            bbox.max.y - strip_thickness,
            bbox.max.x,
            bbox.max.y + strip_thickness,
        );
        let bottom_hit = check_intersection(&expanded_below, &bottom_strip);
        let top_hit = check_intersection(&expanded_below, &top_strip);
        (bottom_hit, top_hit)
    };

    if !has_side_a || !has_side_b {
        return None;
    }

    // All three criteria met.
    Some(BridgeRegion {
        contour: region.clone(),
        span_direction,
        span_length,
        layer_index: 0,
        z: 0.0,
    })
}

/// Detects bridge regions among overhang regions and separates them from
/// regular overhangs.
///
/// # Parameters
///
/// - `overhang_regions`: Overhang regions detected on this layer.
/// - `below_contours`: Polygons from the layer below.
/// - `layer_index`: Index of the current layer.
/// - `z`: Z height of the current layer in mm.
/// - `min_span_mm`: Minimum span length to qualify as a bridge.
///
/// # Returns
///
/// A tuple: `(bridges, remaining_overhangs)`.
/// - `bridges`: Overhang regions that qualify as bridges.
/// - `remaining_overhangs`: Overhang regions that do NOT qualify as bridges.
pub fn detect_bridges(
    overhang_regions: &[ValidPolygon],
    below_contours: &[ValidPolygon],
    layer_index: usize,
    z: f64,
    min_span_mm: f64,
) -> (Vec<BridgeRegion>, Vec<ValidPolygon>) {
    let mut bridges = Vec::new();
    let mut non_bridges = Vec::new();

    for region in overhang_regions {
        if let Some(mut bridge) = is_bridge_candidate(region, below_contours, min_span_mm) {
            bridge.layer_index = layer_index;
            bridge.z = z;
            bridges.push(bridge);
        } else {
            non_bridges.push(region.clone());
        }
    }

    (bridges, non_bridges)
}

/// Computes the optimal infill angle for a bridge region.
///
/// Bridge infill lines should run perpendicular to the span direction so
/// that each individual line crosses the shortest possible unsupported
/// distance.
///
/// # Returns
///
/// Angle in radians, perpendicular to `bridge.span_direction`.
pub fn compute_bridge_infill_angle(bridge: &BridgeRegion) -> f64 {
    bridge.span_direction + std::f64::consts::FRAC_PI_2
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Creates a thin rectangular strip as a `ValidPolygon`.
///
/// Used as a probe to check for support material on a specific side
/// of a bridge region.
fn make_strip(min_x: i64, min_y: i64, max_x: i64, max_y: i64) -> Option<ValidPolygon> {
    use slicecore_geo::polygon::Polygon;

    // Ensure the strip has non-zero area.
    if max_x <= min_x || max_y <= min_y {
        return None;
    }

    let poly = Polygon::new(vec![
        IPoint2::new(min_x, min_y),
        IPoint2::new(max_x, min_y),
        IPoint2::new(max_x, max_y),
        IPoint2::new(min_x, max_y),
    ]);

    poly.validate().ok()
}

/// Checks whether any of the `polygons` intersect with the `probe` strip.
fn check_intersection(polygons: &[ValidPolygon], probe: &Option<ValidPolygon>) -> bool {
    let probe = match probe {
        Some(p) => p,
        None => return false,
    };

    match polygon_intersection(polygons, std::slice::from_ref(probe)) {
        Ok(result) => !result.is_empty(),
        Err(_) => false,
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
    fn make_rect(x: f64, y: f64, w: f64, h: f64) -> ValidPolygon {
        Polygon::from_mm(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)])
            .validate()
            .unwrap()
    }

    #[test]
    fn rectangular_gap_between_two_walls_detected_as_bridge() {
        // Two walls on left and right, with a bridge region spanning between them.
        //
        // Wall A: x=[0,5], y=[0,20]     Bridge: x=[5,15], y=[0,20]     Wall B: x=[15,20], y=[0,20]
        //
        // The bridge region is 10mm wide (X span) between two supported walls.
        let bridge_region = make_rect(5.0, 0.0, 10.0, 20.0);
        let wall_a = make_rect(0.0, 0.0, 5.0, 20.0);
        let wall_b = make_rect(15.0, 0.0, 5.0, 20.0);

        let below_contours = vec![wall_a, wall_b];

        let result = is_bridge_candidate(&bridge_region, &below_contours, 5.0);
        assert!(
            result.is_some(),
            "Rectangular gap between two walls should be detected as bridge"
        );

        let bridge = result.unwrap();
        assert!(
            (bridge.span_length - 10.0).abs() < 0.1,
            "Span length should be ~10mm, got {}",
            bridge.span_length
        );
        // Span direction should be 0.0 (horizontal / X direction) since X is the shorter bbox dimension.
        assert!(
            bridge.span_direction.abs() < 0.01,
            "Span direction should be ~0.0 (X), got {}",
            bridge.span_direction
        );
    }

    #[test]
    fn short_gap_below_min_span_not_bridge() {
        // A narrow 3mm gap -- below the 5mm minimum span.
        let narrow_region = make_rect(5.0, 0.0, 3.0, 20.0);
        let wall_a = make_rect(0.0, 0.0, 5.0, 20.0);
        let wall_b = make_rect(8.0, 0.0, 5.0, 20.0);

        let below_contours = vec![wall_a, wall_b];

        let result = is_bridge_candidate(&narrow_region, &below_contours, 5.0);
        assert!(
            result.is_none(),
            "Gap shorter than min_span_mm should NOT be classified as bridge"
        );
    }

    #[test]
    fn single_side_support_not_bridge() {
        // Support only on the left side, nothing on the right.
        let bridge_region = make_rect(5.0, 0.0, 10.0, 20.0);
        let wall_a = make_rect(0.0, 0.0, 5.0, 20.0);
        // No wall_b -- single-sided support.

        let below_contours = vec![wall_a];

        let result = is_bridge_candidate(&bridge_region, &below_contours, 5.0);
        assert!(
            result.is_none(),
            "Region with support only on one side should NOT be a bridge"
        );
    }

    #[test]
    fn bridge_infill_angle_perpendicular_to_span() {
        let bridge = BridgeRegion {
            contour: make_rect(0.0, 0.0, 10.0, 20.0),
            span_direction: 0.0, // X direction
            span_length: 10.0,
            layer_index: 5,
            z: 1.0,
        };

        let angle = compute_bridge_infill_angle(&bridge);
        // Should be PI/2 (perpendicular to X = Y direction).
        assert!(
            (angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
            "Bridge infill angle should be PI/2 for X-span, got {}",
            angle
        );

        let bridge_y = BridgeRegion {
            contour: make_rect(0.0, 0.0, 20.0, 10.0),
            span_direction: std::f64::consts::FRAC_PI_2, // Y direction
            span_length: 10.0,
            layer_index: 5,
            z: 1.0,
        };

        let angle_y = compute_bridge_infill_angle(&bridge_y);
        // Should be PI (perpendicular to Y = horizontal X direction + PI/2).
        assert!(
            (angle_y - std::f64::consts::PI).abs() < 1e-9,
            "Bridge infill angle should be PI for Y-span, got {}",
            angle_y
        );
    }

    #[test]
    fn detect_bridges_separates_bridges_from_overhangs() {
        // Two overhang regions: one is a bridge (between two walls), one is not (single-sided).
        let bridge_region = make_rect(5.0, 0.0, 10.0, 20.0);
        let overhang_region = make_rect(25.0, 0.0, 10.0, 20.0);

        let wall_a = make_rect(0.0, 0.0, 5.0, 20.0);
        let wall_b = make_rect(15.0, 0.0, 5.0, 20.0);
        // wall_b is only adjacent to bridge_region, not overhang_region.

        let below_contours = vec![wall_a, wall_b];
        let overhangs = vec![bridge_region, overhang_region];

        let (bridges, non_bridges) = detect_bridges(&overhangs, &below_contours, 5, 1.0, 5.0);

        assert_eq!(bridges.len(), 1, "Should detect exactly 1 bridge");
        assert_eq!(non_bridges.len(), 1, "Should have exactly 1 non-bridge overhang");

        assert_eq!(bridges[0].layer_index, 5, "Bridge layer_index should be set");
        assert!((bridges[0].z - 1.0).abs() < 1e-9, "Bridge z should be set");
    }

    #[test]
    fn no_below_contours_no_bridges() {
        let region = make_rect(5.0, 0.0, 10.0, 20.0);
        let result = is_bridge_candidate(&region, &[], 5.0);
        assert!(
            result.is_none(),
            "No below contours should mean no bridge"
        );
    }

    #[test]
    fn y_direction_bridge_detected() {
        // Bridge spanning in the Y direction (taller than wide gap).
        // Walls on top and bottom, gap in the middle.
        let bridge_region = make_rect(0.0, 5.0, 20.0, 10.0);
        let wall_top = make_rect(0.0, 15.0, 20.0, 5.0);
        let wall_bottom = make_rect(0.0, 0.0, 20.0, 5.0);

        let below_contours = vec![wall_top, wall_bottom];

        let result = is_bridge_candidate(&bridge_region, &below_contours, 5.0);
        assert!(
            result.is_some(),
            "Y-direction bridge should be detected"
        );

        let bridge = result.unwrap();
        // Span direction should be PI/2 (Y direction) since Y is the shorter bbox dimension.
        assert!(
            (bridge.span_direction - std::f64::consts::FRAC_PI_2).abs() < 0.01,
            "Span direction should be ~PI/2 (Y), got {}",
            bridge.span_direction
        );
        assert!(
            (bridge.span_length - 10.0).abs() < 0.1,
            "Span length should be ~10mm, got {}",
            bridge.span_length
        );
    }
}
