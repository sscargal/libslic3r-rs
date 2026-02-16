//! 3MF file parser.
//!
//! Parses 3MF files (ZIP+XML containers) into [`TriangleMesh`] using the
//! [`lib3mf`] crate. 3MF is the modern successor to STL, used by
//! Bambu Studio, OrcaSlicer, and other contemporary slicers.
//!
//! Multi-object 3MF files are merged into a single `TriangleMesh` with
//! correct vertex index offsets.

use std::io::Cursor;

use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;

use crate::error::FileIOError;

/// Parse a 3MF file from raw bytes into a [`TriangleMesh`].
///
/// # 3MF format
///
/// A 3MF file is a ZIP archive containing XML files that describe 3D model
/// data. The parser extracts mesh geometry (vertices and triangles) from all
/// objects in the model and merges them into a single `TriangleMesh`.
///
/// When merging multiple objects, vertex indices are offset so that each
/// object's triangles correctly reference the global vertex buffer.
///
/// # Errors
///
/// - [`FileIOError::ThreeMfError`] if lib3mf cannot parse the data.
/// - [`FileIOError::EmptyModel`] if no mesh geometry is found.
/// - [`FileIOError::MeshError`] if mesh construction fails.
pub fn parse(data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    let cursor = Cursor::new(data);

    let model = lib3mf::Model::from_reader(cursor)
        .map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;

    let mut all_vertices: Vec<Point3> = Vec::new();
    let mut all_indices: Vec<[u32; 3]> = Vec::new();

    for object in &model.resources.objects {
        if let Some(mesh) = &object.mesh {
            let vertex_offset = all_vertices.len() as u32;

            // Convert lib3mf Vertex (f64) to Point3.
            for v in &mesh.vertices {
                all_vertices.push(Point3::new(v.x, v.y, v.z));
            }

            // Convert lib3mf Triangle indices with vertex offset.
            for tri in &mesh.triangles {
                all_indices.push([
                    tri.v1 as u32 + vertex_offset,
                    tri.v2 as u32 + vertex_offset,
                    tri.v3 as u32 + vertex_offset,
                ]);
            }
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

    /// Create a minimal 3MF file in-memory using lib3mf's write API.
    fn create_test_3mf(objects: Vec<lib3mf::Object>) -> Vec<u8> {
        let mut model = lib3mf::Model::new();
        for (i, obj) in objects.into_iter().enumerate() {
            model.build.items.push(lib3mf::BuildItem::new(obj.id));
            // Ensure parse order is set to avoid validation issues.
            let mut obj = obj;
            obj.parse_order = i;
            model.resources.objects.push(obj);
        }

        let mut buffer = Cursor::new(Vec::new());
        model
            .to_writer(&mut buffer)
            .expect("failed to write test 3MF");
        buffer.into_inner()
    }

    /// Create a single-object 3MF with a tetrahedron (4 vertices, 4 triangles).
    fn tetrahedron_3mf() -> Vec<u8> {
        let mut mesh = lib3mf::Mesh::new();
        mesh.vertices.push(lib3mf::Vertex::new(0.0, 0.0, 0.0));
        mesh.vertices.push(lib3mf::Vertex::new(1.0, 0.0, 0.0));
        mesh.vertices.push(lib3mf::Vertex::new(0.5, 1.0, 0.0));
        mesh.vertices.push(lib3mf::Vertex::new(0.5, 0.5, 1.0));

        mesh.triangles.push(lib3mf::Triangle::new(0, 1, 2));
        mesh.triangles.push(lib3mf::Triangle::new(0, 1, 3));
        mesh.triangles.push(lib3mf::Triangle::new(1, 2, 3));
        mesh.triangles.push(lib3mf::Triangle::new(0, 2, 3));

        let mut object = lib3mf::Object::new(1);
        object.mesh = Some(mesh);

        create_test_3mf(vec![object])
    }

    #[test]
    fn parse_single_object_3mf() {
        let data = tetrahedron_3mf();
        let mesh = parse(&data).unwrap();

        assert_eq!(mesh.vertex_count(), 4);
        assert_eq!(mesh.triangle_count(), 4);
    }

    #[test]
    fn parse_multi_object_3mf_with_correct_offsets() {
        // Create two separate triangles as two objects.
        let mut mesh1 = lib3mf::Mesh::new();
        mesh1.vertices.push(lib3mf::Vertex::new(0.0, 0.0, 0.0));
        mesh1.vertices.push(lib3mf::Vertex::new(1.0, 0.0, 0.0));
        mesh1.vertices.push(lib3mf::Vertex::new(0.5, 1.0, 0.0));
        mesh1.triangles.push(lib3mf::Triangle::new(0, 1, 2));

        let mut obj1 = lib3mf::Object::new(1);
        obj1.mesh = Some(mesh1);

        let mut mesh2 = lib3mf::Mesh::new();
        mesh2.vertices.push(lib3mf::Vertex::new(2.0, 0.0, 0.0));
        mesh2.vertices.push(lib3mf::Vertex::new(3.0, 0.0, 0.0));
        mesh2.vertices.push(lib3mf::Vertex::new(2.5, 1.0, 0.0));
        mesh2.triangles.push(lib3mf::Triangle::new(0, 1, 2));

        let mut obj2 = lib3mf::Object::new(2);
        obj2.mesh = Some(mesh2);

        let data = create_test_3mf(vec![obj1, obj2]);
        let mesh = parse(&data).unwrap();

        // 3 vertices from each object = 6 total.
        assert_eq!(mesh.vertex_count(), 6);
        // 1 triangle from each object = 2 total.
        assert_eq!(mesh.triangle_count(), 2);

        // Verify the second triangle references vertices 3,4,5 (offset by 3).
        let indices = mesh.indices();
        assert_eq!(indices[0], [0, 1, 2]);
        assert_eq!(indices[1], [3, 4, 5]);
    }

    #[test]
    fn empty_3mf_returns_empty_model() {
        // Create a 3MF with an object but no mesh.
        let mut model = lib3mf::Model::new();
        let obj = lib3mf::Object::new(1);
        model.resources.objects.push(obj);
        model.build.items.push(lib3mf::BuildItem::new(1));

        let mut buffer = Cursor::new(Vec::new());
        model
            .to_writer(&mut buffer)
            .expect("failed to write test 3MF");
        let data = buffer.into_inner();

        let result = parse(&data);
        assert!(
            matches!(result, Err(FileIOError::EmptyModel)),
            "expected EmptyModel for 3MF with no mesh data"
        );
    }

    #[test]
    fn invalid_3mf_returns_threemf_error() {
        // Not a valid ZIP file.
        let data = b"this is not a 3MF file";
        let result = parse(data);
        assert!(
            matches!(result, Err(FileIOError::ThreeMfError(_))),
            "expected ThreeMfError for invalid data"
        );
    }
}
