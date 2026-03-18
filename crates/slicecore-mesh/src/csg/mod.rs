//! Constructive Solid Geometry (CSG) module.
//!
//! Provides boolean operations (union, difference, intersection, xor) on
//! triangle meshes, along with primitive mesh generators, error types, and
//! diagnostic reports.
//!
//! # Module Structure
//!
//! - [`boolean`] -- Public boolean API: union, difference, intersection, xor
//! - [`volume`] -- Signed volume and surface area computation
//! - [`error`] -- Error types for CSG operations
//! - [`report`] -- Diagnostic report for operation results
//! - [`types`] -- Core types: [`BooleanOp`], [`CsgOptions`], [`TriangleAttributes`]
//! - [`primitives`] -- Nine watertight mesh primitive generators
//! - [`split`] -- Plane splitting of triangle meshes
//! - [`offset`] -- Vertex-normal mesh offset operations
//! - [`hollow`] -- Mesh hollowing via offset and CSG difference

pub mod boolean;
pub mod classify;
pub mod error;
pub mod hollow;
pub mod intersect;
pub mod offset;
pub mod perturb;
pub mod primitives;
pub mod report;
pub mod retriangulate;
pub mod split;
pub mod types;
pub mod volume;

// Re-export key types at module level.
pub use boolean::{
    mesh_difference, mesh_difference_with, mesh_intersection, mesh_intersection_with, mesh_union,
    mesh_union_many, mesh_union_with, mesh_xor, mesh_xor_with,
};
pub use error::CsgError;
pub use hollow::{hollow_mesh, DrainHole, HollowOptions};
pub use offset::mesh_offset;
pub use primitives::{
    primitive_box, primitive_cone, primitive_cylinder, primitive_ngon_prism, primitive_plane,
    primitive_rounded_box, primitive_sphere, primitive_torus, primitive_wedge,
};
pub use report::CsgReport;
pub use split::{mesh_split_at_plane, SplitOptions, SplitPlane, SplitResult};
pub use types::{BooleanOp, CsgCancellationToken, CsgOptions, TriangleAttributes};
