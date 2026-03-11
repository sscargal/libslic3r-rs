//! Result types for build plate arrangement.
//!
//! Contains the output structures produced by the arrangement algorithm:
//! [`ArrangementResult`] at the top level, with per-plate [`PlateArrangement`]
//! and per-part [`PartPlacement`] details.

use serde::{Deserialize, Serialize};

/// The complete result of an arrangement operation.
///
/// Contains one or more plates, each with a list of part placements.
/// Parts that could not be placed are listed in `unplaced_parts`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArrangementResult {
    /// Arranged plates, each containing a set of part placements.
    pub plates: Vec<PlateArrangement>,
    /// Total number of plates used.
    pub total_plates: usize,
    /// IDs of parts that could not be placed on any plate.
    pub unplaced_parts: Vec<String>,
}

/// A single plate in the arrangement, containing zero or more parts.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlateArrangement {
    /// Zero-based index of this plate.
    pub plate_index: usize,
    /// Parts placed on this plate.
    pub placements: Vec<PartPlacement>,
}

/// The placement of a single part on a plate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartPlacement {
    /// ID of the placed part.
    pub part_id: String,
    /// Position on the bed as (x, y) in mm.
    pub position: (f64, f64),
    /// Rotation around the Z axis in degrees.
    pub rotation_deg: f64,
    /// Optional XYZ orientation angles (rx, ry, rz) in degrees from auto-orient.
    pub orientation: Option<(f64, f64, f64)>,
    /// Index of the plate this part is placed on.
    pub plate_index: usize,
    /// Print order for sequential mode (lower = printed first).
    pub print_order: Option<usize>,
}
