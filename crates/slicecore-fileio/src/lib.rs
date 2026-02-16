//! File I/O for 3D mesh formats.
//!
//! This crate provides parsers for common 3D model file formats used in
//! 3D printing: STL (binary and ASCII), 3MF, and OBJ. It also provides
//! magic-byte format detection to automatically identify file types.
//!
//! # Supported Formats
//!
//! | Format     | Import | Export | Module         |
//! |------------|--------|--------|----------------|
//! | Binary STL | Yes    | -      | [`stl_binary`] |
//! | ASCII STL  | Yes    | -      | [`stl_ascii`]  |
//! | 3MF        | Yes*   | -      | [`threemf`]    |
//! | OBJ        | Yes    | -      | [`obj`]        |
//!
//! *3MF support requires native targets. On `wasm32-unknown-unknown`, 3MF
//! parsing is unavailable because lib3mf depends on `zip` -> `zstd-sys`
//! (C library). The [`load_mesh`] function returns
//! [`FileIOError::ThreeMfError`] for 3MF data on WASM targets.
//!
//! # Format Detection
//!
//! Use [`detect_format`] to identify the format of a byte buffer before
//! parsing. This handles the well-known "binary STL starting with solid"
//! ambiguity.
//!
//! # Unified Interface
//!
//! Use [`load_mesh`] to auto-detect format and parse any supported file.
//! Use [`parse_stl`] for STL-only parsing.

pub mod detect;
pub mod error;
pub mod obj;
pub mod stl;
pub mod stl_ascii;
pub mod stl_binary;

// 3MF support is only available on native targets.
// lib3mf -> zip -> zstd-sys (C library) cannot compile for wasm32-unknown-unknown.
#[cfg(not(target_arch = "wasm32"))]
pub mod threemf;

// Re-export primary types at crate root.
pub use detect::{detect_format, MeshFormat};
pub use error::FileIOError;
pub use stl::parse_stl;

use slicecore_mesh::TriangleMesh;

/// Load a mesh from raw bytes, auto-detecting the file format.
///
/// Uses [`detect_format`] to identify the format, then dispatches to the
/// appropriate parser:
/// - [`MeshFormat::StlBinary`] -> [`stl_binary::parse`]
/// - [`MeshFormat::StlAscii`] -> [`stl_ascii::parse`]
/// - [`MeshFormat::ThreeMf`] -> [`threemf::parse`] (native only)
/// - [`MeshFormat::Obj`] -> [`obj::parse`]
///
/// On WASM targets, 3MF format returns [`FileIOError::ThreeMfError`] because
/// the lib3mf dependency is not available.
///
/// # Errors
///
/// - [`FileIOError::FileTooSmall`] or [`FileIOError::UnrecognizedFormat`] if
///   format detection fails.
/// - Any error from the underlying format-specific parser.
pub fn load_mesh(data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    let format = detect_format(data)?;

    match format {
        MeshFormat::StlBinary => stl_binary::parse(data),
        MeshFormat::StlAscii => stl_ascii::parse(data),
        MeshFormat::ThreeMf => parse_threemf_dispatch(data),
        MeshFormat::Obj => obj::parse(data),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_threemf_dispatch(data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    threemf::parse(data)
}

#[cfg(target_arch = "wasm32")]
fn parse_threemf_dispatch(_data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    Err(FileIOError::ThreeMfError(
        "3MF parsing is not available on WASM targets (lib3mf depends on native C libraries)"
            .to_string(),
    ))
}

/// Load a mesh from a reader, auto-detecting the file format.
///
/// Reads the entire content into memory, then delegates to [`load_mesh`].
///
/// # Errors
///
/// - [`FileIOError::IoError`] if reading from the reader fails.
/// - Any error from [`load_mesh`].
pub fn load_mesh_from_reader<R: std::io::Read>(
    reader: &mut R,
) -> Result<TriangleMesh, FileIOError> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;
    load_mesh(&data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a binary STL byte buffer from raw triangle data.
    fn build_binary_stl(triangles: &[([f32; 3], [f32; 3], [f32; 3], [f32; 3])]) -> Vec<u8> {
        let mut data = Vec::new();

        // 80-byte header
        let mut header = b"binary STL test".to_vec();
        header.resize(80, 0u8);
        data.extend_from_slice(&header);

        // Triangle count
        data.extend_from_slice(&(triangles.len() as u32).to_le_bytes());

        for (normal, v0, v1, v2) in triangles {
            for c in normal {
                data.extend_from_slice(&c.to_le_bytes());
            }
            for c in v0 {
                data.extend_from_slice(&c.to_le_bytes());
            }
            for c in v1 {
                data.extend_from_slice(&c.to_le_bytes());
            }
            for c in v2 {
                data.extend_from_slice(&c.to_le_bytes());
            }
            data.extend_from_slice(&0u16.to_le_bytes());
        }

        data
    }

    fn single_triangle_binary_stl() -> Vec<u8> {
        build_binary_stl(&[(
            [0.0f32, 0.0, 1.0],
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        )])
    }

    #[test]
    fn load_mesh_dispatches_binary_stl() {
        let data = single_triangle_binary_stl();
        let mesh = load_mesh(&data).unwrap();
        assert_eq!(mesh.triangle_count(), 1);
        assert_eq!(mesh.vertex_count(), 3);
    }

    #[test]
    fn load_mesh_dispatches_ascii_stl() {
        let data = b"solid test
  facet normal 0 0 1
    outer loop
      vertex 0 0 0
      vertex 1 0 0
      vertex 0 1 0
    endloop
  endfacet
endsolid test
";
        let mesh = load_mesh(data).unwrap();
        assert_eq!(mesh.triangle_count(), 1);
        assert_eq!(mesh.vertex_count(), 3);
    }

    #[test]
    fn load_mesh_dispatches_obj() {
        let data = b"v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 0.5 1.0 0.0
f 1 2 3
";
        let mesh = load_mesh(data).unwrap();
        assert_eq!(mesh.triangle_count(), 1);
        assert_eq!(mesh.vertex_count(), 3);
    }

    #[test]
    fn load_mesh_dispatches_3mf() {
        // Create a minimal 3MF using lib3mf's write API.
        use std::io::Cursor;

        let mut model = lib3mf::Model::new();
        let mut mesh = lib3mf::Mesh::new();
        mesh.vertices.push(lib3mf::Vertex::new(0.0, 0.0, 0.0));
        mesh.vertices.push(lib3mf::Vertex::new(1.0, 0.0, 0.0));
        mesh.vertices.push(lib3mf::Vertex::new(0.5, 1.0, 0.0));
        mesh.triangles.push(lib3mf::Triangle::new(0, 1, 2));

        let mut object = lib3mf::Object::new(1);
        object.mesh = Some(mesh);
        model.resources.objects.push(object);
        model.build.items.push(lib3mf::BuildItem::new(1));

        let mut buffer = Cursor::new(Vec::new());
        model.to_writer(&mut buffer).expect("write 3MF");
        let data = buffer.into_inner();

        let mesh = load_mesh(&data).unwrap();
        assert_eq!(mesh.triangle_count(), 1);
        assert_eq!(mesh.vertex_count(), 3);
    }

    #[test]
    fn load_mesh_from_reader_works() {
        let stl_data = single_triangle_binary_stl();
        let mut cursor = std::io::Cursor::new(stl_data);
        let mesh = load_mesh_from_reader(&mut cursor).unwrap();
        assert_eq!(mesh.triangle_count(), 1);
        assert_eq!(mesh.vertex_count(), 3);
    }

    #[test]
    fn load_mesh_unrecognized_format() {
        let data = b"this is just random text that doesn't match any format really at all and is long enough";
        let result = load_mesh(data);
        assert!(matches!(result, Err(FileIOError::UnrecognizedFormat)));
    }
}
