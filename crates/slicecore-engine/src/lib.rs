//! Slicing pipeline orchestrator for the slicecore 3D slicing engine.
//!
//! This crate ties together all pipeline stages: mesh loading, slicing,
//! perimeter generation, infill, toolpath planning, and G-code emission.
//!
//! Current pipeline modules:
//! - [`config`]: Print configuration with TOML deserialization
//! - [`perimeter`]: Perimeter shell generation via polygon offsetting
//! - [`infill`]: Rectilinear infill pattern generation
//! - [`surface`]: Top/bottom solid layer classification
//! - [`extrusion`]: E-axis value computation (Slic3r cross-section model)
//! - [`toolpath`]: Toolpath segment types and layer toolpath assembly
//!
//! Future pipeline modules:
//! - G-code generation (plan 03-05)
//!
//! # Configuration
//!
//! [`PrintConfig`] contains all parameters controlling the pipeline.
//! It supports TOML deserialization with `#[serde(default)]`, so any
//! unspecified fields use sensible FDM defaults.

pub mod config;
pub mod error;
pub mod extrusion;
pub mod infill;
pub mod perimeter;
pub mod surface;
pub mod toolpath;

// Future pipeline modules:
// pub mod planner;
// pub mod gcode_gen;

// Re-export primary types at crate root.
pub use config::{PrintConfig, WallOrder};
pub use error::EngineError;
pub use extrusion::{compute_e_value, extrusion_cross_section, move_length};
pub use infill::{
    alternate_infill_angle, generate_rectilinear_infill, InfillLine, LayerInfill,
};
pub use perimeter::{generate_perimeters, ContourPerimeters, PerimeterShell};
pub use surface::{classify_surfaces, SurfaceClassification};
pub use toolpath::{
    assemble_layer_toolpath, FeatureType, LayerToolpath, ToolpathSegment,
};
