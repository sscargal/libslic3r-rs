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
//! Future patterns (AdaptiveCubic, Lightning) currently fall back to Rectilinear.

pub mod cubic;
pub mod grid;
pub mod gyroid;
pub mod honeycomb;
pub mod monotonic;
pub mod rectilinear;

use serde::{Deserialize, Serialize};
use slicecore_geo::polygon::ValidPolygon;
use slicecore_math::{mm_to_coord, Coord, IPoint2};

/// A line segment in integer coordinate space representing one infill extrusion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfillLine {
    /// Start point of the infill line.
    pub start: IPoint2,
    /// End point of the infill line.
    pub end: IPoint2,
}

/// Result of infill generation for a single layer.
#[derive(Clone, Debug)]
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
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
}

/// Generates infill lines for the given pattern, dispatching to the correct submodule.
///
/// # Parameters
/// - `pattern`: The infill pattern to use.
/// - `infill_region`: The boundary polygons defining the infill area.
/// - `density`: Fill density as a fraction (0.0 = empty, 1.0 = solid).
/// - `layer_index`: Current layer index (used for angle alternation).
/// - `layer_z`: Z height of the current layer (used by Gyroid and future 3D patterns).
/// - `line_width`: Extrusion line width in mm.
///
/// # Returns
/// A vector of [`InfillLine`] segments for the requested pattern.
pub fn generate_infill(
    pattern: InfillPattern,
    infill_region: &[ValidPolygon],
    density: f64,
    layer_index: usize,
    layer_z: f64,
    line_width: f64,
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
        // TODO: implement in plan 04-06
        InfillPattern::AdaptiveCubic => {
            rectilinear::generate(infill_region, density, angle, line_width)
        }
        InfillPattern::Cubic => {
            cubic::generate(infill_region, density, layer_index, layer_z, line_width)
        }
        // TODO: implement in plan 04-08
        InfillPattern::Lightning => {
            rectilinear::generate(infill_region, density, angle, line_width)
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
