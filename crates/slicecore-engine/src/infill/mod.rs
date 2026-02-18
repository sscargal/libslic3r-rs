//! Infill pattern generation with per-pattern submodules.
//!
//! This module provides a dispatch-based infill system where each pattern
//! is implemented in its own submodule. The [`generate_infill`] function
//! routes to the correct pattern based on [`InfillPattern`].
//!
//! Supported patterns:
//! - [`Rectilinear`](InfillPattern::Rectilinear) -- parallel scanlines alternating 0/90 degrees
//! - [`Grid`](InfillPattern::Grid) -- crosshatch (both 0 and 90 degrees on same layer)
//! - [`Monotonic`](InfillPattern::Monotonic) -- unidirectional scanlines (no bidirectional overlap)
//! - [`Honeycomb`](InfillPattern::Honeycomb) -- hexagonal pattern for strength-to-weight ratio
//! - [`Gyroid`](InfillPattern::Gyroid) -- TPMS-based smooth curves for isotropic strength
//! - [`Cubic`](InfillPattern::Cubic) -- 3-angle cycling with Z-dependent offset for 3D cubes
//!
//! - [`TpmsD`](InfillPattern::TpmsD) -- TPMS Schwarz Diamond surface for tetrahedral strength
//! - [`TpmsFk`](InfillPattern::TpmsFk) -- TPMS Fischer-Koch S surface for interconnected channels
//! - [`AdaptiveCubic`](InfillPattern::AdaptiveCubic) -- variable density using quadtree subdivision
//! - [`Lightning`](InfillPattern::Lightning) -- minimal tree-branching support for top surfaces

pub mod adaptive_cubic;
pub mod cubic;
pub mod grid;
pub mod gyroid;
pub mod honeycomb;
pub mod lightning;
pub mod monotonic;
pub mod rectilinear;
pub mod tpms_d;
pub mod tpms_fk;

use serde::{Deserialize, Serialize};
use slicecore_geo::polygon::ValidPolygon;
use slicecore_math::{mm_to_coord, Coord, IPoint2};

/// A line segment in integer coordinate space representing one infill extrusion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfillLine {
    /// Start point of the infill line.
    pub start: IPoint2,
    /// End point of the infill line.
    pub end: IPoint2,
}

/// Result of infill generation for a single layer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayerInfill {
    /// Infill extrusion segments.
    pub lines: Vec<InfillLine>,
    /// True if this is a solid infill region (top/bottom).
    pub is_solid: bool,
}

/// Infill pattern selection.
///
/// Each variant maps to a specific infill algorithm. The default is
/// [`Rectilinear`](InfillPattern::Rectilinear).
///
/// The [`Plugin`](InfillPattern::Plugin) variant selects a plugin-provided
/// infill pattern by its registered name. Plugin dispatch is handled by
/// the engine, not by [`generate_infill`].
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InfillPattern {
    /// Parallel lines alternating between 0 and 90 degrees per layer.
    #[default]
    Rectilinear,
    /// Crosshatch pattern: lines at both 0 and 90 degrees on the same layer.
    Grid,
    /// Hexagonal honeycomb pattern for high strength-to-weight ratio.
    Honeycomb,
    /// Triply periodic minimal surface pattern for isotropic strength.
    Gyroid,
    /// Adaptive cubic infill that increases density near surfaces.
    AdaptiveCubic,
    /// Regular cubic infill pattern.
    Cubic,
    /// Tree-like support structure that uses minimal material.
    Lightning,
    /// Unidirectional lines (left-to-right) for smooth top surfaces.
    Monotonic,
    /// TPMS Schwarz Diamond surface for tetrahedral stress distribution.
    TpmsD,
    /// TPMS Fischer-Koch S surface for interconnected channel topology.
    TpmsFk,
    /// A plugin-provided infill pattern, identified by registered name.
    ///
    /// The engine checks for this variant before calling [`generate_infill`]
    /// and routes the request to the plugin registry. If generate_infill is
    /// called directly with this variant (without engine interception), it
    /// returns an empty vector as a fallback.
    #[serde(rename = "plugin")]
    Plugin(String),
}

/// Generates infill lines for the given pattern, dispatching to the correct submodule.
///
/// # Parameters
/// - `pattern`: The infill pattern to use (by reference to avoid cloning).
/// - `infill_region`: The boundary polygons defining the infill area.
/// - `density`: Fill density as a fraction (0.0 = empty, 1.0 = solid).
/// - `layer_index`: Current layer index (used for angle alternation).
/// - `layer_z`: Z height of the current layer (used by Gyroid and future 3D patterns).
/// - `line_width`: Extrusion line width in mm.
/// - `lightning_context`: Optional cross-layer context for Lightning infill.
///   Ignored by all other patterns. Built by [`lightning::build_lightning_context`].
///
/// # Returns
/// A vector of [`InfillLine`] segments for the requested pattern.
///
/// # Plugin patterns
/// The [`InfillPattern::Plugin`] variant is handled by the engine's plugin
/// dispatch layer, not by this function. If reached here (engine didn't
/// intercept), returns an empty vector as fallback.
pub fn generate_infill(
    pattern: &InfillPattern,
    infill_region: &[ValidPolygon],
    density: f64,
    layer_index: usize,
    layer_z: f64,
    line_width: f64,
    lightning_context: Option<&lightning::LightningContext>,
) -> Vec<InfillLine> {
    let angle = alternate_infill_angle(layer_index);

    match pattern {
        InfillPattern::Rectilinear => {
            rectilinear::generate(infill_region, density, angle, line_width)
        }
        InfillPattern::Grid => grid::generate(infill_region, density, layer_index, line_width),
        InfillPattern::Monotonic => {
            monotonic::generate(infill_region, density, layer_index, line_width)
        }
        InfillPattern::Honeycomb => {
            honeycomb::generate(infill_region, density, layer_index, line_width)
        }
        InfillPattern::Gyroid => {
            gyroid::generate(infill_region, density, layer_index, layer_z, line_width)
        }
        InfillPattern::AdaptiveCubic => {
            adaptive_cubic::generate(infill_region, density, layer_index, layer_z, line_width)
        }
        InfillPattern::Cubic => {
            cubic::generate(infill_region, density, layer_index, layer_z, line_width)
        }
        InfillPattern::Lightning => {
            lightning::generate(infill_region, density, layer_index, line_width, lightning_context)
        }
        InfillPattern::TpmsD => {
            tpms_d::generate(infill_region, density, layer_index, layer_z, line_width)
        }
        InfillPattern::TpmsFk => {
            tpms_fk::generate(infill_region, density, layer_index, layer_z, line_width)
        }
        InfillPattern::Plugin(_) => {
            // Plugin dispatch is handled by the engine (Engine::generate_infill_for_layer).
            // If we reach here, it means the engine didn't intercept the Plugin variant.
            // Return empty as fallback.
            Vec::new()
        }
    }
}

/// Returns the infill angle for a given layer index.
///
/// Even layers use 0 degrees (horizontal), odd layers use 90 degrees (vertical).
/// This creates a cross-hatching pattern for structural strength.
pub fn alternate_infill_angle(layer_index: usize) -> f64 {
    if layer_index % 2 == 0 {
        0.0
    } else {
        90.0
    }
}

/// Backward-compatible wrapper for direct rectilinear infill generation.
///
/// Prefer [`generate_infill`] with [`InfillPattern::Rectilinear`] for new code.
pub fn generate_rectilinear_infill(
    infill_region: &[ValidPolygon],
    density: f64,
    angle_degrees: f64,
    line_width: f64,
) -> Vec<InfillLine> {
    rectilinear::generate(infill_region, density, angle_degrees, line_width)
}

/// Computes the axis-aligned bounding box of all polygons.
///
/// Returns `(min_x, min_y, max_x, max_y)` in integer coordinate space.
pub(crate) fn compute_bounding_box(polygons: &[ValidPolygon]) -> (Coord, Coord, Coord, Coord) {
    let mut min_x = Coord::MAX;
    let mut min_y = Coord::MAX;
    let mut max_x = Coord::MIN;
    let mut max_y = Coord::MIN;

    for poly in polygons {
        for pt in poly.points() {
            min_x = min_x.min(pt.x);
            min_y = min_y.min(pt.y);
            max_x = max_x.max(pt.x);
            max_y = max_y.max(pt.y);
        }
    }

    (min_x, min_y, max_x, max_y)
}

/// Computes line spacing in coordinate units from density and line width.
///
/// Returns `None` if the resulting spacing is zero or negative.
pub(crate) fn compute_spacing(density: f64, line_width: f64) -> Option<Coord> {
    let spacing_mm = line_width / density;
    let spacing = mm_to_coord(spacing_mm);
    if spacing <= 0 {
        None
    } else {
        Some(spacing)
    }
}
