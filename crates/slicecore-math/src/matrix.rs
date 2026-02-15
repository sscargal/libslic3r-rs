//! Matrix types for affine transformations.
//!
//! Provides 3x3 matrices (2D transforms) and 4x4 matrices (3D transforms)
//! with factory methods for common operations like translation, rotation,
//! scaling, and mirroring.

use serde::{Deserialize, Serialize};

/// A 3x3 matrix for 2D affine transformations (using homogeneous coordinates).
///
/// Stored in row-major order: `data[row][col]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Matrix3x3 {
    pub data: [[f64; 3]; 3],
}

/// A 4x4 matrix for 3D affine transformations (using homogeneous coordinates).
///
/// Stored in row-major order: `data[row][col]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Matrix4x4 {
    pub data: [[f64; 4]; 4],
}
