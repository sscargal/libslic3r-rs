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

use md5::{Digest, Md5};

use crate::error::FileIOError;
use crate::project_config::{
    build_filament_settings_config, build_machine_settings_config,
    build_process_settings_config, build_project_metadata_config,
};
use crate::threemf::ThreeMfObjectConfig;
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

/// Convert a [`TriangleMesh`] to a [`lib3mf_core::Model`] for export,
/// optionally embedding thumbnail image data at `Metadata/thumbnail.png`.
///
/// Note: the data must be PNG for 3MF compatibility.
fn triangle_mesh_to_model_with_thumbnail(
    mesh: &TriangleMesh,
    thumbnail_data: Option<&[u8]>,
) -> Result<Model, FileIOError> {
    let mut model = triangle_mesh_to_model(mesh)?;

    if let Some(data) = thumbnail_data {
        let thumb_path = "Metadata/thumbnail.png".to_string();
        model.attachments.insert(thumb_path.clone(), data.to_vec());
        // Set the thumbnail path on the first object
        if let Some(obj) = model.resources.iter_objects_mut().next() {
            obj.thumbnail = Some(thumb_path);
        }
    }

    Ok(model)
}

/// Save a mesh to a file, auto-detecting the format from the file extension,
/// optionally embedding thumbnail image data in 3MF output (must be PNG for 3MF).
pub fn save_mesh_with_thumbnail(
    mesh: &TriangleMesh,
    path: &Path,
    thumbnail_data: Option<&[u8]>,
) -> Result<(), FileIOError> {
    let format = format_from_extension(path)?;
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    save_mesh_to_writer_with_thumbnail(mesh, writer, format, thumbnail_data)
}

/// Save a mesh to any writer that implements `Write + Seek`, optionally
/// embedding thumbnail image data. For non-3MF formats, the thumbnail is ignored.
pub fn save_mesh_to_writer_with_thumbnail<W: Write + Seek>(
    mesh: &TriangleMesh,
    mut writer: W,
    format: ExportFormat,
    thumbnail_data: Option<&[u8]>,
) -> Result<(), FileIOError> {
    match format {
        ExportFormat::ThreeMf => {
            let model = triangle_mesh_to_model_with_thumbnail(mesh, thumbnail_data)?;
            model
                .write(&mut writer)
                .map_err(|e| FileIOError::WriteError(e.to_string()))?;
        }
        _ => {
            // Non-3MF formats ignore thumbnail
            save_mesh_to_writer(mesh, writer, format)?;
        }
    }
    Ok(())
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

/// Maps a SliceCore field name to a PrusaSlicer-compatible field name for export.
///
/// Returns `None` for fields that have no PrusaSlicer equivalent.
fn map_to_slicer_field(slicecore_key: &str) -> Option<&str> {
    match slicecore_key {
        "infill_density" => Some("fill_density"),
        "infill_pattern" => Some("fill_pattern"),
        "wall_count" => Some("perimeters"),
        "layer_height" => Some("layer_height"),
        "speeds.perimeter" => Some("perimeter_speed"),
        "speeds.infill" => Some("infill_speed"),
        "top_solid_layers" => Some("top_solid_layers"),
        "bottom_solid_layers" => Some("bottom_solid_layers"),
        "support_enabled" => Some("support_material"),
        _ => None,
    }
}

/// Converts a TOML value to a string suitable for slicer config files.
fn toml_value_to_slicer_string(key: &str, value: &toml::Value) -> String {
    match value {
        toml::Value::Float(f) => {
            // Convert fractions back to percentages for density fields.
            if key == "fill_density" {
                format!("{}%", f * 100.0)
            } else {
                f.to_string()
            }
        }
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Boolean(b) => {
            if *b {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        toml::Value::String(s) => s.clone(),
        _ => value.to_string(),
    }
}

/// Builds a `Metadata/model_settings.config` XML string for per-object overrides.
///
/// Each object's overrides are written as `<metadata key="..." value="..."/>` elements
/// inside an `<object>` block. Both SliceCore-native keys (prefixed `slicecore:`) and
/// best-effort PrusaSlicer-compatible keys are written.
fn build_model_settings_config(object_configs: &[ThreeMfObjectConfig]) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<config>\n");

    for (idx, config) in object_configs.iter().enumerate() {
        let obj_id = idx + 1;
        xml.push_str(&format!("  <object id=\"{}\">\n", obj_id));

        // Write object name if present.
        if let Some(name) = &config.name {
            xml.push_str(&format!(
                "    <metadata key=\"name\" value=\"{}\"/>\n",
                xml_escape(name)
            ));
        }

        // Write mapped overrides in both slicecore: and slicer-compatible namespaces.
        for (slicecore_key, value) in &config.overrides {
            // SliceCore namespace entry.
            xml.push_str(&format!(
                "    <metadata key=\"slicecore:{}\" value=\"{}\"/>\n",
                xml_escape(slicecore_key),
                xml_escape(&value.to_string()),
            ));

            // Best-effort PrusaSlicer-compatible entry.
            if let Some(slicer_key) = map_to_slicer_field(slicecore_key) {
                let slicer_value = toml_value_to_slicer_string(slicer_key, value);
                xml.push_str(&format!(
                    "    <metadata key=\"{}\" value=\"{}\"/>\n",
                    xml_escape(slicer_key),
                    xml_escape(&slicer_value),
                ));
            }
        }

        // Write unmapped pass-through metadata (for round-tripping).
        for (key, value) in &config.unmapped {
            xml.push_str(&format!(
                "    <metadata key=\"{}\" value=\"{}\"/>\n",
                xml_escape(key),
                xml_escape(value),
            ));
        }

        xml.push_str("  </object>\n");
    }

    xml.push_str("</config>\n");
    xml
}

/// Options for writing a full 3MF project file with embedded G-code, settings, and metadata.
#[derive(Debug, Clone)]
pub struct ProjectExportOptions {
    /// G-code bytes per plate (complete, identical to standalone output). 1-indexed plates.
    pub gcode_per_plate: Vec<Vec<u8>>,
    /// PNG thumbnail data per plate (`None` = skip thumbnail for that plate).
    pub thumbnails_per_plate: Vec<Option<Vec<u8>>>,
    /// Plate metadata (positions, stats, filament usage) per plate.
    pub plate_metadata: Vec<crate::plate_metadata::PlateMetadata>,
    /// Full `PrintConfig` snapshot serialized as TOML string for `slicecore_config.toml`.
    pub config_toml: String,
    /// Process settings as slicer-compatible `(key, value)` pairs for XML config.
    pub process_settings: Vec<(String, String)>,
    /// Filament settings as slicer-compatible `(key, value)` pairs for XML config.
    pub filament_settings: Vec<(String, String)>,
    /// Machine settings as slicer-compatible `(key, value)` pairs for XML config.
    pub machine_settings: Vec<(String, String)>,
    /// Project-level metadata (version, timestamp, provenance).
    pub project_metadata: crate::project_config::ProjectMetadata,
    /// AMS filament slot mapping (omit if `None`).
    pub ams_mapping: Option<crate::project_config::AmsMapping>,
}

/// Builds a `lib3mf_core::Model` from multiple meshes and per-object configs.
///
/// Shared between [`export_plate_to_3mf`] and [`export_project_to_3mf`] to
/// avoid duplicating model-building logic.
fn build_plate_model(
    meshes: &[&TriangleMesh],
    object_configs: &[ThreeMfObjectConfig],
) -> Result<Model, FileIOError> {
    let mut model = Model::default();
    model
        .metadata
        .insert("Application".to_string(), "SliceCore".to_string());

    for (idx, mesh) in meshes.iter().enumerate() {
        let resource_id = ResourceId(u32::try_from(idx + 1).unwrap_or(1));

        let mut lib3mf_mesh = Mesh::new();
        for v in mesh.vertices() {
            lib3mf_mesh.add_vertex(v.x as f32, v.y as f32, v.z as f32);
        }
        for tri in mesh.indices() {
            lib3mf_mesh.add_triangle(tri[0], tri[1], tri[2]);
        }

        let name = object_configs.get(idx).and_then(|c| c.name.clone());

        let object = Object {
            id: resource_id,
            object_type: ObjectType::Model,
            name,
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
            object_id: resource_id,
            uuid: None,
            path: None,
            part_number: None,
            transform: glam::Mat4::IDENTITY,
            printable: None,
        });
    }

    Ok(model)
}

/// Exports multiple meshes to a full 3MF project file with embedded G-code,
/// settings configs, thumbnails, plate metadata, and project-level metadata.
///
/// This produces a "project" 3MF, as used by Bambu Studio and OrcaSlicer,
/// containing everything needed to reproduce the print.
///
/// # Errors
///
/// - [`FileIOError::WriteError`] if archive writing fails.
pub fn export_project_to_3mf<W: Write + Seek>(
    meshes: &[&TriangleMesh],
    object_configs: &[ThreeMfObjectConfig],
    project_options: &ProjectExportOptions,
    mut writer: W,
) -> Result<(), FileIOError> {
    let mut model = build_plate_model(meshes, object_configs)?;

    // Attach per-object settings if any overrides exist.
    let has_overrides = object_configs
        .iter()
        .any(|c| !c.overrides.is_empty() || !c.unmapped.is_empty());
    if has_overrides {
        let config_xml = build_model_settings_config(object_configs);
        model.attachments.insert(
            "Metadata/model_settings.config".to_string(),
            config_xml.into_bytes(),
        );
    }

    // Embed G-code, MD5 checksums, thumbnails, and plate metadata per plate.
    for (i, gcode_bytes) in project_options.gcode_per_plate.iter().enumerate() {
        let plate_num = i + 1;

        // G-code
        model.attachments.insert(
            format!("Metadata/plate_{plate_num}.gcode"),
            gcode_bytes.clone(),
        );

        // MD5 checksum
        let hash = Md5::digest(gcode_bytes);
        let md5_hex = format!("{hash:x}");
        model.attachments.insert(
            format!("Metadata/plate_{plate_num}.gcode.md5"),
            md5_hex.into_bytes(),
        );

        // Thumbnail (if provided)
        if let Some(Some(png_bytes)) = project_options.thumbnails_per_plate.get(i) {
            model.attachments.insert(
                format!("Metadata/plate_{plate_num}.png"),
                png_bytes.clone(),
            );
        }

        // Plate metadata JSON
        if let Some(plate_meta) = project_options.plate_metadata.get(i) {
            let json = serde_json::to_string_pretty(plate_meta)
                .map_err(|e| FileIOError::WriteError(e.to_string()))?;
            model.attachments.insert(
                format!("Metadata/plate_{plate_num}.json"),
                json.into_bytes(),
            );
        }
    }

    // Settings configs
    model.attachments.insert(
        "Metadata/process_settings.config".to_string(),
        build_process_settings_config(&project_options.process_settings).into_bytes(),
    );
    model.attachments.insert(
        "Metadata/filament_settings.config".to_string(),
        build_filament_settings_config(&project_options.filament_settings).into_bytes(),
    );
    model.attachments.insert(
        "Metadata/machine_settings.config".to_string(),
        build_machine_settings_config(&project_options.machine_settings).into_bytes(),
    );

    // SliceCore native config (TOML)
    model.attachments.insert(
        "Metadata/slicecore_config.toml".to_string(),
        project_options.config_toml.as_bytes().to_vec(),
    );

    // Project metadata
    model.attachments.insert(
        "Metadata/project_settings.config".to_string(),
        build_project_metadata_config(&project_options.project_metadata).into_bytes(),
    );

    // AMS mapping (optional)
    if let Some(ref ams) = project_options.ams_mapping {
        let json = serde_json::to_string_pretty(ams)
            .map_err(|e| FileIOError::WriteError(e.to_string()))?;
        model.attachments.insert(
            "Metadata/ams_mapping.json".to_string(),
            json.into_bytes(),
        );
    }

    model
        .write(&mut writer)
        .map_err(|e| FileIOError::WriteError(e.to_string()))?;

    Ok(())
}

/// Minimal XML attribute value escaping.
pub(crate) fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Exports multiple meshes to a 3MF file with per-object settings.
///
/// Writes models, per-object settings, and transforms. Uses `slicecore:` namespace
/// for SliceCore-specific metadata and includes best-effort PrusaSlicer/OrcaSlicer
/// compatible metadata for basic settings.
///
/// The per-object settings are stored in `Metadata/model_settings.config` inside
/// the 3MF archive, matching the format used by OrcaSlicer/Bambu Studio.
///
/// # Errors
///
/// - [`FileIOError::WriteError`] if the export fails.
pub fn export_plate_to_3mf<W: Write + Seek>(
    meshes: &[&TriangleMesh],
    object_configs: &[ThreeMfObjectConfig],
    mut writer: W,
) -> Result<(), FileIOError> {
    let mut model = build_plate_model(meshes, object_configs)?;

    // Build and attach per-object settings config if any overrides exist.
    let has_overrides = object_configs
        .iter()
        .any(|c| !c.overrides.is_empty() || !c.unmapped.is_empty());

    if has_overrides {
        let config_xml = build_model_settings_config(object_configs);
        model.attachments.insert(
            "Metadata/model_settings.config".to_string(),
            config_xml.into_bytes(),
        );
    }

    model
        .write(&mut writer)
        .map_err(|e| FileIOError::WriteError(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lib3mf_core::archive::ArchiveReader;
    use slicecore_math::Point3;
    use std::collections::HashMap;
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
        assert!(matches!(
            result,
            Err(FileIOError::UnsupportedExportFormat(_))
        ));
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

    #[test]
    fn save_mesh_with_thumbnail_3mf_larger_than_without() {
        let mesh = tetrahedron_mesh();

        // Without thumbnail
        let mut buf_without = Cursor::new(Vec::new());
        save_mesh_to_writer(&mesh, &mut buf_without, ExportFormat::ThreeMf).unwrap();
        let size_without = buf_without.into_inner().len();

        // With thumbnail (fake PNG data)
        let fake_png = vec![
            0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3, 4, 5,
        ];
        let mut buf_with = Cursor::new(Vec::new());
        save_mesh_to_writer_with_thumbnail(
            &mesh,
            &mut buf_with,
            ExportFormat::ThreeMf,
            Some(&fake_png),
        )
        .unwrap();
        let size_with = buf_with.into_inner().len();

        assert!(
            size_with > size_without,
            "3MF with thumbnail ({}) should be larger than without ({})",
            size_with,
            size_without
        );
    }

    // --- Per-object export tests ---

    #[test]
    fn export_plate_no_overrides() {
        let mesh = tetrahedron_mesh();
        let configs = vec![ThreeMfObjectConfig::default()];
        let mut buf = Cursor::new(Vec::new());

        export_plate_to_3mf(&[&mesh], &configs, &mut buf).unwrap();

        let data = buf.into_inner();
        assert!(!data.is_empty());

        // Should be importable as a regular 3MF.
        let reimported = crate::load_mesh(&data).unwrap();
        assert_eq!(reimported.vertex_count(), mesh.vertex_count());
        assert_eq!(reimported.triangle_count(), mesh.triangle_count());
    }

    #[test]
    fn export_plate_with_overrides_writes_slicecore_namespace() {
        let mesh = tetrahedron_mesh();
        let mut overrides = toml::map::Map::new();
        overrides.insert("infill_density".to_string(), toml::Value::Float(0.5));
        overrides.insert("wall_count".to_string(), toml::Value::Integer(3));

        let config = ThreeMfObjectConfig {
            name: Some("Part A".to_string()),
            overrides,
            unmapped: HashMap::new(),
            transform: None,
            modifiers: Vec::new(),
        };

        let mut buf = Cursor::new(Vec::new());
        export_plate_to_3mf(&[&mesh], &[config], &mut buf).unwrap();

        let data = buf.into_inner();
        assert!(!data.is_empty());

        // Verify the config file is in the archive by reading it back.
        let cursor = std::io::Cursor::new(data.as_slice());
        let mut archiver = lib3mf_core::archive::ZipArchiver::new(cursor).unwrap();
        assert!(archiver.entry_exists("Metadata/model_settings.config"));

        let config_data = archiver
            .read_entry("Metadata/model_settings.config")
            .unwrap();
        let config_str = std::str::from_utf8(&config_data).unwrap();

        // Should contain slicecore: namespace entries.
        assert!(
            config_str.contains("slicecore:"),
            "config should contain slicecore: namespace: {}",
            config_str
        );
        assert!(
            config_str.contains("infill_density"),
            "config should contain infill_density: {}",
            config_str
        );
        // Should also contain PrusaSlicer-compatible key.
        assert!(
            config_str.contains("fill_density"),
            "config should contain fill_density (slicer compat): {}",
            config_str
        );
    }

    #[test]
    fn export_import_round_trip_preserves_overrides() {
        let mesh = tetrahedron_mesh();
        let mut overrides = toml::map::Map::new();
        overrides.insert("wall_count".to_string(), toml::Value::Integer(4));
        overrides.insert("layer_height".to_string(), toml::Value::Float(0.15));

        let config = ThreeMfObjectConfig {
            name: Some("Test Object".to_string()),
            overrides,
            unmapped: HashMap::new(),
            transform: None,
            modifiers: Vec::new(),
        };

        // Export.
        let mut buf = Cursor::new(Vec::new());
        export_plate_to_3mf(&[&mesh], &[config], &mut buf).unwrap();
        let data = buf.into_inner();

        // Re-import with config.
        let result = crate::threemf::parse_with_config(&data).unwrap();

        assert_eq!(result.meshes.len(), 1);
        assert_eq!(result.meshes[0].vertex_count(), mesh.vertex_count());
        assert_eq!(result.object_configs.len(), 1);

        // The slicer-compatible keys should have been re-imported.
        // perimeters -> wall_count, layer_height -> layer_height
        let re_overrides = &result.object_configs[0].overrides;
        assert_eq!(
            re_overrides
                .get("wall_count")
                .unwrap()
                .as_integer()
                .unwrap(),
            4,
            "wall_count should round-trip"
        );
        assert!(
            (re_overrides
                .get("layer_height")
                .unwrap()
                .as_float()
                .unwrap()
                - 0.15)
                .abs()
                < 0.001,
            "layer_height should round-trip"
        );
    }

    #[test]
    fn reverse_field_mapping_roundtrip() {
        // Test that map_to_slicer_field covers all fields from import.
        let known_fields = [
            "infill_density",
            "infill_pattern",
            "wall_count",
            "layer_height",
            "speeds.perimeter",
            "speeds.infill",
            "top_solid_layers",
            "bottom_solid_layers",
            "support_enabled",
        ];

        for field in &known_fields {
            assert!(
                map_to_slicer_field(field).is_some(),
                "map_to_slicer_field should handle: {}",
                field
            );
        }
    }

    // --- Project export tests (Task 2) ---

    fn minimal_project_options() -> ProjectExportOptions {
        let gcode = b"G28\nG1 X10 Y10 Z0.2\n".to_vec();
        ProjectExportOptions {
            gcode_per_plate: vec![gcode],
            thumbnails_per_plate: vec![None],
            plate_metadata: vec![crate::plate_metadata::PlateMetadata {
                plate_index: 1,
                objects: vec![crate::plate_metadata::PlateObject {
                    name: "Tetra".to_string(),
                    position: [0.0, 0.0, 0.0],
                    bounding_box: [0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                    triangle_count: 4,
                }],
                plate_size: [256.0, 256.0],
                statistics: crate::plate_metadata::PlateStatistics {
                    filament_length_mm: 1000.0,
                    filament_weight_g: 3.0,
                    filament_cost: 0.10,
                    estimated_time_seconds: 600.0,
                    layer_count: 50,
                },
                filament_mapping: None,
            }],
            config_toml: "[layer]\nheight = 0.2\n".to_string(),
            process_settings: vec![("layer_height".to_string(), "0.2".to_string())],
            filament_settings: vec![("filament_type".to_string(), "PLA".to_string())],
            machine_settings: vec![("printer_model".to_string(), "X1C".to_string())],
            project_metadata: crate::project_config::ProjectMetadata {
                slicecore_version: "0.1.0".to_string(),
                created_at: "2026-03-26T00:00:00Z".to_string(),
                source_hashes: vec![],
                reproduce_command: None,
                printer_model: None,
                filament_type: None,
                filament_brand: None,
                filament_color: None,
                nozzle_diameter: None,
                profile_names: vec![],
            },
            ams_mapping: None,
        }
    }

    #[test]
    fn export_project_roundtrip_valid_3mf() {
        let mesh = tetrahedron_mesh();
        let configs = vec![ThreeMfObjectConfig::default()];
        let opts = minimal_project_options();
        let mut buf = Cursor::new(Vec::new());

        export_project_to_3mf(&[&mesh], &configs, &opts, &mut buf).unwrap();

        let data = buf.into_inner();
        // Should be re-readable by lib3mf-core
        let reimported = crate::load_mesh(&data).unwrap();
        assert_eq!(reimported.vertex_count(), mesh.vertex_count());
    }

    #[test]
    fn export_project_gcode_embedding_byte_identical() {
        let mesh = tetrahedron_mesh();
        let configs = vec![ThreeMfObjectConfig::default()];
        let gcode = b"G28\nG1 X50 Y50 Z0.3 F1200\nM104 S200\n".to_vec();
        let mut opts = minimal_project_options();
        opts.gcode_per_plate = vec![gcode.clone()];

        let mut buf = Cursor::new(Vec::new());
        export_project_to_3mf(&[&mesh], &configs, &opts, &mut buf).unwrap();

        let data = buf.into_inner();
        let cursor = Cursor::new(data.as_slice());
        let mut archiver = lib3mf_core::archive::ZipArchiver::new(cursor).unwrap();
        let embedded = archiver.read_entry("Metadata/plate_1.gcode").unwrap();
        assert_eq!(embedded, gcode, "G-code must be byte-identical");
    }

    #[test]
    fn export_project_gcode_md5_checksum() {
        let mesh = tetrahedron_mesh();
        let configs = vec![ThreeMfObjectConfig::default()];
        let gcode = b"G28\nG1 X10\n".to_vec();
        let mut opts = minimal_project_options();
        opts.gcode_per_plate = vec![gcode.clone()];

        let mut buf = Cursor::new(Vec::new());
        export_project_to_3mf(&[&mesh], &configs, &opts, &mut buf).unwrap();

        let data = buf.into_inner();
        let cursor = Cursor::new(data.as_slice());
        let mut archiver = lib3mf_core::archive::ZipArchiver::new(cursor).unwrap();
        let md5_bytes = archiver.read_entry("Metadata/plate_1.gcode.md5").unwrap();
        let md5_str = std::str::from_utf8(&md5_bytes).unwrap();

        // Compute expected MD5
        use md5::{Digest, Md5};
        let expected = format!("{:x}", Md5::digest(&gcode));
        assert_eq!(md5_str, expected);
    }

    #[test]
    fn export_project_thumbnail_present() {
        let mesh = tetrahedron_mesh();
        let configs = vec![ThreeMfObjectConfig::default()];
        let fake_png = vec![0x89, b'P', b'N', b'G', 1, 2, 3, 4];
        let mut opts = minimal_project_options();
        opts.thumbnails_per_plate = vec![Some(fake_png.clone())];

        let mut buf = Cursor::new(Vec::new());
        export_project_to_3mf(&[&mesh], &configs, &opts, &mut buf).unwrap();

        let data = buf.into_inner();
        let cursor = Cursor::new(data.as_slice());
        let mut archiver = lib3mf_core::archive::ZipArchiver::new(cursor).unwrap();
        assert!(archiver.entry_exists("Metadata/plate_1.png"));
        let embedded_png = archiver.read_entry("Metadata/plate_1.png").unwrap();
        assert_eq!(embedded_png, fake_png);
    }

    #[test]
    fn export_project_plate_metadata_json() {
        let mesh = tetrahedron_mesh();
        let configs = vec![ThreeMfObjectConfig::default()];
        let opts = minimal_project_options();

        let mut buf = Cursor::new(Vec::new());
        export_project_to_3mf(&[&mesh], &configs, &opts, &mut buf).unwrap();

        let data = buf.into_inner();
        let cursor = Cursor::new(data.as_slice());
        let mut archiver = lib3mf_core::archive::ZipArchiver::new(cursor).unwrap();
        let json_bytes = archiver.read_entry("Metadata/plate_1.json").unwrap();
        let json_str = std::str::from_utf8(&json_bytes).unwrap();
        assert!(json_str.contains("\"plate_index\""));
        assert!(json_str.contains("\"statistics\""));
    }

    #[test]
    fn export_project_process_settings_config() {
        let mesh = tetrahedron_mesh();
        let configs = vec![ThreeMfObjectConfig::default()];
        let opts = minimal_project_options();

        let mut buf = Cursor::new(Vec::new());
        export_project_to_3mf(&[&mesh], &configs, &opts, &mut buf).unwrap();

        let data = buf.into_inner();
        let cursor = Cursor::new(data.as_slice());
        let mut archiver = lib3mf_core::archive::ZipArchiver::new(cursor).unwrap();
        let config_bytes = archiver
            .read_entry("Metadata/process_settings.config")
            .unwrap();
        let config_str = std::str::from_utf8(&config_bytes).unwrap();
        assert!(config_str.contains("<config>"));
        assert!(config_str.contains("<plate>"));
        assert!(config_str.contains("<metadata"));
    }

    #[test]
    fn export_project_slicecore_config_toml() {
        let mesh = tetrahedron_mesh();
        let configs = vec![ThreeMfObjectConfig::default()];
        let opts = minimal_project_options();

        let mut buf = Cursor::new(Vec::new());
        export_project_to_3mf(&[&mesh], &configs, &opts, &mut buf).unwrap();

        let data = buf.into_inner();
        let cursor = Cursor::new(data.as_slice());
        let mut archiver = lib3mf_core::archive::ZipArchiver::new(cursor).unwrap();
        assert!(archiver.entry_exists("Metadata/slicecore_config.toml"));
        let toml_bytes = archiver
            .read_entry("Metadata/slicecore_config.toml")
            .unwrap();
        let toml_str = std::str::from_utf8(&toml_bytes).unwrap();
        assert!(toml_str.contains("height = 0.2"));
    }

    #[test]
    fn export_project_settings_config_bambu_version() {
        let mesh = tetrahedron_mesh();
        let configs = vec![ThreeMfObjectConfig::default()];
        let opts = minimal_project_options();

        let mut buf = Cursor::new(Vec::new());
        export_project_to_3mf(&[&mesh], &configs, &opts, &mut buf).unwrap();

        let data = buf.into_inner();
        let cursor = Cursor::new(data.as_slice());
        let mut archiver = lib3mf_core::archive::ZipArchiver::new(cursor).unwrap();
        let config_bytes = archiver
            .read_entry("Metadata/project_settings.config")
            .unwrap();
        let config_str = std::str::from_utf8(&config_bytes).unwrap();
        assert!(config_str.contains("BambuStudio:3mfVersion"));
    }

    #[test]
    fn export_project_multi_plate() {
        let mesh = tetrahedron_mesh();
        let configs = vec![ThreeMfObjectConfig::default()];
        let gcode1 = b"G28\nplate1\n".to_vec();
        let gcode2 = b"G28\nplate2\n".to_vec();
        let mut opts = minimal_project_options();
        opts.gcode_per_plate = vec![gcode1.clone(), gcode2.clone()];
        opts.thumbnails_per_plate = vec![None, None];
        opts.plate_metadata = vec![
            opts.plate_metadata[0].clone(),
            {
                let mut p2 = opts.plate_metadata[0].clone();
                p2.plate_index = 2;
                p2
            },
        ];

        let mut buf = Cursor::new(Vec::new());
        export_project_to_3mf(&[&mesh], &configs, &opts, &mut buf).unwrap();

        let data = buf.into_inner();
        let cursor = Cursor::new(data.as_slice());
        let mut archiver = lib3mf_core::archive::ZipArchiver::new(cursor).unwrap();
        assert!(archiver.entry_exists("Metadata/plate_1.gcode"));
        assert!(archiver.entry_exists("Metadata/plate_2.gcode"));
        assert!(archiver.entry_exists("Metadata/plate_1.json"));
        assert!(archiver.entry_exists("Metadata/plate_2.json"));

        let g1 = archiver.read_entry("Metadata/plate_1.gcode").unwrap();
        let g2 = archiver.read_entry("Metadata/plate_2.gcode").unwrap();
        assert_eq!(g1, gcode1);
        assert_eq!(g2, gcode2);
    }
}
