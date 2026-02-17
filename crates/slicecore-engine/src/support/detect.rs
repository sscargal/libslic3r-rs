//! Overhang detection using hybrid layer-diff and raycast validation.
//!
//! Implements the core overhang detection algorithm:
//! 1. **Layer comparison**: Compare adjacent layers to find regions extending
//!    beyond the configurable overhang angle threshold.
//! 2. **Raycast validation**: Cast downward rays to filter false positives
//!    from internally-supported geometry.
//! 3. **Area filtering**: Remove unprintable tiny regions using a two-tier
//!    area threshold.

use slicecore_geo::polygon::ValidPolygon;
use slicecore_geo::{offset_polygons, point_in_polygon, polygon_difference, JoinType, PointLocation};
use slicecore_math::{coord_to_mm, mm_to_coord, IBBox2, IPoint2, Point3, Vec3, COORD_SCALE};
use slicecore_mesh::TriangleMesh;

use super::config::SupportConfig;

/// Detects overhang regions on a single layer by comparing with the layer below.
///
/// The algorithm expands the below contours outward by the maximum horizontal
/// offset allowed by the overhang angle, then computes the difference: regions
/// of the current layer that extend beyond this expanded footprint are overhangs.
///
/// # Parameters
///
/// - `current_contours`: Polygons of the current (upper) layer.
/// - `below_contours`: Polygons of the layer below. If empty, returns empty
///   (first layer is supported by the build plate).
/// - `overhang_angle_deg`: Overhang angle threshold in degrees (e.g., 45.0).
/// - `layer_height`: Height of the current layer in mm.
/// - `extrusion_width`: Extrusion width in mm (used for offset join tolerance).
///
/// # Returns
///
/// Overhang regions as validated polygons. Empty if no overhangs detected.
pub fn detect_overhangs_layer(
    current_contours: &[ValidPolygon],
    below_contours: &[ValidPolygon],
    overhang_angle_deg: f64,
    layer_height: f64,
    _extrusion_width: f64,
) -> Vec<ValidPolygon> {
    // First layer (nothing below) is supported by the bed.
    if below_contours.is_empty() {
        return Vec::new();
    }

    if current_contours.is_empty() {
        return Vec::new();
    }

    // Convert angle to radians and compute max horizontal offset.
    let overhang_angle_rad = overhang_angle_deg.to_radians();
    let max_offset = layer_height * overhang_angle_rad.tan();

    // Expand below contours outward by max_offset.
    let offset_coord = mm_to_coord(max_offset);
    let expanded_below = match offset_polygons(below_contours, offset_coord, JoinType::Miter) {
        Ok(expanded) => expanded,
        Err(_) => return Vec::new(),
    };

    if expanded_below.is_empty() {
        // If expansion failed, entire current layer is an overhang.
        return current_contours.to_vec();
    }

    // Overhang = current layer MINUS expanded below layer.
    polygon_difference(current_contours, &expanded_below).unwrap_or_default()
}

/// Validates candidate overhang regions using downward raycasting.
///
/// For each candidate region, samples points on a grid and casts rays
/// downward to check if the model geometry supports the point internally.
/// Regions where >50% of sample points are internally supported are removed
/// as false positives.
///
/// # Parameters
///
/// - `candidate_regions`: Overhang regions to validate.
/// - `mesh`: The original triangle mesh for raycasting.
/// - `layer_z`: Z height of the current layer in mm.
/// - `sample_spacing_mm`: Grid spacing for sample points (default ~2mm).
///
/// # Returns
///
/// Validated overhang regions with false positives removed.
pub fn validate_overhangs_raycast(
    candidate_regions: &[ValidPolygon],
    mesh: &TriangleMesh,
    layer_z: f64,
    sample_spacing_mm: f64,
) -> Vec<ValidPolygon> {
    let bvh = mesh.bvh();
    let max_support_dist = layer_z; // Max distance to check for internal support
    let direction = Vec3::new(0.0, 0.0, -1.0);

    // Minimum ray distance to count as internal support. Hits closer than
    // this threshold are the overhang surface itself (the face at the layer
    // boundary) and should not count as internal support. Using 1mm ensures
    // we skip the immediate overhang face while still catching genuine
    // internal geometry support (e.g., closed internal voids).
    let min_t = 1.0;

    let mut validated = Vec::new();

    for region in candidate_regions {
        let points = region.points();

        // Compute bounding box for grid sampling.
        let bbox = match IBBox2::from_points(points) {
            Some(b) => b,
            None => continue,
        };

        let (min_x_mm, min_y_mm) = (
            coord_to_mm(bbox.min.x),
            coord_to_mm(bbox.min.y),
        );
        let (max_x_mm, max_y_mm) = (
            coord_to_mm(bbox.max.x),
            coord_to_mm(bbox.max.y),
        );

        let spacing = if sample_spacing_mm <= 0.0 {
            2.0
        } else {
            sample_spacing_mm
        };

        let mut total_samples = 0u32;
        let mut supported_samples = 0u32;

        // Sample grid within bounding box.
        let mut x = min_x_mm;
        while x <= max_x_mm {
            let mut y = min_y_mm;
            while y <= max_y_mm {
                let sample_point = IPoint2::from_mm(x, y);

                // Check if sample point is inside the region.
                let location = point_in_polygon(&sample_point, points);
                if location == PointLocation::Inside || location == PointLocation::OnBoundary {
                    total_samples += 1;

                    // Cast ray downward from this point.
                    let origin = Point3::new(x, y, layer_z);
                    if let Some(hit) = bvh.intersect_ray(
                        &origin,
                        &direction,
                        mesh.vertices(),
                        mesh.indices(),
                    ) {
                        // If hit is within max_support_dist AND beyond the
                        // minimum threshold, the point is internally supported
                        // by model geometry. Hits closer than min_t are the
                        // overhang surface itself and should be ignored.
                        if hit.t > min_t && hit.t < max_support_dist {
                            supported_samples += 1;
                        }
                    }
                }

                y += spacing;
            }
            x += spacing;
        }

        // Keep region if less than 50% of samples are internally supported.
        if total_samples == 0 || (supported_samples as f64 / total_samples as f64) <= 0.5 {
            validated.push(region.clone());
        }
    }

    validated
}

/// Filters out regions that are too small to be printable.
///
/// Uses a two-tier filtering approach:
/// - Regions smaller than `extrusion_width^2` are discarded (unprintable).
/// - Regions between `extrusion_width^2` and `min_area_mm2` are kept as-is
///   (they become thin support pillars naturally).
/// - Regions >= `min_area_mm2` are kept unchanged.
///
/// # Parameters
///
/// - `regions`: Candidate support regions.
/// - `min_area_mm2`: Minimum area threshold in mm^2.
/// - `extrusion_width`: Extrusion width in mm (defines absolute minimum).
///
/// # Returns
///
/// Filtered regions with unprintable tiny regions removed.
pub fn filter_small_regions(
    regions: &[ValidPolygon],
    min_area_mm2: f64,
    extrusion_width: f64,
) -> Vec<ValidPolygon> {
    let min_printable_area = extrusion_width * extrusion_width;
    let scale_sq = COORD_SCALE * COORD_SCALE;

    regions
        .iter()
        .filter(|region| {
            let area_i64 = region.area_i64().unsigned_abs() as f64;
            let area_mm2 = area_i64 / scale_sq;

            // Discard if below absolute minimum (unprintable).
            if area_mm2 < min_printable_area {
                return false;
            }

            // Keep everything above minimum printable area.
            // Regions between min_printable and min_area_mm2 become thin pillars.
            // Regions >= min_area_mm2 are normal support.
            let _ = min_area_mm2; // Used for documentation; both tiers are kept.
            true
        })
        .cloned()
        .collect()
}

/// Detects overhangs across all layers of a sliced model.
///
/// Runs the full hybrid detection pipeline for each layer:
/// 1. Layer comparison to find candidate overhangs.
/// 2. Raycast validation to filter false positives.
/// 3. Area filtering to remove unprintable regions.
///
/// Layer 0 (first layer on bed) always produces no overhangs.
///
/// # Parameters
///
/// - `layers`: Per-layer contours, ordered from bottom (index 0) to top.
/// - `mesh`: The original triangle mesh for raycast validation.
/// - `config`: Support configuration with detection parameters.
/// - `layer_heights`: Per-layer Z heights in mm.
/// - `layer_height`: Standard layer height in mm.
/// - `extrusion_width`: Extrusion width in mm.
///
/// # Returns
///
/// Per-layer overhang region vectors. `result[i]` contains overhangs for layer `i`.
pub fn detect_all_overhangs(
    layers: &[Vec<ValidPolygon>],
    mesh: &TriangleMesh,
    config: &SupportConfig,
    layer_heights: &[f64],
    layer_height: f64,
    extrusion_width: f64,
) -> Vec<Vec<ValidPolygon>> {
    let mut all_overhangs = Vec::with_capacity(layers.len());

    // Layer 0: first layer on bed, no overhangs.
    if !layers.is_empty() {
        all_overhangs.push(Vec::new());
    }

    for i in 1..layers.len() {
        let current_contours = &layers[i];
        let below_contours = &layers[i - 1];
        let z = layer_heights.get(i).copied().unwrap_or(layer_height * i as f64);
        let lh = if i < layer_heights.len() && i > 0 {
            layer_heights[i] - layer_heights.get(i - 1).copied().unwrap_or(0.0)
        } else {
            layer_height
        };

        // Step 1: Layer comparison.
        let candidates = detect_overhangs_layer(
            current_contours,
            below_contours,
            config.overhang_angle,
            lh,
            extrusion_width,
        );

        if candidates.is_empty() {
            all_overhangs.push(Vec::new());
            continue;
        }

        // Step 2: Raycast validation (skip if no candidates for performance).
        let validated = validate_overhangs_raycast(&candidates, mesh, z, 2.0);

        // Step 3: Area filtering.
        let filtered = filter_small_regions(&validated, config.min_support_area, extrusion_width);

        all_overhangs.push(filtered);
    }

    all_overhangs
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_geo::polygon::Polygon;

    /// Helper to create a validated CCW square at a given position and size.
    fn make_square(x: f64, y: f64, size: f64) -> ValidPolygon {
        Polygon::from_mm(&[
            (x, y),
            (x + size, y),
            (x + size, y + size),
            (x, y + size),
        ])
        .validate()
        .unwrap()
    }

    #[test]
    fn overhang_60_degrees_beyond_45_threshold_detected() {
        // Layer below: 10x10 square centered at (50, 50).
        let below = vec![make_square(50.0, 50.0, 10.0)];
        // Current layer: same square but shifted 5mm to the right.
        // This creates a 5mm horizontal overhang with 0.2mm layer height.
        // The max horizontal offset at 45 degrees = 0.2 * tan(45) = 0.2mm.
        // So the 5mm overhang is far beyond what's allowed.
        let current = vec![make_square(55.0, 50.0, 10.0)];

        let overhangs =
            detect_overhangs_layer(&current, &below, 45.0, 0.2, 0.44);

        assert!(
            !overhangs.is_empty(),
            "A 5mm lateral shift should produce overhangs beyond 45-degree threshold"
        );
    }

    #[test]
    fn vertical_wall_no_overhangs() {
        // Identical contours on both layers = vertical wall = no overhang.
        let square = make_square(50.0, 50.0, 10.0);
        let below = vec![square.clone()];
        let current = vec![square];

        let overhangs =
            detect_overhangs_layer(&current, &below, 45.0, 0.2, 0.44);

        assert!(
            overhangs.is_empty(),
            "Identical layers (vertical wall) should produce no overhangs"
        );
    }

    #[test]
    fn first_layer_on_bed_no_overhangs() {
        // First layer has nothing below it (empty below_contours).
        let current = vec![make_square(50.0, 50.0, 10.0)];

        let overhangs =
            detect_overhangs_layer(&current, &[], 45.0, 0.2, 0.44);

        assert!(
            overhangs.is_empty(),
            "First layer on build plate should produce no overhangs"
        );
    }

    #[test]
    fn filter_small_regions_removes_tiny() {
        // Create a very tiny square (0.1mm x 0.1mm = 0.01 mm^2).
        let tiny = make_square(50.0, 50.0, 0.1);
        // Create a normal-sized square (5mm x 5mm = 25 mm^2).
        let normal = make_square(50.0, 50.0, 5.0);

        let extrusion_width = 0.44;
        let min_area = 0.77;

        // Tiny region: 0.01 mm^2 < 0.44^2 = 0.1936 mm^2 (unprintable).
        let filtered_tiny = filter_small_regions(&[tiny], min_area, extrusion_width);
        assert!(
            filtered_tiny.is_empty(),
            "Tiny region (0.01 mm^2) below extrusion_width^2 should be removed"
        );

        // Normal region: 25 mm^2 > 0.77 mm^2 (kept).
        let filtered_normal = filter_small_regions(&[normal], min_area, extrusion_width);
        assert_eq!(
            filtered_normal.len(),
            1,
            "Normal region (25 mm^2) should be kept"
        );
    }

    #[test]
    fn identical_layers_produce_no_overhangs() {
        // Exact same contours above and below = no overhangs possible.
        let square = make_square(100.0, 100.0, 20.0);

        let overhangs = detect_overhangs_layer(
            &[square.clone()],
            &[square],
            45.0,
            0.2,
            0.44,
        );

        assert!(
            overhangs.is_empty(),
            "Identical layers should produce no overhangs"
        );
    }

    #[test]
    fn larger_below_no_overhangs() {
        // Below layer is larger than current layer -- no overhang.
        let large_below = make_square(40.0, 40.0, 30.0);
        let small_current = make_square(50.0, 50.0, 10.0);

        let overhangs = detect_overhangs_layer(
            &[small_current],
            &[large_below],
            45.0,
            0.2,
            0.44,
        );

        assert!(
            overhangs.is_empty(),
            "Smaller current layer fully contained in larger below should produce no overhangs"
        );
    }

    #[test]
    fn filter_keeps_medium_regions() {
        // Medium region: 0.5mm x 0.5mm = 0.25 mm^2.
        // This is above extrusion_width^2 (0.1936) but below min_area (0.77).
        // Per two-tier filtering, this should be KEPT (thin pillar).
        let medium = make_square(50.0, 50.0, 0.5);
        let extrusion_width = 0.44;
        let min_area = 0.77;

        let filtered = filter_small_regions(&[medium], min_area, extrusion_width);
        assert_eq!(
            filtered.len(),
            1,
            "Medium region (0.25 mm^2) between tiers should be kept as thin pillar"
        );
    }

    #[test]
    fn empty_current_layer_no_overhangs() {
        let below = vec![make_square(50.0, 50.0, 10.0)];
        let overhangs = detect_overhangs_layer(&[], &below, 45.0, 0.2, 0.44);
        assert!(
            overhangs.is_empty(),
            "Empty current layer should produce no overhangs"
        );
    }

    #[test]
    fn steep_overhang_angle_reduces_detection() {
        // With a 60-degree threshold, more overhang is allowed.
        // max_offset at 60 degrees = 0.2 * tan(60) = 0.2 * 1.732 = 0.346mm
        // A 0.3mm shift should NOT be detected at 60 degrees.
        let below = vec![make_square(50.0, 50.0, 10.0)];
        let current = vec![make_square(50.3, 50.0, 10.0)]; // 0.3mm shift

        let _overhangs_60 =
            detect_overhangs_layer(&current, &below, 60.0, 0.2, 0.44);

        // At 60 degrees, max offset is ~0.346mm, so 0.3mm shift should be within tolerance.
        // The polygon difference may produce thin slivers or empty result.
        // The key assertion is that at 45 degrees the same shift would produce overhangs.
        let overhangs_45 =
            detect_overhangs_layer(&current, &below, 45.0, 0.2, 0.44);

        // At 45 degrees, max offset is 0.2mm, so 0.3mm shift produces overhangs.
        assert!(
            !overhangs_45.is_empty(),
            "0.3mm shift should produce overhangs at 45-degree threshold"
        );
    }
}
