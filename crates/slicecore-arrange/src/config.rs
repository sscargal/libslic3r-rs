//! Configuration types for build plate arrangement.
//!
//! Contains [`ArrangeConfig`] for controlling arrangement behavior,
//! [`ArrangePart`] for describing parts to arrange, and supporting
//! enums for orientation and gantry models.

use serde::{Deserialize, Serialize};
use slicecore_math::Point3;

/// Controls how auto-orient selects the best orientation for a part.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum OrientCriterion {
    /// Minimize the estimated support material volume.
    #[default]
    MinimizeSupport,
    /// Maximize the area of flat faces contacting the bed.
    MaximizeFlatContact,
    /// Weighted combination of support minimization and flat contact maximization.
    MultiCriteria {
        /// Weight for support minimization (0.0 to 1.0).
        support_weight: f64,
        /// Weight for flat contact maximization (0.0 to 1.0).
        contact_weight: f64,
    },
}

/// Models the gantry clearance zone for sequential (by-object) printing.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum GantryModel {
    /// Cylindrical clearance zone around the extruder.
    Cylinder {
        /// Radius of the clearance cylinder in mm.
        radius: f64,
    },
    /// Rectangular clearance zone.
    Rectangular {
        /// Width of the gantry clearance zone in mm.
        width: f64,
        /// Depth of the gantry clearance zone in mm.
        depth: f64,
    },
    /// User-defined polygon clearance shape, vertices in mm.
    CustomPolygon {
        /// Vertices of the custom clearance polygon as (x, y) pairs in mm.
        vertices: Vec<(f64, f64)>,
    },
    /// No gantry model (clearance checking disabled).
    #[default]
    None,
}

/// Configuration for the arrangement algorithm.
///
/// All distance values are in millimeters. Use `Default::default()` for
/// sensible starting values.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent boolean config flags"
)]
pub struct ArrangeConfig {
    /// Minimum spacing between parts in mm.
    pub part_spacing: f64,
    /// Margin from the bed edge in mm.
    pub bed_margin: f64,
    /// Rotation increment in degrees for trying different orientations.
    pub rotation_step: f64,
    /// Whether to automatically orient parts for optimal printing.
    pub auto_orient: bool,
    /// Criterion for auto-orientation selection.
    pub orient_criterion: OrientCriterion,
    /// Whether sequential (by-object) printing mode is active.
    pub sequential_mode: bool,
    /// Gantry clearance model for sequential mode collision avoidance.
    pub gantry_model: GantryModel,
    /// Brim width in mm (footprint expansion).
    pub brim_width: f64,
    /// Raft margin in mm (footprint expansion).
    pub raft_margin: f64,
    /// Skirt distance from outermost part in mm.
    pub skirt_distance: f64,
    /// Number of skirt loops.
    pub skirt_loops: u32,
    /// Whether to group parts by material on the same plate.
    pub material_grouping: bool,
    /// Whether to center the arrangement on the bed after packing.
    pub center_after_packing: bool,
    /// Nozzle diameter in mm (affects minimum spacing).
    pub nozzle_diameter: f64,
}

impl Default for ArrangeConfig {
    fn default() -> Self {
        Self {
            part_spacing: 2.0,
            bed_margin: 5.0,
            rotation_step: 45.0,
            auto_orient: true,
            orient_criterion: OrientCriterion::default(),
            sequential_mode: false,
            gantry_model: GantryModel::default(),
            brim_width: 0.0,
            raft_margin: 0.0,
            skirt_distance: 0.0,
            skirt_loops: 0,
            material_grouping: true,
            center_after_packing: true,
            nozzle_diameter: 0.4,
        }
    }
}

/// A part to be arranged on the build plate.
///
/// Describes a single mesh to be placed, including its geometry,
/// material, and constraint flags.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ArrangePart {
    /// Unique identifier for this part.
    pub id: String,
    /// 3D vertices of the mesh (used for footprint computation).
    pub vertices: Vec<Point3>,
    /// Height of the mesh in mm (Z extent).
    pub mesh_height: f64,
    /// Material identifier for grouping (e.g., "PLA", "ABS").
    pub material: Option<String>,
    /// If true, the arranger will not rotate this part.
    pub rotation_locked: bool,
    /// If true, the arranger will not change this part's orientation.
    pub orientation_locked: bool,
    /// Whether to mirror this part.
    pub mirror: bool,
    /// User-specified initial rotation in degrees.
    pub pre_rotation_deg: f64,
}

impl Default for ArrangePart {
    fn default() -> Self {
        Self {
            id: String::new(),
            vertices: Vec::new(),
            mesh_height: 0.0,
            material: None,
            rotation_locked: false,
            orientation_locked: false,
            mirror: false,
            pre_rotation_deg: 0.0,
        }
    }
}
