//! 3MF file parser.
//!
//! Parses 3MF files (ZIP+XML containers) into [`TriangleMesh`] using the
//! [`lib3mf_core`] crate (pure Rust, WASM-compatible). 3MF is the modern
//! successor to STL, used by Bambu Studio, OrcaSlicer, and other
//! contemporary slicers.
//!
//! Multi-object 3MF files are merged into a single `TriangleMesh` with
//! correct vertex index offsets.

use std::io::Cursor;

use lib3mf_core::archive::{find_model_path, ArchiveReader, ZipArchiver};
use lib3mf_core::parser::parse_model;
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
/// - [`FileIOError::ThreeMfError`] if lib3mf-core cannot parse the data.
/// - [`FileIOError::EmptyModel`] if no mesh geometry is found.
/// - [`FileIOError::MeshError`] if mesh construction fails.
pub fn parse(data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    let cursor = Cursor::new(data);

    let mut archiver = ZipArchiver::new(cursor)
        .map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;
    let model_path = find_model_path(&mut archiver)
        .map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;
    let model_data = archiver
        .read_entry(&model_path)
        .map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;
    let model = parse_model(Cursor::new(model_data))
        .map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;

    let mut all_vertices: Vec<Point3> = Vec::new();
    let mut all_indices: Vec<[u32; 3]> = Vec::new();

    for object in model.resources.iter_objects() {
        if let lib3mf_core::model::Geometry::Mesh(mesh) = &object.geometry {
            let vertex_offset = all_vertices.len() as u32;

            // Convert lib3mf-core Vertex (f32) to Point3 (f64). f32->f64 is lossless.
            for v in &mesh.vertices {
                all_vertices.push(Point3::new(v.x as f64, v.y as f64, v.z as f64));
            }

            // Convert lib3mf-core Triangle indices (u32) with vertex offset.
            for tri in &mesh.triangles {
                all_indices.push([
                    tri.v1 + vertex_offset,
                    tri.v2 + vertex_offset,
                    tri.v3 + vertex_offset,
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
    use lib3mf_core::model::{Geometry, Mesh, Object, ObjectType, ResourceId};
    use lib3mf_core::Model;

    /// Create a minimal 3MF file in-memory using lib3mf-core's write API.
    fn create_test_3mf(objects: Vec<Object>) -> Vec<u8> {
        let mut model = Model::default();
        for obj in objects {
            let obj_id = obj.id;
            model
                .resources
                .add_object(obj)
                .expect("failed to add object");
            model.build.items.push(lib3mf_core::model::BuildItem {
                object_id: obj_id,
                uuid: None,
                path: None,
                part_number: None,
                transform: glam::Mat4::IDENTITY,
                printable: None,
            });
        }

        let mut buffer = Cursor::new(Vec::new());
        model
            .write(&mut buffer)
            .expect("failed to write test 3MF");
        buffer.into_inner()
    }

    /// Create a single-object 3MF with a tetrahedron (4 vertices, 4 triangles).
    fn tetrahedron_3mf() -> Vec<u8> {
        let mut mesh = Mesh::new();
        mesh.add_vertex(0.0, 0.0, 0.0);
        mesh.add_vertex(1.0, 0.0, 0.0);
        mesh.add_vertex(0.5, 1.0, 0.0);
        mesh.add_vertex(0.5, 0.5, 1.0);

        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 1, 3);
        mesh.add_triangle(1, 2, 3);
        mesh.add_triangle(0, 2, 3);

        let object = Object {
            id: ResourceId(1),
            object_type: ObjectType::Model,
            name: None,
            part_number: None,
            uuid: None,
            pid: None,
            pindex: None,
            thumbnail: None,
            geometry: Geometry::Mesh(mesh),
        };

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
        let mut mesh1 = Mesh::new();
        mesh1.add_vertex(0.0, 0.0, 0.0);
        mesh1.add_vertex(1.0, 0.0, 0.0);
        mesh1.add_vertex(0.5, 1.0, 0.0);
        mesh1.add_triangle(0, 1, 2);

        let obj1 = Object {
            id: ResourceId(1),
            object_type: ObjectType::Model,
            name: None,
            part_number: None,
            uuid: None,
            pid: None,
            pindex: None,
            thumbnail: None,
            geometry: Geometry::Mesh(mesh1),
        };

        let mut mesh2 = Mesh::new();
        mesh2.add_vertex(2.0, 0.0, 0.0);
        mesh2.add_vertex(3.0, 0.0, 0.0);
        mesh2.add_vertex(2.5, 1.0, 0.0);
        mesh2.add_triangle(0, 1, 2);

        let obj2 = Object {
            id: ResourceId(2),
            object_type: ObjectType::Model,
            name: None,
            part_number: None,
            uuid: None,
            pid: None,
            pindex: None,
            thumbnail: None,
            geometry: Geometry::Mesh(mesh2),
        };

        let data = create_test_3mf(vec![obj1, obj2]);
        let mesh = parse(&data).unwrap();

        // 3 vertices from each object = 6 total.
        assert_eq!(mesh.vertex_count(), 6);
        // 1 triangle from each object = 2 total.
        assert_eq!(mesh.triangle_count(), 2);

        // HashMap iteration order is nondeterministic, so verify that both
        // index sets exist rather than asserting specific ordering.
        let indices = mesh.indices();
        let mut sorted_indices: Vec<[u32; 3]> = indices.to_vec();
        sorted_indices.sort();
        assert_eq!(sorted_indices[0], [0, 1, 2]);
        assert_eq!(sorted_indices[1], [3, 4, 5]);
    }

    #[test]
    fn empty_3mf_returns_empty_model() {
        // Create a 3MF with an object that has an empty mesh (no geometry content).
        let mut model = Model::default();
        let obj = Object {
            id: ResourceId(1),
            object_type: ObjectType::Model,
            name: None,
            part_number: None,
            uuid: None,
            pid: None,
            pindex: None,
            thumbnail: None,
            geometry: Geometry::Mesh(Mesh::new()),
        };
        model
            .resources
            .add_object(obj)
            .expect("failed to add object");
        model.build.items.push(lib3mf_core::model::BuildItem {
            object_id: ResourceId(1),
            uuid: None,
            path: None,
            part_number: None,
            transform: glam::Mat4::IDENTITY,
            printable: None,
        });

        let mut buffer = Cursor::new(Vec::new());
        model
            .write(&mut buffer)
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
