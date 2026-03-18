//! Integration tests for slicecore-fileio: synthetic fixtures, load pipeline,
//! and load-repair end-to-end tests.

use slicecore_fileio::{load_mesh, FileIOError};
use slicecore_mesh::{compute_stats, repair, scale};

// ---------------------------------------------------------------------------
// Synthetic fixture helpers
// ---------------------------------------------------------------------------

/// Build a valid binary STL of a unit cube (8 unique vertices, 12 triangles).
///
/// Binary STL format:
///   80-byte header
///   4-byte u32 triangle count (little-endian)
///   For each triangle (50 bytes):
///     12 bytes normal (3 x f32 LE)
///     36 bytes 3 vertices (9 x f32 LE)
///     2 bytes attribute count (u16 LE, always 0)
fn make_binary_stl_cube() -> Vec<u8> {
    // 12 triangles (2 per face, 6 faces)
    // Vertices of unit cube: (0,0,0)-(1,1,1)
    #[rustfmt::skip]
    let triangles: Vec<([f32; 3], [[f32; 3]; 3])> = vec![
        // Front face (z=1)
        ([0.0, 0.0, 1.0], [[0.0,0.0,1.0], [1.0,0.0,1.0], [1.0,1.0,1.0]]),
        ([0.0, 0.0, 1.0], [[0.0,0.0,1.0], [1.0,1.0,1.0], [0.0,1.0,1.0]]),
        // Back face (z=0)
        ([0.0, 0.0,-1.0], [[1.0,0.0,0.0], [0.0,0.0,0.0], [0.0,1.0,0.0]]),
        ([0.0, 0.0,-1.0], [[1.0,0.0,0.0], [0.0,1.0,0.0], [1.0,1.0,0.0]]),
        // Right face (x=1)
        ([1.0, 0.0, 0.0], [[1.0,0.0,0.0], [1.0,1.0,0.0], [1.0,1.0,1.0]]),
        ([1.0, 0.0, 0.0], [[1.0,0.0,0.0], [1.0,1.0,1.0], [1.0,0.0,1.0]]),
        // Left face (x=0)
        ([-1.0, 0.0, 0.0], [[0.0,0.0,0.0], [0.0,0.0,1.0], [0.0,1.0,1.0]]),
        ([-1.0, 0.0, 0.0], [[0.0,0.0,0.0], [0.0,1.0,1.0], [0.0,1.0,0.0]]),
        // Top face (y=1)
        ([0.0, 1.0, 0.0], [[0.0,1.0,0.0], [0.0,1.0,1.0], [1.0,1.0,1.0]]),
        ([0.0, 1.0, 0.0], [[0.0,1.0,0.0], [1.0,1.0,1.0], [1.0,1.0,0.0]]),
        // Bottom face (y=0)
        ([0.0,-1.0, 0.0], [[0.0,0.0,0.0], [1.0,0.0,0.0], [1.0,0.0,1.0]]),
        ([0.0,-1.0, 0.0], [[0.0,0.0,0.0], [1.0,0.0,1.0], [0.0,0.0,1.0]]),
    ];

    let mut data = Vec::new();

    // 80-byte header
    let mut header = b"binary STL cube test fixture".to_vec();
    header.resize(80, 0u8);
    data.extend_from_slice(&header);

    // Triangle count
    data.extend_from_slice(&(triangles.len() as u32).to_le_bytes());

    for (normal, verts) in &triangles {
        // Normal
        for c in normal {
            data.extend_from_slice(&c.to_le_bytes());
        }
        // 3 vertices
        for v in verts {
            for c in v {
                data.extend_from_slice(&c.to_le_bytes());
            }
        }
        // Attribute byte count
        data.extend_from_slice(&0u16.to_le_bytes());
    }

    data
}

/// Build a valid ASCII STL of a unit cube.
fn make_ascii_stl_cube() -> Vec<u8> {
    let mut s = String::from("solid cube\n");

    #[rustfmt::skip]
    let triangles: Vec<([f32; 3], [[f32; 3]; 3])> = vec![
        // Front face (z=1)
        ([0.0, 0.0, 1.0], [[0.0,0.0,1.0], [1.0,0.0,1.0], [1.0,1.0,1.0]]),
        ([0.0, 0.0, 1.0], [[0.0,0.0,1.0], [1.0,1.0,1.0], [0.0,1.0,1.0]]),
        // Back face (z=0)
        ([0.0, 0.0,-1.0], [[1.0,0.0,0.0], [0.0,0.0,0.0], [0.0,1.0,0.0]]),
        ([0.0, 0.0,-1.0], [[1.0,0.0,0.0], [0.0,1.0,0.0], [1.0,1.0,0.0]]),
        // Right face (x=1)
        ([1.0, 0.0, 0.0], [[1.0,0.0,0.0], [1.0,1.0,0.0], [1.0,1.0,1.0]]),
        ([1.0, 0.0, 0.0], [[1.0,0.0,0.0], [1.0,1.0,1.0], [1.0,0.0,1.0]]),
        // Left face (x=0)
        ([-1.0, 0.0, 0.0], [[0.0,0.0,0.0], [0.0,0.0,1.0], [0.0,1.0,1.0]]),
        ([-1.0, 0.0, 0.0], [[0.0,0.0,0.0], [0.0,1.0,1.0], [0.0,1.0,0.0]]),
        // Top face (y=1)
        ([0.0, 1.0, 0.0], [[0.0,1.0,0.0], [0.0,1.0,1.0], [1.0,1.0,1.0]]),
        ([0.0, 1.0, 0.0], [[0.0,1.0,0.0], [1.0,1.0,1.0], [1.0,1.0,0.0]]),
        // Bottom face (y=0)
        ([0.0,-1.0, 0.0], [[0.0,0.0,0.0], [1.0,0.0,0.0], [1.0,0.0,1.0]]),
        ([0.0,-1.0, 0.0], [[0.0,0.0,0.0], [1.0,0.0,1.0], [0.0,0.0,1.0]]),
    ];

    for (normal, verts) in &triangles {
        s.push_str(&format!(
            "  facet normal {} {} {}\n",
            normal[0], normal[1], normal[2]
        ));
        s.push_str("    outer loop\n");
        for v in verts {
            s.push_str(&format!("      vertex {} {} {}\n", v[0], v[1], v[2]));
        }
        s.push_str("    endloop\n");
        s.push_str("  endfacet\n");
    }
    s.push_str("endsolid cube\n");

    s.into_bytes()
}

/// Build a valid OBJ file of a unit cube (8 vertices, 12 triangular faces).
fn make_obj_cube() -> Vec<u8> {
    let obj = "\
# Unit cube OBJ fixture
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 1.0 1.0 0.0
v 0.0 1.0 0.0
v 0.0 0.0 1.0
v 1.0 0.0 1.0
v 1.0 1.0 1.0
v 0.0 1.0 1.0
# Front face (z=1)
f 5 6 7
f 5 7 8
# Back face (z=0)
f 2 1 4
f 2 4 3
# Right face (x=1)
f 2 3 7
f 2 7 6
# Left face (x=0)
f 1 5 8
f 1 8 4
# Top face (y=1)
f 4 8 7
f 4 7 3
# Bottom face (y=0)
f 1 2 6
f 1 6 5
";
    obj.as_bytes().to_vec()
}

// ---------------------------------------------------------------------------
// File loading integration tests
// ---------------------------------------------------------------------------

#[test]
fn load_mesh_binary_stl() {
    let data = make_binary_stl_cube();
    let mesh = load_mesh(&data).expect("should load binary STL cube");
    assert_eq!(mesh.vertex_count(), 8, "cube has 8 unique vertices");
    assert_eq!(mesh.triangle_count(), 12, "cube has 12 triangles");
}

#[test]
fn load_mesh_ascii_stl() {
    let data = make_ascii_stl_cube();
    let mesh = load_mesh(&data).expect("should load ASCII STL cube");
    assert_eq!(mesh.vertex_count(), 8, "cube has 8 unique vertices");
    assert_eq!(mesh.triangle_count(), 12, "cube has 12 triangles");
}

#[test]
fn load_mesh_obj() {
    let data = make_obj_cube();
    let mesh = load_mesh(&data).expect("should load OBJ cube");
    assert_eq!(mesh.vertex_count(), 8, "cube has 8 unique vertices");
    assert_eq!(mesh.triangle_count(), 12, "cube has 12 triangles");
}

#[test]
fn load_mesh_unrecognized() {
    let data = b"this is random garbage that does not match any known 3D format whatsoever padding";
    let result = load_mesh(data);
    assert!(
        matches!(result, Err(FileIOError::UnrecognizedFormat)),
        "expected UnrecognizedFormat, got: {:?}",
        result.as_ref().err()
    );
}

#[test]
fn format_detection_binary_stl_with_solid_header() {
    // Create a binary STL whose 80-byte header begins with "solid" --
    // this must still be detected as binary (not ASCII) because it lacks
    // "facet normal" in the text.
    let mut data = Vec::new();
    let mut header = b"solid misleading".to_vec();
    header.resize(80, 0u8);
    data.extend_from_slice(&header);
    // 1 triangle
    data.extend_from_slice(&1u32.to_le_bytes());
    // 50 bytes of triangle data (zeros are fine for parsing)
    data.extend_from_slice(&[0u8; 50]);

    let mesh = load_mesh(&data).expect("should load binary STL with solid header");
    assert_eq!(mesh.triangle_count(), 1);
}

// ---------------------------------------------------------------------------
// Load-repair pipeline integration test
// ---------------------------------------------------------------------------

#[test]
fn load_binary_stl_then_repair_produces_valid_mesh() {
    let data = make_binary_stl_cube();
    let mesh = load_mesh(&data).expect("load binary STL");

    // Pass through repair pipeline
    let (repaired, report) =
        repair(mesh.vertices().to_vec(), mesh.indices().to_vec()).expect("repair should succeed");

    // A clean cube should report already clean (or very minimal changes)
    assert_eq!(
        report.degenerate_removed, 0,
        "clean cube should have no degenerate triangles"
    );
    assert_eq!(repaired.triangle_count(), 12);
    assert_eq!(repaired.vertex_count(), 8);
}

// ---------------------------------------------------------------------------
// Load-transform pipeline integration test (SC3: transforms work on loaded meshes)
// ---------------------------------------------------------------------------

#[test]
fn load_binary_stl_then_scale_doubles_bounding_box() {
    let data = make_binary_stl_cube();
    let mesh = load_mesh(&data).expect("load binary STL");

    // Original bounding box: 0..1 in all axes
    let orig_aabb = mesh.aabb();
    let orig_x_size = orig_aabb.max.x - orig_aabb.min.x;
    let orig_y_size = orig_aabb.max.y - orig_aabb.min.y;
    let orig_z_size = orig_aabb.max.z - orig_aabb.min.z;

    // Scale by 2x in all axes
    let scaled = scale(&mesh, 2.0, 2.0, 2.0);
    let new_aabb = scaled.aabb();
    let new_x_size = new_aabb.max.x - new_aabb.min.x;
    let new_y_size = new_aabb.max.y - new_aabb.min.y;
    let new_z_size = new_aabb.max.z - new_aabb.min.z;

    assert!(
        (new_x_size - orig_x_size * 2.0).abs() < 1e-6,
        "X size should double: {} -> {}",
        orig_x_size,
        new_x_size
    );
    assert!(
        (new_y_size - orig_y_size * 2.0).abs() < 1e-6,
        "Y size should double: {} -> {}",
        orig_y_size,
        new_y_size
    );
    assert!(
        (new_z_size - orig_z_size * 2.0).abs() < 1e-6,
        "Z size should double: {} -> {}",
        orig_z_size,
        new_z_size
    );

    // Volume should be 8x the original
    let stats = compute_stats(&scaled);
    assert!(
        (stats.volume - 8.0).abs() < 1e-4,
        "Volume should be ~8.0, got {}",
        stats.volume
    );
}
