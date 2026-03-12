//! Core types for CSG boolean operations.

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
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CsgOptions {
    /// Whether to validate the output mesh is manifold after the operation.
    pub validate_output: bool,
    /// Whether to use parallel computation (requires the `parallel` feature).
    pub parallel: bool,
}

impl Default for CsgOptions {
    fn default() -> Self {
        Self {
            validate_output: true,
            parallel: false,
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
