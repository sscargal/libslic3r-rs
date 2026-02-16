//! Triangle-plane intersection and segment chaining for contour extraction.
//!
//! This module implements the core slicing algorithm:
//! 1. Intersect each triangle with a horizontal Z-plane to produce 2D line segments
//! 2. Chain segments into closed contour polygons
//! 3. Validate contours and classify winding (CCW = outer, CW = hole)

use std::collections::HashMap;

use slicecore_geo::{Polygon, ValidPolygon};
use slicecore_math::{IPoint2, Point2, Point3};
use slicecore_mesh::{query_triangles_at_z, TriangleMesh};

/// Epsilon for vertex-on-plane classification.
const PLANE_EPSILON: f64 = 1e-12;

/// Intersects a triangle with a horizontal plane at the given Z height.
///
/// Returns `Some((start, end))` when exactly two distinct intersection points
/// are found (the triangle crosses the plane). Returns `None` for degenerate
/// cases: coplanar triangles, single-vertex touches, or triangles entirely
/// above/below the plane.
///
/// The intersection points are in floating-point (mm) 2D space.
pub fn intersect_triangle_z_plane(
    v0: Point3,
    v1: Point3,
    v2: Point3,
    z: f64,
) -> Option<(Point2, Point2)> {
    let d0 = v0.z - z;
    let d1 = v1.z - z;
    let d2 = v2.z - z;

    // Classify each vertex as above (+1), on (0), or below (-1)
    let c0 = classify(d0);
    let c1 = classify(d1);
    let c2 = classify(d2);

    // Count vertices on the plane
    let on_count = [c0, c1, c2].iter().filter(|&&c| c == 0).count();

    // Coplanar triangle: all three vertices on the plane
    if on_count == 3 {
        return None;
    }

    // Two vertices on the plane: the edge between them is the intersection
    if on_count == 2 {
        let verts = [(v0, c0), (v1, c1), (v2, c2)];
        let on_verts: Vec<Point2> = verts
            .iter()
            .filter(|(_, c)| *c == 0)
            .map(|(v, _)| Point2::new(v.x, v.y))
            .collect();
        return Some((on_verts[0], on_verts[1]));
    }

    // One vertex on the plane: only produces a segment if the other two are
    // on opposite sides
    if on_count == 1 {
        let verts = [(v0, c0, d0), (v1, c1, d1), (v2, c2, d2)];
        let on_vert = verts.iter().find(|(_, c, _)| *c == 0).unwrap();
        let others: Vec<_> = verts.iter().filter(|(_, c, _)| *c != 0).collect();

        // Both other vertices on the same side -> single point touch, skip
        if others[0].1 == others[1].1 {
            return None;
        }

        // Other two vertices on opposite sides: intersection is the on-vertex
        // plus the edge crossing between the other two
        let p_on = Point2::new(on_vert.0.x, on_vert.0.y);
        let p_cross = interpolate_edge(&others[0].0, &others[1].0, others[0].2, others[1].2);
        return Some((p_on, p_cross));
    }

    // No vertices on the plane: find edges that cross
    let edges = [
        (v0, v1, d0, d1),
        (v1, v2, d1, d2),
        (v2, v0, d2, d0),
    ];

    let mut intersections = Vec::with_capacity(2);
    for &(va, vb, da, db) in &edges {
        let ca = classify(da);
        let cb = classify(db);

        if ca == 0 {
            intersections.push(Point2::new(va.x, va.y));
        } else if cb == 0 {
            // Will be picked up when this vertex is va in another edge
            continue;
        } else if ca != cb {
            intersections.push(interpolate_edge(&va, &vb, da, db));
        }
    }

    if intersections.len() == 2 {
        Some((intersections[0], intersections[1]))
    } else {
        None
    }
}

/// Chains a set of line segments (in integer coordinates) into closed contour
/// polygons.
///
/// Segments are connected endpoint-to-endpoint using exact integer coordinate
/// matching. Each connected chain that closes (last point == first point) and
/// has at least 3 points is returned as a contour.
///
/// Open chains (mesh defects) are silently skipped.
pub fn chain_segments(segments: Vec<(IPoint2, IPoint2)>) -> Vec<Vec<IPoint2>> {
    if segments.is_empty() {
        return Vec::new();
    }

    // Build adjacency map: start_point -> Vec<(end_point, segment_index)>
    let mut adjacency: HashMap<IPoint2, Vec<(IPoint2, usize)>> =
        HashMap::with_capacity(segments.len());
    for (idx, &(start, end)) in segments.iter().enumerate() {
        adjacency
            .entry(start)
            .or_default()
            .push((end, idx));
    }

    let mut used = vec![false; segments.len()];
    let mut contours = Vec::new();

    for start_idx in 0..segments.len() {
        if used[start_idx] {
            continue;
        }

        let first_point = segments[start_idx].0;
        let mut chain = vec![first_point];
        let mut current = segments[start_idx].1;
        used[start_idx] = true;
        chain.push(current);

        // Walk the chain
        let mut closed = false;
        loop {
            if current == first_point {
                closed = true;
                break;
            }

            // Find an unused segment starting at `current`
            let next = adjacency.get(&current).and_then(|neighbors| {
                neighbors
                    .iter()
                    .find(|(_, idx)| !used[*idx])
                    .copied()
            });

            match next {
                Some((end, idx)) => {
                    used[idx] = true;
                    current = end;
                    chain.push(current);
                }
                None => break, // Open chain (mesh defect)
            }
        }

        if closed && chain.len() > 3 {
            // Remove the closing duplicate point (first == last)
            chain.pop();
            contours.push(chain);
        }
    }

    contours
}

/// Slices a mesh at a specific Z height, returning validated contour polygons.
///
/// Uses BVH-accelerated triangle lookup via `query_triangles_at_z`, then
/// intersects each candidate triangle with the Z-plane, chains resulting
/// segments into closed contours, and validates them.
///
/// Winding convention: CCW = outer boundary, CW = hole.
pub fn slice_at_height(mesh: &TriangleMesh, z: f64) -> Vec<ValidPolygon> {
    // Get candidate triangles from BVH
    let tri_indices = query_triangles_at_z(mesh, z);

    // Intersect each triangle with the Z-plane
    let mut segments: Vec<(IPoint2, IPoint2)> = Vec::new();
    for tri_idx in tri_indices {
        let [v0, v1, v2] = mesh.triangle_vertices(tri_idx);
        if let Some((p0, p1)) = intersect_triangle_z_plane(v0, v1, v2, z) {
            let ip0 = IPoint2::from_mm(p0.x, p0.y);
            let ip1 = IPoint2::from_mm(p1.x, p1.y);
            // Skip degenerate segments (same start and end)
            if ip0 != ip1 {
                segments.push((ip0, ip1));
            }
        }
    }

    // Chain segments into closed contours
    let chains = chain_segments(segments);

    // Validate each contour and collect successful ones
    let mut contours = Vec::new();
    for chain in chains {
        let polygon = Polygon::new(chain);
        if let Ok(valid) = polygon.validate() {
            contours.push(valid);
        }
    }

    contours
}

/// Classifies a signed distance as above (+1), on (0), or below (-1) the plane.
#[inline]
fn classify(d: f64) -> i32 {
    if d.abs() < PLANE_EPSILON {
        0
    } else if d > 0.0 {
        1
    } else {
        -1
    }
}

/// Linearly interpolates the intersection point on an edge between two
/// vertices with signed distances `da` and `db` from the cutting plane.
#[inline]
fn interpolate_edge(va: &Point3, vb: &Point3, da: f64, db: f64) -> Point2 {
    let t = da / (da - db);
    Point2::new(va.x + t * (vb.x - va.x), va.y + t * (vb.y - va.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: create a unit cube mesh (0,0,0) to (1,1,1) with 12 triangles
    fn unit_cube() -> TriangleMesh {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0), // 0
            Point3::new(1.0, 0.0, 0.0), // 1
            Point3::new(1.0, 1.0, 0.0), // 2
            Point3::new(0.0, 1.0, 0.0), // 3
            Point3::new(0.0, 0.0, 1.0), // 4
            Point3::new(1.0, 0.0, 1.0), // 5
            Point3::new(1.0, 1.0, 1.0), // 6
            Point3::new(0.0, 1.0, 1.0), // 7
        ];
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
    fn intersect_triangle_crossing_plane() {
        // Triangle from z=0 to z=1, crossing z=0.5
        let v0 = Point3::new(0.0, 0.0, 0.0);
        let v1 = Point3::new(1.0, 0.0, 1.0);
        let v2 = Point3::new(0.0, 1.0, 1.0);

        let result = intersect_triangle_z_plane(v0, v1, v2, 0.5);
        assert!(result.is_some(), "Triangle crossing z=0.5 should produce a segment");

        let (p0, p1) = result.unwrap();
        // At z=0.5, the intersection line should have y or x coordinates at 0.5
        // v0->v1: t=0.5, point = (0.5, 0.0)
        // v0->v2: t=0.5, point = (0.0, 0.5)
        let points = [p0, p1];
        let has_half_x = points.iter().any(|p| (p.x - 0.5).abs() < 1e-9 && p.y.abs() < 1e-9);
        let has_half_y = points.iter().any(|p| p.x.abs() < 1e-9 && (p.y - 0.5).abs() < 1e-9);
        assert!(has_half_x, "Expected intersection at (0.5, 0.0), got {:?}", points);
        assert!(has_half_y, "Expected intersection at (0.0, 0.5), got {:?}", points);
    }

    #[test]
    fn intersect_triangle_fully_above() {
        let v0 = Point3::new(0.0, 0.0, 2.0);
        let v1 = Point3::new(1.0, 0.0, 3.0);
        let v2 = Point3::new(0.0, 1.0, 4.0);

        let result = intersect_triangle_z_plane(v0, v1, v2, 0.5);
        assert!(result.is_none(), "Triangle fully above plane should return None");
    }

    #[test]
    fn intersect_triangle_fully_below() {
        let v0 = Point3::new(0.0, 0.0, -3.0);
        let v1 = Point3::new(1.0, 0.0, -2.0);
        let v2 = Point3::new(0.0, 1.0, -1.0);

        let result = intersect_triangle_z_plane(v0, v1, v2, 0.5);
        assert!(result.is_none(), "Triangle fully below plane should return None");
    }

    #[test]
    fn intersect_triangle_vertex_on_plane_others_opposite() {
        // v0 is on the plane, v1 above, v2 below
        let v0 = Point3::new(0.0, 0.0, 0.5);
        let v1 = Point3::new(1.0, 0.0, 1.0);
        let v2 = Point3::new(0.0, 1.0, 0.0);

        let result = intersect_triangle_z_plane(v0, v1, v2, 0.5);
        assert!(
            result.is_some(),
            "Vertex on plane with others on opposite sides should produce a segment"
        );
    }

    #[test]
    fn intersect_triangle_vertex_on_plane_others_same_side() {
        // v0 is on the plane, v1 and v2 both above
        let v0 = Point3::new(0.0, 0.0, 0.5);
        let v1 = Point3::new(1.0, 0.0, 1.0);
        let v2 = Point3::new(0.0, 1.0, 2.0);

        let result = intersect_triangle_z_plane(v0, v1, v2, 0.5);
        assert!(
            result.is_none(),
            "Vertex on plane with others on same side should return None"
        );
    }

    #[test]
    fn intersect_triangle_coplanar() {
        // All vertices on the plane
        let v0 = Point3::new(0.0, 0.0, 0.5);
        let v1 = Point3::new(1.0, 0.0, 0.5);
        let v2 = Point3::new(0.0, 1.0, 0.5);

        let result = intersect_triangle_z_plane(v0, v1, v2, 0.5);
        assert!(result.is_none(), "Coplanar triangle should return None");
    }

    #[test]
    fn chain_segments_square() {
        // 4 segments forming a square (CCW)
        let p0 = IPoint2::from_mm(0.0, 0.0);
        let p1 = IPoint2::from_mm(1.0, 0.0);
        let p2 = IPoint2::from_mm(1.0, 1.0);
        let p3 = IPoint2::from_mm(0.0, 1.0);

        let segments = vec![(p0, p1), (p1, p2), (p2, p3), (p3, p0)];
        let contours = chain_segments(segments);

        assert_eq!(contours.len(), 1, "Should produce exactly 1 contour");
        assert_eq!(contours[0].len(), 4, "Square contour should have 4 points");
    }

    #[test]
    fn chain_segments_two_separate_contours() {
        // Two separate squares
        let a0 = IPoint2::from_mm(0.0, 0.0);
        let a1 = IPoint2::from_mm(1.0, 0.0);
        let a2 = IPoint2::from_mm(1.0, 1.0);
        let a3 = IPoint2::from_mm(0.0, 1.0);

        let b0 = IPoint2::from_mm(5.0, 5.0);
        let b1 = IPoint2::from_mm(6.0, 5.0);
        let b2 = IPoint2::from_mm(6.0, 6.0);
        let b3 = IPoint2::from_mm(5.0, 6.0);

        let segments = vec![
            (a0, a1),
            (a1, a2),
            (a2, a3),
            (a3, a0),
            (b0, b1),
            (b1, b2),
            (b2, b3),
            (b3, b0),
        ];
        let contours = chain_segments(segments);
        assert_eq!(contours.len(), 2, "Should produce exactly 2 contours");
    }

    #[test]
    fn chain_segments_empty() {
        let contours = chain_segments(Vec::new());
        assert!(contours.is_empty());
    }

    #[test]
    fn slice_at_height_unit_cube() {
        let mesh = unit_cube();
        let contours = slice_at_height(&mesh, 0.5);

        assert_eq!(
            contours.len(),
            1,
            "Slicing unit cube at z=0.5 should produce exactly 1 contour"
        );

        let contour = &contours[0];
        assert_eq!(
            contour.len(),
            4,
            "Unit cube cross-section should be a square with 4 vertices"
        );

        // Check area is approximately 1mm^2 (unit cube cross-section)
        let area = contour.area_mm2();
        assert!(
            (area - 1.0).abs() < 0.01,
            "Expected area ~1.0 mm^2, got {} mm^2",
            area
        );
    }
}
