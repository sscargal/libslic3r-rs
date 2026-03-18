//! OBJ file parser.
//!
//! Parses Wavefront OBJ files into [`TriangleMesh`] using the [`tobj`] crate.
//! OBJ is a widely used format in CAD and 3D modeling. The parser automatically
//! triangulates quad and n-gon faces via fan triangulation.
//!
//! Material (.mtl) files are not loaded -- only mesh geometry is extracted.

use std::io::{BufReader, Cursor};
use std::path::Path;

use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;

use crate::error::FileIOError;

/// Parse an OBJ file from raw bytes into a [`TriangleMesh`].
///
/// # OBJ format
///
/// The parser extracts vertex positions (`v X Y Z`) and face indices
/// (`f V1 V2 V3 ...`). Quad and n-gon faces are automatically triangulated
/// using fan triangulation (via tobj's `triangulate` option).
///
/// Multiple objects/groups in the OBJ are merged into a single `TriangleMesh`
/// with a shared vertex buffer (positions are global in OBJ format).
///
/// # Errors
///
/// - [`FileIOError::ObjError`] if tobj cannot parse the data.
/// - [`FileIOError::EmptyModel`] if no mesh geometry is found.
/// - [`FileIOError::MeshError`] if mesh construction fails.
pub fn parse(data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    let cursor = Cursor::new(data);
    let mut reader = BufReader::new(cursor);

    let load_options = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ignore_points: true,
        ignore_lines: true,
    };

    // Pass a material loader that returns an empty material set.
    // OBJ materials are not relevant for mesh geometry.
    let (models, _materials) = tobj::load_obj_buf(&mut reader, &load_options, |_path: &Path| {
        Ok((Vec::new(), std::collections::HashMap::new()))
    })
    .map_err(|e| FileIOError::ObjError(e.to_string()))?;

    let mut all_vertices: Vec<Point3> = Vec::new();
    let mut all_indices: Vec<[u32; 3]> = Vec::new();

    for model in &models {
        let mesh = &model.mesh;

        // OBJ positions are stored flat: [x, y, z, x, y, z, ...]
        let positions = &mesh.positions;
        if positions.is_empty() {
            continue;
        }

        let vertex_offset = all_vertices.len() as u32;

        // Extract vertices from the flat positions array.
        for chunk in positions.chunks_exact(3) {
            all_vertices.push(Point3::new(
                chunk[0] as f64,
                chunk[1] as f64,
                chunk[2] as f64,
            ));
        }

        // Extract triangle indices (groups of 3 after triangulation).
        let indices = &mesh.indices;
        for chunk in indices.chunks_exact(3) {
            all_indices.push([
                chunk[0] + vertex_offset,
                chunk[1] + vertex_offset,
                chunk[2] + vertex_offset,
            ]);
        }
    }

    if all_vertices.is_empty() || all_indices.is_empty() {
        return Err(FileIOError::EmptyModel);
    }

    let mesh = TriangleMesh::new(all_vertices, all_indices)?;
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal OBJ cube: 8 vertices, 12 triangular faces.
    fn cube_obj() -> &'static [u8] {
        b"# Cube
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 1.0 1.0 0.0
v 0.0 1.0 0.0
v 0.0 0.0 1.0
v 1.0 0.0 1.0
v 1.0 1.0 1.0
v 0.0 1.0 1.0
f 1 2 3
f 1 3 4
f 5 6 7
f 5 7 8
f 1 2 6
f 1 6 5
f 3 4 8
f 3 8 7
f 2 3 7
f 2 7 6
f 1 4 8
f 1 8 5
"
    }

    #[test]
    fn parse_cube_obj() {
        let data = cube_obj();
        let mesh = parse(data).unwrap();

        assert_eq!(mesh.vertex_count(), 8);
        assert_eq!(mesh.triangle_count(), 12);
    }

    #[test]
    fn parse_obj_with_quad_faces_triangulates() {
        // Quad faces: each quad should be split into 2 triangles.
        let data = b"# Quad plane
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 1.0 1.0 0.0
v 0.0 1.0 0.0
v 2.0 0.0 0.0
v 2.0 1.0 0.0
f 1 2 3 4
f 2 5 6 3
";
        let mesh = parse(data).unwrap();

        // 6 vertices, 2 quads = 4 triangles.
        assert_eq!(mesh.vertex_count(), 6);
        assert_eq!(mesh.triangle_count(), 4);
    }

    #[test]
    fn parse_single_triangle_obj() {
        let data = b"v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 0.5 1.0 0.0
f 1 2 3
";
        let mesh = parse(data).unwrap();

        assert_eq!(mesh.vertex_count(), 3);
        assert_eq!(mesh.triangle_count(), 1);
    }

    #[test]
    fn empty_obj_returns_empty_model() {
        let data = b"# No geometry here\n";
        let result = parse(data);
        assert!(
            matches!(result, Err(FileIOError::EmptyModel)),
            "expected EmptyModel for empty OBJ"
        );
    }

    #[test]
    fn obj_with_comments_and_blank_lines() {
        let data = b"# A simple triangle
# with comments

v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 0.0 1.0 0.0

# face
f 1 2 3
";
        let mesh = parse(data).unwrap();
        assert_eq!(mesh.vertex_count(), 3);
        assert_eq!(mesh.triangle_count(), 1);
    }

    #[test]
    fn invalid_obj_returns_obj_error() {
        // Binary data that cannot be parsed as OBJ text.
        let data: &[u8] = &[0xFF, 0xFE, 0x00, 0x00, 0xFF];
        let result = parse(data);
        // tobj may return an error or an empty model; either is acceptable.
        assert!(
            matches!(
                result,
                Err(FileIOError::ObjError(_)) | Err(FileIOError::EmptyModel)
            ),
            "expected ObjError or EmptyModel for invalid data"
        );
    }
}
