//! Core types for CSG boolean operations.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// The four boolean operations supported by the CSG module.
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::BooleanOp;
///
/// let op = BooleanOp::Union;
/// assert_eq!(format!("{op:?}"), "Union");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BooleanOp {
    /// Combine two meshes into one (A + B).
    Union,
    /// Subtract mesh B from mesh A (A - B).
    Difference,
    /// Keep only the overlapping region (A & B).
    Intersection,
    /// Keep everything except the overlapping region (A ^ B).
    Xor,
}

/// A lightweight cancellation handle for long-running CSG operations.
///
/// Wraps an `Arc<AtomicBool>` so all clones observe cancellation immediately.
/// Compatible with the `CancellationToken` in `slicecore-engine` (same
/// semantics) but defined independently to avoid a circular dependency.
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::CsgCancellationToken;
///
/// let token = CsgCancellationToken::new();
/// assert!(!token.is_cancelled());
/// token.cancel();
/// assert!(token.is_cancelled());
/// ```
#[derive(Clone, Debug)]
pub struct CsgCancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CsgCancellationToken {
    /// Creates a new token in the non-cancelled state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Requests cancellation. All clones observe this immediately.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Returns `true` if cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

impl Default for CsgCancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// Options controlling CSG operation behavior.
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::CsgOptions;
///
/// let opts = CsgOptions::default();
/// assert!(opts.validate_output);
/// assert!(!opts.parallel);
/// assert!(opts.cancellation_token.is_none());
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CsgOptions {
    /// Whether to validate the output mesh is manifold after the operation.
    pub validate_output: bool,
    /// Whether to use parallel computation (requires the `parallel` feature).
    pub parallel: bool,
    /// Optional cancellation token to stop long-running operations early.
    ///
    /// When set and triggered, the pipeline returns [`super::CsgError::Cancelled`].
    #[serde(skip)]
    pub cancellation_token: Option<CsgCancellationToken>,
}

impl Default for CsgOptions {
    fn default() -> Self {
        Self {
            validate_output: true,
            parallel: false,
            cancellation_token: None,
        }
    }
}

/// Per-triangle attribute data for CSG operations.
///
/// Tracks material assignment and optional color for each triangle.
/// These attributes propagate through boolean operations so the output
/// mesh retains provenance information.
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::TriangleAttributes;
///
/// let attr = TriangleAttributes::default();
/// assert_eq!(attr.material_id, 0);
/// assert!(attr.color.is_none());
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriangleAttributes {
    /// Material identifier for this triangle.
    pub material_id: u32,
    /// Optional RGBA color for this triangle.
    pub color: Option<[u8; 4]>,
}
