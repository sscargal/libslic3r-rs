//! Constructive Solid Geometry (CSG) module.
//!
//! Provides boolean operations (union, difference, intersection, xor) on
//! triangle meshes, along with primitive mesh generators, error types, and
//! diagnostic reports.
//!
//! # Module Structure
//!
//! - [`error`] -- Error types for CSG operations
//! - [`report`] -- Diagnostic report for operation results
//! - [`types`] -- Core types: [`BooleanOp`], [`CsgOptions`], [`TriangleAttributes`]
//! - [`primitives`] -- Nine watertight mesh primitive generators

pub mod classify;
pub mod error;
pub mod intersect;
pub mod perturb;
pub mod primitives;
pub mod report;
pub mod retriangulate;
pub mod types;

// Re-export key types at module level.
pub use error::CsgError;
pub use primitives::{
    primitive_box, primitive_cone, primitive_cylinder, primitive_ngon_prism, primitive_plane,
    primitive_rounded_box, primitive_sphere, primitive_torus, primitive_wedge,
};
pub use report::CsgReport;
pub use types::{BooleanOp, CsgOptions, TriangleAttributes};
