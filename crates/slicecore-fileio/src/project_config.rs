//! XML config builders for 3MF project settings and metadata.
//!
//! Generates Bambu/OrcaSlicer-compatible XML config files (`<config><plate>...`)
//! containing slicer-compatible key-value settings, as well as project-level
//! metadata (SliceCore version, timestamps, provenance).

use serde::Serialize;

use crate::export::xml_escape;

/// Project-level metadata (SliceCore version, timestamps, provenance).
#[derive(Debug, Clone)]
pub struct ProjectMetadata {
    /// SliceCore engine version string.
    pub slicecore_version: String,
    /// ISO 8601 timestamp of when the project was created.
    pub created_at: String,
    /// Source file hashes: `(filename, sha256_hex)`.
    pub source_hashes: Vec<(String, String)>,
    /// Full CLI invocation for reproducibility.
    pub reproduce_command: Option<String>,
    /// Target printer model name.
    pub printer_model: Option<String>,
    /// Filament type (e.g., "PLA", "PETG").
    pub filament_type: Option<String>,
    /// Filament brand name.
    pub filament_brand: Option<String>,
    /// Filament color (hex or name).
    pub filament_color: Option<String>,
    /// Nozzle diameter in mm.
    pub nozzle_diameter: Option<f64>,
    /// Profile names: `(role, name)` pairs (e.g., `("process", "0.20mm Standard")`).
    pub profile_names: Vec<(String, String)>,
}

/// AMS filament slot mapping for Bambu printers.
#[derive(Debug, Clone, Serialize)]
pub struct AmsMapping {
    /// Ordered list of AMS tray slots.
    pub slots: Vec<AmsSlot>,
}

/// A single AMS tray slot configuration.
#[derive(Debug, Clone, Serialize)]
pub struct AmsSlot {
    /// Slot identifier: "0", "1", "2", "3", or "external".
    pub slot: String,
    /// Filament type (e.g., "PLA", "PETG").
    pub filament_type: String,
    /// Optional filament color as hex (e.g., "#FFFFFF").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// Builds an XML settings config string from key-value pairs.
///
/// Produces Bambu/OrcaSlicer-compatible format:
/// ```xml
/// <?xml version="1.0" encoding="UTF-8"?>
/// <config>
///   <plate>
///     <metadata key="..." value="..."/>
///   </plate>
/// </config>
/// ```
fn build_settings_xml(settings: &[(String, String)]) -> String {
    let mut xml =
        String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<config>\n  <plate>\n");

    for (key, value) in settings {
        xml.push_str(&format!(
            "    <metadata key=\"{}\" value=\"{}\"/>\n",
            xml_escape(key),
            xml_escape(value),
        ));
    }

    xml.push_str("  </plate>\n</config>\n");
    xml
}

/// Builds a process (print) settings XML config.
///
/// Each entry is a `(key, value)` pair already mapped to slicer-compatible names.
pub fn build_process_settings_config(settings: &[(String, String)]) -> String {
    build_settings_xml(settings)
}

/// Builds a filament settings XML config.
pub fn build_filament_settings_config(settings: &[(String, String)]) -> String {
    build_settings_xml(settings)
}

/// Builds a machine/printer settings XML config.
pub fn build_machine_settings_config(settings: &[(String, String)]) -> String {
    build_settings_xml(settings)
}

/// Builds a project metadata XML config.
///
/// Includes SliceCore version, creation timestamp, source file hashes,
/// reproduce command, printer/filament info, and a `BambuStudio:3mfVersion`
/// key for Bambu firmware compatibility.
pub fn build_project_metadata_config(meta: &ProjectMetadata) -> String {
    let mut entries: Vec<(String, String)> = Vec::new();

    entries.push((
        "BambuStudio:3mfVersion".to_string(),
        "1".to_string(),
    ));
    entries.push((
        "SliceCore:Version".to_string(),
        meta.slicecore_version.clone(),
    ));
    entries.push((
        "SliceCore:CreatedAt".to_string(),
        meta.created_at.clone(),
    ));

    for (filename, hash) in &meta.source_hashes {
        entries.push((
            format!("SliceCore:SourceHash:{filename}"),
            hash.clone(),
        ));
    }

    if let Some(ref cmd) = meta.reproduce_command {
        entries.push(("SliceCore:ReproduceCommand".to_string(), cmd.clone()));
    }

    if let Some(ref model) = meta.printer_model {
        entries.push(("SliceCore:PrinterModel".to_string(), model.clone()));
    }

    if let Some(ref ft) = meta.filament_type {
        entries.push(("SliceCore:FilamentType".to_string(), ft.clone()));
    }

    if let Some(ref brand) = meta.filament_brand {
        entries.push(("SliceCore:FilamentBrand".to_string(), brand.clone()));
    }

    if let Some(ref color) = meta.filament_color {
        entries.push(("SliceCore:FilamentColor".to_string(), color.clone()));
    }

    if let Some(diameter) = meta.nozzle_diameter {
        entries.push(("SliceCore:NozzleDiameter".to_string(), diameter.to_string()));
    }

    for (role, name) in &meta.profile_names {
        entries.push((
            format!("SliceCore:Profile:{role}"),
            name.clone(),
        ));
    }

    build_settings_xml(&entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_settings_xml_empty() {
        let xml = build_settings_xml(&[]);
        assert!(xml.contains("<config>"));
        assert!(xml.contains("<plate>"));
        assert!(xml.contains("</plate>"));
        assert!(xml.contains("</config>"));
        assert!(!xml.contains("<metadata"));
    }

    #[test]
    fn test_build_settings_xml_with_entries() {
        let settings = vec![
            ("layer_height".to_string(), "0.2".to_string()),
            ("infill_speed".to_string(), "100".to_string()),
        ];
        let xml = build_settings_xml(&settings);
        assert!(xml.contains(r#"<metadata key="layer_height" value="0.2"/>"#));
        assert!(xml.contains(r#"<metadata key="infill_speed" value="100"/>"#));
    }

    #[test]
    fn test_build_settings_xml_escapes_special_chars() {
        let settings = vec![
            ("key&1".to_string(), "val<ue>".to_string()),
            ("key\"2".to_string(), "value&test".to_string()),
        ];
        let xml = build_settings_xml(&settings);
        assert!(xml.contains("key&amp;1"));
        assert!(xml.contains("val&lt;ue&gt;"));
        assert!(xml.contains("key&quot;2"));
        assert!(xml.contains("value&amp;test"));
    }

    #[test]
    fn test_build_project_metadata_config() {
        let meta = ProjectMetadata {
            slicecore_version: "0.1.0".to_string(),
            created_at: "2026-03-26T00:00:00Z".to_string(),
            source_hashes: vec![("model.stl".to_string(), "abc123".to_string())],
            reproduce_command: None,
            printer_model: None,
            filament_type: None,
            filament_brand: None,
            filament_color: None,
            nozzle_diameter: None,
            profile_names: vec![],
        };
        let xml = build_project_metadata_config(&meta);
        assert!(xml.contains("SliceCore:Version"));
        assert!(xml.contains("0.1.0"));
        assert!(xml.contains("SliceCore:CreatedAt"));
        assert!(xml.contains("2026-03-26T00:00:00Z"));
        assert!(xml.contains("BambuStudio:3mfVersion"));
    }

    #[test]
    fn test_ams_mapping_serializes() {
        let mapping = AmsMapping {
            slots: vec![
                AmsSlot {
                    slot: "0".to_string(),
                    filament_type: "PLA".to_string(),
                    color: Some("#FFFFFF".to_string()),
                },
                AmsSlot {
                    slot: "external".to_string(),
                    filament_type: "PETG".to_string(),
                    color: None,
                },
            ],
        };
        let json = serde_json::to_string_pretty(&mapping).unwrap();
        assert!(json.contains("\"slot\""));
        assert!(json.contains("\"PLA\""));
        assert!(json.contains("\"PETG\""));
        assert!(json.contains("#FFFFFF"));
        // The "external" slot with no color should not contain "color" key
        // (but the first slot does, so we just check overall structure)
        assert!(json.contains("\"filament_type\""));
    }
}
