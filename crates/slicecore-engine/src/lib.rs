//! Slicing pipeline orchestrator for the slicecore 3D slicing engine.
//!
//! This crate ties together all pipeline stages: mesh loading, slicing,
//! perimeter generation, infill, toolpath planning, and G-code emission.
//!
//! Pipeline modules:
//! - [`config`]: Print configuration with TOML deserialization
//! - [`engine`]: Pipeline orchestrator (Engine struct)
//! - [`perimeter`]: Perimeter shell generation via polygon offsetting
//! - [`infill`]: Rectilinear infill pattern generation
//! - [`surface`]: Top/bottom solid layer classification
//! - [`extrusion`]: E-axis value computation (Slic3r cross-section model)
//! - [`toolpath`]: Toolpath segment types and layer toolpath assembly
//! - [`planner`]: Skirt/brim generation, retraction, temperature, fan control
//! - [`gcode_gen`]: Toolpath-to-GcodeCommand conversion
//!
//! # Configuration
//!
//! [`PrintConfig`] contains all parameters controlling the pipeline.
//! It supports TOML deserialization with `#[serde(default)]`, so any
//! unspecified fields use sensible FDM defaults.

pub mod config;
pub mod engine;
pub mod error;
pub mod extrusion;
pub mod gcode_gen;
pub mod infill;
pub mod perimeter;
pub mod planner;
pub mod scarf;
pub mod seam;
pub mod surface;
pub mod toolpath;

// Re-export primary types at crate root.
pub use config::{PrintConfig, ScarfJointConfig, ScarfJointType, WallOrder};
pub use seam::{select_seam_point, SeamPosition};
pub use engine::{Engine, SliceResult};
pub use error::EngineError;
pub use extrusion::{compute_e_value, extrusion_cross_section, move_length};
pub use infill::{
    alternate_infill_angle, generate_infill, generate_rectilinear_infill, InfillLine,
    InfillPattern, LayerInfill,
};
pub use perimeter::{generate_perimeters, ContourPerimeters, PerimeterShell};
pub use surface::{classify_surfaces, SurfaceClassification};
pub use planner::{
    generate_brim, generate_skirt, plan_fan, plan_retraction, plan_temperatures, RetractionMove,
};
pub use gcode_gen::{generate_full_gcode, generate_layer_gcode};
pub use scarf::apply_scarf_joint;
pub use toolpath::{
    assemble_layer_toolpath, FeatureType, LayerToolpath, ToolpathSegment,
};
