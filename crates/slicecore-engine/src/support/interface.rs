//! Support interface layer generation.
//!
//! Interface layers are dense contact layers placed between the support body
//! and the part surface. They produce better surface finish on supported faces
//! while remaining removable. This module provides:
//!
//! - **Material defaults**: Per-material Z-gap and XY-gap presets.
//! - **Interface identification**: Marks the topmost/bottommost N layers of each
//!   support column as interface layers.
//! - **Z-gap application**: Removes the topmost support layers to create a
//!   physical gap between support top and model bottom.
//! - **Interface infill**: Generates dense infill for interface layers using
//!   configurable patterns (rectilinear, grid, concentric).
//! - **Quality presets**: Adjusts multiple support parameters at once.

use serde::{Deserialize, Serialize};
use slicecore_geo::polygon::ValidPolygon;
use slicecore_geo::{offset_polygons, JoinType};

use super::config::{InterfacePattern, SupportConfig};
use crate::infill::{self, InfillLine};

// ---------------------------------------------------------------------------
// Material types
// ---------------------------------------------------------------------------

/// 3D printing material type for material-specific support defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Material {
    /// PLA (Polylactic Acid) -- most common FDM material.
    Pla,
    /// PETG (Polyethylene Terephthalate Glycol) -- strong and flexible.
    Petg,
    /// ABS (Acrylonitrile Butadiene Styrene) -- heat-resistant.
    Abs,
    /// TPU (Thermoplastic Polyurethane) -- flexible/elastic.
    Tpu,
    /// Nylon (Polyamide) -- strong and durable.
    Nylon,
    /// Generic material with conservative defaults.
    Generic,
}

/// Material-specific gap values for support generation.
#[derive(Debug, Clone, Copy)]
pub struct MaterialGaps {
    /// Z-axis gap between support top and model bottom in mm.
    pub z_gap: f64,
    /// XY-axis gap between support and model walls in mm.
    pub xy_gap: f64,
}

/// Provides material-specific default gap values for support structures.
pub struct MaterialDefaults;

impl MaterialDefaults {
    /// Returns the default Z-gap and XY-gap for a given material.
    ///
    /// Values are based on slicer community research and testing:
    /// - PLA: z_gap=0.2mm, xy_gap=0.4mm (standard defaults)
    /// - PETG: z_gap=0.25mm, xy_gap=0.4mm (slightly more gap due to stringing)
    /// - ABS: z_gap=0.2mm, xy_gap=0.4mm (similar to PLA)
    /// - TPU: z_gap=0.3mm, xy_gap=0.5mm (more gap for flexible material)
    /// - Nylon: z_gap=0.25mm, xy_gap=0.4mm (similar to PETG)
    /// - Generic: z_gap=0.2mm, xy_gap=0.4mm (conservative PLA-like defaults)
    pub fn for_material(material: Material) -> MaterialGaps {
        match material {
            Material::Pla => MaterialGaps {
                z_gap: 0.2,
                xy_gap: 0.4,
            },
            Material::Petg => MaterialGaps {
                z_gap: 0.25,
                xy_gap: 0.4,
            },
            Material::Abs => MaterialGaps {
                z_gap: 0.2,
                xy_gap: 0.4,
            },
            Material::Tpu => MaterialGaps {
                z_gap: 0.3,
                xy_gap: 0.5,
            },
            Material::Nylon => MaterialGaps {
                z_gap: 0.25,
                xy_gap: 0.4,
            },
            Material::Generic => MaterialGaps {
                z_gap: 0.2,
                xy_gap: 0.4,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Interface layer identification
// ---------------------------------------------------------------------------

/// Identifies which layers of a support column are interface layers.
///
/// Interface layers are the topmost `interface_layer_count` layers of each
/// support column (closest to the model surface) and the bottommost
/// `interface_layer_count` layers (near the build plate or model surface below).
///
/// A support layer is considered "near the model" if any layer within
/// `interface_layer_count` layers above it has model contours that overlap
/// with support regions at that layer.
///
/// # Parameters
///
/// - `support_regions`: Per-layer support region polygons. `support_regions[i]`
///   contains the support polygons at layer `i`. Empty means no support.
/// - `model_contours`: Per-layer model contour polygons.
/// - `interface_layer_count`: Number of interface layers at top/bottom of
///   each support column.
///
/// # Returns
///
/// Per-layer boolean flags. `true` = interface layer, `false` = body layer.
/// Length matches `support_regions.len()`.
pub fn identify_interface_layers(
    support_regions: &[Vec<ValidPolygon>],
    model_contours: &[Vec<ValidPolygon>],
    interface_layer_count: u32,
) -> Vec<bool> {
    let n = support_regions.len();
    if n == 0 {
        return Vec::new();
    }

    let count = interface_layer_count as usize;
    let mut is_interface = vec![false; n];

    // Identify top interface layers: for each layer with support, check if
    // any layer within `count` layers above has model contours. If so, this
    // is a top interface layer.
    for i in 0..n {
        if support_regions[i].is_empty() {
            continue;
        }

        // Check if any of the next `count` layers above have model contours.
        let mut near_model_above = false;
        for offset in 1..=count {
            let above = i + offset;
            if above < n
                && above < model_contours.len()
                && !model_contours[above].is_empty()
            {
                near_model_above = true;
                break;
            }
        }

        if near_model_above {
            is_interface[i] = true;
        }
    }

    // Identify bottom interface layers: the bottommost `count` layers of
    // each contiguous support column. A support column starts where support
    // appears and the layer below has no support.
    for i in 0..n {
        if support_regions[i].is_empty() {
            continue;
        }

        // Check if this is the start of a support column (no support below).
        let is_column_start = i == 0 || support_regions[i - 1].is_empty();

        if is_column_start {
            // Mark the bottommost `count` layers as interface.
            for j in i..n.min(i + count) {
                if support_regions[j].is_empty() {
                    break;
                }
                is_interface[j] = true;
            }
        }
    }

    is_interface
}

// ---------------------------------------------------------------------------
// Z-gap application
// ---------------------------------------------------------------------------

/// Applies Z-gap by removing the topmost support layers.
///
/// Removes the top N layers of support where N = ceil(z_gap / layer_height),
/// creating a physical gap between the top of support and the bottom of the
/// model. For partial gaps (z_gap not an exact multiple of layer_height),
/// the top layer is removed if z_gap > 0.5 * layer_height.
///
/// # Parameters
///
/// - `support_regions`: Per-layer support region polygons (modified in place).
/// - `model_contours`: Per-layer model contour polygons.
/// - `z_gap_mm`: Z-axis gap distance in mm.
/// - `layer_height`: Layer height in mm.
pub fn apply_z_gap(
    support_regions: &mut [Vec<ValidPolygon>],
    model_contours: &[Vec<ValidPolygon>],
    z_gap_mm: f64,
    layer_height: f64,
) {
    if z_gap_mm <= 0.0 || layer_height <= 0.0 {
        return;
    }

    // Number of layers to remove from the top of each support column.
    let layers_to_remove = (z_gap_mm / layer_height).ceil() as usize;

    if layers_to_remove == 0 {
        return;
    }

    let n = support_regions.len();

    // For each layer, check if the support column ends here (model starts
    // above). If so, remove layers from the top of the column.
    for i in 0..n {
        if support_regions[i].is_empty() {
            continue;
        }

        // A layer is the "top" of a support column if the layer above has
        // model contours but no support, or if the layer above has no support
        // and is near model contours.
        let is_top = if i + 1 >= n {
            // Topmost layer in the stack -- could be top of column.
            true
        } else {
            // Top of column if next layer has no support.
            support_regions[i + 1].is_empty()
        };

        if is_top {
            // Remove the topmost `layers_to_remove` layers from this column.
            let start = if i >= layers_to_remove {
                i + 1 - layers_to_remove
            } else {
                0
            };
            for region in support_regions.iter_mut().take(i + 1).skip(start) {
                // Only clear if this layer actually has support.
                if !region.is_empty() {
                    region.clear();
                }
            }
        }
    }

    // Suppress unused variable warning -- model_contours reserved for future
    // more sophisticated Z-gap where partial removal depends on model proximity.
    let _ = model_contours;
}

// ---------------------------------------------------------------------------
// Interface infill generation
// ---------------------------------------------------------------------------

/// Generates dense infill for interface layers.
///
/// Interface infill is much denser than body infill (typically 80% vs 15%)
/// to provide a smooth contact surface between support and model.
///
/// # Parameters
///
/// - `interface_regions`: Polygons defining the interface region boundary.
/// - `density`: Interface density as a fraction (0.0 - 1.0, typically 0.80).
/// - `pattern`: Interface fill pattern.
/// - `layer_index`: Current layer index (for angle alternation).
/// - `extrusion_width`: Extrusion width in mm.
///
/// # Returns
///
/// Infill lines for the interface region.
pub fn generate_interface_infill(
    interface_regions: &[ValidPolygon],
    density: f64,
    pattern: InterfacePattern,
    layer_index: usize,
    extrusion_width: f64,
) -> Vec<InfillLine> {
    if interface_regions.is_empty() || density <= 0.0 {
        return Vec::new();
    }

    match pattern {
        InterfacePattern::Rectilinear => {
            // Alternating 0/90 rectilinear at interface density.
            let angle = infill::alternate_infill_angle(layer_index);
            infill::rectilinear::generate(interface_regions, density, angle, extrusion_width)
        }
        InterfacePattern::Grid => {
            // Cross-hatched grid at interface density.
            infill::grid::generate(interface_regions, density, layer_index, extrusion_width)
        }
        InterfacePattern::Concentric => {
            // Inward offset rings following region contour.
            generate_concentric_interface(interface_regions, density, extrusion_width)
        }
    }
}

/// Generates concentric interface infill by inward-offsetting the region boundary.
///
/// Creates concentric rings by repeatedly offsetting the boundary polygons
/// inward by the line spacing until the region collapses.
fn generate_concentric_interface(
    interface_regions: &[ValidPolygon],
    density: f64,
    extrusion_width: f64,
) -> Vec<InfillLine> {
    use slicecore_math::mm_to_coord;

    if interface_regions.is_empty() || density <= 0.0 || extrusion_width <= 0.0 {
        return Vec::new();
    }

    let density = density.min(1.0);
    let spacing_mm = extrusion_width / density;
    let spacing = mm_to_coord(spacing_mm);

    if spacing <= 0 {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut current_regions = interface_regions.to_vec();
    let mut iteration = 0;
    let max_iterations = 500; // Safety limit.

    while !current_regions.is_empty() && iteration < max_iterations {
        // Convert each polygon boundary to infill lines.
        for poly in &current_regions {
            let pts = poly.points();
            let n = pts.len();
            if n < 3 {
                continue;
            }
            for i in 0..n {
                let start = pts[i];
                let end = pts[(i + 1) % n];
                if start != end {
                    lines.push(InfillLine { start, end });
                }
            }
        }

        // Offset inward for next ring.
        current_regions = match offset_polygons(&current_regions, -spacing, JoinType::Miter) {
            Ok(result) => result,
            Err(_) => break,
        };

        iteration += 1;
    }

    lines
}

// ---------------------------------------------------------------------------
// Quality preset application
// ---------------------------------------------------------------------------

/// Applies a quality preset to the support configuration if one is set.
///
/// If `config.quality_preset` is `Some`, applies the preset values for
/// density, interface density, z_gap, and interface layer count. This
/// function should be called early in the support generation pipeline.
///
/// Preset values:
/// - **Low**: density=0.10, interface_density=0.50, z_gap=0.30, interface_layers=1
/// - **Medium**: density=0.15, interface_density=0.80, z_gap=0.20, interface_layers=2
/// - **High**: density=0.20, interface_density=1.0, z_gap=0.15, interface_layers=3
pub fn apply_quality_preset(config: &mut SupportConfig) {
    if let Some(preset) = config.quality_preset {
        preset.apply(config);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::config::QualityPreset;
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

    // -- Interface identification tests --

    #[test]
    fn topmost_2_layers_of_10_layer_column_are_interface() {
        // RED: Write assertion first.
        // 10 layers of support (layers 0-9). Model contours at layer 10
        // (one above the top of support).
        let n = 11;
        let support_square = make_square(50.0, 50.0, 10.0);

        let mut support_regions: Vec<Vec<ValidPolygon>> = vec![Vec::new(); n];
        for i in 0..10 {
            support_regions[i] = vec![support_square.clone()];
        }

        let mut model_contours: Vec<Vec<ValidPolygon>> = vec![Vec::new(); n];
        model_contours[10] = vec![make_square(50.0, 50.0, 10.0)];

        let flags = identify_interface_layers(&support_regions, &model_contours, 2);

        assert_eq!(flags.len(), n, "Should have flags for all layers");

        // GREEN: The topmost 2 layers of the 10-layer column (layers 8 and 9)
        // should be marked as interface because model contours exist at layer 10.
        assert!(flags[8], "Layer 8 (top-2) should be interface");
        assert!(flags[9], "Layer 9 (top-1) should be interface");

        // Middle body layers should not be interface (except bottom interface).
        // Layers 2..8 should be body layers (not interface).
        assert!(!flags[3], "Layer 3 (middle) should be body, not interface");
        assert!(!flags[4], "Layer 4 (middle) should be body, not interface");
        assert!(!flags[5], "Layer 5 (middle) should be body, not interface");
    }

    #[test]
    fn bottom_interface_layers_identified() {
        // A 10-layer support column starting at layer 0.
        let n = 11;
        let support_square = make_square(50.0, 50.0, 10.0);

        let mut support_regions: Vec<Vec<ValidPolygon>> = vec![Vec::new(); n];
        for i in 0..10 {
            support_regions[i] = vec![support_square.clone()];
        }

        let mut model_contours: Vec<Vec<ValidPolygon>> = vec![Vec::new(); n];
        model_contours[10] = vec![make_square(50.0, 50.0, 10.0)];

        let flags = identify_interface_layers(&support_regions, &model_contours, 2);

        // Bottom 2 layers (0 and 1) should also be interface (column start).
        assert!(flags[0], "Layer 0 (bottom) should be interface");
        assert!(flags[1], "Layer 1 (bottom+1) should be interface");
    }

    // -- Z-gap tests --

    #[test]
    fn z_gap_removes_exactly_1_layer_when_gap_equals_layer_height() {
        // z_gap=0.2mm, layer_height=0.2mm -> remove exactly 1 layer from top.
        let support_square = make_square(50.0, 50.0, 10.0);

        let mut support_regions: Vec<Vec<ValidPolygon>> = vec![Vec::new(); 5];
        for i in 0..5 {
            support_regions[i] = vec![support_square.clone()];
        }

        let model_contours: Vec<Vec<ValidPolygon>> = vec![Vec::new(); 5];

        apply_z_gap(&mut support_regions, &model_contours, 0.2, 0.2);

        // The topmost layer (4) should be removed.
        assert!(
            support_regions[4].is_empty(),
            "Top layer should be removed by Z-gap"
        );
        // Layer 3 should remain (only 1 layer removed).
        assert!(
            !support_regions[3].is_empty(),
            "Layer below top should remain when z_gap = layer_height"
        );
    }

    #[test]
    fn z_gap_removes_2_layers_with_ceil_rounding() {
        // z_gap=0.3mm, layer_height=0.2mm -> ceil(0.3/0.2) = ceil(1.5) = 2 layers.
        let support_square = make_square(50.0, 50.0, 10.0);

        let mut support_regions: Vec<Vec<ValidPolygon>> = vec![Vec::new(); 5];
        for i in 0..5 {
            support_regions[i] = vec![support_square.clone()];
        }

        let model_contours: Vec<Vec<ValidPolygon>> = vec![Vec::new(); 5];

        apply_z_gap(&mut support_regions, &model_contours, 0.3, 0.2);

        // Top 2 layers (3 and 4) should be removed.
        assert!(
            support_regions[4].is_empty(),
            "Top layer should be removed"
        );
        assert!(
            support_regions[3].is_empty(),
            "Second-from-top layer should also be removed (ceil rounding)"
        );
        // Layer 2 should remain.
        assert!(
            !support_regions[2].is_empty(),
            "Layer 2 should remain (only 2 layers removed)"
        );
    }

    // -- Material defaults tests --

    #[test]
    fn material_defaults_pla() {
        let gaps = MaterialDefaults::for_material(Material::Pla);
        assert!(
            (gaps.z_gap - 0.2).abs() < 1e-9,
            "PLA z_gap should be 0.2mm"
        );
        assert!(
            (gaps.xy_gap - 0.4).abs() < 1e-9,
            "PLA xy_gap should be 0.4mm"
        );
    }

    #[test]
    fn material_defaults_petg() {
        let gaps = MaterialDefaults::for_material(Material::Petg);
        assert!(
            (gaps.z_gap - 0.25).abs() < 1e-9,
            "PETG z_gap should be 0.25mm"
        );
    }

    #[test]
    fn material_defaults_tpu() {
        let gaps = MaterialDefaults::for_material(Material::Tpu);
        assert!(
            (gaps.z_gap - 0.3).abs() < 1e-9,
            "TPU z_gap should be 0.3mm"
        );
        assert!(
            (gaps.xy_gap - 0.5).abs() < 1e-9,
            "TPU xy_gap should be 0.5mm"
        );
    }

    // -- Quality preset tests --

    #[test]
    fn quality_preset_low_applies_correctly() {
        let mut config = SupportConfig::default();
        config.quality_preset = Some(QualityPreset::Low);
        apply_quality_preset(&mut config);

        assert!(
            (config.interface_density - 0.50).abs() < 1e-9,
            "Low preset interface_density should be 0.50"
        );
        assert!(
            (config.z_gap - 0.30).abs() < 1e-9,
            "Low preset z_gap should be 0.30"
        );
        assert!(
            (config.support_density - 0.10).abs() < 1e-9,
            "Low preset support_density should be 0.10"
        );
        assert_eq!(
            config.interface_layers, 1,
            "Low preset interface_layers should be 1"
        );
    }

    #[test]
    fn quality_preset_high_applies_correctly() {
        let mut config = SupportConfig::default();
        config.quality_preset = Some(QualityPreset::High);
        apply_quality_preset(&mut config);

        assert!(
            (config.interface_density - 1.0).abs() < 1e-9,
            "High preset interface_density should be 1.0"
        );
        assert!(
            (config.z_gap - 0.15).abs() < 1e-9,
            "High preset z_gap should be 0.15"
        );
        assert!(
            (config.support_density - 0.20).abs() < 1e-9,
            "High preset support_density should be 0.20"
        );
        assert_eq!(
            config.interface_layers, 3,
            "High preset interface_layers should be 3"
        );
    }

    // -- Interface infill tests --

    #[test]
    fn interface_infill_denser_than_body_infill() {
        let region = vec![make_square(50.0, 50.0, 10.0)];
        let extrusion_width = 0.4;

        // Body infill at 15% density.
        let body_lines = infill::rectilinear::generate(&region, 0.15, 0.0, extrusion_width);

        // Interface infill at 80% density.
        let interface_lines = generate_interface_infill(
            &region,
            0.80,
            InterfacePattern::Rectilinear,
            0,
            extrusion_width,
        );

        assert!(
            !body_lines.is_empty(),
            "Body infill should produce lines"
        );
        assert!(
            !interface_lines.is_empty(),
            "Interface infill should produce lines"
        );
        assert!(
            interface_lines.len() > body_lines.len(),
            "Interface infill ({}) should produce more lines than body infill ({}) due to higher density",
            interface_lines.len(),
            body_lines.len()
        );
    }

    #[test]
    fn interface_infill_grid_pattern() {
        let region = vec![make_square(50.0, 50.0, 10.0)];

        let lines = generate_interface_infill(&region, 0.80, InterfacePattern::Grid, 0, 0.4);

        assert!(
            !lines.is_empty(),
            "Grid interface infill should produce lines"
        );

        // Grid should have both horizontal and vertical lines.
        let has_horizontal = lines.iter().any(|l| l.start.y == l.end.y);
        let has_vertical = lines.iter().any(|l| l.start.x == l.end.x);
        assert!(has_horizontal, "Grid interface should have horizontal lines");
        assert!(has_vertical, "Grid interface should have vertical lines");
    }

    #[test]
    fn interface_infill_concentric_pattern() {
        let region = vec![make_square(50.0, 50.0, 10.0)];

        let lines = generate_interface_infill(&region, 0.80, InterfacePattern::Concentric, 0, 0.4);

        assert!(
            !lines.is_empty(),
            "Concentric interface infill should produce lines"
        );
    }

    #[test]
    fn interface_infill_empty_region_returns_empty() {
        let lines = generate_interface_infill(&[], 0.80, InterfacePattern::Rectilinear, 0, 0.4);
        assert!(lines.is_empty(), "Empty region should return empty lines");
    }

    #[test]
    fn quality_preset_none_does_not_modify() {
        let mut config = SupportConfig::default();
        let original_density = config.support_density;
        let original_z_gap = config.z_gap;

        apply_quality_preset(&mut config);

        assert!(
            (config.support_density - original_density).abs() < 1e-9,
            "No preset should not modify density"
        );
        assert!(
            (config.z_gap - original_z_gap).abs() < 1e-9,
            "No preset should not modify z_gap"
        );
    }

    #[test]
    fn material_serde_round_trip() {
        let materials = [
            Material::Pla,
            Material::Petg,
            Material::Abs,
            Material::Tpu,
            Material::Nylon,
            Material::Generic,
        ];
        for m in &materials {
            let json = serde_json::to_string(m).unwrap();
            let deserialized: Material = serde_json::from_str(&json).unwrap();
            assert_eq!(*m, deserialized, "Serde round-trip failed for {:?}", m);
        }
    }

    #[test]
    fn z_gap_zero_does_not_modify() {
        let support_square = make_square(50.0, 50.0, 10.0);

        let mut support_regions: Vec<Vec<ValidPolygon>> = vec![Vec::new(); 5];
        for i in 0..5 {
            support_regions[i] = vec![support_square.clone()];
        }
        let model_contours: Vec<Vec<ValidPolygon>> = vec![Vec::new(); 5];

        apply_z_gap(&mut support_regions, &model_contours, 0.0, 0.2);

        for i in 0..5 {
            assert!(
                !support_regions[i].is_empty(),
                "Zero z_gap should not remove any layers, but layer {} was cleared",
                i
            );
        }
    }
}
