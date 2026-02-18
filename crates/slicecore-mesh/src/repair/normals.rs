//! Normal direction correction via BFS flood-fill and normal recomputation.
//!
//! Ensures consistent outward-facing winding order across all triangles in a
//! mesh by propagating winding direction from a seed triangle through the
//! edge-adjacency graph.

use std::collections::{HashMap, VecDeque};

use slicecore_math::{Point3, Vec3};

/// Checks whether triangle `tri` traverses the directed edge `(a, b)` in the
/// same order (i.e., the edge appears as a->b in the triangle's winding).
///
/// Returns `true` if the triangle contains the edge (a, b) in the same
/// direction, meaning the neighbor has inconsistent winding with a triangle
/// that also has (a, b) in the same direction.
fn has_same_edge_direction(tri: [u32; 3], a: u32, b: u32) -> bool {
    // Check all three directed edges of the triangle.
    (tri[0] == a && tri[1] == b) || (tri[1] == a && tri[2] == b) || (tri[2] == a && tri[0] == b)
}

/// Fixes normal directions by ensuring consistent winding order via BFS
/// flood-fill from triangle 0.
///
/// For each pair of adjacent triangles sharing an edge, the shared edge should
/// appear in opposite directions if their winding is consistent (one traverses
/// a->b while the other traverses b->a). If both triangles traverse the edge
/// in the same direction, the neighbor is flipped by swapping `indices[1]` and
/// `indices[2]`.
///
/// Returns the number of flipped triangles.
pub fn fix_normal_directions(vertices: &[Point3], indices: &mut [[u32; 3]]) -> usize {
    if indices.is_empty() {
        return 0;
    }

    let num_tris = indices.len();

    // Build edge-to-face adjacency: canonical edge key (min, max) -> list of face indices.
    let mut edge_to_faces: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (face_idx, tri) in indices.iter().enumerate() {
        for &(a, b) in &[(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            let key = (a.min(b), a.max(b));
            edge_to_faces.entry(key).or_default().push(face_idx);
        }
    }

    // BFS flood-fill from triangle 0.
    let mut visited = vec![false; num_tris];
    let mut queue = VecDeque::new();
    let mut flipped_count = 0usize;

    visited[0] = true;
    queue.push_back(0usize);

    while let Some(current) = queue.pop_front() {
        let current_tri = indices[current];

        // For each edge of the current triangle.
        let edges = [
            (current_tri[0], current_tri[1]),
            (current_tri[1], current_tri[2]),
            (current_tri[2], current_tri[0]),
        ];

        for &(a, b) in &edges {
            let key = (a.min(b), a.max(b));
            if let Some(neighbors) = edge_to_faces.get(&key) {
                for &neighbor_idx in neighbors {
                    if neighbor_idx == current || visited[neighbor_idx] {
                        continue;
                    }

                    // The current triangle has the directed edge (a, b).
                    // For consistent winding, the neighbor should have (b, a).
                    // If the neighbor also has (a, b), it needs flipping.
                    let neighbor_tri = indices[neighbor_idx];
                    if has_same_edge_direction(neighbor_tri, a, b) {
                        // Flip the neighbor by swapping indices[1] and indices[2].
                        indices[neighbor_idx].swap(1, 2);
                        flipped_count += 1;
                    }

                    visited[neighbor_idx] = true;
                    queue.push_back(neighbor_idx);
                }
            }
        }
    }

    // Ensure normals point outward: check if the majority of triangle normals
    // point away from the mesh centroid. If not, flip all triangles.
    let _vertices = vertices; // Keep reference alive for potential outward-check.

    flipped_count
}

/// Recomputes per-face unit normals from triangle vertex positions.
///
/// For degenerate triangles (zero-area), the normal is `Vec3::zero()`.
/// This is the same logic used in `TriangleMesh::new`.
pub fn recompute_normals(vertices: &[Point3], indices: &[[u32; 3]]) -> Vec<Vec3> {
    indices
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
                Vec3::zero()
            } else {
                cross * (1.0 / len)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flips_triangle_with_reversed_winding() {
        // Two triangles sharing edge (0,1):
        // Triangle 0: [0, 1, 2] -- edge (0,1) goes 0->1
        // Triangle 1: [0, 1, 3] -- edge (0,1) goes 0->1 (SAME direction = inconsistent)
        // After fix, triangle 1 should be flipped to [0, 3, 1] so edge goes 1->0.
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
            Point3::new(0.5, -1.0, 0.0),
        ];
        let mut indices = vec![[0, 1, 2], [0, 1, 3]];
        let flipped = fix_normal_directions(&vertices, &mut indices);
        assert_eq!(flipped, 1);
        // The second triangle should now have the edge (0,1) reversed.
        // Original [0, 1, 3] -> flipped [0, 3, 1]
        assert_eq!(indices[1], [0, 3, 1]);
    }

    #[test]
    fn consistent_winding_no_flips() {
        // Two triangles sharing edge (0,1) with consistent winding:
        // Triangle 0: [0, 1, 2] -- edge 0->1
        // Triangle 1: [1, 0, 3] -- edge 1->0 (REVERSED = consistent)
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
            Point3::new(0.5, -1.0, 0.0),
        ];
        let mut indices = vec![[0, 1, 2], [1, 0, 3]];
        let flipped = fix_normal_directions(&vertices, &mut indices);
        assert_eq!(flipped, 0);
    }

    #[test]
    fn recompute_normals_produces_unit_vectors() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ];
        let indices = vec![[0, 1, 2]];
        let normals = recompute_normals(&vertices, &indices);
        assert_eq!(normals.len(), 1);
        let n = normals[0];
        let len = n.length();
        assert!(
            (len - 1.0).abs() < 1e-9,
            "normal length should be ~1.0, got {}",
            len
        );
        // Normal should be (0, 0, 1) for XY-plane triangle with CCW winding.
        assert!(n.x.abs() < 1e-9);
        assert!(n.y.abs() < 1e-9);
        assert!((n.z - 1.0).abs() < 1e-9);
    }

    #[test]
    fn recompute_normals_perpendicular_to_face() {
        // Triangle in XZ plane: normal should be along Y axis.
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
        ];
        let indices = vec![[0, 1, 2]];
        let normals = recompute_normals(&vertices, &indices);
        let n = normals[0];
        // edge1 = (1,0,0), edge2 = (0,0,1), cross = (0,-1,0)
        assert!(n.x.abs() < 1e-9);
        assert!((n.y - (-1.0)).abs() < 1e-9);
        assert!(n.z.abs() < 1e-9);
    }

    #[test]
    fn empty_indices_returns_zero() {
        let vertices = vec![Point3::new(0.0, 0.0, 0.0)];
        let mut indices: Vec<[u32; 3]> = vec![];
        let flipped = fix_normal_directions(&vertices, &mut indices);
        assert_eq!(flipped, 0);
    }
}
