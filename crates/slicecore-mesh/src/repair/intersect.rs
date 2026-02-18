//! Self-intersection detection using BVH acceleration.
//!
//! Detects pairs of triangles that intersect each other (not counting shared
//! edges/vertices). Uses the BVH for broad-phase spatial culling and the
//! Moller triangle-triangle intersection algorithm for narrow-phase testing.
//!
//! This module performs detection only -- it does not resolve intersections.

use slicecore_math::{BBox3, Point3, Vec3};

use crate::bvh::BVH;

/// Detects self-intersecting triangle pairs in the mesh.
///
/// Uses BVH for broad-phase AABB overlap culling, then applies the Moller
/// triangle-triangle intersection test for narrow-phase verification.
///
/// Only counts pairs where `i < j` and the triangles do not share any
/// vertices (adjacent triangles are expected to share edges).
///
/// Returns the number of intersecting triangle pairs.
pub fn detect_self_intersections(vertices: &[Point3], indices: &[[u32; 3]]) -> usize {
    find_intersecting_pairs(vertices, indices).len()
}

/// Finds all self-intersecting triangle pairs in the mesh.
///
/// Uses BVH for broad-phase AABB overlap culling, then applies the Moller
/// triangle-triangle intersection test for narrow-phase verification.
///
/// Only returns pairs where `i < j` and the triangles do not share any
/// vertices (adjacent triangles are expected to share edges).
///
/// Returns `Vec<(usize, usize)>` of intersecting pair indices.
pub fn find_intersecting_pairs(vertices: &[Point3], indices: &[[u32; 3]]) -> Vec<(usize, usize)> {
    if indices.len() < 2 {
        return Vec::new();
    }

    // Build BVH for broad-phase.
    let bvh = BVH::build(vertices, indices);

    // Precompute per-triangle AABBs.
    let tri_aabbs: Vec<Option<BBox3>> = indices
        .iter()
        .map(|tri| {
            let v0 = vertices[tri[0] as usize];
            let v1 = vertices[tri[1] as usize];
            let v2 = vertices[tri[2] as usize];
            BBox3::from_points(&[v0, v1, v2])
        })
        .collect();

    let mut pairs = Vec::new();

    // For each triangle, query BVH for overlapping AABBs, then narrow-phase test.
    for i in 0..indices.len() {
        let tri_i = &indices[i];
        let v0 = vertices[tri_i[0] as usize];
        let v1 = vertices[tri_i[1] as usize];
        let v2 = vertices[tri_i[2] as usize];

        let aabb_i = match &tri_aabbs[i] {
            Some(bb) => bb,
            None => continue, // Degenerate triangle
        };

        // Use BVH broad-phase to find candidate triangles.
        let candidates = bvh.query_aabb_overlaps(aabb_i);

        for j in candidates {
            // Only count pairs where i < j to avoid duplicates.
            if j <= i {
                continue;
            }

            let tri_j = &indices[j];

            // Skip if triangles share any vertex (adjacent triangles).
            if shares_vertex(tri_i, tri_j) {
                continue;
            }

            let u0 = vertices[tri_j[0] as usize];
            let u1 = vertices[tri_j[1] as usize];
            let u2 = vertices[tri_j[2] as usize];

            if triangles_intersect(&v0, &v1, &v2, &u0, &u1, &u2) {
                pairs.push((i, j));
            }
        }
    }

    pairs
}

/// Computes the Z-range spanned by vertices of intersecting triangle pairs.
///
/// Returns `Some((z_min, z_max))` if pairs is non-empty, representing the
/// Z-band in which self-intersections occur. Returns `None` if pairs is empty.
pub fn intersection_z_range(
    vertices: &[Point3],
    indices: &[[u32; 3]],
    pairs: &[(usize, usize)],
) -> Option<(f64, f64)> {
    if pairs.is_empty() {
        return None;
    }

    let mut z_min = f64::INFINITY;
    let mut z_max = f64::NEG_INFINITY;

    for &(i, j) in pairs {
        for &tri_idx in &[i, j] {
            let tri = &indices[tri_idx];
            for &vi in tri {
                let z = vertices[vi as usize].z;
                if z < z_min {
                    z_min = z;
                }
                if z > z_max {
                    z_max = z;
                }
            }
        }
    }

    Some((z_min, z_max))
}

/// Checks if two triangles share any vertex index.
fn shares_vertex(a: &[u32; 3], b: &[u32; 3]) -> bool {
    for &ai in a {
        for &bi in b {
            if ai == bi {
                return true;
            }
        }
    }
    false
}

/// Moller triangle-triangle intersection test.
///
/// Implementation of the algorithm from "A Fast Triangle-Triangle Intersection
/// Test" by Moller (1997).
///
/// Tests whether triangle (v0, v1, v2) intersects triangle (u0, u1, u2).
fn triangles_intersect(
    v0: &Point3,
    v1: &Point3,
    v2: &Point3,
    u0: &Point3,
    u1: &Point3,
    u2: &Point3,
) -> bool {
    // Step 1: Compute plane of triangle 2 (u0, u1, u2).
    let e1 = Vec3::from_points(*u0, *u1);
    let e2 = Vec3::from_points(*u0, *u2);
    let n2 = e1.cross(e2);

    if n2.length_squared() < 1e-30 {
        return false; // Degenerate triangle.
    }

    let d2 = -n2.dot(Vec3::from(Point3::new(u0.x, u0.y, u0.z)));

    // Signed distances from triangle 1 vertices to plane of triangle 2.
    let dv0 = n2.dot(Vec3::from(*v0)) + d2;
    let dv1 = n2.dot(Vec3::from(*v1)) + d2;
    let dv2 = n2.dot(Vec3::from(*v2)) + d2;

    // If all triangle 1 vertices are on the same side, no intersection.
    if dv0 > 0.0 && dv1 > 0.0 && dv2 > 0.0 {
        return false;
    }
    if dv0 < 0.0 && dv1 < 0.0 && dv2 < 0.0 {
        return false;
    }

    // Step 2: Compute plane of triangle 1 (v0, v1, v2).
    let f1 = Vec3::from_points(*v0, *v1);
    let f2 = Vec3::from_points(*v0, *v2);
    let n1 = f1.cross(f2);

    if n1.length_squared() < 1e-30 {
        return false; // Degenerate triangle.
    }

    let d1 = -n1.dot(Vec3::from(Point3::new(v0.x, v0.y, v0.z)));

    // Signed distances from triangle 2 vertices to plane of triangle 1.
    let du0 = n1.dot(Vec3::from(*u0)) + d1;
    let du1 = n1.dot(Vec3::from(*u1)) + d1;
    let du2 = n1.dot(Vec3::from(*u2)) + d1;

    // If all triangle 2 vertices are on the same side, no intersection.
    if du0 > 0.0 && du1 > 0.0 && du2 > 0.0 {
        return false;
    }
    if du0 < 0.0 && du1 < 0.0 && du2 < 0.0 {
        return false;
    }

    // Step 3: Compute the line of intersection between the two planes.
    let dir = n1.cross(n2);

    // Project onto the axis with maximum absolute value for numerical stability.
    let ax = dir.x.abs();
    let ay = dir.y.abs();
    let az = dir.z.abs();

    let project = if ax >= ay && ax >= az {
        |p: &Point3| p.x
    } else if ay >= az {
        |p: &Point3| p.y
    } else {
        |p: &Point3| p.z
    };

    // Step 4: Compute intervals for each triangle on the line of intersection.
    let pv0 = project(v0);
    let pv1 = project(v1);
    let pv2 = project(v2);

    let pu0 = project(u0);
    let pu1 = project(u1);
    let pu2 = project(u2);

    // For triangle 1: find the interval [t1_min, t1_max] on the intersection line.
    let interval1 = compute_interval(pv0, pv1, pv2, dv0, dv1, dv2);
    let interval2 = compute_interval(pu0, pu1, pu2, du0, du1, du2);

    match (interval1, interval2) {
        (Some((t1_min, t1_max)), Some((t2_min, t2_max))) => {
            // Check if intervals overlap.
            t1_min <= t2_max && t2_min <= t1_max
        }
        _ => false,
    }
}

/// Computes the interval of a triangle projected onto the line of plane
/// intersection, given the projected vertex positions and their signed
/// distances to the other triangle's plane.
///
/// Returns `Some((min, max))` if the triangle straddles the plane, `None` if
/// all vertices are on the same side (should be caught earlier).
fn compute_interval(p0: f64, p1: f64, p2: f64, d0: f64, d1: f64, d2: f64) -> Option<(f64, f64)> {
    // Find the "odd one out" vertex (the one on the opposite side of the plane).
    let eps = 1e-12;

    // Classify vertices.
    let s0 = if d0.abs() < eps {
        0
    } else if d0 > 0.0 {
        1
    } else {
        -1
    };
    let s1 = if d1.abs() < eps {
        0
    } else if d1 > 0.0 {
        1
    } else {
        -1
    };
    let s2 = if d2.abs() < eps {
        0
    } else if d2 > 0.0 {
        1
    } else {
        -1
    };

    // Compute two intersection points along the projected axis.
    let mut params = Vec::new();

    // Edge 0-1
    if s0 != s1 && s0 != 0 && s1 != 0 {
        let t = d0 / (d0 - d1);
        params.push(p0 + t * (p1 - p0));
    }
    // Edge 1-2
    if s1 != s2 && s1 != 0 && s2 != 0 {
        let t = d1 / (d1 - d2);
        params.push(p1 + t * (p2 - p1));
    }
    // Edge 0-2
    if s0 != s2 && s0 != 0 && s2 != 0 {
        let t = d0 / (d0 - d2);
        params.push(p0 + t * (p2 - p0));
    }

    // Vertices on the plane contribute directly.
    if s0 == 0 {
        params.push(p0);
    }
    if s1 == 0 {
        params.push(p1);
    }
    if s2 == 0 {
        params.push(p2);
    }

    if params.len() < 2 {
        // Touching at a single point or no intersection.
        return if params.len() == 1 {
            Some((params[0], params[0]))
        } else {
            None
        };
    }

    let mut min = params[0];
    let mut max = params[0];
    for &p in &params[1..] {
        if p < min {
            min = p;
        }
        if p > max {
            max = p;
        }
    }

    Some((min, max))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_intersection_for_separated_triangles() {
        let vertices = vec![
            // Triangle 0 in XY plane at z=0.
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
            // Triangle 1 in XY plane at z=2.
            Point3::new(0.0, 0.0, 2.0),
            Point3::new(1.0, 0.0, 2.0),
            Point3::new(0.5, 1.0, 2.0),
        ];
        let indices = vec![[0, 1, 2], [3, 4, 5]];
        let count = detect_self_intersections(&vertices, &indices);
        assert_eq!(count, 0);
    }

    #[test]
    fn detects_intersecting_triangles() {
        // Two triangles that cross through each other:
        // Triangle 0 in XY plane at z=0, from (0,0) to (2,0) to (1,2).
        // Triangle 1 in XZ plane at y=0.5, from (0.5, 0.5, -1) to (0.5, 0.5, 1) to (1.5, 0.5, 0).
        let vertices = vec![
            // Triangle 0: large triangle in XY plane.
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(1.0, 2.0, 0.0),
            // Triangle 1: triangle that pierces through triangle 0.
            Point3::new(0.5, 0.5, -1.0),
            Point3::new(0.5, 0.5, 1.0),
            Point3::new(1.5, 0.5, 0.0),
        ];
        let indices = vec![[0, 1, 2], [3, 4, 5]];
        let count = detect_self_intersections(&vertices, &indices);
        assert_eq!(count, 1, "Should detect one intersecting pair");
    }

    #[test]
    fn single_triangle_returns_zero() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
        ];
        let indices = vec![[0, 1, 2]];
        let count = detect_self_intersections(&vertices, &indices);
        assert_eq!(count, 0);
    }

    #[test]
    fn find_intersecting_pairs_separated_returns_empty() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
            Point3::new(0.0, 0.0, 2.0),
            Point3::new(1.0, 0.0, 2.0),
            Point3::new(0.5, 1.0, 2.0),
        ];
        let indices = vec![[0, 1, 2], [3, 4, 5]];
        let pairs = find_intersecting_pairs(&vertices, &indices);
        assert!(pairs.is_empty(), "Separated triangles should have no intersecting pairs");
    }

    #[test]
    fn find_intersecting_pairs_crossing_returns_one() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(1.0, 2.0, 0.0),
            Point3::new(0.5, 0.5, -1.0),
            Point3::new(0.5, 0.5, 1.0),
            Point3::new(1.5, 0.5, 0.0),
        ];
        let indices = vec![[0, 1, 2], [3, 4, 5]];
        let pairs = find_intersecting_pairs(&vertices, &indices);
        assert_eq!(pairs.len(), 1, "Crossing triangles should have one pair");
        assert_eq!(pairs[0], (0, 1));
    }

    #[test]
    fn intersection_z_range_empty_pairs() {
        let vertices = vec![Point3::new(0.0, 0.0, 0.0)];
        let indices: Vec<[u32; 3]> = vec![];
        let result = intersection_z_range(&vertices, &indices, &[]);
        assert!(result.is_none(), "Empty pairs should return None");
    }

    #[test]
    fn intersection_z_range_known_values() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(1.0, 2.0, 0.0),
            Point3::new(0.5, 0.5, -1.0),
            Point3::new(0.5, 0.5, 1.0),
            Point3::new(1.5, 0.5, 0.0),
        ];
        let indices = vec![[0, 1, 2], [3, 4, 5]];
        let pairs = vec![(0usize, 1usize)];
        let result = intersection_z_range(&vertices, &indices, &pairs);
        assert!(result.is_some());
        let (z_min, z_max) = result.unwrap();
        assert!((z_min - (-1.0)).abs() < 1e-9, "z_min should be -1.0, got {}", z_min);
        assert!((z_max - 1.0).abs() < 1e-9, "z_max should be 1.0, got {}", z_max);
    }
}
