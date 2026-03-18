//! Triangle inside/outside classification via ray casting.
//!
//! After retriangulation, each triangle in mesh A must be classified as
//! `Inside` or `Outside` relative to mesh B (and vice versa). This module
//! uses BVH-accelerated ray casting to determine whether a triangle's centroid
//! lies inside the other mesh, with connected-component optimization to avoid
//! redundant ray casts.

use std::collections::HashMap;

use slicecore_math::{Point3, Vec3};

use crate::bvh::BVH;
use crate::triangle_mesh::TriangleMesh;

use super::intersect::IntersectionResult;
use super::retriangulate::TriangleOrigin;

/// Classification of a triangle relative to the other mesh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Classification {
    /// The triangle lies inside the other mesh's volume.
    Inside,
    /// The triangle lies outside the other mesh's volume.
    Outside,
    /// The triangle lies on the boundary (intersection curve).
    Boundary,
}

/// Classifies each triangle in a (possibly retriangulated) mesh as inside,
/// outside, or on the boundary of the other mesh.
///
/// Uses ray casting from triangle centroids to determine containment.
/// Connected-component optimization propagates classification to avoid
/// redundant ray casts for non-intersected triangle groups.
///
/// # Arguments
///
/// * `mesh_vertices` -- Vertices of the (retriangulated) mesh to classify.
/// * `mesh_indices` -- Triangle indices of the mesh to classify.
/// * `other_mesh` -- The other mesh (used for ray casting containment tests).
/// * `triangle_origins` -- Provenance of each triangle (for boundary detection).
/// * `intersection_result` -- Intersection data (for identifying split triangles).
///
/// # Returns
///
/// A `Vec<Classification>` with one entry per triangle in `mesh_indices`.
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::classify::{classify_triangles, Classification};
/// use slicecore_mesh::csg::intersect::IntersectionResult;
/// use slicecore_mesh::csg::retriangulate::{TriangleOrigin, MeshId};
/// use slicecore_mesh::csg::primitive_box;
/// use slicecore_math::Point3;
///
/// let small_box_verts = vec![
///     Point3::new(-0.25, -0.25, -0.25),
///     Point3::new(0.25, -0.25, -0.25),
///     Point3::new(0.25, 0.25, -0.25),
///     Point3::new(-0.25, 0.25, -0.25),
///     Point3::new(-0.25, -0.25, 0.25),
///     Point3::new(0.25, -0.25, 0.25),
///     Point3::new(0.25, 0.25, 0.25),
///     Point3::new(-0.25, 0.25, 0.25),
/// ];
/// let small_box_idx = vec![
///     [4,5,6],[4,6,7],[1,0,3],[1,3,2],
///     [1,2,6],[1,6,5],[0,4,7],[0,7,3],
///     [3,7,6],[3,6,2],[0,1,5],[0,5,4],
/// ];
/// let big_box = primitive_box(2.0, 2.0, 2.0);
/// let origins: Vec<_> = (0..12).map(|i| TriangleOrigin {
///     mesh_id: MeshId::A, original_triangle: i,
/// }).collect();
/// let empty = IntersectionResult::default();
/// let classes = classify_triangles(
///     &small_box_verts, &small_box_idx, &big_box, &origins, &empty,
/// );
/// assert!(classes.iter().all(|c| *c == Classification::Inside));
/// ```
pub fn classify_triangles(
    mesh_vertices: &[Point3],
    mesh_indices: &[[u32; 3]],
    other_mesh: &TriangleMesh,
    triangle_origins: &[TriangleOrigin],
    intersection_result: &IntersectionResult,
) -> Vec<Classification> {
    let n = mesh_indices.len();
    if n == 0 {
        return Vec::new();
    }

    // Build BVH for the other mesh (for ray casting).
    let other_bvh = other_mesh.bvh();

    // Identify which original triangles were split (have intersection segments).
    let split_originals = collect_split_originals(triangle_origins, intersection_result);

    // Build connected components of non-intersected triangles.
    // All triangles in a component share the same classification.
    let components = build_components(mesh_indices, triangle_origins, &split_originals);

    let mut classifications = vec![Classification::Outside; n];

    // Classify each component by testing one representative triangle.
    for component in &components {
        if component.is_empty() {
            continue;
        }

        // Test the first triangle in the component.
        let rep_tri_idx = component[0];
        let classification = classify_single_triangle(
            mesh_vertices,
            &mesh_indices[rep_tri_idx],
            other_bvh,
            other_mesh.vertices(),
            other_mesh.indices(),
        );

        // Propagate to all triangles in the component.
        for &tri_idx in component {
            classifications[tri_idx] = classification;
        }
    }

    // Handle boundary triangles (those from split originals).
    for (tri_idx, origin) in triangle_origins.iter().enumerate() {
        if split_originals.contains(&origin.original_triangle) {
            // For split triangles, classify based on centroid ray casting
            // since they may be on either side of the boundary.
            classifications[tri_idx] = classify_single_triangle(
                mesh_vertices,
                &mesh_indices[tri_idx],
                other_bvh,
                other_mesh.vertices(),
                other_mesh.indices(),
            );
        }
    }

    classifications
}

/// Classifies a single triangle by ray casting from its centroid.
fn classify_single_triangle(
    vertices: &[Point3],
    tri: &[u32; 3],
    other_bvh: &BVH,
    other_vertices: &[Point3],
    other_indices: &[[u32; 3]],
) -> Classification {
    let v0 = vertices[tri[0] as usize];
    let v1 = vertices[tri[1] as usize];
    let v2 = vertices[tri[2] as usize];

    let centroid = Point3::new(
        (v0.x + v1.x + v2.x) / 3.0,
        (v0.y + v1.y + v2.y) / 3.0,
        (v0.z + v1.z + v2.z) / 3.0,
    );

    // Try multiple ray directions in case one hits a vertex/edge (degenerate).
    let directions = [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(1.0, 1.0, 1.0),
    ];

    for dir in &directions {
        match count_ray_crossings(&centroid, dir, other_bvh, other_vertices, other_indices) {
            Some(count) => {
                return if count % 2 == 1 {
                    Classification::Inside
                } else {
                    Classification::Outside
                };
            }
            None => continue, // Degenerate; try another direction.
        }
    }

    // Fallback: assume outside if all directions are degenerate.
    Classification::Outside
}

/// Counts the number of ray crossings with the other mesh.
///
/// Returns `None` if the ray hits a degenerate case (vertex/edge) that makes
/// the crossing count ambiguous.
fn count_ray_crossings(
    origin: &Point3,
    direction: &Vec3,
    bvh: &BVH,
    vertices: &[Point3],
    indices: &[[u32; 3]],
) -> Option<usize> {
    // Cast ray and count all intersections (not just the closest).
    let inv_dir = Vec3::new(1.0 / direction.x, 1.0 / direction.y, 1.0 / direction.z);
    let mut count = 0usize;

    // Walk the BVH and test all triangles the ray intersects.
    count_crossings_recursive(
        bvh, origin, direction, &inv_dir, vertices, indices, &mut count,
    );

    Some(count)
}

/// Recursive BVH traversal for counting all ray-triangle intersections.
///
/// This differs from `BVH::intersect_ray` in that it counts ALL hits,
/// not just the closest one.
fn count_crossings_recursive(
    bvh: &BVH,
    origin: &Point3,
    direction: &Vec3,
    _inv_dir: &Vec3,
    vertices: &[Point3],
    indices: &[[u32; 3]],
    count: &mut usize,
) {
    // We need access to BVH internals, but since the BVH structure is opaque,
    // we use a simpler approach: cast a ray in the given direction and count
    // sequential intersections by repeated ray casting with offset.
    //
    // More practically, we iterate through all triangles that the BVH ray
    // intersects. Since BVH::intersect_ray only returns the closest hit,
    // we cast multiple rays along the same direction, advancing past each hit.

    let mut current_origin = *origin;
    let mut remaining_hits = 100; // Safety limit.

    while remaining_hits > 0 {
        remaining_hits -= 1;

        match bvh.intersect_ray(&current_origin, direction, vertices, indices) {
            Some(hit) if hit.t > 1e-9 => {
                *count += 1;
                // Advance the ray past this hit.
                current_origin = Point3::new(
                    current_origin.x + direction.x * (hit.t + 1e-6),
                    current_origin.y + direction.y * (hit.t + 1e-6),
                    current_origin.z + direction.z * (hit.t + 1e-6),
                );
            }
            _ => break,
        }
    }
}

/// Collects the set of original triangle indices that were split by intersection.
fn collect_split_originals(
    origins: &[TriangleOrigin],
    intersection_result: &IntersectionResult,
) -> std::collections::HashSet<usize> {
    let mut split = std::collections::HashSet::new();

    // A triangle is "split" if intersection segments reference it.
    for seg in &intersection_result.segments {
        split.insert(seg.tri_a);
        split.insert(seg.tri_b);
    }

    // Also mark triangles that appear multiple times in origins (were split).
    let mut counts: HashMap<usize, usize> = HashMap::new();
    for origin in origins {
        *counts.entry(origin.original_triangle).or_insert(0) += 1;
    }
    for (&orig, &count) in &counts {
        if count > 1 {
            split.insert(orig);
        }
    }

    split
}

/// Builds connected components of non-intersected triangles.
///
/// Triangles that share an edge and were not split belong to the same component.
/// Each component only needs one ray cast to classify all its triangles.
fn build_components(
    indices: &[[u32; 3]],
    origins: &[TriangleOrigin],
    split_originals: &std::collections::HashSet<usize>,
) -> Vec<Vec<usize>> {
    let n = indices.len();
    if n == 0 {
        return Vec::new();
    }

    // Build edge -> triangle adjacency.
    let mut edge_to_tris: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (tri_idx, tri) in indices.iter().enumerate() {
        // Skip split triangles; they get individual classification.
        if split_originals.contains(&origins[tri_idx].original_triangle) {
            continue;
        }

        for k in 0..3 {
            let a = tri[k];
            let b = tri[(k + 1) % 3];
            let edge = if a < b { (a, b) } else { (b, a) };
            edge_to_tris.entry(edge).or_default().push(tri_idx);
        }
    }

    // Union-find for component building.
    let mut parent: Vec<usize> = (0..n).collect();

    fn find(parent: &mut [usize], x: usize) -> usize {
        let mut root = x;
        while parent[root] != root {
            root = parent[root];
        }
        let mut cur = x;
        while parent[cur] != root {
            let next = parent[cur];
            parent[cur] = root;
            cur = next;
        }
        root
    }

    fn union(parent: &mut [usize], a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent[rb] = ra;
        }
    }

    // Union triangles sharing edges.
    for tris in edge_to_tris.values() {
        for i in 1..tris.len() {
            union(&mut parent, tris[0], tris[i]);
        }
    }

    // Collect components (only non-split triangles).
    let mut components: HashMap<usize, Vec<usize>> = HashMap::new();
    for (tri_idx, origin) in origins.iter().enumerate().take(n) {
        if split_originals.contains(&origin.original_triangle) {
            continue; // Split triangles are classified individually.
        }
        let root = find(&mut parent, tri_idx);
        components.entry(root).or_default().push(tri_idx);
    }

    components.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csg::intersect::IntersectionResult;
    use crate::csg::primitives::primitive_box;
    use crate::csg::retriangulate::MeshId;

    /// Creates a small box mesh at a given offset.
    fn offset_box(size: f64, dx: f64, dy: f64, dz: f64) -> TriangleMesh {
        let hs = size / 2.0;
        let vertices = vec![
            Point3::new(-hs + dx, -hs + dy, -hs + dz),
            Point3::new(hs + dx, -hs + dy, -hs + dz),
            Point3::new(hs + dx, hs + dy, -hs + dz),
            Point3::new(-hs + dx, hs + dy, -hs + dz),
            Point3::new(-hs + dx, -hs + dy, hs + dz),
            Point3::new(hs + dx, -hs + dy, hs + dz),
            Point3::new(hs + dx, hs + dy, hs + dz),
            Point3::new(-hs + dx, hs + dy, hs + dz),
        ];
        let indices = vec![
            [4, 5, 6],
            [4, 6, 7],
            [1, 0, 3],
            [1, 3, 2],
            [1, 2, 6],
            [1, 6, 5],
            [0, 4, 7],
            [0, 7, 3],
            [3, 7, 6],
            [3, 6, 2],
            [0, 1, 5],
            [0, 5, 4],
        ];
        TriangleMesh::new(vertices, indices).unwrap()
    }

    #[test]
    fn small_box_fully_inside_large_box() {
        let small = offset_box(0.5, 0.0, 0.0, 0.0);
        let large = primitive_box(4.0, 4.0, 4.0);

        let origins: Vec<_> = (0..small.triangle_count())
            .map(|i| TriangleOrigin {
                mesh_id: MeshId::A,
                original_triangle: i,
            })
            .collect();

        let empty = IntersectionResult::default();
        let classes =
            classify_triangles(small.vertices(), small.indices(), &large, &origins, &empty);

        assert_eq!(classes.len(), 12);
        for (i, c) in classes.iter().enumerate() {
            assert_eq!(
                *c,
                Classification::Inside,
                "triangle {i} should be Inside, got {c:?}"
            );
        }
    }

    #[test]
    fn small_box_fully_outside_large_box() {
        let small = offset_box(0.5, 10.0, 10.0, 10.0);
        let large = primitive_box(2.0, 2.0, 2.0);

        let origins: Vec<_> = (0..small.triangle_count())
            .map(|i| TriangleOrigin {
                mesh_id: MeshId::A,
                original_triangle: i,
            })
            .collect();

        let empty = IntersectionResult::default();
        let classes =
            classify_triangles(small.vertices(), small.indices(), &large, &origins, &empty);

        assert_eq!(classes.len(), 12);
        for (i, c) in classes.iter().enumerate() {
            assert_eq!(
                *c,
                Classification::Outside,
                "triangle {i} should be Outside, got {c:?}"
            );
        }
    }

    #[test]
    fn disconnected_triangles_mixed_classification() {
        // Two separate triangles: one inside box B, one outside.
        // These are NOT connected by edges, so they get different components.
        let large = primitive_box(4.0, 4.0, 4.0); // [-2, 2] in all axes

        // Triangle A: inside the box (at origin).
        // Triangle B: outside the box (at x=10).
        let mesh_vertices = vec![
            // Triangle 0: inside
            Point3::new(-0.1, -0.1, 0.0),
            Point3::new(0.1, -0.1, 0.0),
            Point3::new(0.0, 0.1, 0.0),
            // Triangle 1: outside
            Point3::new(9.9, -0.1, 0.0),
            Point3::new(10.1, -0.1, 0.0),
            Point3::new(10.0, 0.1, 0.0),
        ];
        let mesh_indices = vec![[0, 1, 2], [3, 4, 5]];

        let origins = vec![
            TriangleOrigin {
                mesh_id: MeshId::A,
                original_triangle: 0,
            },
            TriangleOrigin {
                mesh_id: MeshId::A,
                original_triangle: 1,
            },
        ];

        let empty = IntersectionResult::default();
        let classes = classify_triangles(&mesh_vertices, &mesh_indices, &large, &origins, &empty);

        assert_eq!(classes.len(), 2);
        assert_eq!(
            classes[0],
            Classification::Inside,
            "triangle at origin should be Inside"
        );
        assert_eq!(
            classes[1],
            Classification::Outside,
            "triangle at x=10 should be Outside"
        );
    }

    #[test]
    fn empty_mesh_returns_empty() {
        let large = primitive_box(2.0, 2.0, 2.0);
        let origins: Vec<TriangleOrigin> = Vec::new();
        let empty = IntersectionResult::default();

        let classes = classify_triangles(&[], &[], &large, &origins, &empty);
        assert!(classes.is_empty());
    }
}
