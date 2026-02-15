//! Foundation math types for the slicecore 3D slicing engine.
//!
//! This crate provides the core geometric primitives used by every other crate
//! in the slicing pipeline:
//!
//! - **Integer coordinates** ([`Coord`], [`IPoint2`]) for deterministic polygon
//!   operations with nanometer precision
//! - **Floating-point points** ([`Point2`], [`Point3`]) for mesh vertices and
//!   continuous-space calculations
//! - **Vectors** ([`Vec2`], [`Vec3`]) for directions, normals, and displacements
//! - **Bounding boxes** ([`BBox2`], [`BBox3`], [`IBBox2`]) for spatial queries
//! - **Matrices** ([`Matrix3x3`], [`Matrix4x4`]) for affine transformations
//! - **Conversion utilities** ([`mm_to_coord`], [`coord_to_mm`]) for bridging
//!   float and integer coordinate spaces
//! - **Epsilon constants** ([`EPSILON`], [`AREA_EPSILON`]) for floating-point
//!   comparison
//!
//! # Coordinate System
//!
//! The engine uses two coordinate spaces:
//!
//! 1. **Float space** (millimeters): Used for mesh vertices, user-facing
//!    dimensions, and geometric calculations where floating-point is appropriate.
//!
//! 2. **Integer space** (nanometers): Used for polygon boolean operations,
//!    path planning, and any algorithm requiring deterministic arithmetic.
//!    1 mm = 1,000,000 internal units ([`COORD_SCALE`]).

pub mod bbox;
pub mod convert;
pub mod coord;
pub mod epsilon;
pub mod matrix;
pub mod point;
pub mod vec;

// Re-export core types at crate root for ergonomic imports.
pub use bbox::{BBox2, BBox3, IBBox2};
pub use convert::{coord_to_mm, ipoints_to_points, mm_to_coord, points_to_ipoints};
pub use coord::{Coord, IPoint2, COORD_SCALE};
pub use epsilon::{approx_eq, approx_zero, AREA_EPSILON, EPSILON};
pub use matrix::{Matrix3x3, Matrix4x4};
pub use point::{Point2, Point3};
pub use vec::{Vec2, Vec3};
