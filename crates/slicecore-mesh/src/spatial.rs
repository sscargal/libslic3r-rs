//! Spatial query interface for triangle meshes.
//!
//! Provides convenience functions that delegate to the BVH spatial index
//! for accelerated queries, plus a brute-force closest-point query.

use slicecore_math::{Point3, Vec3};

use crate::bvh::RayHit;
use crate::triangle_mesh::TriangleMesh;

/// Returns the indices of all triangles whose AABBs span the given Z height.
///
/// This is the primary query for slicing: called once per layer to find
/// all triangles that potentially intersect the horizontal cutting plane.
///
/// Delegates to the BVH spatial index, which is built lazily on first call.
pub fn query_triangles_at_z(mesh: &TriangleMesh, z: f64) -> Vec<usize> {
    mesh.bvh().query_plane(z)
}

/// Casts a ray from `origin` in `direction` and returns the closest triangle
/// hit, if any.
///
/// Used for support generation (checking if a point on a surface can "see"
/// the build plate) and for general ray-mesh intersection queries.
///
/// Delegates to the BVH spatial index for accelerated traversal.
pub fn ray_cast(mesh: &TriangleMesh, origin: &Point3, direction: &Vec3) -> Option<RayHit> {
    mesh.bvh()
        .intersect_ray(origin, direction, mesh.vertices(), mesh.indices())
}

/// Finds the closest point on the mesh surface to the given query point.
///
/// Returns a tuple of (closest_point, triangle_index).
///
/// Currently uses brute-force iteration over all triangles. This is acceptable
/// for Phase 1 as closest-point queries are not a hot path in the slicing
/// pipeline.
///
// TODO: BVH-accelerated closest point query
pub fn closest_point_on_mesh(mesh: &TriangleMesh, point: &Point3) -> (Point3, usize) {
    let mut best_dist_sq = f64::INFINITY;
    let mut best_point = *point;
    let mut best_tri = 0usize;

    for i in 0..mesh.triangle_count() {
        let [v0, v1, v2] = mesh.triangle_vertices(i);
        let closest = closest_point_on_triangle(point, &v0, &v1, &v2);
        let dist_sq = distance_squared(point, &closest);

        if dist_sq < best_dist_sq {
            best_dist_sq = dist_sq;
            best_point = closest;
            best_tri = i;
        }
    }

    (best_point, best_tri)
}

/// Computes the squared distance between two points.
fn distance_squared(a: &Point3, b: &Point3) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    dx * dx + dy * dy + dz * dz
}

/// Projects a point onto a triangle and clamps to the triangle boundary.
///
/// Uses the method from "Real-Time Collision Detection" (Ericson, 2004):
/// project point onto triangle plane, then use barycentric coordinates to
/// determine if the projection is inside the triangle. If outside, clamp
/// to the nearest edge or vertex.
fn closest_point_on_triangle(p: &Point3, a: &Point3, b: &Point3, c: &Point3) -> Point3 {
    let ab = Vec3::from_points(*a, *b);
    let ac = Vec3::from_points(*a, *c);
    let ap = Vec3::new(p.x - a.x, p.y - a.y, p.z - a.z);

    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return *a; // Closest to vertex A
    }

    let bp = Vec3::new(p.x - b.x, p.y - b.y, p.z - b.z);
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return *b; // Closest to vertex B
    }

    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return Point3::new(a.x + ab.x * v, a.y + ab.y * v, a.z + ab.z * v); // On edge AB
    }

    let cp = Vec3::new(p.x - c.x, p.y - c.y, p.z - c.z);
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return *c; // Closest to vertex C
    }

    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return Point3::new(a.x + ac.x * w, a.y + ac.y * w, a.z + ac.z * w); // On edge AC
    }

    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return Point3::new(
            b.x + (c.x - b.x) * w,
            b.y + (c.y - b.y) * w,
            b.z + (c.z - b.z) * w,
        ); // On edge BC
    }

    // Inside triangle -- project onto plane
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    Point3::new(
        a.x + ab.x * v + ac.x * w,
        a.y + ab.y * v + ac.y * w,
        a.z + ab.z * v + ac.z * w,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::triangle_mesh::tests::unit_cube;

    #[test]
    fn query_triangles_at_z_multiple_heights() {
        let mesh = unit_cube();
        // At z=0.5, all triangles are candidates (cube spans 0-1)
        let at_half = query_triangles_at_z(&mesh, 0.5);
        assert!(!at_half.is_empty());

        // At z=0.0, some triangles may touch (bottom face at z=0)
        let at_zero = query_triangles_at_z(&mesh, 0.0);
        assert!(!at_zero.is_empty());

        // At z=1.0, some triangles may touch (top face at z=1)
        let at_one = query_triangles_at_z(&mesh, 1.0);
        assert!(!at_one.is_empty());
    }

    #[test]
    fn ray_cast_from_outside_cube_hits() {
        let mesh = unit_cube();
        let origin = Point3::new(0.5, 0.5, 5.0);
        let direction = Vec3::new(0.0, 0.0, -1.0);
        let hit = ray_cast(&mesh, &origin, &direction);
        assert!(hit.is_some(), "Expected ray from above to hit cube");
    }

    #[test]
    fn closest_point_above_cube_returns_top_face() {
        let mesh = unit_cube();
        let point = Point3::new(0.5, 0.5, 5.0); // directly above center of top face
        let (closest, _tri_idx) = closest_point_on_mesh(&mesh, &point);
        // Closest point should be on the top face at approximately (0.5, 0.5, 1.0)
        assert!((closest.x - 0.5).abs() < 1e-6, "x: {}", closest.x);
        assert!((closest.y - 0.5).abs() < 1e-6, "y: {}", closest.y);
        assert!((closest.z - 1.0).abs() < 1e-6, "z: {}", closest.z);
    }
}
