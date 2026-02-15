//! Mesh statistics computation.
//!
//! Provides [`MeshStats`] which computes volume, surface area, manifold/watertight
//! checks, and other mesh quality metrics from a [`TriangleMesh`].

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use slicecore_math::{BBox3, Vec3};

use crate::triangle_mesh::TriangleMesh;

/// Comprehensive statistics about a triangle mesh.
///
/// Computed by [`compute_stats`] from a [`TriangleMesh`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeshStats {
    /// Number of vertices in the mesh.
    pub vertex_count: usize,
    /// Number of triangles in the mesh.
    pub triangle_count: usize,
    /// Number of degenerate triangles (zero area).
    pub degenerate_count: usize,
    /// Axis-aligned bounding box of the mesh.
    pub aabb: BBox3,
    /// Signed volume via the divergence theorem.
    /// Positive when normals face outward.
    pub volume: f64,
    /// Total surface area (sum of triangle areas).
    pub surface_area: f64,
    /// Whether every edge is shared by exactly 2 triangles.
    pub is_manifold: bool,
    /// Whether the mesh is manifold with no boundary edges.
    pub is_watertight: bool,
    /// Whether all triangle normals face consistently outward.
    pub has_consistent_winding: bool,
}

/// Computes comprehensive statistics for a triangle mesh.
///
/// # Volume Calculation
///
/// Uses the divergence theorem: for each triangle, compute the signed volume
/// of the tetrahedron formed with the origin. Formula: `v0 . (v1 x v2) / 6.0`.
///
/// # Manifold Check
///
/// Builds an edge-to-face-count map. Each edge (sorted vertex pair) should
/// have exactly 2 adjacent faces for a manifold mesh.
///
/// # Winding Consistency
///
/// For shared edges, adjacent triangles should have opposite vertex ordering
/// on the shared edge, indicating consistent outward normals.
pub fn compute_stats(mesh: &TriangleMesh) -> MeshStats {
    let vertex_count = mesh.vertex_count();
    let triangle_count = mesh.triangle_count();
    let aabb = *mesh.aabb();

    // Count degenerate triangles (zero-area normal).
    let normals = mesh.normals();
    let degenerate_count = normals
        .iter()
        .filter(|n| n.length_squared() < 1e-20)
        .count();

    // Compute volume and surface area.
    let mut volume = 0.0f64;
    let mut surface_area = 0.0f64;

    for i in 0..triangle_count {
        let [v0, v1, v2] = mesh.triangle_vertices(i);

        // Signed tetrahedron volume: v0 . (v1 x v2) / 6.0
        let v1_vec = Vec3::new(v1.x, v1.y, v1.z);
        let v2_vec = Vec3::new(v2.x, v2.y, v2.z);
        let cross = v1_vec.cross(v2_vec);
        let v0_vec = Vec3::new(v0.x, v0.y, v0.z);
        volume += v0_vec.dot(cross);

        // Triangle area: 0.5 * |edge1 x edge2|
        let edge1 = Vec3::from_points(v0, v1);
        let edge2 = Vec3::from_points(v0, v2);
        let area_cross = edge1.cross(edge2);
        surface_area += area_cross.length() * 0.5;
    }
    volume /= 6.0;

    // Build edge map for manifold and winding checks.
    // Key: sorted (min, max) vertex indices. Value: list of (tri_idx, directed edge).
    type EdgeFaces = Vec<(usize, u32, u32)>;
    let mut edge_map: HashMap<(u32, u32), EdgeFaces> = HashMap::new();

    for i in 0..triangle_count {
        let tri = mesh.indices()[i];
        // Three edges per triangle.
        let edges = [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])];
        for &(a, b) in &edges {
            let key = if a < b { (a, b) } else { (b, a) };
            edge_map.entry(key).or_default().push((i, a, b));
        }
    }

    // Manifold: every edge shared by exactly 2 triangles.
    // Watertight: manifold + no boundary edges.
    let mut is_manifold = true;
    let mut has_boundary = false;
    let mut has_consistent_winding = true;

    for faces in edge_map.values() {
        match faces.len() {
            1 => {
                has_boundary = true;
                // Single face = boundary edge, not manifold in strict sense
                // but we keep is_manifold true unless >2 faces share an edge
            }
            2 => {
                // Check winding consistency: for a manifold mesh with consistent
                // outward normals, two triangles sharing an edge should traverse
                // that edge in opposite directions.
                let (_, a1, b1) = faces[0];
                let (_, a2, b2) = faces[1];
                // If both triangles traverse the edge in the same direction,
                // winding is inconsistent.
                if a1 == a2 && b1 == b2 {
                    has_consistent_winding = false;
                }
            }
            _ => {
                // More than 2 faces share this edge = non-manifold.
                is_manifold = false;
            }
        }
    }

    let is_watertight = is_manifold && !has_boundary;

    MeshStats {
        vertex_count,
        triangle_count,
        degenerate_count,
        aabb,
        volume,
        surface_area,
        is_manifold,
        is_watertight,
        has_consistent_winding,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::triangle_mesh::tests::unit_cube;
    use slicecore_math::Point3;

    #[test]
    fn unit_cube_volume_is_one() {
        let mesh = unit_cube();
        let stats = compute_stats(&mesh);
        assert!(
            (stats.volume - 1.0).abs() < 1e-6,
            "Expected volume ~1.0, got {}",
            stats.volume
        );
    }

    #[test]
    fn unit_cube_surface_area_is_six() {
        let mesh = unit_cube();
        let stats = compute_stats(&mesh);
        assert!(
            (stats.surface_area - 6.0).abs() < 1e-6,
            "Expected surface area ~6.0, got {}",
            stats.surface_area
        );
    }

    #[test]
    fn unit_cube_is_manifold_and_watertight() {
        let mesh = unit_cube();
        let stats = compute_stats(&mesh);
        assert!(stats.is_manifold, "Expected unit cube to be manifold");
        assert!(stats.is_watertight, "Expected unit cube to be watertight");
    }

    #[test]
    fn single_triangle_is_not_watertight() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ];
        let mesh = TriangleMesh::new(vertices, vec![[0, 1, 2]]).unwrap();
        let stats = compute_stats(&mesh);
        assert!(
            !stats.is_watertight,
            "Single triangle should not be watertight"
        );
    }

    #[test]
    fn degenerate_count_zero_for_clean_cube() {
        let mesh = unit_cube();
        let stats = compute_stats(&mesh);
        assert_eq!(
            stats.degenerate_count, 0,
            "Unit cube should have no degenerate triangles"
        );
    }

    #[test]
    fn degenerate_triangle_counted() {
        // Create a mesh with one valid triangle and one degenerate (collinear vertices).
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(2.0, 0.0, 0.0), // collinear with 0 and 1
        ];
        let mesh = TriangleMesh::new(vertices, vec![[0, 1, 2], [0, 1, 3]]).unwrap();
        let stats = compute_stats(&mesh);
        assert_eq!(
            stats.degenerate_count, 1,
            "Should have 1 degenerate triangle"
        );
    }

    #[test]
    fn non_manifold_edge_detected() {
        // Create 3 triangles sharing one edge (0-1), which is non-manifold.
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
            Point3::new(0.5, -1.0, 0.0),
            Point3::new(0.5, 0.5, 1.0),
        ];
        let mesh = TriangleMesh::new(vertices, vec![[0, 1, 2], [0, 1, 3], [0, 1, 4]]).unwrap();
        let stats = compute_stats(&mesh);
        assert!(
            !stats.is_manifold,
            "3 triangles sharing one edge is non-manifold"
        );
    }

    #[test]
    fn single_triangle_returns_correct_stats() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ];
        let mesh = TriangleMesh::new(vertices, vec![[0, 1, 2]]).unwrap();
        let stats = compute_stats(&mesh);
        assert_eq!(stats.vertex_count, 3);
        assert_eq!(stats.triangle_count, 1);
        assert!((stats.surface_area - 0.5).abs() < 1e-6);
    }
}
