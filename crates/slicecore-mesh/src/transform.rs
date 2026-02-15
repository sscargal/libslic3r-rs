//! Mesh transformation operations.
//!
//! All transform functions return **new** meshes (immutable pattern).
//! The original mesh is unchanged. This follows the research recommendation
//! of immutable-after-construction for mesh data.
//!
//! Available transforms:
//! - [`translate`]: Move mesh by offset
//! - [`scale`]: Scale mesh about origin
//! - [`rotate`]: Rotate mesh around arbitrary axis (Rodrigues formula)
//! - [`mirror`]: Mirror mesh about a coordinate plane
//! - [`transform`]: General affine transform via Matrix4x4
//! - [`center_on_origin`]: Center mesh AABB at origin
//! - [`place_on_bed`]: Move mesh so AABB min Z = 0

use slicecore_math::{Matrix4x4, Point3, Vec3};

use crate::triangle_mesh::TriangleMesh;

/// Axis for mirror operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MirrorAxis {
    /// Mirror about the YZ plane (negate X).
    X,
    /// Mirror about the XZ plane (negate Y).
    Y,
    /// Mirror about the XY plane (negate Z).
    Z,
}

/// Translates all vertices by `(dx, dy, dz)`.
///
/// Normals are unchanged by translation. AABB is recomputed.
pub fn translate(mesh: &TriangleMesh, dx: f64, dy: f64, dz: f64) -> TriangleMesh {
    let vertices: Vec<Point3> = mesh
        .vertices()
        .iter()
        .map(|v| Point3::new(v.x + dx, v.y + dy, v.z + dz))
        .collect();

    TriangleMesh::new(vertices, mesh.indices().to_vec())
        .expect("translated mesh should be valid (same topology)")
}

/// Scales all vertices about the origin by `(sx, sy, sz)`.
///
/// Normals and AABB are recomputed. If any scale factor is negative,
/// triangle winding is reversed to maintain consistent outward normals.
pub fn scale(mesh: &TriangleMesh, sx: f64, sy: f64, sz: f64) -> TriangleMesh {
    let vertices: Vec<Point3> = mesh
        .vertices()
        .iter()
        .map(|v| Point3::new(v.x * sx, v.y * sy, v.z * sz))
        .collect();

    // Negative scale flips winding; we need to reverse triangle winding
    // to maintain consistent normals.
    let neg_count = [sx, sy, sz].iter().filter(|&&s| s < 0.0).count();
    let flip_winding = neg_count % 2 == 1; // Odd number of negative scales flips orientation

    let indices = if flip_winding {
        mesh.indices()
            .iter()
            .map(|tri| [tri[0], tri[2], tri[1]]) // Swap two vertices to reverse winding
            .collect()
    } else {
        mesh.indices().to_vec()
    };

    TriangleMesh::new(vertices, indices)
        .expect("scaled mesh should be valid (same topology)")
}

/// Rotates all vertices around an arbitrary axis by `angle_rad` radians.
///
/// Uses the Rodrigues rotation formula. Normals and AABB are recomputed.
pub fn rotate(mesh: &TriangleMesh, axis: &Vec3, angle_rad: f64) -> TriangleMesh {
    let k = axis.normalize();
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();

    let vertices: Vec<Point3> = mesh
        .vertices()
        .iter()
        .map(|v| {
            // Rodrigues formula: v_rot = v*cos(a) + (k x v)*sin(a) + k*(k.v)*(1-cos(a))
            let vv = Vec3::new(v.x, v.y, v.z);
            let k_cross_v = k.cross(vv);
            let k_dot_v = k.dot(vv);
            let rotated = vv * cos_a + k_cross_v * sin_a + k * (k_dot_v * (1.0 - cos_a));
            Point3::new(rotated.x, rotated.y, rotated.z)
        })
        .collect();

    TriangleMesh::new(vertices, mesh.indices().to_vec())
        .expect("rotated mesh should be valid (same topology)")
}

/// Mirrors the mesh about the specified coordinate plane.
///
/// Triangle winding is reversed to maintain consistent outward normals
/// after mirroring.
pub fn mirror(mesh: &TriangleMesh, axis: MirrorAxis) -> TriangleMesh {
    let vertices: Vec<Point3> = mesh
        .vertices()
        .iter()
        .map(|v| match axis {
            MirrorAxis::X => Point3::new(-v.x, v.y, v.z),
            MirrorAxis::Y => Point3::new(v.x, -v.y, v.z),
            MirrorAxis::Z => Point3::new(v.x, v.y, -v.z),
        })
        .collect();

    // Mirror flips winding, so reverse triangle vertex order.
    let indices: Vec<[u32; 3]> = mesh
        .indices()
        .iter()
        .map(|tri| [tri[0], tri[2], tri[1]])
        .collect();

    TriangleMesh::new(vertices, indices)
        .expect("mirrored mesh should be valid (same topology)")
}

/// Applies a general affine transformation via a 4x4 matrix.
///
/// Vertices are transformed using the full matrix. Normals are recomputed
/// from the transformed vertices (the correct approach is using the inverse
/// transpose of the upper 3x3, but since we recompute from scratch in
/// the constructor, this is handled automatically).
///
/// If the matrix has a negative determinant (e.g., contains a mirror),
/// triangle winding is reversed to maintain consistent normals.
pub fn transform(mesh: &TriangleMesh, matrix: &Matrix4x4) -> TriangleMesh {
    let vertices: Vec<Point3> = mesh
        .vertices()
        .iter()
        .map(|v| matrix.transform_point3(*v))
        .collect();

    // If determinant is negative, the transform includes a reflection
    // and we need to flip winding.
    let det = matrix.determinant();
    let indices = if det < 0.0 {
        mesh.indices()
            .iter()
            .map(|tri| [tri[0], tri[2], tri[1]])
            .collect()
    } else {
        mesh.indices().to_vec()
    };

    TriangleMesh::new(vertices, indices)
        .expect("transformed mesh should be valid (same topology)")
}

/// Translates the mesh so its AABB center is at the origin.
pub fn center_on_origin(mesh: &TriangleMesh) -> TriangleMesh {
    let center = mesh.aabb().center();
    translate(mesh, -center.x, -center.y, -center.z)
}

/// Translates the mesh so its AABB minimum Z is at 0 (placed on the build plate).
pub fn place_on_bed(mesh: &TriangleMesh) -> TriangleMesh {
    let min_z = mesh.aabb().min.z;
    translate(mesh, 0.0, 0.0, -min_z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::compute_stats;
    use crate::triangle_mesh::tests::unit_cube;

    #[test]
    fn translate_shifts_vertices() {
        let mesh = unit_cube();
        let moved = translate(&mesh, 0.0, 0.0, 1.0);
        let aabb = moved.aabb();
        assert!((aabb.min.z - 1.0).abs() < 1e-9, "min.z: {}", aabb.min.z);
        assert!((aabb.max.z - 2.0).abs() < 1e-9, "max.z: {}", aabb.max.z);
    }

    #[test]
    fn scale_doubles_dimensions_and_volume() {
        let mesh = unit_cube();
        let scaled = scale(&mesh, 2.0, 2.0, 2.0);
        let aabb = scaled.aabb();
        assert!((aabb.max.x - 2.0).abs() < 1e-9);
        assert!((aabb.max.y - 2.0).abs() < 1e-9);
        assert!((aabb.max.z - 2.0).abs() < 1e-9);

        let stats = compute_stats(&scaled);
        assert!(
            (stats.volume - 8.0).abs() < 1e-4,
            "Expected volume ~8.0, got {}",
            stats.volume
        );
    }

    #[test]
    fn mirror_x_negates_x_coordinates() {
        let mesh = unit_cube();
        let mirrored = mirror(&mesh, MirrorAxis::X);
        let aabb = mirrored.aabb();
        assert!((aabb.min.x - (-1.0)).abs() < 1e-9);
        assert!((aabb.max.x - 0.0).abs() < 1e-9);

        // Winding should be reversed, so normals still consistent
        let stats = compute_stats(&mirrored);
        assert!(stats.has_consistent_winding, "Winding should be consistent after mirror");
    }

    #[test]
    fn center_on_origin_centers_aabb() {
        let mesh = unit_cube();
        let centered = center_on_origin(&mesh);
        let center = centered.aabb().center();
        assert!((center.x).abs() < 1e-9, "center.x: {}", center.x);
        assert!((center.y).abs() < 1e-9, "center.y: {}", center.y);
        assert!((center.z).abs() < 1e-9, "center.z: {}", center.z);
    }

    #[test]
    fn place_on_bed_sets_min_z_to_zero() {
        let mesh = unit_cube();
        // First translate up
        let lifted = translate(&mesh, 0.0, 0.0, 5.0);
        let placed = place_on_bed(&lifted);
        assert!((placed.aabb().min.z).abs() < 1e-9, "min.z: {}", placed.aabb().min.z);
    }

    #[test]
    fn rotate_z_by_half_pi() {
        let mesh = unit_cube();
        let axis = Vec3::new(0.0, 0.0, 1.0);
        let rotated = rotate(&mesh, &axis, std::f64::consts::FRAC_PI_2);

        // Vertex (1,0,0) should become approximately (0,1,0) after 90-degree Z rotation.
        // Find the transformed vertex closest to what was (1,0,0).
        // The original vertex 1 is (1,0,0). After rotation: (0,1,0).
        let v = rotated.vertices()[1]; // vertex 1 was (1,0,0)
        assert!(
            (v.x - 0.0).abs() < 1e-9,
            "Expected x ~0.0, got {}",
            v.x
        );
        assert!(
            (v.y - 1.0).abs() < 1e-9,
            "Expected y ~1.0, got {}",
            v.y
        );
    }

    #[test]
    fn transform_with_identity_leaves_mesh_unchanged() {
        let mesh = unit_cube();
        let identity = Matrix4x4::identity();
        let transformed = transform(&mesh, &identity);

        assert_eq!(transformed.vertex_count(), mesh.vertex_count());
        assert_eq!(transformed.triangle_count(), mesh.triangle_count());

        for (orig, trans) in mesh.vertices().iter().zip(transformed.vertices().iter()) {
            assert!((orig.x - trans.x).abs() < 1e-9);
            assert!((orig.y - trans.y).abs() < 1e-9);
            assert!((orig.z - trans.z).abs() < 1e-9);
        }
    }

    #[test]
    fn scale_by_negative_reverses_winding() {
        let mesh = unit_cube();
        let neg_scaled = scale(&mesh, -1.0, 1.0, 1.0);
        let stats = compute_stats(&neg_scaled);
        // Volume should be positive (winding corrected).
        assert!(
            stats.volume > 0.0,
            "Expected positive volume after negative scale with winding fix, got {}",
            stats.volume
        );
    }
}
