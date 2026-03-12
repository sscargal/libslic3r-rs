//! Error types for CSG (Constructive Solid Geometry) operations.

use thiserror::Error;

use crate::error::MeshError;

/// Errors that can occur during CSG boolean operations.
///
/// Each variant captures a specific failure mode so callers can decide
/// whether to retry, repair, or abort.
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::CsgError;
///
/// let err = CsgError::Cancelled;
/// assert_eq!(format!("{err}"), "operation cancelled");
/// ```
#[derive(Debug, Error)]
pub enum CsgError {
    /// Mesh A could not be repaired to a valid manifold before the operation.
    #[error("mesh A repair failed: {0}")]
    RepairFailedA(#[source] MeshError),

    /// Mesh B could not be repaired to a valid manifold before the operation.
    #[error("mesh B repair failed: {0}")]
    RepairFailedB(#[source] MeshError),

    /// The boolean operation produced zero triangles.
    #[error("boolean operation `{operation}` produced no triangles")]
    EmptyResult {
        /// The operation that was attempted.
        operation: String,
    },

    /// A triangle-triangle intersection computation failed.
    #[error("intersection failed between triangles {tri_a} and {tri_b}: {reason}")]
    IntersectionFailed {
        /// Index of the first triangle.
        tri_a: usize,
        /// Index of the second triangle.
        tri_b: usize,
        /// Human-readable reason for the failure.
        reason: String,
    },

    /// The output mesh could not be constructed from the result triangles.
    #[error("result mesh construction failed: {0}")]
    ResultConstruction(#[source] MeshError),

    /// The operation was cancelled via a cancellation token.
    #[error("operation cancelled")]
    Cancelled,

    /// The output mesh is not watertight (has non-manifold edges).
    #[error("result mesh has {non_manifold_edges} non-manifold edges")]
    NonManifoldResult {
        /// Number of edges that are not shared by exactly two triangles.
        non_manifold_edges: usize,
    },
}
