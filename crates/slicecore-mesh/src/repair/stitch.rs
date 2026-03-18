//! Edge stitching for nearby unconnected edges.
//!
//! Merges nearby vertices (within a configurable tolerance) to close gaps in
//! meshes that have duplicated vertices at seams. Uses spatial hashing for
//! efficient neighbor lookup.

use std::collections::HashMap;

use slicecore_math::{Point3, Vec3};

/// Default tolerance for edge stitching: 0.1 micron (well below FDM print
/// resolution of ~50 microns). This catches vertices that are effectively
/// at the same position but stored separately due to import/export rounding.
pub const STITCH_TOLERANCE: f64 = 1e-4;

/// Stitches nearby unconnected edges by merging vertices within `tolerance`.
///
/// Uses spatial hashing to find merge candidates efficiently:
/// 1. Quantize vertex positions into grid cells of size `tolerance`
/// 2. For each vertex, check the 27 neighboring cells for merge candidates
/// 3. Merge: update all index references from the duplicate vertex to the
///    kept vertex
///
/// After merging, removes any degenerate triangles created by the merge.
///
/// Returns the number of edges stitched (vertex pairs merged).
pub fn stitch_edges(vertices: &[Point3], indices: &mut Vec<[u32; 3]>, tolerance: f64) -> usize {
    if vertices.is_empty() || indices.is_empty() {
        return 0;
    }

    // Find boundary edges (edges appearing in only one triangle).
    let mut edge_count: HashMap<(u32, u32), usize> = HashMap::new();
    for tri in indices.iter() {
        for &(a, b) in &[(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            let key = (a.min(b), a.max(b));
            *edge_count.entry(key).or_insert(0) += 1;
        }
    }

    // Collect boundary vertex indices (vertices on edges with count == 1).
    let mut boundary_vertices: Vec<u32> = Vec::new();
    let mut is_boundary = vec![false; vertices.len()];
    for (&(a, b), &count) in &edge_count {
        if count == 1 {
            if !is_boundary[a as usize] {
                is_boundary[a as usize] = true;
                boundary_vertices.push(a);
            }
            if !is_boundary[b as usize] {
                is_boundary[b as usize] = true;
                boundary_vertices.push(b);
            }
        }
    }

    if boundary_vertices.is_empty() {
        return 0;
    }

    // Spatial hashing: quantize positions to grid cells.
    let inv_cell = 1.0 / tolerance;
    let mut grid: HashMap<(i64, i64, i64), Vec<u32>> = HashMap::new();

    for &vi in &boundary_vertices {
        let p = vertices[vi as usize];
        let cell = (
            (p.x * inv_cell).floor() as i64,
            (p.y * inv_cell).floor() as i64,
            (p.z * inv_cell).floor() as i64,
        );
        grid.entry(cell).or_default().push(vi);
    }

    // Build merge map: for each vertex, the canonical vertex it maps to.
    let mut merge_to: Vec<u32> = (0..vertices.len() as u32).collect();
    let mut stitched = 0usize;

    let tol_sq = tolerance * tolerance;

    for &vi in &boundary_vertices {
        // Skip if already merged to another vertex.
        if merge_to[vi as usize] != vi {
            continue;
        }

        let p = vertices[vi as usize];
        let cell = (
            (p.x * inv_cell).floor() as i64,
            (p.y * inv_cell).floor() as i64,
            (p.z * inv_cell).floor() as i64,
        );

        // Check 27 neighboring cells.
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let neighbor_cell = (cell.0 + dx, cell.1 + dy, cell.2 + dz);
                    if let Some(candidates) = grid.get(&neighbor_cell) {
                        for &cand in candidates {
                            if cand <= vi || merge_to[cand as usize] != cand {
                                continue;
                            }
                            let q = vertices[cand as usize];
                            let d = Vec3::from_points(p, q);
                            if d.length_squared() < tol_sq {
                                // Merge cand -> vi
                                merge_to[cand as usize] = vi;
                                stitched += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    if stitched == 0 {
        return 0;
    }

    // Resolve transitive merges.
    for i in 0..merge_to.len() {
        let mut target = merge_to[i];
        while merge_to[target as usize] != target {
            target = merge_to[target as usize];
        }
        merge_to[i] = target;
    }

    // Update all indices to use the canonical vertex.
    for tri in indices.iter_mut() {
        tri[0] = merge_to[tri[0] as usize];
        tri[1] = merge_to[tri[1] as usize];
        tri[2] = merge_to[tri[2] as usize];
    }

    // Remove degenerate triangles created by merging.
    indices.retain(|tri| tri[0] != tri[1] && tri[1] != tri[2] && tri[0] != tri[2]);

    stitched
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stitches_nearby_duplicate_vertices() {
        // Two triangles with vertices at almost the same position on the shared edge.
        let eps = 1e-5; // Well within tolerance of 1e-4.
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),       // 0
            Point3::new(1.0, 0.0, 0.0),       // 1
            Point3::new(0.5, 1.0, 0.0),       // 2
            Point3::new(0.0 + eps, 0.0, 0.0), // 3 (near-duplicate of 0)
            Point3::new(1.0 + eps, 0.0, 0.0), // 4 (near-duplicate of 1)
            Point3::new(0.5, -1.0, 0.0),      // 5
        ];
        // Triangle 0: [0,1,2], Triangle 1: [4,3,5]
        // Edge (0,1) of tri 0 and edge (3,4) of tri 1 are at the same position.
        let mut indices = vec![[0, 1, 2], [4, 3, 5]];
        let stitched = stitch_edges(&vertices, &mut indices, STITCH_TOLERANCE);
        assert!(stitched > 0, "Expected stitching, got {}", stitched);
        // After stitching, the duplicate vertices should have been merged.
        // Both triangles should reference the same vertices for the shared edge.
        assert_eq!(indices.len(), 2, "Both triangles should still exist");
    }

    #[test]
    fn does_not_stitch_beyond_tolerance() {
        // Two triangles with vertices clearly far apart (> tolerance).
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
            Point3::new(10.0, 0.0, 0.0), // Far from all triangle 0 vertices
            Point3::new(11.0, 0.0, 0.0), // Far from all triangle 0 vertices
            Point3::new(10.5, -1.0, 0.0), // Far from all triangle 0 vertices
        ];
        let mut indices = vec![[0, 1, 2], [3, 4, 5]];
        let stitched = stitch_edges(&vertices, &mut indices, STITCH_TOLERANCE);
        assert_eq!(stitched, 0, "Should not stitch vertices beyond tolerance");
    }

    #[test]
    fn empty_mesh_returns_zero() {
        let vertices: Vec<Point3> = vec![];
        let mut indices: Vec<[u32; 3]> = vec![];
        let stitched = stitch_edges(&vertices, &mut indices, STITCH_TOLERANCE);
        assert_eq!(stitched, 0);
    }
}
