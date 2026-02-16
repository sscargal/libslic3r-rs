//! Slicing pipeline orchestrator for the slicecore 3D slicing engine.
//!
//! This crate ties together all pipeline stages: mesh loading, slicing,
//! perimeter generation, infill, toolpath planning, and G-code emission.
//!
//! In Phase 3, only the [`config`] module is implemented. Pipeline modules
//! will be added in subsequent plans:
//!
//! - Perimeter generation (plan 03-02)
//! - Infill pattern generation (plan 03-03)
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

// Future pipeline modules:
// pub mod perimeter;
// pub mod infill;
// pub mod surface;
// pub mod toolpath;
// pub mod planner;
// pub mod gcode_gen;

// Re-export primary types at crate root.
pub use config::{PrintConfig, WallOrder};
pub use error::EngineError;
