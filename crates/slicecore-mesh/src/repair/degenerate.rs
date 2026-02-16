//! Degenerate triangle removal.
//!
//! Removes triangles that have zero area: either because two or more vertex
//! indices are identical, or because the vertices are collinear (cross product
//! magnitude below threshold).

use slicecore_math::{Point3, Vec3};

/// Threshold for cross product magnitude squared below which a triangle is
/// considered degenerate (zero area). This is ~1e-20 in area, well below
/// any meaningful geometry.
const DEGENERATE_AREA_THRESHOLD: f64 = 1e-20;

/// Removes degenerate triangles from the index list.
///
/// A triangle is degenerate if:
/// - Any two vertex indices are equal (e.g., `tri[0] == tri[1]`)
/// - The triangle area is below threshold (cross product magnitude < threshold)
///
/// Returns the number of removed triangles.
pub fn remove_degenerate_triangles(vertices: &[Point3], indices: &mut Vec<[u32; 3]>) -> usize {
    let before = indices.len();
    indices.retain(|tri| {
        // Check for duplicate vertex indices.
        if tri[0] == tri[1] || tri[1] == tri[2] || tri[0] == tri[2] {
            return false;
        }

        // Check for zero-area triangle (collinear vertices).
        let v0 = vertices[tri[0] as usize];
        let v1 = vertices[tri[1] as usize];
        let v2 = vertices[tri[2] as usize];

        let edge1 = Vec3::from_points(v0, v1);
        let edge2 = Vec3::from_points(v0, v2);
        let cross = edge1.cross(edge2);

        cross.length_squared() >= DEGENERATE_AREA_THRESHOLD
    });
    before - indices.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_triangle_with_duplicate_vertex_indices() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ];
        // One valid triangle, one degenerate (indices 0,0,1).
        let mut indices = vec![[0, 1, 2], [0, 0, 1]];
        let removed = remove_degenerate_triangles(&vertices, &mut indices);
        assert_eq!(removed, 1);
        assert_eq!(indices.len(), 1);
        assert_eq!(indices[0], [0, 1, 2]);
    }

    #[test]
    fn removes_triangle_with_collinear_vertices() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0), // collinear with first two
            Point3::new(0.0, 1.0, 0.0), // non-collinear
        ];
        // Triangle [0,1,2] is collinear (zero area), [0,1,3] is valid.
        let mut indices = vec![[0, 1, 2], [0, 1, 3]];
        let removed = remove_degenerate_triangles(&vertices, &mut indices);
        assert_eq!(removed, 1);
        assert_eq!(indices.len(), 1);
        assert_eq!(indices[0], [0, 1, 3]);
    }

    #[test]
    fn retains_all_valid_triangles() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
        ];
        let mut indices = vec![[0, 1, 2], [0, 1, 3], [0, 2, 3], [1, 2, 3]];
        let removed = remove_degenerate_triangles(&vertices, &mut indices);
        assert_eq!(removed, 0);
        assert_eq!(indices.len(), 4);
    }

    #[test]
    fn removes_all_degenerate_from_mixed_set() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ];
        let mut indices = vec![
            [0, 1, 2], // valid
            [0, 0, 1], // degenerate: duplicate
            [1, 1, 2], // degenerate: duplicate
            [0, 2, 0], // degenerate: duplicate
        ];
        let removed = remove_degenerate_triangles(&vertices, &mut indices);
        assert_eq!(removed, 3);
        assert_eq!(indices.len(), 1);
    }
}
