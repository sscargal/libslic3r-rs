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

use std::fs::File;
use std::io::{BufWriter, Seek, Write};
use std::path::Path;

use lib3mf_converters::obj::ObjExporter;
use lib3mf_converters::stl::BinaryStlExporter;
use lib3mf_core::model::{BuildItem, Geometry, Mesh, Object, ObjectType, ResourceId};
use lib3mf_core::Model;

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
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("stl") => Ok(ExportFormat::Stl),
        Some("3mf") => Ok(ExportFormat::ThreeMf),
        Some("obj") => Ok(ExportFormat::Obj),
        Some(ext) => Err(FileIOError::UnsupportedExportFormat(ext.to_string())),
        None => Err(FileIOError::UnsupportedExportFormat(
            "no extension".to_string(),
        )),
    }
}

/// Convert a [`TriangleMesh`] to a [`lib3mf_core::Model`] for export.
///
/// The conversion is lossy: f64 vertices are cast to f32, which is acceptable
/// for all mesh file formats (STL is inherently f32, OBJ typically f32, and
/// the 3MF spec uses float).
fn triangle_mesh_to_model(mesh: &TriangleMesh) -> Result<Model, FileIOError> {
    let mut lib3mf_mesh = Mesh::new();

    for v in mesh.vertices() {
        lib3mf_mesh.add_vertex(v.x as f32, v.y as f32, v.z as f32);
    }

    for tri in mesh.indices() {
        lib3mf_mesh.add_triangle(tri[0], tri[1], tri[2]);
    }

    let mut model = Model::default();
    let object = Object {
        id: ResourceId(1),
        object_type: ObjectType::Model,
        name: None,
        part_number: None,
        uuid: None,
        pid: None,
        pindex: None,
        thumbnail: None,
        geometry: Geometry::Mesh(lib3mf_mesh),
    };
    model
        .resources
        .add_object(object)
        .map_err(|e| FileIOError::WriteError(e.to_string()))?;
    model.build.items.push(BuildItem {
        object_id: ResourceId(1),
        uuid: None,
        path: None,
        part_number: None,
        transform: glam::Mat4::IDENTITY,
        printable: None,
    });

    Ok(model)
}

/// Save a mesh to a file, auto-detecting the format from the file extension.
///
/// # Errors
///
/// - [`FileIOError::UnsupportedExportFormat`] if the extension is not recognized.
/// - [`FileIOError::WriteError`] if the export fails.
/// - [`FileIOError::IoError`] if file creation fails.
pub fn save_mesh(mesh: &TriangleMesh, path: &Path) -> Result<(), FileIOError> {
    let format = format_from_extension(path)?;
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    save_mesh_to_writer(mesh, writer, format)
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
    mut writer: W,
    format: ExportFormat,
) -> Result<(), FileIOError> {
    let model = triangle_mesh_to_model(mesh)?;

    match format {
        ExportFormat::ThreeMf => {
            model
                .write(&mut writer)
                .map_err(|e| FileIOError::WriteError(e.to_string()))?;
        }
        ExportFormat::Stl => {
            BinaryStlExporter::write(&model, &mut writer)
                .map_err(|e| FileIOError::WriteError(e.to_string()))?;
        }
        ExportFormat::Obj => {
            ObjExporter::write(&model, &mut writer)
                .map_err(|e| FileIOError::WriteError(e.to_string()))?;
        }
    }

    Ok(())
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
