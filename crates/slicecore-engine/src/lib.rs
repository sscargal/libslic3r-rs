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

pub mod arachne;
pub mod config;
pub mod custom_gcode;
pub mod engine;
pub mod error;
pub mod estimation;
pub mod extrusion;
pub mod filament;
pub mod flow_control;
pub mod gap_fill;
pub mod gcode_gen;
pub mod infill;
pub mod ironing;
pub mod modifier;
pub mod perimeter;
pub mod planner;
pub mod polyhole;
pub mod preview;
pub mod scarf;
pub mod seam;
pub mod support;
pub mod surface;
pub mod toolpath;

// Re-export primary types at crate root.
pub use config::{PrintConfig, ScarfJointConfig, ScarfJointType, SettingOverrides, WallOrder};
pub use custom_gcode::{substitute_placeholders, CustomGcodeHooks};
pub use flow_control::PerFeatureFlow;
pub use seam::{select_seam_point, SeamPosition};
pub use engine::{Engine, SliceResult};
pub use error::EngineError;
pub use estimation::{estimate_print_time, trapezoid_time, PrintTimeEstimate};
pub use filament::{estimate_filament_usage, FilamentUsage};
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
pub use gap_fill::{detect_and_fill_gaps, GapFillPath};
pub use arachne::{generate_arachne_perimeters, ArachnePerimeter, ArachneResult};
pub use preview::{generate_preview, LayerPreview, SlicePreview};
pub use ironing::{generate_ironing_passes, IroningConfig};
pub use modifier::{slice_modifier, split_by_modifiers, ModifierMesh, ModifierRegion};
pub use polyhole::{convert_polyholes, convert_to_polyhole, is_circular_hole, polyhole_radius, polyhole_sides};
pub use scarf::apply_scarf_joint;
pub use support::config::SupportConfig;
pub use support::{SupportRegion, SupportResult};
pub use toolpath::{
    assemble_layer_toolpath, FeatureType, LayerToolpath, ToolpathSegment,
};
