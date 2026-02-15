//! Triangle mesh data structures with BVH spatial indexing.
//!
//! This crate provides the 3D mesh representation that the slicing pipeline
//! operates on. Key design choices:
//!
//! - **Arena+index pattern**: Vertices stored in flat `Vec<Point3>`, triangles
//!   reference vertices by `u32` index. No `Rc`/`RefCell`/`Cell` anywhere.
//! - **Send+Sync**: [`TriangleMesh`] is automatically thread-safe via `OnceLock`.
//! - **Lazy BVH**: The [`BVH`] spatial index is built on first query, not on
//!   construction. This avoids paying the build cost for meshes that are only
//!   read, not spatially queried.
//! - **SAH-based BVH**: Uses Surface Area Heuristic for optimal partitioning,
//!   following PBRT Chapter 4 methodology.
//!
//! # Key Queries
//!
//! - [`query_triangles_at_z`]: Finds triangles intersecting a horizontal plane
//!   (called once per slice layer).
//! - [`ray_cast`]: Finds the closest triangle hit by a ray (used for support
//!   generation).

pub mod bvh;
pub mod error;
pub mod spatial;
pub mod stats;
pub mod transform;
pub mod triangle_mesh;

// Re-export primary types at crate root.
pub use bvh::{RayHit, BVH};
pub use error::MeshError;
pub use spatial::{closest_point_on_mesh, query_triangles_at_z, ray_cast};
pub use stats::{compute_stats, MeshStats};
pub use transform::{center_on_origin, mirror, place_on_bed, rotate, scale, transform, translate, MirrorAxis};
pub use triangle_mesh::TriangleMesh;
