//! Support structure generation for the slicing pipeline.
//!
//! This module provides:
//! - **Configuration types** ([`config`]): `SupportConfig`, `SupportType`, `SupportPattern`,
//!   `TreeSupportConfig`, `BridgeConfig`, quality presets, and conflict resolution.
//! - **Overhang detection** ([`detect`]): Hybrid layer-diff + raycast algorithm for
//!   identifying regions that need support.
//!
//! # Architecture
//!
//! Support generation follows a multi-stage pipeline:
//! 1. **Detection**: Identify overhang regions by comparing adjacent layers.
//! 2. **Validation**: Filter false positives using downward raycasting.
//! 3. **Filtering**: Remove unprintable tiny regions below area thresholds.
//! 4. **Generation**: Build support geometry from validated regions (traditional or tree).

pub mod bridge;
pub mod config;
pub mod conflict;
pub mod detect;
pub mod interface;
pub mod override_system;
pub mod overhang_perimeter;
pub mod traditional;
pub mod tree;
pub mod tree_node;

use serde::{Deserialize, Serialize};
use slicecore_geo::ValidPolygon;
use slicecore_mesh::TriangleMesh;
use slicecore_slicer::SliceLayer;

use crate::infill::InfillLine;

use self::bridge::detect_bridges;
use self::config::{SupportConfig, SupportType};
use self::detect::detect_all_overhangs;
use self::interface::{apply_quality_preset, apply_z_gap, generate_interface_infill, identify_interface_layers};
use self::overhang_perimeter::auto_select_support_type;
use self::traditional::generate_traditional_supports;
use self::tree::generate_tree_supports;

/// A support region on a single layer.
///
/// Contains the contours defining the support boundary, along with metadata
/// about the layer position and whether this region was detected as a bridge.
/// The `infill` field contains generated infill lines for the support body.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SupportRegion {
    /// Polygonal contours defining the support region boundary.
    pub contours: Vec<ValidPolygon>,
    /// Z height of this layer in mm.
    pub z: f64,
    /// Index of this layer in the layer stack.
    pub layer_index: usize,
    /// Whether this region was detected as a bridge (unsupported horizontal span).
    pub is_bridge: bool,
    /// Infill lines generated for this support region.
    pub infill: Vec<InfillLine>,
}

/// Result of support detection across all layers.
///
/// Contains per-layer support regions, indexed by layer number.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SupportResult {
    /// Per-layer support regions. `regions[i]` contains all support regions
    /// for layer `i`. Empty vectors indicate no support needed for that layer.
    pub regions: Vec<Vec<SupportRegion>>,
    /// Per-layer bridge regions detected during support generation.
    pub bridge_regions: Vec<Vec<bridge::BridgeRegion>>,
}

impl SupportResult {
    /// Creates an empty support result (no support needed).
    pub fn empty() -> Self {
        Self {
            regions: Vec::new(),
            bridge_regions: Vec::new(),
        }
    }
}

/// Top-level support generation entry point.
///
/// Runs the full support pipeline:
/// 1. Detect overhang regions via hybrid layer-diff + raycast.
/// 2. Separate bridges from regular overhangs.
/// 3. Determine support type (tree vs traditional) if set to auto.
/// 4. Generate support structures (traditional and/or tree).
/// 5. Identify interface layers and apply Z-gap.
/// 6. Apply quality preset if configured.
///
/// # Parameters
///
/// - `layers`: Sliced model layers with contours and Z heights.
/// - `mesh`: The original triangle mesh for raycast validation.
/// - `config`: Support configuration.
/// - `extrusion_width`: Extrusion width in mm.
///
/// # Returns
///
/// A [`SupportResult`] with per-layer support regions and bridge regions.
pub fn generate_supports(
    layers: &[SliceLayer],
    mesh: &TriangleMesh,
    config: &SupportConfig,
    extrusion_width: f64,
) -> SupportResult {
    if !config.enabled {
        return SupportResult::empty();
    }

    let n = layers.len();
    if n == 0 {
        return SupportResult::empty();
    }

    // Apply quality preset if set.
    let mut config = config.clone();
    apply_quality_preset(&mut config);

    // Extract per-layer contours and Z heights.
    let layer_contours: Vec<Vec<ValidPolygon>> =
        layers.iter().map(|l| l.contours.clone()).collect();
    let layer_heights: Vec<f64> = layers.iter().map(|l| l.z).collect();
    let layer_height = if layers.len() > 1 {
        layers[1].z - layers[0].z
    } else {
        layers[0].layer_height
    };

    // Step 1: Detect all overhangs.
    let all_overhangs = detect_all_overhangs(
        &layer_contours,
        mesh,
        &config,
        &layer_heights,
        layer_height,
        extrusion_width,
    );

    // Step 2: Separate bridges from regular overhangs.
    let mut regular_overhangs: Vec<Vec<ValidPolygon>> = Vec::with_capacity(n);
    let mut all_bridge_regions: Vec<Vec<bridge::BridgeRegion>> = Vec::with_capacity(n);

    for (layer_idx, layer_overhangs) in all_overhangs.iter().enumerate() {
        if layer_overhangs.is_empty() || layer_idx == 0 {
            regular_overhangs.push(layer_overhangs.clone());
            all_bridge_regions.push(Vec::new());
            continue;
        }

        if config.bridge_detection {
            let below = &layer_contours[layer_idx - 1];
            let z = layer_heights.get(layer_idx).copied().unwrap_or(0.0);
            let (bridges, non_bridges) =
                detect_bridges(layer_overhangs, below, layer_idx, z, 5.0);
            regular_overhangs.push(non_bridges);
            all_bridge_regions.push(bridges);
        } else {
            regular_overhangs.push(layer_overhangs.clone());
            all_bridge_regions.push(Vec::new());
        }
    }

    // Step 3: Determine support type.
    let support_type = match config.support_type {
        SupportType::Auto => auto_select_support_type(&regular_overhangs, extrusion_width),
        other => other,
    };

    // Step 4: Generate support structures.
    let support_regions = match support_type {
        SupportType::Traditional => {
            generate_traditional_supports(&regular_overhangs, layers, &config, extrusion_width)
        }
        SupportType::Tree => {
            generate_tree_supports(&regular_overhangs, layers, &config, extrusion_width)
        }
        SupportType::Auto => {
            // Auto after auto_select returned Auto means mixed.
            // Use traditional as the primary (handles large regions well).
            generate_traditional_supports(&regular_overhangs, layers, &config, extrusion_width)
        }
        SupportType::None => {
            vec![Vec::new(); n]
        }
    };

    // Step 5: Apply Z-gap and identify interface layers.
    let mut support_contours_per_layer: Vec<Vec<ValidPolygon>> = support_regions
        .iter()
        .map(|layer_regions| {
            layer_regions
                .iter()
                .flat_map(|r| r.contours.clone())
                .collect()
        })
        .collect();

    // Apply Z-gap.
    apply_z_gap(
        &mut support_contours_per_layer,
        &layer_contours,
        config.z_gap,
        layer_height,
    );

    // Identify interface layers.
    let is_interface = identify_interface_layers(
        &support_contours_per_layer,
        &layer_contours,
        config.interface_layers,
    );

    // Step 6: Rebuild support regions with interface infill.
    let mut final_regions: Vec<Vec<SupportRegion>> = Vec::with_capacity(n);

    for layer_idx in 0..n {
        let layer_support = if layer_idx < support_regions.len() {
            &support_regions[layer_idx]
        } else {
            final_regions.push(Vec::new());
            continue;
        };

        let mut layer_final = Vec::new();

        for region in layer_support {
            // Check if this region still has contours after Z-gap removal.
            if region.contours.is_empty() {
                continue;
            }

            let is_interface_layer = layer_idx < is_interface.len() && is_interface[layer_idx];

            let infill = if is_interface_layer {
                // Generate dense interface infill.
                generate_interface_infill(
                    &region.contours,
                    config.interface_density,
                    config.interface_pattern,
                    layer_idx,
                    extrusion_width,
                )
            } else {
                // Keep the body infill from the generation step.
                region.infill.clone()
            };

            layer_final.push(SupportRegion {
                contours: region.contours.clone(),
                z: region.z,
                layer_index: region.layer_index,
                is_bridge: region.is_bridge,
                infill,
            });
        }

        final_regions.push(layer_final);
    }

    SupportResult {
        regions: final_regions,
        bridge_regions: all_bridge_regions,
    }
}
