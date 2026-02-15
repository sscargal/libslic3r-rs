//! TriangleMesh data structure with arena+index pattern.
//!
//! The mesh stores vertices in a flat `Vec<Point3>` and triangle face indices
//! in `Vec<[u32; 3]>`, following the arena+index pattern for cache-friendly
//! access. Per-face normals and the axis-aligned bounding box are computed
//! on construction.
//!
//! A BVH spatial index is built lazily on the first spatial query call,
//! using `std::sync::OnceLock` for thread-safe lazy initialization.

use std::sync::OnceLock;

use slicecore_math::{BBox3, Point3, Vec3};

use crate::bvh::BVH;
use crate::error::MeshError;

/// A triangle mesh stored in the arena+index pattern.
///
/// Vertices are stored in a flat array, and triangles reference vertices
/// by index. Per-face normals and the overall AABB are computed during
/// construction. The BVH spatial index is built lazily on first access.
///
/// `TriangleMesh` is automatically `Send + Sync` because all its fields
/// (`Vec<Point3>`, `Vec<[u32; 3]>`, `Vec<Vec3>`, `BBox3`, `OnceLock<BVH>`)
/// are `Send + Sync`.
pub struct TriangleMesh {
    /// Vertex positions.
    vertices: Vec<Point3>,
    /// Triangle face indices into the vertices array.
    indices: Vec<[u32; 3]>,
    /// Per-face normals, computed on construction.
    /// Degenerate triangles (zero area) have `Vec3::zero()`.
    normals: Vec<Vec3>,
    /// Axis-aligned bounding box enclosing all vertices.
    aabb: BBox3,
    /// Lazily-built BVH spatial index.
    bvh: OnceLock<BVH>,
}

impl TriangleMesh {
    /// Constructs a new `TriangleMesh` from vertices and triangle indices.
    ///
    /// # Errors
    ///
    /// - [`MeshError::EmptyMesh`] if `vertices` is empty.
    /// - [`MeshError::NoTriangles`] if `indices` is empty.
    /// - [`MeshError::IndexOutOfBounds`] if any triangle index exceeds the
    ///   vertex array length.
    pub fn new(vertices: Vec<Point3>, indices: Vec<[u32; 3]>) -> Result<Self, MeshError> {
        if vertices.is_empty() {
            return Err(MeshError::EmptyMesh);
        }
        if indices.is_empty() {
            return Err(MeshError::NoTriangles);
        }

        // Validate all indices are within bounds.
        let vertex_count = vertices.len();
        for tri in &indices {
            for &idx in tri {
                if idx as usize >= vertex_count {
                    return Err(MeshError::IndexOutOfBounds(idx, vertex_count));
                }
            }
        }

        // Compute per-face normals.
        let normals: Vec<Vec3> = indices
            .iter()
            .map(|tri| {
                let v0 = vertices[tri[0] as usize];
                let v1 = vertices[tri[1] as usize];
                let v2 = vertices[tri[2] as usize];

                let edge1 = Vec3::from_points(v0, v1);
                let edge2 = Vec3::from_points(v0, v2);
                let cross = edge1.cross(edge2);
                let len = cross.length();

                if len < 1e-30 {
                    Vec3::zero() // degenerate triangle
                } else {
                    cross * (1.0 / len) // normalize
                }
            })
            .collect();

        // Compute AABB from all vertices.
        let aabb = BBox3::from_points(&vertices)
            .expect("vertices is non-empty, so BBox3::from_points returns Some");

        Ok(Self {
            vertices,
            indices,
            normals,
            aabb,
            bvh: OnceLock::new(),
        })
    }

    /// Returns a reference to the vertex positions.
    #[inline]
    pub fn vertices(&self) -> &[Point3] {
        &self.vertices
    }

    /// Returns a reference to the triangle face indices.
    #[inline]
    pub fn indices(&self) -> &[[u32; 3]] {
        &self.indices
    }

    /// Returns a reference to the per-face normals.
    #[inline]
    pub fn normals(&self) -> &[Vec3] {
        &self.normals
    }

    /// Returns a reference to the mesh bounding box.
    #[inline]
    pub fn aabb(&self) -> &BBox3 {
        &self.aabb
    }

    /// Returns the number of vertices.
    #[inline]
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Returns the number of triangles.
    #[inline]
    pub fn triangle_count(&self) -> usize {
        self.indices.len()
    }

    /// Returns the three vertex positions for the triangle at `tri_idx`.
    ///
    /// # Panics
    ///
    /// Panics if `tri_idx` is out of bounds.
    #[inline]
    pub fn triangle_vertices(&self, tri_idx: usize) -> [Point3; 3] {
        let tri = self.indices[tri_idx];
        [
            self.vertices[tri[0] as usize],
            self.vertices[tri[1] as usize],
            self.vertices[tri[2] as usize],
        ]
    }

    /// Returns a reference to the BVH spatial index, building it lazily on
    /// first access.
    ///
    /// This is thread-safe: if multiple threads call `bvh()` concurrently,
    /// the BVH is built exactly once.
    #[inline]
    pub fn bvh(&self) -> &BVH {
        self.bvh
            .get_or_init(|| BVH::build(&self.vertices, &self.indices))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Creates a unit cube mesh (vertices from (0,0,0) to (1,1,1)) with 12 triangles.
    pub(crate) fn unit_cube() -> TriangleMesh {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0), // 0: left-bottom-back
            Point3::new(1.0, 0.0, 0.0), // 1: right-bottom-back
            Point3::new(1.0, 1.0, 0.0), // 2: right-top-back
            Point3::new(0.0, 1.0, 0.0), // 3: left-top-back
            Point3::new(0.0, 0.0, 1.0), // 4: left-bottom-front
            Point3::new(1.0, 0.0, 1.0), // 5: right-bottom-front
            Point3::new(1.0, 1.0, 1.0), // 6: right-top-front
            Point3::new(0.0, 1.0, 1.0), // 7: left-top-front
        ];

        // Two triangles per face, 6 faces = 12 triangles.
        // Winding order: outward-facing normals (CCW when viewed from outside).
        let indices = vec![
            // Front face (z=1): 4,5,6 and 4,6,7
            [4, 5, 6],
            [4, 6, 7],
            // Back face (z=0): 1,0,3 and 1,3,2
            [1, 0, 3],
            [1, 3, 2],
            // Right face (x=1): 1,2,6 and 1,6,5
            [1, 2, 6],
            [1, 6, 5],
            // Left face (x=0): 0,4,7 and 0,7,3
            [0, 4, 7],
            [0, 7, 3],
            // Top face (y=1): 3,7,6 and 3,6,2
            [3, 7, 6],
            [3, 6, 2],
            // Bottom face (y=0): 0,1,5 and 0,5,4
            [0, 1, 5],
            [0, 5, 4],
        ];

        TriangleMesh::new(vertices, indices).expect("unit cube should be valid")
    }

    #[test]
    fn construct_from_valid_vertices_and_indices() {
        let mesh = unit_cube();
        assert_eq!(mesh.vertex_count(), 8);
        assert_eq!(mesh.triangle_count(), 12);
    }

    #[test]
    fn empty_vertices_returns_empty_mesh_error() {
        let result = TriangleMesh::new(vec![], vec![[0, 1, 2]]);
        assert!(matches!(result, Err(MeshError::EmptyMesh)));
    }

    #[test]
    fn out_of_bounds_index_returns_error() {
        let vertices = vec![Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)];
        let result = TriangleMesh::new(vertices, vec![[0, 1, 5]]);
        assert!(matches!(result, Err(MeshError::IndexOutOfBounds(5, 2))));
    }

    #[test]
    fn normals_computed_correctly_for_known_triangle() {
        // Triangle in XY plane: (0,0,0), (1,0,0), (0,1,0)
        // Normal should be (0,0,1) (pointing up in Z).
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ];
        let mesh = TriangleMesh::new(vertices, vec![[0, 1, 2]]).unwrap();
        let normal = mesh.normals()[0];
        assert!((normal.x).abs() < 1e-9);
        assert!((normal.y).abs() < 1e-9);
        assert!((normal.z - 1.0).abs() < 1e-9);
    }

    #[test]
    fn aabb_matches_expected_for_unit_cube() {
        let mesh = unit_cube();
        let aabb = mesh.aabb();
        assert!((aabb.min.x - 0.0).abs() < 1e-9);
        assert!((aabb.min.y - 0.0).abs() < 1e-9);
        assert!((aabb.min.z - 0.0).abs() < 1e-9);
        assert!((aabb.max.x - 1.0).abs() < 1e-9);
        assert!((aabb.max.y - 1.0).abs() < 1e-9);
        assert!((aabb.max.z - 1.0).abs() < 1e-9);
    }

    #[test]
    fn send_sync_compile_time_check() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TriangleMesh>();
    }

    #[test]
    fn triangle_vertices_accessor() {
        let mesh = unit_cube();
        let verts = mesh.triangle_vertices(0);
        // First triangle is [4,5,6]: front face
        assert_eq!(verts[0], Point3::new(0.0, 0.0, 1.0));
        assert_eq!(verts[1], Point3::new(1.0, 0.0, 1.0));
        assert_eq!(verts[2], Point3::new(1.0, 1.0, 1.0));
    }

    #[test]
    fn no_triangles_returns_error() {
        let vertices = vec![Point3::new(0.0, 0.0, 0.0)];
        let result = TriangleMesh::new(vertices, vec![]);
        assert!(matches!(result, Err(MeshError::NoTriangles)));
    }
}
