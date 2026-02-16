//! Slicing pipeline orchestrator for the slicecore 3D slicing engine.
//!
//! This crate ties together all pipeline stages: mesh loading, slicing,
//! perimeter generation, infill, toolpath planning, and G-code emission.
//!
//! Current pipeline modules:
//! - [`config`]: Print configuration with TOML deserialization
//! - [`perimeter`]: Perimeter shell generation via polygon offsetting
//! - [`infill`]: Rectilinear infill pattern generation
//!
//! Future pipeline modules:
//! - Surface classification (plan 03-03)
//! - Toolpath planning (plan 03-04)
//! - G-code generation (plan 03-05)
//!
//! # Configuration
//!
//! [`PrintConfig`] contains all parameters controlling the pipeline.
//! It supports TOML deserialization with `#[serde(default)]`, so any
//! unspecified fields use sensible FDM defaults.

pub mod config;
pub mod error;
pub mod infill;
pub mod perimeter;

// Future pipeline modules:
// pub mod surface;
// pub mod toolpath;
// pub mod planner;
// pub mod gcode_gen;

// Re-export primary types at crate root.
pub use config::{PrintConfig, WallOrder};
pub use error::EngineError;
pub use infill::{
    alternate_infill_angle, generate_rectilinear_infill, InfillLine, LayerInfill,
};
pub use perimeter::{generate_perimeters, ContourPerimeters, PerimeterShell};
