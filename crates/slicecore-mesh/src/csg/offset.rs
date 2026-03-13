//! Vertex-normal mesh offset operations.
//!
//! Displaces each vertex of a triangle mesh along its computed vertex normal
//! by a specified distance. Positive distance grows the mesh outward; negative
//! distance shrinks it inward.

use std::time::Instant;

use slicecore_math::{Point3, Vec3};

use crate::triangle_mesh::TriangleMesh;

use super::error::CsgError;
use super::report::CsgReport;
use super::volume;

/// Offsets a triangle mesh by displacing each vertex along its vertex normal.
///
/// Vertex normals are computed as the angle-weighted average of adjacent face
/// normals. Positive `distance` grows the mesh outward; negative shrinks it.
///
/// # Errors
///
/// Returns [`CsgError::ResultConstruction`] if the offset mesh cannot be
/// constructed (e.g., all vertices collapsed to a single point).
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::offset::mesh_offset;
/// use slicecore_mesh::csg::primitive_box;
///
/// let mesh = primitive_box(2.0, 2.0, 2.0);
/// let (bigger, report) = mesh_offset(&mesh, 0.5).unwrap();
/// assert!(report.output_triangles > 0);
/// ```
pub fn mesh_offset(
    mesh: &TriangleMesh,
    distance: f64,
) -> Result<(TriangleMesh, CsgReport), CsgError> {
    let start = Instant::now();
    let verts = mesh.vertices();
    let indices = mesh.indices();

    // Compute angle-weighted vertex normals.
    let vertex_normals = compute_vertex_normals(verts, indices);

    // Displace each vertex.
    let new_verts: Vec<Point3> = verts
        .iter()
        .zip(vertex_normals.iter())
        .map(|(&p, &n)| {
            let disp = n * distance;
            Point3::new(p.x + disp.x, p.y + disp.y, p.z + disp.z)
        })
        .collect();

    // Keep the same indices.
    let new_indices = indices.to_vec();

    let result = TriangleMesh::new(new_verts, new_indices)
        .map_err(CsgError::ResultConstruction)?;

    let mut report = CsgReport {
        input_triangles_a: mesh.triangle_count(),
        output_triangles: result.triangle_count(),
        ..CsgReport::default()
    };

    report.volume = Some(volume::signed_volume(
        result.vertices(),
        result.indices(),
    ));
    report.surface_area = Some(volume::surface_area(
        result.vertices(),
        result.indices(),
    ));
    report.duration_ms = start.elapsed().as_millis() as u64;

    Ok((result, report))
}

/// Computes angle-weighted vertex normals.
///
/// For each vertex, the normal is the weighted sum of adjacent face normals,
/// where the weight is the angle of the triangle at that vertex.
fn compute_vertex_normals(verts: &[Point3], indices: &[[u32; 3]]) -> Vec<Vec3> {
    let mut normals = vec![Vec3::zero(); verts.len()];

    for tri in indices {
        let v0 = verts[tri[0] as usize];
        let v1 = verts[tri[1] as usize];
        let v2 = verts[tri[2] as usize];

        let e01 = Vec3::from_points(v0, v1);
        let e02 = Vec3::from_points(v0, v2);
        let e10 = Vec3::from_points(v1, v0);
        let e12 = Vec3::from_points(v1, v2);
        let e20 = Vec3::from_points(v2, v0);
        let e21 = Vec3::from_points(v2, v1);

        let face_normal = e01.cross(e02);
        let face_normal_len = face_normal.length();
        if face_normal_len < 1e-30 {
            continue; // Degenerate triangle.
        }
        let fn_unit = face_normal * (1.0 / face_normal_len);

        // Compute angle at each vertex using dot product.
        let angle0 = safe_angle(e01, e02);
        let angle1 = safe_angle(e10, e12);
        let angle2 = safe_angle(e20, e21);

        // Accumulate weighted normal.
        normals[tri[0] as usize] = normals[tri[0] as usize] + fn_unit * angle0;
        normals[tri[1] as usize] = normals[tri[1] as usize] + fn_unit * angle1;
        normals[tri[2] as usize] = normals[tri[2] as usize] + fn_unit * angle2;
    }

    // Normalize all vertex normals.
    for n in &mut normals {
        let len = n.length();
        if len > 1e-30 {
            *n = *n * (1.0 / len);
        }
    }

    normals
}

/// Computes the angle between two vectors safely, clamping the cosine to [-1, 1].
fn safe_angle(a: Vec3, b: Vec3) -> f64 {
    let la = a.length();
    let lb = b.length();
    if la < 1e-30 || lb < 1e-30 {
        return 0.0;
    }
    let cos_angle = a.dot(b) / (la * lb);
    cos_angle.clamp(-1.0, 1.0).acos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csg::primitives::primitive_box;

    #[test]
    fn offset_box_positive_grows() {
        let mesh = primitive_box(2.0, 2.0, 2.0);
        let orig_vol = volume::signed_volume(mesh.vertices(), mesh.indices());
        let (result, report) = mesh_offset(&mesh, 0.5).unwrap();
        assert!(report.output_triangles > 0);
        let new_vol = volume::signed_volume(result.vertices(), result.indices());
        assert!(
            new_vol > orig_vol,
            "positive offset should increase volume: {new_vol} > {orig_vol}"
        );
    }

    #[test]
    fn offset_box_negative_shrinks() {
        let mesh = primitive_box(4.0, 4.0, 4.0);
        let orig_vol = volume::signed_volume(mesh.vertices(), mesh.indices());
        let (result, _) = mesh_offset(&mesh, -0.5).unwrap();
        let new_vol = volume::signed_volume(result.vertices(), result.indices());
        assert!(
            new_vol < orig_vol,
            "negative offset should decrease volume: {new_vol} < {orig_vol}"
        );
    }

    #[test]
    fn offset_zero_preserves_volume() {
        let mesh = primitive_box(2.0, 2.0, 2.0);
        let orig_vol = volume::signed_volume(mesh.vertices(), mesh.indices());
        let (result, _) = mesh_offset(&mesh, 0.0).unwrap();
        let new_vol = volume::signed_volume(result.vertices(), result.indices());
        assert!(
            (new_vol - orig_vol).abs() < 1e-6,
            "zero offset should preserve volume: {new_vol} vs {orig_vol}"
        );
    }
}
