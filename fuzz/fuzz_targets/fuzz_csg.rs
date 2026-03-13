#![no_main]

use libfuzzer_sys::fuzz_target;
use slicecore_mesh::TriangleMesh;

fuzz_target!(|data: &[u8]| {
    // Need at least 144 bytes for two single-triangle meshes.
    // Each triangle: 3 vertices * 3 coords * 8 bytes = 72 bytes per mesh.
    if data.len() < 144 {
        return;
    }

    let mid = data.len() / 2;
    let a_data = &data[..mid];
    let b_data = &data[mid..];

    if let (Some(mesh_a), Some(mesh_b)) = (try_build_mesh(a_data), try_build_mesh(b_data)) {
        // Try each boolean operation -- none should panic.
        let _ = slicecore_mesh::csg::mesh_union(&mesh_a, &mesh_b);
        let _ = slicecore_mesh::csg::mesh_difference(&mesh_a, &mesh_b);
        let _ = slicecore_mesh::csg::mesh_intersection(&mesh_a, &mesh_b);
        let _ = slicecore_mesh::csg::mesh_xor(&mesh_a, &mesh_b);
    }
});

/// Attempts to build a `TriangleMesh` from raw bytes.
///
/// Interprets sequential groups of 24 bytes (3 x f64) as vertex coordinates.
/// Every three vertices form one triangle. Returns `None` if not enough data
/// or the resulting mesh is invalid.
fn try_build_mesh(data: &[u8]) -> Option<TriangleMesh> {
    // Each f64 = 8 bytes, each vertex = 3 f64 = 24 bytes, each triangle = 3 vertices = 72 bytes.
    let bytes_per_tri = 72;
    let num_tris = data.len() / bytes_per_tri;
    if num_tris == 0 {
        return None;
    }

    let mut vertices = Vec::with_capacity(num_tris * 3);
    let mut indices = Vec::with_capacity(num_tris);

    for t in 0..num_tris {
        let base = t * bytes_per_tri;
        for v in 0..3u32 {
            let vbase = base + (v as usize) * 24;
            let x = f64::from_le_bytes(data[vbase..vbase + 8].try_into().ok()?);
            let y = f64::from_le_bytes(data[vbase + 8..vbase + 16].try_into().ok()?);
            let z = f64::from_le_bytes(data[vbase + 16..vbase + 24].try_into().ok()?);

            // Skip NaN/Inf values -- they would cause arithmetic issues, not CSG bugs.
            if !x.is_finite() || !y.is_finite() || !z.is_finite() {
                return None;
            }

            vertices.push(slicecore_math::Point3::new(x, y, z));
        }
        let vi = (t as u32) * 3;
        indices.push([vi, vi + 1, vi + 2]);
    }

    TriangleMesh::new(vertices, indices).ok()
}
