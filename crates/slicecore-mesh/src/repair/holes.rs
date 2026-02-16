//! Hole detection and filling.
//!
//! Detects boundary edge loops (holes) in a mesh and fills them using fan
//! triangulation from the first vertex of each loop.

use slicecore_math::Point3;

use std::collections::HashMap;

/// Fills holes in the mesh by detecting boundary edge loops and triangulating
/// them with a fan from the first vertex of each loop.
///
/// A boundary edge is a directed edge that has no matching reverse edge in any
/// other triangle. Boundary edges form loops around holes in the mesh.
///
/// Returns the number of triangles added to fill the holes.
pub fn fill_holes(_vertices: &[Point3], indices: &mut Vec<[u32; 3]>) -> usize {
    if indices.is_empty() {
        return 0;
    }

    // Build directed edge set: count how many times each directed edge (a->b)
    // appears across all triangles.
    let mut directed_edges: HashMap<(u32, u32), usize> = HashMap::new();
    for tri in indices.iter() {
        for &(a, b) in &[(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            *directed_edges.entry((a, b)).or_insert(0) += 1;
        }
    }

    // Find boundary edges: directed edge (a, b) where (b, a) does not exist.
    // These are edges on the boundary of a hole.
    let mut boundary: HashMap<u32, Vec<u32>> = HashMap::new();
    for &(a, b) in directed_edges.keys() {
        if !directed_edges.contains_key(&(b, a)) {
            boundary.entry(a).or_default().push(b);
        }
    }

    if boundary.is_empty() {
        return 0;
    }

    // Chain boundary edges into loops.
    let mut visited_edges: HashMap<(u32, u32), bool> = HashMap::new();
    let mut loops: Vec<Vec<u32>> = Vec::new();

    for &start in boundary.keys() {
        // Try to trace a loop from each unvisited boundary vertex.
        let nexts = match boundary.get(&start) {
            Some(n) => n.clone(),
            None => continue,
        };

        for next in nexts {
            if *visited_edges.get(&(start, next)).unwrap_or(&false) {
                continue;
            }

            let mut loop_verts = vec![start];
            let mut current = next;
            visited_edges.insert((start, next), true);

            loop {
                loop_verts.push(current);

                // Find the next boundary edge from current.
                let next_options = match boundary.get(&current) {
                    Some(opts) => opts,
                    None => break,
                };

                let mut found_next = None;
                for &candidate in next_options {
                    if !*visited_edges.get(&(current, candidate)).unwrap_or(&false) {
                        found_next = Some(candidate);
                        break;
                    }
                }

                match found_next {
                    Some(n) => {
                        visited_edges.insert((current, n), true);
                        if n == start {
                            // Loop closed.
                            break;
                        }
                        current = n;
                    }
                    None => break, // Dead end, not a full loop.
                }
            }

            // Only keep loops that actually close (at least 3 vertices).
            if loop_verts.len() >= 3 {
                loops.push(loop_verts);
            }
        }
    }

    // Fill each hole using fan triangulation from the first vertex.
    let mut triangles_added = 0usize;
    for hole_loop in &loops {
        if hole_loop.len() < 3 {
            continue;
        }

        // Fan triangulation: connect vertex 0 to each consecutive pair.
        // Reverse winding to match the surrounding mesh (boundary edges go
        // opposite to the fill triangles).
        let anchor = hole_loop[0];
        for i in 1..hole_loop.len() - 1 {
            let a = hole_loop[i];
            let b = hole_loop[i + 1];
            // Use reversed winding so the fill faces inward consistently
            // with the boundary edge direction.
            indices.push([anchor, b, a]);
            triangles_added += 1;
        }
    }

    triangles_added
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a unit cube mesh with one face (2 triangles) removed.
    fn cube_missing_top_face() -> (Vec<Point3>, Vec<[u32; 3]>) {
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

        // Unit cube with consistent CCW winding (outward-facing), minus the
        // top face (y=1).
        let indices = vec![
            // Front face (z=1)
            [4, 5, 6],
            [4, 6, 7],
            // Back face (z=0)
            [1, 0, 3],
            [1, 3, 2],
            // Right face (x=1)
            [1, 2, 6],
            [1, 6, 5],
            // Left face (x=0)
            [0, 4, 7],
            [0, 7, 3],
            // Bottom face (y=0)
            [0, 1, 5],
            [0, 5, 4],
            // Top face (y=1) REMOVED: [3, 7, 6] and [3, 6, 2]
        ];

        (vertices, indices)
    }

    #[test]
    fn fills_hole_in_cube_missing_one_face() {
        let (vertices, mut indices) = cube_missing_top_face();
        let initial_count = indices.len();
        let filled = fill_holes(&vertices, &mut indices);
        assert!(
            filled > 0,
            "Expected at least one triangle to fill the hole"
        );
        assert!(
            indices.len() > initial_count,
            "Triangles should have been added"
        );
    }

    #[test]
    fn complete_cube_has_no_holes() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(1.0, 1.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
        ];
        let mut indices = vec![
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
        let filled = fill_holes(&vertices, &mut indices);
        assert_eq!(filled, 0);
        assert_eq!(indices.len(), 12);
    }

    #[test]
    fn empty_mesh_returns_zero() {
        let vertices: Vec<Point3> = vec![];
        let mut indices: Vec<[u32; 3]> = vec![];
        let filled = fill_holes(&vertices, &mut indices);
        assert_eq!(filled, 0);
    }
}
