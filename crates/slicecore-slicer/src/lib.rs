//! Mesh-to-contour slicing for the slicecore 3D slicing engine.
//!
//! This crate transforms a [`TriangleMesh`](slicecore_mesh::TriangleMesh) into
//! sliced layer contours by:
//!
//! 1. Computing layer heights from the mesh bounding box
//! 2. Intersecting triangles with horizontal Z-planes
//! 3. Chaining intersection segments into closed contour polygons
//! 4. Validating contours with known winding (CCW = outer, CW = hole)
//!
//! # Key Functions
//!
//! - [`slice_mesh`]: Main entry point -- slices a mesh into layers
//! - [`slice_at_height`]: Slices a mesh at a single Z height
//! - [`compute_layer_heights`]: Computes Z heights for slicing
//!
//! # Example
//!
//! ```ignore
//! use slicecore_slicer::{slice_mesh, SliceLayer};
//!
//! let layers = slice_mesh(&mesh, 0.2, 0.3);
//! for layer in &layers {
//!     println!("z={:.2}: {} contours", layer.z, layer.contours.len());
//! }
//! ```

pub mod adaptive;
pub mod contour;
pub mod layer;
pub mod resolve;

// Re-export primary types and functions at crate root.
pub use adaptive::compute_adaptive_layer_heights;
pub use contour::{slice_at_height, slice_at_height_resolved};
pub use layer::{
    compute_layer_heights, slice_mesh, slice_mesh_adaptive, slice_mesh_adaptive_resolved,
    slice_mesh_resolved, SliceLayer,
};
pub use resolve::resolve_contour_intersections;
