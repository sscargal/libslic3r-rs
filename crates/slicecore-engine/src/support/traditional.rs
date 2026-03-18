//! Traditional grid/line support structure generation.
//!
//! Generates printable support structures from detected overhang regions using
//! traditional grid or line patterns. Supports project downward to the build
//! plate and include proper XY/Z gaps for clean removal.
//!
//! # Pipeline
//!
//! 1. **Projection**: Overhang regions are projected downward through each layer
//!    to the build plate (or first model surface). At each layer, support is
//!    clipped away from the model interior via polygon difference.
//! 2. **XY gap**: Support regions are inset by the configured XY gap to prevent
//!    support from fusing to model walls.
//! 3. **Infill generation**: Sparse infill lines are generated within the
//!    gap-adjusted support regions using the configured pattern (line, grid,
//!    or rectilinear).

use slicecore_geo::polygon::ValidPolygon;
use slicecore_geo::{offset_polygons, polygon_difference, polygon_union, JoinType};
use slicecore_math::mm_to_coord;
use slicecore_slicer::SliceLayer;

use super::config::{SupportConfig, SupportPattern};
use super::SupportRegion;
use crate::infill::{self, InfillLine, InfillPattern};

/// Projects overhang regions downward through the layer stack.
///
/// For each layer with overhangs, projects the overhang region downward
/// through all layers below it to the build plate. At each layer below,
/// the support cross-section is the overhang region minus the model
/// contours at that layer (to avoid support inside the model).
///
/// # Parameters
///
/// - `overhang_regions_per_layer`: Per-layer overhang regions from detection.
///   `overhang_regions_per_layer[i]` contains overhangs at layer `i`.
/// - `layers`: The sliced model layers with contours.
/// - `build_plate_only`: If true, only support that reaches layer 0 is kept.
///   If false, support can rest on model surfaces (not yet implemented; acts
///   as build-plate-only for now).
///
/// # Returns
///
/// Per-layer support regions. `result[i]` contains the union of all projected
/// support at layer `i`.
pub fn project_support_regions(
    overhang_regions_per_layer: &[Vec<ValidPolygon>],
    layers: &[SliceLayer],
    _build_plate_only: bool,
) -> Vec<Vec<ValidPolygon>> {
    let n = layers.len();
    if n == 0 {
        return Vec::new();
    }

    // Accumulate support regions per layer.
    let mut support_per_layer: Vec<Vec<ValidPolygon>> = vec![Vec::new(); n];

    // For each layer that has overhangs, project downward.
    for (layer_idx, overhangs) in overhang_regions_per_layer.iter().enumerate().take(n) {
        if overhangs.is_empty() {
            continue;
        }

        // Project these overhangs downward from layer_idx-1 to layer 0.
        // (The overhang is at layer_idx; support goes from layer_idx-1 down to 0.)
        let mut current_support = overhangs.clone();

        // Start from the layer just below the overhang and go downward.
        let start = if layer_idx > 0 {
            layer_idx - 1
        } else {
            continue;
        };

        for below_idx in (0..=start).rev() {
            if current_support.is_empty() {
                break;
            }

            // Clip support away from model contours at this layer.
            let model_contours = &layers[below_idx].contours;
            if !model_contours.is_empty() {
                current_support =
                    polygon_difference(&current_support, model_contours).unwrap_or_default();
            }

            if current_support.is_empty() {
                break;
            }

            // Add this support to the layer's support regions.
            // Union with any existing support at this layer.
            if support_per_layer[below_idx].is_empty() {
                support_per_layer[below_idx] = current_support.clone();
            } else {
                let merged = polygon_union(&support_per_layer[below_idx], &current_support)
                    .unwrap_or_default();
                if merged.is_empty() {
                    // If union fails, just concatenate.
                    support_per_layer[below_idx].extend(current_support.clone());
                } else {
                    support_per_layer[below_idx] = merged;
                }
            }
        }
    }

    support_per_layer
}

/// Applies XY gap to support regions to create clearance from model walls.
///
/// Inward-offsets support regions by `xy_gap_mm` and additionally subtracts
/// expanded model contours to ensure clearance on all sides.
///
/// # Parameters
///
/// - `support_regions`: Support regions at a single layer.
/// - `model_contours`: Model contours at the same layer.
/// - `xy_gap_mm`: XY gap distance in mm.
///
/// # Returns
///
/// Gap-adjusted support regions. Regions that collapse to empty after
/// offsetting are filtered out.
pub fn apply_xy_gap(
    support_regions: &[ValidPolygon],
    model_contours: &[ValidPolygon],
    xy_gap_mm: f64,
) -> Vec<ValidPolygon> {
    if support_regions.is_empty() || xy_gap_mm <= 0.0 {
        return support_regions.to_vec();
    }

    let gap_coord = mm_to_coord(xy_gap_mm);

    // Inward-offset support regions by xy_gap.
    let offset_support = match offset_polygons(support_regions, -gap_coord, JoinType::Miter) {
        Ok(result) => result,
        Err(_) => return Vec::new(),
    };

    if offset_support.is_empty() {
        return Vec::new();
    }

    // Additionally, expand model contours by xy_gap and subtract from support.
    if model_contours.is_empty() {
        return offset_support;
    }

    let expanded_model = match offset_polygons(model_contours, gap_coord, JoinType::Miter) {
        Ok(result) => result,
        Err(_) => return offset_support,
    };

    if expanded_model.is_empty() {
        return offset_support;
    }

    polygon_difference(&offset_support, &expanded_model).unwrap_or(offset_support)
}

/// Generates support infill lines within support regions.
///
/// Uses the configured pattern and density to generate sparse infill lines
/// appropriate for support material.
///
/// # Parameters
///
/// - `support_regions`: Support regions at a single layer.
/// - `density`: Support body density as a fraction (0.0 - 1.0).
/// - `pattern`: The support fill pattern.
/// - `layer_index`: Current layer index (for angle alternation in grid/rectilinear).
/// - `extrusion_width`: Extrusion width in mm.
///
/// # Returns
///
/// Infill lines for this layer's support region.
pub fn generate_support_infill(
    support_regions: &[ValidPolygon],
    density: f64,
    pattern: SupportPattern,
    layer_index: usize,
    extrusion_width: f64,
) -> Vec<InfillLine> {
    if support_regions.is_empty() || density <= 0.0 {
        return Vec::new();
    }

    match pattern {
        SupportPattern::Line => {
            // Single-direction parallel lines at 0 degrees (no alternating).
            // Easy peeling direction.
            infill::rectilinear::generate(support_regions, density, 0.0, extrusion_width)
        }
        SupportPattern::Grid => {
            // Cross-hatched grid using the Grid infill dispatch.
            infill::generate_infill(
                &InfillPattern::Grid,
                support_regions,
                density,
                layer_index,
                0.0, // layer_z not used by grid
                extrusion_width,
                None,
            )
        }
        SupportPattern::Rectilinear => {
            // Alternating 0/90 rectilinear using the Rectilinear infill dispatch.
            infill::generate_infill(
                &InfillPattern::Rectilinear,
                support_regions,
                density,
                layer_index,
                0.0, // layer_z not used by rectilinear
                extrusion_width,
                None,
            )
        }
        SupportPattern::Honeycomb => {
            // Honeycomb pattern via the Honeycomb infill dispatch.
            infill::generate_infill(
                &InfillPattern::Honeycomb,
                support_regions,
                density,
                layer_index,
                0.0,
                extrusion_width,
                None,
            )
        }
        SupportPattern::Lightning => {
            // Lightning pattern via the Lightning infill dispatch.
            infill::generate_infill(
                &InfillPattern::Lightning,
                support_regions,
                density,
                layer_index,
                0.0,
                extrusion_width,
                None,
            )
        }
    }
}

/// Main entry point for traditional support structure generation.
///
/// Runs the full traditional support pipeline:
/// 1. Project overhang regions downward through the layer stack.
/// 2. Apply XY gap at each layer.
/// 3. Generate support infill per layer.
/// 4. Package results as per-layer `SupportRegion` vectors.
///
/// # Parameters
///
/// - `overhang_regions`: Per-layer overhang regions from detection.
/// - `layers`: The sliced model layers with contours.
/// - `config`: Support configuration with generation parameters.
/// - `extrusion_width`: Extrusion width in mm.
///
/// # Returns
///
/// Per-layer support region vectors. `result[i]` contains support regions
/// for layer `i`.
pub fn generate_traditional_supports(
    overhang_regions: &[Vec<ValidPolygon>],
    layers: &[SliceLayer],
    config: &SupportConfig,
    extrusion_width: f64,
) -> Vec<Vec<SupportRegion>> {
    let n = layers.len();
    if n == 0 {
        return Vec::new();
    }

    // Step 1: Project overhang regions downward.
    let projected = project_support_regions(overhang_regions, layers, config.build_plate_only);

    // Step 2 & 3: Apply XY gap and generate infill per layer.
    let mut result: Vec<Vec<SupportRegion>> = Vec::with_capacity(n);

    for (layer_idx, support_regions) in projected.into_iter().enumerate() {
        if support_regions.is_empty() {
            result.push(Vec::new());
            continue;
        }

        let model_contours = &layers[layer_idx].contours;

        // Apply XY gap.
        let gapped = apply_xy_gap(&support_regions, model_contours, config.xy_gap);

        if gapped.is_empty() {
            result.push(Vec::new());
            continue;
        }

        // Generate support infill.
        let infill_lines = generate_support_infill(
            &gapped,
            config.support_density,
            config.support_pattern,
            layer_idx,
            extrusion_width,
        );

        let z = layers[layer_idx].z;

        result.push(vec![SupportRegion {
            contours: gapped,
            z,
            layer_index: layer_idx,
            is_bridge: false,
            infill: infill_lines,
        }]);
    }

    result
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
        Polygon::from_mm(&[(x, y), (x + size, y), (x + size, y + size), (x, y + size)])
            .validate()
            .unwrap()
    }

    /// Helper to create a SliceLayer with the given contours.
    fn make_layer(z: f64, contours: Vec<ValidPolygon>) -> SliceLayer {
        SliceLayer {
            z,
            layer_height: 0.2,
            contours,
        }
    }

    #[test]
    fn support_projects_from_overhang_layer_to_layer_0() {
        // 6 layers: layer 0-4 have a 10x10 square at (50,50).
        // Layer 5 has a 10x10 square shifted right to (55,50) creating overhang.
        let base_square = make_square(50.0, 50.0, 10.0);
        let layers: Vec<SliceLayer> = (0..6)
            .map(|i| make_layer(0.2 * (i as f64 + 0.5), vec![base_square.clone()]))
            .collect();

        // Overhang is at layer 5 (shifted square).
        let overhang_square = make_square(55.0, 50.0, 10.0);
        let mut overhang_regions = vec![Vec::new(); 6];
        overhang_regions[5] = vec![overhang_square];

        let projected = project_support_regions(&overhang_regions, &layers, true);

        assert_eq!(
            projected.len(),
            6,
            "Should have 6 layers of projected support"
        );

        // Layer 5 itself should NOT have support (support goes below overhang).
        assert!(
            projected[5].is_empty(),
            "Overhang layer itself should not have support below it added to itself"
        );

        // Layers 0-4 should have support in the region outside the model.
        // The overhang extends from x=55 to x=65 but the model occupies x=50 to x=60.
        // So the support region is the overhang (55-65) minus the model (50-60) = roughly 60-65.
        let has_support_at_layer_0 = !projected[0].is_empty();
        assert!(
            has_support_at_layer_0,
            "Support should project down to layer 0"
        );

        // All intermediate layers should also have support.
        for i in 0..5 {
            assert!(
                !projected[i].is_empty(),
                "Layer {} should have projected support",
                i
            );
        }
    }

    #[test]
    fn xy_gap_makes_support_smaller_than_overhang() {
        let support = vec![make_square(50.0, 50.0, 10.0)];
        let model = vec![make_square(40.0, 50.0, 10.0)]; // Adjacent model at left

        let original_area: f64 = support.iter().map(|p| p.area_mm2()).sum();

        let gapped = apply_xy_gap(&support, &model, 0.4);

        let gapped_area: f64 = gapped.iter().map(|p| p.area_mm2()).sum();

        assert!(
            gapped_area < original_area,
            "XY gap should reduce support area: original={}, gapped={}",
            original_area,
            gapped_area
        );

        assert!(
            !gapped.is_empty(),
            "Support should not collapse entirely with 0.4mm gap on 10mm square"
        );
    }

    #[test]
    fn support_infill_lines_generated_within_regions() {
        let region = vec![make_square(50.0, 50.0, 10.0)];

        let lines = generate_support_infill(&region, 0.15, SupportPattern::Line, 0, 0.4);

        assert!(
            !lines.is_empty(),
            "Support infill should generate lines for a 10mm square at 15% density"
        );

        // All lines should be within the bounding box of the region (roughly).
        let min_coord = slicecore_math::mm_to_coord(50.0);
        let max_coord = slicecore_math::mm_to_coord(60.0);

        for line in &lines {
            assert!(
                line.start.x >= min_coord && line.start.x <= max_coord,
                "Infill line start x out of bounds"
            );
            assert!(
                line.end.x >= min_coord && line.end.x <= max_coord,
                "Infill line end x out of bounds"
            );
        }
    }

    #[test]
    fn support_does_not_overlap_model_contours() {
        // Model is a 20x20 square at (50,50). Overhang extends 10mm to the right.
        let model_square = make_square(50.0, 50.0, 20.0);
        let overhang = make_square(60.0, 50.0, 20.0); // Extends past model to the right

        let layers: Vec<SliceLayer> = (0..4)
            .map(|i| make_layer(0.2 * (i as f64 + 0.5), vec![model_square.clone()]))
            .collect();

        let mut overhang_regions = vec![Vec::new(); 4];
        overhang_regions[3] = vec![overhang];

        let projected = project_support_regions(&overhang_regions, &layers, true);

        // Support at layer 0 should not overlap with model contours.
        if !projected[0].is_empty() {
            // Intersect support with model -- should be empty.
            let overlap =
                slicecore_geo::polygon_intersection(&projected[0], &[model_square.clone()])
                    .unwrap_or_default();

            let overlap_area: f64 = overlap.iter().map(|p| p.area_mm2()).sum();
            assert!(
                overlap_area < 0.01,
                "Support should not overlap model contours, overlap area = {} mm^2",
                overlap_area
            );
        }
    }

    #[test]
    fn build_plate_only_projects_to_layer_0() {
        let model_square = make_square(50.0, 50.0, 10.0);
        let overhang = make_square(55.0, 50.0, 10.0);

        let layers: Vec<SliceLayer> = (0..6)
            .map(|i| make_layer(0.2 * (i as f64 + 0.5), vec![model_square.clone()]))
            .collect();

        let mut overhang_regions = vec![Vec::new(); 6];
        overhang_regions[5] = vec![overhang];

        let projected = project_support_regions(&overhang_regions, &layers, true);

        // Layer 0 should have support (projects all the way down).
        assert!(
            !projected[0].is_empty(),
            "Build-plate-only support should reach layer 0"
        );
    }

    #[test]
    fn generate_traditional_supports_end_to_end() {
        let model_square = make_square(50.0, 50.0, 10.0);
        let overhang = make_square(55.0, 50.0, 10.0);

        let layers: Vec<SliceLayer> = (0..6)
            .map(|i| make_layer(0.2 * (i as f64 + 0.5), vec![model_square.clone()]))
            .collect();

        let mut overhang_regions = vec![Vec::new(); 6];
        overhang_regions[5] = vec![overhang];

        let config = SupportConfig {
            enabled: true,
            support_density: 0.15,
            support_pattern: SupportPattern::Line,
            xy_gap: 0.4,
            build_plate_only: true,
            ..Default::default()
        };

        let result = generate_traditional_supports(&overhang_regions, &layers, &config, 0.4);

        assert_eq!(result.len(), 6, "Should have 6 layers of results");

        // At least some layers should have support regions with infill.
        let layers_with_support: Vec<usize> = result
            .iter()
            .enumerate()
            .filter(|(_, regions)| !regions.is_empty())
            .map(|(i, _)| i)
            .collect();

        assert!(
            !layers_with_support.is_empty(),
            "Should have at least some layers with support"
        );

        // Check that support regions have infill lines.
        for layer_idx in &layers_with_support {
            for region in &result[*layer_idx] {
                assert!(
                    !region.contours.is_empty(),
                    "Support region at layer {} should have contours",
                    layer_idx
                );
                assert!(
                    !region.infill.is_empty(),
                    "Support region at layer {} should have infill lines",
                    layer_idx
                );
            }
        }
    }

    #[test]
    fn grid_pattern_generates_infill() {
        let region = vec![make_square(50.0, 50.0, 10.0)];

        let lines = generate_support_infill(&region, 0.15, SupportPattern::Grid, 0, 0.4);

        assert!(
            !lines.is_empty(),
            "Grid pattern should generate infill lines"
        );
    }

    #[test]
    fn rectilinear_pattern_generates_infill() {
        let region = vec![make_square(50.0, 50.0, 10.0)];

        let lines = generate_support_infill(&region, 0.15, SupportPattern::Rectilinear, 0, 0.4);

        assert!(
            !lines.is_empty(),
            "Rectilinear pattern should generate infill lines"
        );
    }

    #[test]
    fn empty_overhangs_produce_no_support() {
        let model_square = make_square(50.0, 50.0, 10.0);
        let layers: Vec<SliceLayer> = (0..4)
            .map(|i| make_layer(0.2 * (i as f64 + 0.5), vec![model_square.clone()]))
            .collect();

        let overhang_regions = vec![Vec::new(); 4]; // No overhangs anywhere.

        let projected = project_support_regions(&overhang_regions, &layers, true);

        for (i, layer_support) in projected.iter().enumerate() {
            assert!(
                layer_support.is_empty(),
                "Layer {} should have no support when there are no overhangs",
                i
            );
        }
    }

    #[test]
    fn xy_gap_zero_returns_original() {
        let support = vec![make_square(50.0, 50.0, 10.0)];
        let model = vec![make_square(40.0, 50.0, 10.0)];

        let result = apply_xy_gap(&support, &model, 0.0);

        assert_eq!(
            result.len(),
            support.len(),
            "Zero XY gap should return original support regions"
        );
    }

    #[test]
    fn xy_gap_collapses_tiny_support() {
        // A very small support region (0.5mm x 0.5mm) with a 0.4mm gap should nearly or fully collapse.
        let tiny_support = vec![make_square(50.0, 50.0, 0.5)];
        let model: Vec<ValidPolygon> = Vec::new();

        let result = apply_xy_gap(&tiny_support, &model, 0.4);

        // 0.5mm square with 0.4mm inward offset on each side = fully collapsed.
        assert!(
            result.is_empty(),
            "Tiny support (0.5mm) with 0.4mm gap should collapse to empty"
        );
    }
}
