//! Surface classification for top/bottom solid layer detection.
//!
//! In FDM 3D printing, the top and bottom surfaces of a model must be printed
//! with solid infill (100% density) rather than sparse pattern infill. This
//! module classifies which regions of each layer need solid fill based on
//! their proximity to the top/bottom of the model.
//!
//! # Algorithm
//!
//! For Phase 3, the classification uses a simplified approach:
//!
//! - **Bottom layers**: The first `bottom_layers` layers are entirely solid.
//! - **Top layers**: The last `top_layers` layers are entirely solid.
//! - **Interior layers**: Use polygon boolean operations to detect exposed
//!   surfaces by comparing with adjacent layers. If the layer above/below
//!   has a different footprint, the exposed regions are marked solid.
//!
//! # Example
//!
//! ```ignore
//! use slicecore_engine::surface::classify_surfaces;
//!
//! let classification = classify_surfaces(&layers, 2, 3, 3);
//! // classification.solid_regions: regions needing 100% infill
//! // classification.sparse_regions: regions using configured density
//! ```

use slicecore_geo::polygon::ValidPolygon;
use slicecore_geo::polygon_difference;
use slicecore_slicer::SliceLayer;

/// Surface classification for a layer's infill regions.
///
/// After classification, `solid_regions` should be filled at 100% density
/// and `sparse_regions` at the user-configured infill density.
#[derive(Clone, Debug)]
pub struct SurfaceClassification {
    /// Regions needing 100% solid infill (top/bottom surfaces).
    pub solid_regions: Vec<ValidPolygon>,
    /// Regions using configured infill density (interior).
    pub sparse_regions: Vec<ValidPolygon>,
}

/// Classifies a layer's regions into solid (top/bottom surface) and sparse (interior).
///
/// # Parameters
/// - `layers`: All slice layers in the model.
/// - `layer_index`: The index of the layer to classify.
/// - `top_layers`: Number of solid top layers (from config).
/// - `bottom_layers`: Number of solid bottom layers (from config).
///
/// # Returns
/// A [`SurfaceClassification`] with solid and sparse regions for the given layer.
///
/// # Panics
/// Panics if `layer_index >= layers.len()`.
pub fn classify_surfaces(
    layers: &[SliceLayer],
    layer_index: usize,
    top_layers: u32,
    bottom_layers: u32,
) -> SurfaceClassification {
    assert!(
        layer_index < layers.len(),
        "layer_index {} out of bounds (len={})",
        layer_index,
        layers.len()
    );

    let total_layers = layers.len();
    let current_contours = &layers[layer_index].contours;

    if current_contours.is_empty() {
        return SurfaceClassification {
            solid_regions: Vec::new(),
            sparse_regions: Vec::new(),
        };
    }

    // Bottom detection: first `bottom_layers` layers are entirely solid.
    if (layer_index as u32) < bottom_layers {
        return SurfaceClassification {
            solid_regions: current_contours.clone(),
            sparse_regions: Vec::new(),
        };
    }

    // Top detection: last `top_layers` layers are entirely solid.
    if layer_index >= total_layers.saturating_sub(top_layers as usize) {
        return SurfaceClassification {
            solid_regions: current_contours.clone(),
            sparse_regions: Vec::new(),
        };
    }

    // Interior layer: detect exposed surfaces by comparing with adjacent layers.
    // Compute top surface = current MINUS intersection with layer above.
    // Compute bottom surface = current MINUS intersection with layer below.
    let mut solid_regions: Vec<ValidPolygon> = Vec::new();

    // Top surface detection: check layer above.
    if layer_index + 1 < total_layers {
        let above_contours = &layers[layer_index + 1].contours;
        if above_contours.is_empty() {
            // Nothing above -> entire layer is a top surface.
            return SurfaceClassification {
                solid_regions: current_contours.clone(),
                sparse_regions: Vec::new(),
            };
        }

        // Top surface = current region MINUS the above region.
        // Parts of this layer with nothing above them need solid infill.
        if let Ok(top_surface) = polygon_difference(current_contours, above_contours) {
            solid_regions.extend(top_surface);
        }
    } else {
        // No layer above -> entire layer is top.
        return SurfaceClassification {
            solid_regions: current_contours.clone(),
            sparse_regions: Vec::new(),
        };
    }

    // Bottom surface detection: check layer below.
    if layer_index > 0 {
        let below_contours = &layers[layer_index - 1].contours;
        if below_contours.is_empty() {
            // Nothing below -> entire layer is bottom surface.
            return SurfaceClassification {
                solid_regions: current_contours.clone(),
                sparse_regions: Vec::new(),
            };
        }

        // Bottom surface = current region MINUS the below region.
        if let Ok(bottom_surface) = polygon_difference(current_contours, below_contours) {
            solid_regions.extend(bottom_surface);
        }
    }

    // Compute sparse regions = current contours MINUS solid regions.
    if solid_regions.is_empty() {
        // No exposed surfaces -> entirely sparse (interior).
        return SurfaceClassification {
            solid_regions: Vec::new(),
            sparse_regions: current_contours.clone(),
        };
    }

    // sparse = current - solid
    let sparse_regions = polygon_difference(current_contours, &solid_regions).unwrap_or_default();

    SurfaceClassification {
        solid_regions,
        sparse_regions,
    }
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

    /// Helper to create a SliceLayer with the given contours.
    fn make_layer(z: f64, contours: Vec<ValidPolygon>) -> SliceLayer {
        SliceLayer {
            z,
            layer_height: 0.2,
            contours,
        }
    }

    #[test]
    fn five_layers_identical_contours() {
        // 5-layer stack with identical contours.
        // bottom_layers=2, top_layers=2
        // layers 0,1: bottom solid
        // layer 2: sparse (interior)
        // layers 3,4: top solid
        let square = make_square(0.0, 0.0, 10.0);
        let layers: Vec<SliceLayer> = (0..5)
            .map(|i| make_layer(0.2 * (i as f64 + 1.0), vec![square.clone()]))
            .collect();

        // Layer 0: bottom solid.
        let c0 = classify_surfaces(&layers, 0, 2, 2);
        assert!(
            !c0.solid_regions.is_empty(),
            "Layer 0 should be solid (bottom)"
        );
        assert!(c0.sparse_regions.is_empty(), "Layer 0 should have no sparse");

        // Layer 1: bottom solid.
        let c1 = classify_surfaces(&layers, 1, 2, 2);
        assert!(
            !c1.solid_regions.is_empty(),
            "Layer 1 should be solid (bottom)"
        );
        assert!(c1.sparse_regions.is_empty(), "Layer 1 should have no sparse");

        // Layer 2: interior -> sparse (identical contours above and below).
        let c2 = classify_surfaces(&layers, 2, 2, 2);
        assert!(
            c2.solid_regions.is_empty(),
            "Layer 2 should have no solid (interior with identical contours)"
        );
        assert!(
            !c2.sparse_regions.is_empty(),
            "Layer 2 should be sparse (interior)"
        );

        // Layer 3: top solid.
        let c3 = classify_surfaces(&layers, 3, 2, 2);
        assert!(
            !c3.solid_regions.is_empty(),
            "Layer 3 should be solid (top)"
        );
        assert!(c3.sparse_regions.is_empty(), "Layer 3 should have no sparse");

        // Layer 4: top solid.
        let c4 = classify_surfaces(&layers, 4, 2, 2);
        assert!(
            !c4.solid_regions.is_empty(),
            "Layer 4 should be solid (top)"
        );
        assert!(c4.sparse_regions.is_empty(), "Layer 4 should have no sparse");
    }

    #[test]
    fn single_layer_is_solid() {
        let square = make_square(0.0, 0.0, 10.0);
        let layers = vec![make_layer(0.2, vec![square])];

        // Single layer: both top and bottom -> solid.
        let c = classify_surfaces(&layers, 0, 2, 2);
        assert!(
            !c.solid_regions.is_empty(),
            "Single layer should be solid"
        );
        assert!(c.sparse_regions.is_empty());
    }

    #[test]
    fn two_layers_both_solid() {
        let square = make_square(0.0, 0.0, 10.0);
        let layers = vec![
            make_layer(0.2, vec![square.clone()]),
            make_layer(0.4, vec![square]),
        ];

        // With top_layers=2 and bottom_layers=2, both layers are solid.
        let c0 = classify_surfaces(&layers, 0, 2, 2);
        assert!(!c0.solid_regions.is_empty(), "Layer 0 should be solid");
        assert!(c0.sparse_regions.is_empty());

        let c1 = classify_surfaces(&layers, 1, 2, 2);
        assert!(!c1.solid_regions.is_empty(), "Layer 1 should be solid");
        assert!(c1.sparse_regions.is_empty());
    }

    #[test]
    fn empty_contours_returns_empty() {
        let layers = vec![make_layer(0.2, Vec::new())];
        let c = classify_surfaces(&layers, 0, 2, 2);
        assert!(c.solid_regions.is_empty());
        assert!(c.sparse_regions.is_empty());
    }

    #[test]
    fn interior_with_different_above_has_solid_top() {
        // 3 layers: layer 0 = large square, layer 1 = large square, layer 2 = small square
        // Layer 1 should have exposed top surface where the large square extends
        // beyond the small square above it.
        let large = make_square(0.0, 0.0, 20.0);
        let small = make_square(5.0, 5.0, 10.0);
        let layers = vec![
            make_layer(0.2, vec![large.clone()]),
            make_layer(0.4, vec![large]),
            make_layer(0.6, vec![small]),
        ];

        // Layer 1 with bottom_layers=1, top_layers=1:
        // - Not a bottom layer (index 1 >= 1).
        // - Not in top N layers (index 1, total=3, top=1, threshold=2, so 1 < 2).
        // - Has different shape above, so should have some solid regions.
        let c = classify_surfaces(&layers, 1, 1, 1);
        assert!(
            !c.solid_regions.is_empty(),
            "Layer 1 should have solid regions (exposed top surface where shape differs from above)"
        );
    }
}
