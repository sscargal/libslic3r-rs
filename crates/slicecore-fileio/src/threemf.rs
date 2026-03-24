//! 3MF file parser with per-object settings extraction.
//!
//! Parses 3MF files (ZIP+XML containers) into [`TriangleMesh`] using the
//! [`lib3mf_core`] crate (pure Rust, WASM-compatible). 3MF is the modern
//! successor to STL, used by Bambu Studio, OrcaSlicer, and other
//! contemporary slicers.
//!
//! Multi-object 3MF files are merged into a single `TriangleMesh` with
//! correct vertex index offsets.
//!
//! # Per-Object Settings
//!
//! [`parse_with_config`] extracts per-object settings from
//! PrusaSlicer/OrcaSlicer 3MF files. Known slicer fields are mapped to
//! SliceCore field names via [`map_slicer_field`]; unmapped fields are
//! preserved in [`ThreeMfObjectConfig::unmapped`] for round-tripping.

use std::collections::HashMap;
use std::io::Cursor;

use lib3mf_core::archive::{find_model_path, ArchiveReader, ZipArchiver};
use lib3mf_core::parser::parse_model;
use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;

use crate::error::FileIOError;

/// Result of importing a 3MF file with per-object settings.
pub struct ThreeMfImportResult {
    /// Loaded meshes (one per object).
    pub meshes: Vec<TriangleMesh>,
    /// Per-object configurations extracted from 3MF metadata.
    pub object_configs: Vec<ThreeMfObjectConfig>,
    /// Import summary for user display.
    pub summary: ThreeMfImportSummary,
}

/// Per-object configuration extracted from a 3MF file.
#[derive(Default)]
pub struct ThreeMfObjectConfig {
    /// Object name from 3MF.
    pub name: Option<String>,
    /// Mapped override settings (SliceCore field names).
    pub overrides: toml::map::Map<String, toml::Value>,
    /// Unmapped settings preserved as pass-through metadata.
    pub unmapped: HashMap<String, String>,
    /// Object transform from 3MF (3x4 affine matrix, row-major).
    pub transform: Option<[f64; 12]>,
    /// Modifier meshes associated with this object.
    pub modifiers: Vec<ThreeMfModifier>,
}

/// A modifier mesh extracted from a 3MF file.
pub struct ThreeMfModifier {
    /// The modifier mesh geometry.
    pub mesh: TriangleMesh,
    /// Mapped override settings for this modifier region.
    pub overrides: toml::map::Map<String, toml::Value>,
    /// Unmapped settings preserved as pass-through metadata.
    pub unmapped: HashMap<String, String>,
}

/// Summary of a 3MF import operation.
#[derive(Debug, Clone, Default)]
pub struct ThreeMfImportSummary {
    /// Number of objects found in the 3MF file.
    pub objects_found: usize,
    /// Number of override values successfully imported.
    pub overrides_imported: usize,
    /// List of unmapped field names encountered.
    pub unmapped_fields: Vec<String>,
    /// Number of modifier meshes found.
    pub modifiers_found: usize,
}

impl std::fmt::Display for ThreeMfImportSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} objects, {} overrides imported, {} unmapped fields, {} modifiers",
            self.objects_found,
            self.overrides_imported,
            self.unmapped_fields.len(),
            self.modifiers_found,
        )
    }
}

/// Maps a PrusaSlicer/OrcaSlicer field name to a SliceCore field name and parser.
///
/// Returns `None` for unknown fields (they will be preserved as unmapped metadata).
///
/// # Supported Mappings
///
/// | Slicer Field | SliceCore Field |
/// |---|---|
/// | `fill_density` / `sparse_infill_density` | `infill_density` |
/// | `fill_pattern` / `sparse_infill_pattern` | `infill_pattern` |
/// | `perimeters` / `wall_loops` | `wall_count` |
/// | `layer_height` | `layer_height` |
/// | `perimeter_speed` | `speeds.perimeter` |
/// | `infill_speed` / `sparse_infill_speed` | `speeds.infill` |
/// | `top_solid_layers` | `top_solid_layers` |
/// | `bottom_solid_layers` | `bottom_solid_layers` |
/// | `support_material` / `enable_support` | `support_enabled` |
type FieldMapping = (&'static str, fn(&str) -> Option<toml::Value>);

fn map_slicer_field(slicer_key: &str) -> Option<FieldMapping> {
    match slicer_key {
        "fill_density" | "sparse_infill_density" => {
            Some(("infill_density", parse_percent_to_fraction))
        }
        "fill_pattern" | "sparse_infill_pattern" => Some(("infill_pattern", parse_string_value)),
        "perimeters" | "wall_loops" => Some(("wall_count", parse_int_value)),
        "layer_height" => Some(("layer_height", parse_float_value)),
        "perimeter_speed" => Some(("speeds.perimeter", parse_float_value)),
        "infill_speed" | "sparse_infill_speed" => Some(("speeds.infill", parse_float_value)),
        "top_solid_layers" => Some(("top_solid_layers", parse_int_value)),
        "bottom_solid_layers" => Some(("bottom_solid_layers", parse_int_value)),
        "support_material" | "enable_support" => Some(("support_enabled", parse_bool_value)),
        _ => None,
    }
}

/// Parses a percentage string (e.g. `"50%"` or `"0.5"`) to a fraction `0.0..=1.0`.
fn parse_percent_to_fraction(s: &str) -> Option<toml::Value> {
    let trimmed = s.trim();
    if let Some(pct) = trimmed.strip_suffix('%') {
        pct.trim()
            .parse::<f64>()
            .ok()
            .map(|v| toml::Value::Float(v / 100.0))
    } else {
        trimmed.parse::<f64>().ok().map(toml::Value::Float)
    }
}

/// Parses a string value as a TOML string.
fn parse_string_value(s: &str) -> Option<toml::Value> {
    Some(toml::Value::String(s.trim().to_string()))
}

/// Parses an integer string.
fn parse_int_value(s: &str) -> Option<toml::Value> {
    s.trim().parse::<i64>().ok().map(toml::Value::Integer)
}

/// Parses a float string.
fn parse_float_value(s: &str) -> Option<toml::Value> {
    s.trim().parse::<f64>().ok().map(toml::Value::Float)
}

/// Parses a boolean string (`"1"`, `"true"`, `"yes"` are truthy).
fn parse_bool_value(s: &str) -> Option<toml::Value> {
    let trimmed = s.trim().to_lowercase();
    match trimmed.as_str() {
        "1" | "true" | "yes" => Some(toml::Value::Boolean(true)),
        "0" | "false" | "no" => Some(toml::Value::Boolean(false)),
        _ => None,
    }
}

/// Applies field mapping to a set of raw slicer key-value pairs.
///
/// Returns mapped overrides and unmapped fields separately.
fn map_fields(
    raw: &HashMap<String, String>,
) -> (toml::map::Map<String, toml::Value>, HashMap<String, String>) {
    let mut overrides = toml::map::Map::new();
    let mut unmapped = HashMap::new();

    for (key, value) in raw {
        if let Some((slicecore_key, parser)) = map_slicer_field(key) {
            if let Some(parsed) = parser(value) {
                overrides.insert(slicecore_key.to_string(), parsed);
            } else {
                // Parser failed -- treat as unmapped.
                unmapped.insert(key.clone(), value.clone());
            }
        } else {
            unmapped.insert(key.clone(), value.clone());
        }
    }

    (overrides, unmapped)
}

/// Attempts to read slicer config from PrusaSlicer/OrcaSlicer 3MF archives.
///
/// These slicers store per-object settings in `Metadata/model_settings.config`
/// as an INI-like format. We do a best-effort parse of the key=value pairs.
fn extract_slicer_configs(
    archiver: &mut ZipArchiver<Cursor<&[u8]>>,
) -> Vec<HashMap<String, String>> {
    let config_path = "Metadata/model_settings.config";
    if !archiver.entry_exists(config_path) {
        return Vec::new();
    }

    let data = match archiver.read_entry(config_path) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let content = match std::str::from_utf8(&data) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    parse_orca_model_settings(content)
}

/// Parses OrcaSlicer/Bambu Studio `model_settings.config` XML.
///
/// The format is roughly:
/// ```xml
/// <config>
///   <object id="1">
///     <metadata key="name" value="Part A"/>
///     <part id="0">
///       <metadata key="fill_density" value="50%"/>
///     </part>
///   </object>
/// </config>
/// ```
///
/// We do a lightweight line-based parse since these configs are simple.
fn parse_orca_model_settings(content: &str) -> Vec<HashMap<String, String>> {
    let mut objects: Vec<HashMap<String, String>> = Vec::new();
    let mut current: Option<HashMap<String, String>> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("<object ") {
            // Start a new object section.
            if let Some(obj) = current.take() {
                objects.push(obj);
            }
            current = Some(HashMap::new());
        } else if trimmed == "</object>" {
            if let Some(obj) = current.take() {
                objects.push(obj);
            }
        } else if trimmed.starts_with("<metadata ") {
            // Extract key="..." value="..." from the metadata element.
            if let (Some(key), Some(value)) = (
                extract_xml_attr(trimmed, "key"),
                extract_xml_attr(trimmed, "value"),
            ) {
                if let Some(ref mut obj) = current {
                    obj.insert(key.to_string(), value.to_string());
                }
            }
        }
    }

    // Flush the last object if the file didn't close properly.
    if let Some(obj) = current {
        objects.push(obj);
    }

    objects
}

/// Extracts an XML attribute value from a simple element string.
///
/// For `<metadata key="name" value="Part A"/>`, calling with `attr_name="key"`
/// returns `Some("name")`.
fn extract_xml_attr<'a>(element: &'a str, attr_name: &str) -> Option<&'a str> {
    let pattern = format!("{attr_name}=\"");
    let start = element.find(&pattern)? + pattern.len();
    let rest = &element[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

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

    let mut archiver =
        ZipArchiver::new(cursor).map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;
    let model_path =
        find_model_path(&mut archiver).map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;
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

/// Loads a 3MF file with per-object settings extraction.
///
/// Settings are automatically mapped from PrusaSlicer/OrcaSlicer format to
/// SliceCore field names. Unmapped fields are preserved in
/// [`ThreeMfObjectConfig::unmapped`] for round-tripping.
///
/// The existing [`parse`] function is kept for backward compatibility (simple
/// mesh-only loading).
///
/// # Errors
///
/// - [`FileIOError::ThreeMfError`] if lib3mf-core cannot parse the data.
/// - [`FileIOError::EmptyModel`] if no mesh geometry is found.
/// - [`FileIOError::MeshError`] if mesh construction fails.
pub fn parse_with_config(data: &[u8]) -> Result<ThreeMfImportResult, FileIOError> {
    let cursor = Cursor::new(data);
    let mut archiver =
        ZipArchiver::new(cursor).map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;

    // Extract slicer per-object configs from the archive before parsing model XML.
    let slicer_configs = extract_slicer_configs(&mut archiver);

    let model_path =
        find_model_path(&mut archiver).map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;
    let model_data = archiver
        .read_entry(&model_path)
        .map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;
    let model = parse_model(Cursor::new(model_data))
        .map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;

    let mut meshes: Vec<TriangleMesh> = Vec::new();
    let mut object_configs: Vec<ThreeMfObjectConfig> = Vec::new();
    let mut summary = ThreeMfImportSummary::default();

    let objects: Vec<_> = model.resources.iter_objects().collect();
    summary.objects_found = objects.len();

    for (idx, object) in objects.iter().enumerate() {
        let name = object.name.clone();

        // Extract mesh geometry.
        if let lib3mf_core::model::Geometry::Mesh(mesh) = &object.geometry {
            let mut vertices = Vec::with_capacity(mesh.vertices.len());
            let mut indices = Vec::with_capacity(mesh.triangles.len());

            for v in &mesh.vertices {
                vertices.push(Point3::new(v.x as f64, v.y as f64, v.z as f64));
            }
            for tri in &mesh.triangles {
                indices.push([tri.v1, tri.v2, tri.v3]);
            }

            if vertices.is_empty() || indices.is_empty() {
                continue;
            }

            let tri_mesh = TriangleMesh::new(vertices, indices)?;
            meshes.push(tri_mesh);
        } else {
            continue;
        }

        // Map slicer config for this object (by index).
        let raw_config = slicer_configs.get(idx).cloned().unwrap_or_default();
        let (overrides, unmapped) = map_fields(&raw_config);

        summary.overrides_imported += overrides.len();
        for key in unmapped.keys() {
            if !summary.unmapped_fields.contains(key) {
                summary.unmapped_fields.push(key.clone());
            }
        }

        object_configs.push(ThreeMfObjectConfig {
            name,
            overrides,
            unmapped,
            transform: None,
            modifiers: Vec::new(),
        });
    }

    if meshes.is_empty() {
        return Err(FileIOError::EmptyModel);
    }

    // Sort unmapped fields for deterministic output.
    summary.unmapped_fields.sort();

    Ok(ThreeMfImportResult {
        meshes,
        object_configs,
        summary,
    })
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
        model.write(&mut buffer).expect("failed to write test 3MF");
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
        model.write(&mut buffer).expect("failed to write test 3MF");
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

    // --- Per-object settings tests ---

    #[test]
    fn parse_with_config_no_overrides() {
        let data = tetrahedron_3mf();
        let result = parse_with_config(&data).unwrap();

        assert_eq!(result.meshes.len(), 1);
        assert_eq!(result.meshes[0].vertex_count(), 4);
        assert_eq!(result.meshes[0].triangle_count(), 4);
        assert_eq!(result.object_configs.len(), 1);
        assert!(result.object_configs[0].overrides.is_empty());
        assert!(result.object_configs[0].unmapped.is_empty());
        assert_eq!(result.summary.objects_found, 1);
        assert_eq!(result.summary.overrides_imported, 0);
    }

    #[test]
    fn field_mapping_known_fields() {
        let mut raw = HashMap::new();
        raw.insert("fill_density".to_string(), "50%".to_string());
        raw.insert("perimeters".to_string(), "3".to_string());
        raw.insert("layer_height".to_string(), "0.2".to_string());
        raw.insert("support_material".to_string(), "1".to_string());

        let (overrides, unmapped) = map_fields(&raw);

        assert_eq!(
            overrides.get("infill_density").unwrap().as_float().unwrap(),
            0.5
        );
        assert_eq!(
            overrides.get("wall_count").unwrap().as_integer().unwrap(),
            3
        );
        assert_eq!(
            overrides.get("layer_height").unwrap().as_float().unwrap(),
            0.2
        );
        assert_eq!(
            overrides.get("support_enabled").unwrap().as_bool().unwrap(),
            true
        );
        assert!(unmapped.is_empty());
    }

    #[test]
    fn field_mapping_orca_aliases() {
        let mut raw = HashMap::new();
        raw.insert("sparse_infill_density".to_string(), "30%".to_string());
        raw.insert("wall_loops".to_string(), "2".to_string());
        raw.insert("sparse_infill_speed".to_string(), "100".to_string());
        raw.insert("enable_support".to_string(), "false".to_string());

        let (overrides, unmapped) = map_fields(&raw);

        assert!(
            (overrides.get("infill_density").unwrap().as_float().unwrap() - 0.3).abs()
                < f64::EPSILON
        );
        assert_eq!(
            overrides.get("wall_count").unwrap().as_integer().unwrap(),
            2
        );
        assert_eq!(
            overrides.get("speeds.infill").unwrap().as_float().unwrap(),
            100.0
        );
        assert_eq!(
            overrides.get("support_enabled").unwrap().as_bool().unwrap(),
            false
        );
        assert!(unmapped.is_empty());
    }

    #[test]
    fn unmapped_fields_preserved() {
        let mut raw = HashMap::new();
        raw.insert("custom_setting_x".to_string(), "42".to_string());
        raw.insert("fill_density".to_string(), "20%".to_string());
        raw.insert("vendor_specific".to_string(), "foo".to_string());

        let (overrides, unmapped) = map_fields(&raw);

        assert_eq!(overrides.len(), 1); // only fill_density mapped
        assert_eq!(unmapped.len(), 2);
        assert_eq!(unmapped.get("custom_setting_x").unwrap(), "42");
        assert_eq!(unmapped.get("vendor_specific").unwrap(), "foo");
    }

    #[test]
    fn import_summary_counts_correct() {
        let data = tetrahedron_3mf();
        let result = parse_with_config(&data).unwrap();

        assert_eq!(result.summary.objects_found, 1);
        assert_eq!(result.summary.overrides_imported, 0);
        assert!(result.summary.unmapped_fields.is_empty());
        assert_eq!(result.summary.modifiers_found, 0);
    }

    #[test]
    fn parse_percent_to_fraction_with_suffix() {
        assert_eq!(
            parse_percent_to_fraction("50%")
                .unwrap()
                .as_float()
                .unwrap(),
            0.5
        );
        assert_eq!(
            parse_percent_to_fraction("100%")
                .unwrap()
                .as_float()
                .unwrap(),
            1.0
        );
    }

    #[test]
    fn parse_percent_to_fraction_without_suffix() {
        assert_eq!(
            parse_percent_to_fraction("0.3")
                .unwrap()
                .as_float()
                .unwrap(),
            0.3
        );
    }

    #[test]
    fn parse_bool_value_variants() {
        assert_eq!(parse_bool_value("1").unwrap().as_bool().unwrap(), true);
        assert_eq!(parse_bool_value("true").unwrap().as_bool().unwrap(), true);
        assert_eq!(parse_bool_value("yes").unwrap().as_bool().unwrap(), true);
        assert_eq!(parse_bool_value("0").unwrap().as_bool().unwrap(), false);
        assert_eq!(parse_bool_value("false").unwrap().as_bool().unwrap(), false);
        assert_eq!(parse_bool_value("no").unwrap().as_bool().unwrap(), false);
        assert!(parse_bool_value("maybe").is_none());
    }

    #[test]
    fn parse_orca_model_settings_xml() {
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<config>
  <object id="1">
    <metadata key="name" value="Part A"/>
    <metadata key="fill_density" value="50%"/>
    <metadata key="perimeters" value="3"/>
  </object>
  <object id="2">
    <metadata key="name" value="Part B"/>
    <metadata key="layer_height" value="0.1"/>
  </object>
</config>"#;

        let objects = parse_orca_model_settings(content);
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0].get("name").unwrap(), "Part A");
        assert_eq!(objects[0].get("fill_density").unwrap(), "50%");
        assert_eq!(objects[0].get("perimeters").unwrap(), "3");
        assert_eq!(objects[1].get("name").unwrap(), "Part B");
        assert_eq!(objects[1].get("layer_height").unwrap(), "0.1");
    }

    #[test]
    fn extract_xml_attr_basic() {
        let element = r#"<metadata key="name" value="Part A"/>"#;
        assert_eq!(extract_xml_attr(element, "key"), Some("name"));
        assert_eq!(extract_xml_attr(element, "value"), Some("Part A"));
        assert_eq!(extract_xml_attr(element, "missing"), None);
    }
}
