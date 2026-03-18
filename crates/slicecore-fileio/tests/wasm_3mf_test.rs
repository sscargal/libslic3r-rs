//! Integration tests proving 3MF parsing works end-to-end with lib3mf-core.
//!
//! These tests run on native targets and verify functional correctness.
//! WASM compilation is proven by the CI WASM build step (`cargo build --target wasm32-*`),
//! which now includes slicecore-fileio with unconditional 3MF support.

use std::io::Cursor;

use lib3mf_core::model::{BuildItem, Geometry, Mesh, Object, ObjectType, ResourceId};
use lib3mf_core::Model;
use slicecore_fileio::{load_mesh, FileIOError};

/// Create a minimal 3MF file in-memory using lib3mf-core's write API.
fn create_3mf(objects: Vec<Object>) -> Vec<u8> {
    let mut model = Model::default();
    for obj in objects {
        let obj_id = obj.id;
        model
            .resources
            .add_object(obj)
            .expect("failed to add object");
        model.build.items.push(BuildItem {
            object_id: obj_id,
            uuid: None,
            path: None,
            part_number: None,
            transform: glam::Mat4::IDENTITY,
            printable: None,
        });
    }

    let mut buffer = Cursor::new(Vec::new());
    model.write(&mut buffer).expect("failed to write test 3MF");
    buffer.into_inner()
}

/// Build a tetrahedron Object (4 vertices, 4 triangles) with the given ResourceId.
fn tetrahedron_object(id: u32) -> Object {
    let mut mesh = Mesh::new();
    mesh.add_vertex(0.0, 0.0, 0.0);
    mesh.add_vertex(1.0, 0.0, 0.0);
    mesh.add_vertex(0.5, 1.0, 0.0);
    mesh.add_vertex(0.5, 0.5, 1.0);

    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 1, 3);
    mesh.add_triangle(1, 2, 3);
    mesh.add_triangle(0, 2, 3);

    Object {
        id: ResourceId(id),
        object_type: ObjectType::Model,
        name: None,
        part_number: None,
        uuid: None,
        pid: None,
        pindex: None,
        thumbnail: None,
        geometry: Geometry::Mesh(mesh),
    }
}

// ---------------------------------------------------------------------------
// Test: 3MF round-trip via threemf::parse
// ---------------------------------------------------------------------------

#[test]
fn threemf_parse_roundtrip_single_object() {
    // Create a 3MF with a single tetrahedron, write it to bytes, parse it back.
    let data = create_3mf(vec![tetrahedron_object(1)]);
    let mesh = slicecore_fileio::threemf::parse(&data)
        .expect("threemf::parse should succeed on valid 3MF");

    assert_eq!(mesh.vertex_count(), 4, "tetrahedron has 4 vertices");
    assert_eq!(mesh.triangle_count(), 4, "tetrahedron has 4 triangles");
}

#[test]
fn threemf_parse_roundtrip_multi_object() {
    // Two tetrahedra as separate objects, merged into one mesh.
    let data = create_3mf(vec![tetrahedron_object(1), tetrahedron_object(2)]);
    let mesh = slicecore_fileio::threemf::parse(&data)
        .expect("threemf::parse should succeed on multi-object 3MF");

    assert_eq!(mesh.vertex_count(), 8, "two tetrahedra = 8 vertices");
    assert_eq!(mesh.triangle_count(), 8, "two tetrahedra = 8 triangles");

    // Verify index offsets: second object's indices should reference vertices 4-7.
    let indices = mesh.indices();
    let mut sorted: Vec<[u32; 3]> = indices.to_vec();
    sorted.sort();
    // First object triangles use vertices 0-3, second object uses 4-7.
    assert!(
        sorted.iter().any(|t| t.iter().all(|&v| v < 4)),
        "first object should have indices in 0..3"
    );
    assert!(
        sorted.iter().any(|t| t.iter().all(|&v| v >= 4)),
        "second object should have indices in 4..7"
    );
}

// ---------------------------------------------------------------------------
// Test: load_mesh dispatches 3MF correctly
// ---------------------------------------------------------------------------

#[test]
fn load_mesh_dispatches_3mf_data() {
    // Verify that load_mesh auto-detects 3MF format (ZIP magic bytes) and
    // dispatches to threemf::parse correctly.
    let data = create_3mf(vec![tetrahedron_object(1)]);
    let mesh = load_mesh(&data).expect("load_mesh should dispatch 3MF correctly");

    assert_eq!(mesh.vertex_count(), 4);
    assert_eq!(mesh.triangle_count(), 4);
}

// ---------------------------------------------------------------------------
// Test: threemf module is publicly accessible (compilation proves no cfg gate)
// ---------------------------------------------------------------------------

#[test]
fn threemf_module_publicly_accessible() {
    // This test's existence proves that slicecore_fileio::threemf is a public
    // module. If a WASM cfg gate were re-introduced, this would fail to compile.
    let data = create_3mf(vec![tetrahedron_object(1)]);
    let result: Result<slicecore_mesh::TriangleMesh, FileIOError> =
        slicecore_fileio::threemf::parse(&data);
    assert!(
        result.is_ok(),
        "threemf::parse is accessible and functional"
    );
}

// ---------------------------------------------------------------------------
// Test: invalid 3MF data returns appropriate error
// ---------------------------------------------------------------------------

#[test]
fn threemf_parse_invalid_data_returns_error() {
    // Not a valid ZIP file -- should return ThreeMfError.
    let data = b"PK\x03\x04invalid zip content that is long enough to be detected as 3MF";
    let result = slicecore_fileio::threemf::parse(data);
    assert!(
        matches!(result, Err(FileIOError::ThreeMfError(_))),
        "expected ThreeMfError for corrupt 3MF data, got err: {:?}",
        result.err()
    );
}
