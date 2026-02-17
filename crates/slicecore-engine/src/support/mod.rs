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
pub mod detect;
pub mod traditional;
pub mod tree;
pub mod tree_node;

use slicecore_geo::ValidPolygon;

use crate::infill::InfillLine;

/// A support region on a single layer.
///
/// Contains the contours defining the support boundary, along with metadata
/// about the layer position and whether this region was detected as a bridge.
/// The `infill` field contains generated infill lines for the support body.
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
pub struct SupportResult {
    /// Per-layer support regions. `regions[i]` contains all support regions
    /// for layer `i`. Empty vectors indicate no support needed for that layer.
    pub regions: Vec<Vec<SupportRegion>>,
}
