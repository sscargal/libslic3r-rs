//! Error types for mesh operations.

use thiserror::Error;

/// Errors that can occur during mesh construction or validation.
#[derive(Debug, Error)]
pub enum MeshError {
    /// The mesh has no vertices.
    #[error("mesh has no vertices")]
    EmptyMesh,

    /// The mesh has no triangles.
    #[error("mesh has no triangles")]
    NoTriangles,

    /// A triangle index is out of bounds for the vertex array.
    #[error("triangle index {0} out of bounds (mesh has {1} vertices)")]
    IndexOutOfBounds(u32, usize),

    /// A triangle has zero area (degenerate).
    #[error("degenerate triangle at index {0} (zero area)")]
    DegenerateTriangle(usize),

    /// The mesh is not manifold.
    #[error("mesh is not manifold: {0}")]
    NonManifold(String),
}
