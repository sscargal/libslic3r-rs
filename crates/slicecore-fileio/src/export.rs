//! Mesh export module for writing TriangleMesh to STL, 3MF, and OBJ formats.
//!
//! All format-specific writing is delegated to [`lib3mf_core`] (3MF) and
//! [`lib3mf_converters`] (STL, OBJ). The public API mirrors the import API:
//! - [`save_mesh`] writes to a file path, auto-detecting format from extension
//! - [`save_mesh_to_writer`] writes to any `Write + Seek` destination
//!
//! Internally, a [`TriangleMesh`] is converted to a [`lib3mf_core::Model`]
//! before being handed to the appropriate exporter. The f64 -> f32 vertex
//! conversion is lossy but acceptable for all mesh file formats.

use std::io::{BufWriter, Seek, Write};
use std::path::Path;

use crate::error::FileIOError;
use slicecore_mesh::TriangleMesh;

/// Output format for mesh export.
///
/// Separate from [`crate::detect::MeshFormat`] because the import enum
/// distinguishes STL binary/ASCII variants, while export always uses binary STL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Binary STL format.
    Stl,
    /// 3MF format (ZIP archive).
    ThreeMf,
    /// Wavefront OBJ format.
    Obj,
}

/// Detect the export format from a file path's extension.
///
/// Matches (case-insensitive): `.stl` -> [`ExportFormat::Stl`],
/// `.3mf` -> [`ExportFormat::ThreeMf`], `.obj` -> [`ExportFormat::Obj`].
///
/// # Errors
///
/// Returns [`FileIOError::UnsupportedExportFormat`] if the extension is missing
/// or not recognized.
pub fn format_from_extension(path: &Path) -> Result<ExportFormat, FileIOError> {
    todo!("implement format_from_extension")
}

/// Save a mesh to a file, auto-detecting the format from the file extension.
///
/// # Errors
///
/// - [`FileIOError::UnsupportedExportFormat`] if the extension is not recognized.
/// - [`FileIOError::WriteError`] if the export fails.
/// - [`FileIOError::IoError`] if file creation fails.
pub fn save_mesh(mesh: &TriangleMesh, path: &Path) -> Result<(), FileIOError> {
    todo!("implement save_mesh")
}

/// Save a mesh to any writer that implements `Write + Seek`.
///
/// The `Write + Seek` bound is required because 3MF output (ZIP archives)
/// needs seeking. STL and OBJ writers only need `Write`, but the unified
/// API accepts `Write + Seek` for simplicity. Both `File` and
/// `Cursor<Vec<u8>>` satisfy this bound.
///
/// # Errors
///
/// - [`FileIOError::WriteError`] if the export fails.
pub fn save_mesh_to_writer<W: Write + Seek>(
    mesh: &TriangleMesh,
    writer: W,
    format: ExportFormat,
) -> Result<(), FileIOError> {
    todo!("implement save_mesh_to_writer")
}

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_math::Point3;
    use std::io::Cursor;

    /// Build a tetrahedron mesh (4 vertices, 4 triangles) for round-trip tests.
    fn tetrahedron_mesh() -> TriangleMesh {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
            Point3::new(0.5, 0.5, 1.0),
        ];
        let indices = vec![[0, 1, 2], [0, 1, 3], [1, 2, 3], [0, 2, 3]];
        TriangleMesh::new(vertices, indices).expect("valid tetrahedron")
    }

    #[test]
    fn round_trip_3mf() {
        let mesh = tetrahedron_mesh();
        let mut buf = Cursor::new(Vec::new());
        save_mesh_to_writer(&mesh, &mut buf, ExportFormat::ThreeMf).unwrap();

        let data = buf.into_inner();
        assert!(!data.is_empty(), "3MF output should not be empty");

        let reimported = crate::load_mesh(&data).unwrap();
        assert_eq!(reimported.vertex_count(), mesh.vertex_count());
        assert_eq!(reimported.triangle_count(), mesh.triangle_count());
    }

    #[test]
    fn round_trip_binary_stl() {
        let mesh = tetrahedron_mesh();
        let mut buf = Cursor::new(Vec::new());
        save_mesh_to_writer(&mesh, &mut buf, ExportFormat::Stl).unwrap();

        let data = buf.into_inner();
        assert!(!data.is_empty(), "STL output should not be empty");

        let reimported = crate::load_mesh(&data).unwrap();
        // STL vertex deduplication may change vertex count, but triangle count must match
        assert_eq!(reimported.triangle_count(), mesh.triangle_count());
    }

    #[test]
    fn round_trip_obj() {
        let mesh = tetrahedron_mesh();
        let mut buf = Cursor::new(Vec::new());
        save_mesh_to_writer(&mesh, &mut buf, ExportFormat::Obj).unwrap();

        let data = buf.into_inner();
        assert!(!data.is_empty(), "OBJ output should not be empty");

        let reimported = crate::load_mesh(&data).unwrap();
        assert_eq!(reimported.triangle_count(), mesh.triangle_count());
    }

    #[test]
    fn format_from_extension_stl() {
        assert_eq!(
            format_from_extension(Path::new("model.stl")).unwrap(),
            ExportFormat::Stl
        );
        assert_eq!(
            format_from_extension(Path::new("MODEL.STL")).unwrap(),
            ExportFormat::Stl
        );
    }

    #[test]
    fn format_from_extension_3mf() {
        assert_eq!(
            format_from_extension(Path::new("model.3mf")).unwrap(),
            ExportFormat::ThreeMf
        );
    }

    #[test]
    fn format_from_extension_obj() {
        assert_eq!(
            format_from_extension(Path::new("model.obj")).unwrap(),
            ExportFormat::Obj
        );
    }

    #[test]
    fn format_from_extension_unknown_returns_error() {
        let result = format_from_extension(Path::new("model.xyz"));
        assert!(matches!(result, Err(FileIOError::UnsupportedExportFormat(_))));
    }

    #[test]
    fn save_mesh_to_file_round_trip() {
        let mesh = tetrahedron_mesh();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.stl");

        save_mesh(&mesh, &path).unwrap();

        let data = std::fs::read(&path).unwrap();
        let reimported = crate::load_mesh(&data).unwrap();
        assert_eq!(reimported.triangle_count(), mesh.triangle_count());
    }

    #[test]
    fn save_mesh_to_writer_cursor_non_empty() {
        let mesh = tetrahedron_mesh();
        let mut buf = Cursor::new(Vec::new());
        save_mesh_to_writer(&mesh, &mut buf, ExportFormat::Stl).unwrap();
        assert!(!buf.into_inner().is_empty());
    }
}
