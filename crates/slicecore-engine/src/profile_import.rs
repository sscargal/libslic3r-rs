//! Profile import module with content-based format detection and field mapping.
//!
//! This module enables loading printer profiles from OrcaSlicer and BambuStudio
//! JSON format files, in addition to the existing TOML support. The field mapping
//! translates the upstream JSON schema (string values, array wrapping, nil sentinels,
//! percentage strings) into our [`PrintConfig`] struct.
//!
//! # Supported Formats
//!
//! - **Native JSON**: Direct deserialization with field names matching [`PrintConfig`]
//! - **OrcaSlicer/BambuStudio JSON**: Upstream profile format with automatic field mapping
//! - **TOML**: Existing format, detected by content sniffing
//!
//! # Format Detection
//!
//! [`detect_config_format`] inspects the first non-whitespace byte of the file content:
//! - `{` indicates JSON format
//! - Everything else is treated as TOML
//! - UTF-8 BOM prefix is transparently skipped
//!
//! # Usage
//!
//! ```ignore
//! use slicecore_engine::profile_import::{detect_config_format, ConfigFormat, import_upstream_profile};
//! use slicecore_engine::config::PrintConfig;
//!
//! // Auto-detect and load any format
//! let config = PrintConfig::from_file(path)?;
//!
//! // Import upstream profile with field reporting
//! let result = import_upstream_profile(&json_value)?;
//! println!("Mapped: {:?}", result.mapped_fields);
//! println!("Unmapped: {:?}", result.unmapped_fields);
//! ```

use slicecore_gcode_io::GcodeDialect;

use crate::config::PrintConfig;
use crate::error::EngineError;
use crate::infill::InfillPattern;
use crate::seam::SeamPosition;

/// Detected configuration file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    /// TOML configuration format.
    Toml,
    /// JSON configuration format (native or upstream slicer).
    Json,
}

/// Detect config format from file content.
///
/// JSON files start with `{` (after optional whitespace and UTF-8 BOM).
/// Everything else is treated as TOML. Empty content defaults to TOML
/// (empty TOML produces all-default config).
pub fn detect_config_format(data: &[u8]) -> ConfigFormat {
    // Skip UTF-8 BOM if present (0xEF, 0xBB, 0xBF).
    let data = if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &data[3..]
    } else {
        data
    };

    // Find first non-whitespace byte.
    for &byte in data {
        match byte {
            b' ' | b'\t' | b'\n' | b'\r' => continue,
            b'{' => return ConfigFormat::Json,
            _ => return ConfigFormat::Toml,
        }
    }

    // Empty file defaults to TOML (empty TOML = all defaults).
    ConfigFormat::Toml
}

/// Result of importing an upstream slicer profile.
///
/// Contains the mapped [`PrintConfig`], lists of mapped and unmapped field names,
/// and source profile metadata.
#[derive(Debug, Clone)]
pub struct ImportResult {
    /// The mapped PrintConfig (unmapped fields use defaults).
    pub config: PrintConfig,
    /// Fields from the source that were successfully mapped to PrintConfig.
    pub mapped_fields: Vec<String>,
    /// Fields from the source that have no PrintConfig equivalent.
    pub unmapped_fields: Vec<String>,
    /// Source profile metadata (name, type, inherits).
    pub metadata: ProfileMetadata,
}

/// Metadata extracted from an upstream slicer profile.
#[derive(Debug, Clone, Default)]
pub struct ProfileMetadata {
    /// Profile name (e.g., "Generic PLA", "0.20mm Standard").
    pub name: Option<String>,
    /// Profile type (e.g., "filament", "machine", "process").
    pub profile_type: Option<String>,
    /// Parent profile name for inheritance resolution.
    pub inherits: Option<String>,
}

/// Import an upstream slicer profile (OrcaSlicer/BambuStudio JSON) into PrintConfig.
///
/// Iterates over all fields in the JSON object, extracts string values (handling
/// array wrapping and nil sentinels), and maps known fields to [`PrintConfig`].
/// Unknown fields and metadata fields are tracked in [`ImportResult`].
///
/// # Errors
///
/// Returns `EngineError::ConfigError` if the JSON value is not an object.
pub fn import_upstream_profile(value: &serde_json::Value) -> Result<ImportResult, EngineError> {
    let obj = value
        .as_object()
        .ok_or_else(|| EngineError::ConfigError("JSON profile must be an object".into()))?;

    let mut config = PrintConfig::default();
    let mut mapped_fields = Vec::new();
    let mut unmapped_fields = Vec::new();

    // Extract metadata.
    let metadata = ProfileMetadata {
        name: obj
            .get("name")
            .and_then(|v| v.as_str())
            .map(String::from),
        profile_type: obj
            .get("type")
            .and_then(|v| v.as_str())
            .map(String::from),
        inherits: obj
            .get("inherits")
            .and_then(|v| v.as_str())
            .map(String::from),
    };

    // Metadata fields to skip during field mapping.
    const METADATA_KEYS: &[&str] = &[
        "type",
        "name",
        "inherits",
        "from",
        "setting_id",
        "instantiation",
        "compatible_printers",
        "compatible_printers_condition",
        "filament_id",
        "description",
        "version",
        "compatible_prints",
        "compatible_prints_condition",
    ];

    for (key, val) in obj {
        // Skip metadata fields.
        if METADATA_KEYS.contains(&key.as_str()) {
            continue;
        }

        // Extract string value (handles scalar, array-wrapped, and nil sentinel).
        let string_val = match extract_string_value(val) {
            Some(s) => s,
            None => continue, // nil or unextractable -- skip silently
        };

        // Apply field mapping.
        if apply_field_mapping(&mut config, key, &string_val) {
            mapped_fields.push(key.clone());
        } else {
            unmapped_fields.push(key.clone());
        }
    }

    Ok(ImportResult {
        config,
        mapped_fields,
        unmapped_fields,
        metadata,
    })
}

// ---------------------------------------------------------------------------
// Value extraction helpers (private)
// ---------------------------------------------------------------------------

/// Extract a string value from a JSON value.
///
/// Handles:
/// - Plain strings: `"0.2"` -> `Some("0.2")`
/// - Array-wrapped: `["200"]` -> `Some("200")` (first element for primary extruder)
/// - Number values: `0.2` -> `Some("0.2")`
/// - Nil sentinel: `"nil"` or `["nil"]` -> `None`
fn extract_string_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) if s == "nil" => None,
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Array(arr) if !arr.is_empty() => match &arr[0] {
            serde_json::Value::String(s) if s == "nil" => None,
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            _ => None,
        },
        _ => None,
    }
}

/// Extract an f64 value from a JSON value.
#[allow(dead_code)]
fn extract_f64(value: &serde_json::Value) -> Option<f64> {
    extract_string_value(value)?.parse::<f64>().ok()
}

/// Extract a u32 value from a JSON value.
#[allow(dead_code)]
fn extract_u32(value: &serde_json::Value) -> Option<u32> {
    extract_string_value(value)?
        .parse::<f64>()
        .ok()
        .map(|v| v as u32)
}

/// Extract a boolean from a string value ("1"/"true" -> true, "0"/"false" -> false).
#[allow(dead_code)]
fn extract_bool_from_string(value: &serde_json::Value) -> Option<bool> {
    let s = extract_string_value(value)?;
    match s.as_str() {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}

/// Extract a percentage value, stripping `%` suffix and dividing by 100.
///
/// `"15%"` -> `Some(0.15)`, `"100%"` -> `Some(1.0)`, `"20"` -> `Some(0.20)`
#[allow(dead_code)]
fn extract_percentage(value: &serde_json::Value) -> Option<f64> {
    let s = extract_string_value(value)?;
    let cleaned = s.trim_end_matches('%');
    cleaned.parse::<f64>().ok().map(|v| v / 100.0)
}

// ---------------------------------------------------------------------------
// Field mapping
// ---------------------------------------------------------------------------

/// Apply a single field mapping from an upstream JSON key/value to PrintConfig.
///
/// The `value` parameter is the already-extracted plain string (scalar or
/// array-unwrapped). Returns `true` if the field was successfully mapped.
fn apply_field_mapping(config: &mut PrintConfig, key: &str, value: &str) -> bool {
    match key {
        // --- Process fields ---
        "layer_height" => parse_and_set_f64(value, &mut config.layer_height),
        "initial_layer_print_height" => parse_and_set_f64(value, &mut config.first_layer_height),
        "wall_loops" => parse_and_set_u32(value, &mut config.wall_count),
        "sparse_infill_density" => {
            let cleaned = value.trim_end_matches('%');
            if let Ok(pct) = cleaned.parse::<f64>() {
                config.infill_density = pct / 100.0;
                true
            } else {
                false
            }
        }
        "sparse_infill_pattern" => {
            if let Some(pattern) = map_infill_pattern(value) {
                config.infill_pattern = pattern;
                true
            } else {
                false
            }
        }
        "top_shell_layers" => parse_and_set_u32(value, &mut config.top_solid_layers),
        "bottom_shell_layers" => parse_and_set_u32(value, &mut config.bottom_solid_layers),
        "outer_wall_speed" => parse_and_set_f64(value, &mut config.perimeter_speed),
        "sparse_infill_speed" => parse_and_set_f64(value, &mut config.infill_speed),
        "travel_speed" => parse_and_set_f64(value, &mut config.travel_speed),
        "initial_layer_speed" => parse_and_set_f64(value, &mut config.first_layer_speed),
        "skirt_loops" => parse_and_set_u32(value, &mut config.skirt_loops),
        "skirt_distance" => parse_and_set_f64(value, &mut config.skirt_distance),
        "brim_width" => parse_and_set_f64(value, &mut config.brim_width),
        "default_acceleration" => parse_and_set_f64(value, &mut config.print_acceleration),
        "travel_acceleration" => parse_and_set_f64(value, &mut config.travel_acceleration),
        "enable_arc_fitting" => {
            config.arc_fitting_enabled = value == "1" || value == "true";
            true
        }
        "adaptive_layer_height" => {
            config.adaptive_layer_height = value == "1" || value == "true";
            true
        }
        "ironing_type" => {
            config.ironing.enabled = !value.is_empty() && value != "no ironing";
            true
        }
        "ironing_flow" => {
            let cleaned = value.trim_end_matches('%');
            if let Ok(pct) = cleaned.parse::<f64>() {
                config.ironing.flow_rate = pct / 100.0;
                true
            } else {
                false
            }
        }
        "ironing_speed" => {
            if let Ok(v) = value.parse::<f64>() {
                config.ironing.speed = v;
                true
            } else {
                false
            }
        }
        "ironing_spacing" => {
            if let Ok(v) = value.parse::<f64>() {
                config.ironing.spacing = v;
                true
            } else {
                false
            }
        }
        "wall_generator" => {
            config.arachne_enabled = value == "arachne";
            true
        }
        "seam_position" => {
            if let Some(pos) = map_seam_position(value) {
                config.seam_position = pos;
                true
            } else {
                false
            }
        }

        // --- Filament fields ---
        "nozzle_temperature" => parse_and_set_f64(value, &mut config.nozzle_temp),
        "nozzle_temperature_initial_layer" => {
            parse_and_set_f64(value, &mut config.first_layer_nozzle_temp)
        }
        "hot_plate_temp" => parse_and_set_f64(value, &mut config.bed_temp),
        "hot_plate_temp_initial_layer" => {
            parse_and_set_f64(value, &mut config.first_layer_bed_temp)
        }
        "filament_density" => parse_and_set_f64(value, &mut config.filament_density),
        "filament_diameter" => parse_and_set_f64(value, &mut config.filament_diameter),
        "filament_cost" => parse_and_set_f64(value, &mut config.filament_cost_per_kg),
        "filament_flow_ratio" => parse_and_set_f64(value, &mut config.extrusion_multiplier),
        "close_fan_the_first_x_layers" => {
            parse_and_set_u32(value, &mut config.disable_fan_first_layers)
        }
        "fan_cooling_layer_time" => parse_and_set_f64(value, &mut config.fan_below_layer_time),

        // --- Machine fields ---
        "nozzle_diameter" => parse_and_set_f64(value, &mut config.nozzle_diameter),
        "retraction_length" => parse_and_set_f64(value, &mut config.retract_length),
        "retraction_speed" => parse_and_set_f64(value, &mut config.retract_speed),
        "z_hop" => parse_and_set_f64(value, &mut config.retract_z_hop),
        "retraction_minimum_travel" => {
            parse_and_set_f64(value, &mut config.min_travel_for_retract)
        }
        "gcode_flavor" => {
            if let Some(dialect) = map_gcode_dialect(value) {
                config.gcode_dialect = dialect;
                true
            } else {
                false
            }
        }
        "machine_max_jerk_x" => parse_and_set_f64(value, &mut config.jerk_x),
        "machine_max_jerk_y" => parse_and_set_f64(value, &mut config.jerk_y),
        "machine_max_jerk_z" => parse_and_set_f64(value, &mut config.jerk_z),

        // Unknown field.
        _ => false,
    }
}

/// Parse a string as f64 and set the target field. Returns true on success.
fn parse_and_set_f64(value: &str, target: &mut f64) -> bool {
    if let Ok(v) = value.parse::<f64>() {
        *target = v;
        true
    } else {
        false
    }
}

/// Parse a string as u32 and set the target field. Returns true on success.
fn parse_and_set_u32(value: &str, target: &mut u32) -> bool {
    // Parse as f64 first to handle values like "2.0".
    if let Ok(v) = value.parse::<f64>() {
        *target = v as u32;
        true
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// Enum mapping helpers (private)
// ---------------------------------------------------------------------------

/// Map an OrcaSlicer infill pattern name to our InfillPattern enum.
fn map_infill_pattern(value: &str) -> Option<InfillPattern> {
    match value.to_lowercase().as_str() {
        "grid" => Some(InfillPattern::Grid),
        "honeycomb" => Some(InfillPattern::Honeycomb),
        "gyroid" => Some(InfillPattern::Gyroid),
        "cubic" => Some(InfillPattern::Cubic),
        "adaptivecubic" => Some(InfillPattern::AdaptiveCubic),
        "lightning" => Some(InfillPattern::Lightning),
        "monotonic" => Some(InfillPattern::Monotonic),
        "zig-zag" | "rectilinear" | "line" => Some(InfillPattern::Rectilinear),
        _ => None,
    }
}

/// Map an OrcaSlicer seam position name to our SeamPosition enum.
fn map_seam_position(value: &str) -> Option<SeamPosition> {
    match value.to_lowercase().as_str() {
        "aligned" => Some(SeamPosition::Aligned),
        "random" => Some(SeamPosition::Random),
        "rear" => Some(SeamPosition::Rear),
        "nearest" => Some(SeamPosition::NearestCorner),
        _ => None,
    }
}

/// Map an OrcaSlicer gcode_flavor name to our GcodeDialect enum.
fn map_gcode_dialect(value: &str) -> Option<GcodeDialect> {
    match value.to_lowercase().as_str() {
        "marlin" => Some(GcodeDialect::Marlin),
        "klipper" => Some(GcodeDialect::Klipper),
        "reprapfirmware" | "reprap" => Some(GcodeDialect::RepRapFirmware),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_detect_format_json() {
        assert_eq!(detect_config_format(b"{}"), ConfigFormat::Json);
        assert_eq!(
            detect_config_format(b"  {\"type\": \"filament\"}"),
            ConfigFormat::Json
        );
        // UTF-8 BOM prefix.
        assert_eq!(
            detect_config_format(b"\xEF\xBB\xBF{\"key\": 1}"),
            ConfigFormat::Json
        );
        // Whitespace before opening brace.
        assert_eq!(
            detect_config_format(b"\n\t  {\"a\":1}"),
            ConfigFormat::Json
        );
    }

    #[test]
    fn test_detect_format_toml() {
        assert_eq!(
            detect_config_format(b"layer_height = 0.2"),
            ConfigFormat::Toml
        );
        assert_eq!(detect_config_format(b"# comment"), ConfigFormat::Toml);
        assert_eq!(detect_config_format(b""), ConfigFormat::Toml);
        // TOML with leading whitespace (but not a brace).
        assert_eq!(
            detect_config_format(b"  layer_height = 0.2"),
            ConfigFormat::Toml
        );
    }

    #[test]
    fn test_extract_string_scalar() {
        let val = json!("0.2");
        assert_eq!(extract_string_value(&val), Some("0.2".to_string()));
    }

    #[test]
    fn test_extract_string_array() {
        let val = json!(["200"]);
        assert_eq!(extract_string_value(&val), Some("200".to_string()));

        // Multi-element array: takes first element.
        let val = json!(["200", "210"]);
        assert_eq!(extract_string_value(&val), Some("200".to_string()));
    }

    #[test]
    fn test_extract_string_nil() {
        let val = json!("nil");
        assert_eq!(extract_string_value(&val), None);

        let val = json!(["nil"]);
        assert_eq!(extract_string_value(&val), None);
    }

    #[test]
    fn test_extract_percentage() {
        let val = json!("15%");
        assert!((extract_percentage(&val).unwrap() - 0.15).abs() < 1e-9);

        let val = json!("100%");
        assert!((extract_percentage(&val).unwrap() - 1.0).abs() < 1e-9);

        let val = json!("20");
        assert!((extract_percentage(&val).unwrap() - 0.20).abs() < 1e-9);
    }

    #[test]
    fn test_extract_f64() {
        let val = json!("0.2");
        assert!((extract_f64(&val).unwrap() - 0.2).abs() < 1e-9);

        let val = json!(["200"]);
        assert!((extract_f64(&val).unwrap() - 200.0).abs() < 1e-9);
    }

    #[test]
    fn test_extract_u32() {
        let val = json!("3");
        assert_eq!(extract_u32(&val), Some(3));

        let val = json!(["2"]);
        assert_eq!(extract_u32(&val), Some(2));
    }

    #[test]
    fn test_extract_bool_from_string() {
        let val = json!("1");
        assert_eq!(extract_bool_from_string(&val), Some(true));

        let val = json!("0");
        assert_eq!(extract_bool_from_string(&val), Some(false));

        let val = json!("true");
        assert_eq!(extract_bool_from_string(&val), Some(true));

        let val = json!("false");
        assert_eq!(extract_bool_from_string(&val), Some(false));
    }

    #[test]
    fn test_import_process_profile() {
        let json_val = json!({
            "type": "process",
            "name": "0.20mm Standard",
            "layer_height": "0.2",
            "wall_loops": "3",
            "sparse_infill_density": "15%",
            "outer_wall_speed": "200",
            "travel_speed": "500",
            "seam_position": "aligned"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        assert!((config.layer_height - 0.2).abs() < 1e-9);
        assert_eq!(config.wall_count, 3);
        assert!((config.infill_density - 0.15).abs() < 1e-9);
        assert!((config.perimeter_speed - 200.0).abs() < 1e-9);
        assert!((config.travel_speed - 500.0).abs() < 1e-9);
        assert_eq!(config.seam_position, SeamPosition::Aligned);

        // Verify mapped fields are tracked.
        assert!(result.mapped_fields.contains(&"layer_height".to_string()));
        assert!(result.mapped_fields.contains(&"wall_loops".to_string()));
        assert!(result
            .mapped_fields
            .contains(&"sparse_infill_density".to_string()));

        // Verify metadata is extracted.
        assert_eq!(result.metadata.profile_type.as_deref(), Some("process"));
        assert_eq!(result.metadata.name.as_deref(), Some("0.20mm Standard"));
    }

    #[test]
    fn test_import_filament_profile() {
        let json_val = json!({
            "type": "filament",
            "name": "Generic PLA",
            "nozzle_temperature": ["220"],
            "hot_plate_temp": ["55"],
            "filament_density": ["1.24"],
            "filament_flow_ratio": ["0.98"]
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        assert!((config.nozzle_temp - 220.0).abs() < 1e-9);
        assert!((config.bed_temp - 55.0).abs() < 1e-9);
        assert!((config.filament_density - 1.24).abs() < 1e-9);
        assert!((config.extrusion_multiplier - 0.98).abs() < 1e-9);
    }

    #[test]
    fn test_import_machine_profile() {
        let json_val = json!({
            "type": "machine",
            "name": "Generic Printer",
            "nozzle_diameter": ["0.4"],
            "retraction_length": ["0.8"],
            "gcode_flavor": "klipper"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        assert!((config.nozzle_diameter - 0.4).abs() < 1e-9);
        assert!((config.retract_length - 0.8).abs() < 1e-9);
        assert_eq!(config.gcode_dialect, GcodeDialect::Klipper);
    }

    #[test]
    fn test_native_json_format() {
        // Native JSON format with PrintConfig-matching field names and numeric values.
        let json_str = r#"{
            "layer_height": 0.15,
            "nozzle_diameter": 0.6,
            "wall_count": 4,
            "infill_density": 0.3
        }"#;

        let config = PrintConfig::from_json(json_str).unwrap();
        assert!((config.layer_height - 0.15).abs() < 1e-9);
        assert!((config.nozzle_diameter - 0.6).abs() < 1e-9);
        assert_eq!(config.wall_count, 4);
        assert!((config.infill_density - 0.3).abs() < 1e-9);
    }

    #[test]
    fn test_from_file_toml() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("slicecore_test_toml");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_config.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "layer_height = 0.1\nwall_count = 4").unwrap();
        drop(f);

        let config = PrintConfig::from_file(&path).unwrap();
        assert!((config.layer_height - 0.1).abs() < 1e-9);
        assert_eq!(config.wall_count, 4);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_from_file_json() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("slicecore_test_json");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_config.json");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(
            f,
            r#"{{
            "type": "process",
            "name": "Test",
            "layer_height": "0.3",
            "wall_loops": "5"
        }}"#
        )
        .unwrap();
        drop(f);

        let config = PrintConfig::from_file(&path).unwrap();
        assert!((config.layer_height - 0.3).abs() < 1e-9);
        assert_eq!(config.wall_count, 5);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_unmapped_fields_reported() {
        let json_val = json!({
            "type": "process",
            "name": "Test",
            "layer_height": "0.2",
            "some_unknown_field": "value",
            "another_unknown": "42"
        });

        let result = import_upstream_profile(&json_val).unwrap();

        assert!(result.mapped_fields.contains(&"layer_height".to_string()));
        assert!(result
            .unmapped_fields
            .contains(&"some_unknown_field".to_string()));
        assert!(result
            .unmapped_fields
            .contains(&"another_unknown".to_string()));
    }

    #[test]
    fn test_enum_mapping_infill() {
        assert_eq!(map_infill_pattern("grid"), Some(InfillPattern::Grid));
        assert_eq!(
            map_infill_pattern("honeycomb"),
            Some(InfillPattern::Honeycomb)
        );
        assert_eq!(map_infill_pattern("gyroid"), Some(InfillPattern::Gyroid));
        assert_eq!(map_infill_pattern("cubic"), Some(InfillPattern::Cubic));
        assert_eq!(
            map_infill_pattern("adaptivecubic"),
            Some(InfillPattern::AdaptiveCubic)
        );
        assert_eq!(
            map_infill_pattern("lightning"),
            Some(InfillPattern::Lightning)
        );
        assert_eq!(
            map_infill_pattern("monotonic"),
            Some(InfillPattern::Monotonic)
        );
        assert_eq!(
            map_infill_pattern("zig-zag"),
            Some(InfillPattern::Rectilinear)
        );
        assert_eq!(
            map_infill_pattern("rectilinear"),
            Some(InfillPattern::Rectilinear)
        );
        assert_eq!(
            map_infill_pattern("line"),
            Some(InfillPattern::Rectilinear)
        );
        assert_eq!(map_infill_pattern("unknown_pattern"), None);
    }

    #[test]
    fn test_enum_mapping_seam() {
        assert_eq!(map_seam_position("aligned"), Some(SeamPosition::Aligned));
        assert_eq!(map_seam_position("random"), Some(SeamPosition::Random));
        assert_eq!(map_seam_position("rear"), Some(SeamPosition::Rear));
        assert_eq!(
            map_seam_position("nearest"),
            Some(SeamPosition::NearestCorner)
        );
        assert_eq!(map_seam_position("unknown"), None);
    }

    #[test]
    fn test_enum_mapping_gcode_dialect() {
        assert_eq!(map_gcode_dialect("marlin"), Some(GcodeDialect::Marlin));
        assert_eq!(map_gcode_dialect("klipper"), Some(GcodeDialect::Klipper));
        assert_eq!(
            map_gcode_dialect("reprapfirmware"),
            Some(GcodeDialect::RepRapFirmware)
        );
        assert_eq!(
            map_gcode_dialect("reprap"),
            Some(GcodeDialect::RepRapFirmware)
        );
        assert_eq!(map_gcode_dialect("unknown_dialect"), None);
    }

    #[test]
    fn test_nil_sentinel_skipped() {
        let json_val = json!({
            "type": "process",
            "name": "Test",
            "layer_height": "nil",
            "wall_loops": "3"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        // layer_height should remain at default (0.2) since nil is skipped.
        assert!((result.config.layer_height - 0.2).abs() < 1e-9);
        assert_eq!(result.config.wall_count, 3);
    }

    #[test]
    fn test_ironing_fields() {
        let json_val = json!({
            "type": "process",
            "name": "Test Ironing",
            "ironing_type": "top",
            "ironing_flow": "15%",
            "ironing_speed": "20",
            "ironing_spacing": "0.15"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        assert!(result.config.ironing.enabled);
        assert!((result.config.ironing.flow_rate - 0.15).abs() < 1e-9);
        assert!((result.config.ironing.speed - 20.0).abs() < 1e-9);
        assert!((result.config.ironing.spacing - 0.15).abs() < 1e-9);
    }

    #[test]
    fn test_ironing_no_ironing() {
        let json_val = json!({
            "type": "process",
            "name": "No Ironing",
            "ironing_type": "no ironing"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        assert!(!result.config.ironing.enabled);
    }

    #[test]
    fn test_arachne_wall_generator() {
        let json_val = json!({
            "type": "process",
            "name": "Arachne Test",
            "wall_generator": "arachne"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        assert!(result.config.arachne_enabled);

        let json_val2 = json!({
            "type": "process",
            "name": "Classic Test",
            "wall_generator": "classic"
        });

        let result2 = import_upstream_profile(&json_val2).unwrap();
        assert!(!result2.config.arachne_enabled);
    }

    #[test]
    fn test_machine_jerk_fields() {
        let json_val = json!({
            "type": "machine",
            "name": "Jerk Test",
            "machine_max_jerk_x": ["10"],
            "machine_max_jerk_y": ["10"],
            "machine_max_jerk_z": ["0.5"]
        });

        let result = import_upstream_profile(&json_val).unwrap();
        assert!((result.config.jerk_x - 10.0).abs() < 1e-9);
        assert!((result.config.jerk_y - 10.0).abs() < 1e-9);
        assert!((result.config.jerk_z - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_full_filament_profile() {
        let json_val = json!({
            "type": "filament",
            "name": "Generic PLA",
            "from": "system",
            "instantiation": "true",
            "nozzle_temperature": ["220"],
            "nozzle_temperature_initial_layer": ["225"],
            "filament_density": ["1.24"],
            "filament_diameter": ["1.75"],
            "filament_flow_ratio": ["0.98"],
            "filament_cost": ["20"],
            "close_fan_the_first_x_layers": ["1"],
            "hot_plate_temp": ["55"],
            "hot_plate_temp_initial_layer": ["60"],
            "fan_cooling_layer_time": ["30"]
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        assert!((config.nozzle_temp - 220.0).abs() < 1e-9);
        assert!((config.first_layer_nozzle_temp - 225.0).abs() < 1e-9);
        assert!((config.filament_density - 1.24).abs() < 1e-9);
        assert!((config.filament_diameter - 1.75).abs() < 1e-9);
        assert!((config.extrusion_multiplier - 0.98).abs() < 1e-9);
        assert!((config.filament_cost_per_kg - 20.0).abs() < 1e-9);
        assert_eq!(config.disable_fan_first_layers, 1);
        assert!((config.bed_temp - 55.0).abs() < 1e-9);
        assert!((config.first_layer_bed_temp - 60.0).abs() < 1e-9);
        assert!((config.fan_below_layer_time - 30.0).abs() < 1e-9);
    }

    #[test]
    fn test_full_process_profile() {
        let json_val = json!({
            "type": "process",
            "name": "0.20mm Standard",
            "from": "system",
            "layer_height": "0.2",
            "initial_layer_print_height": "0.28",
            "wall_loops": "2",
            "sparse_infill_density": "15%",
            "sparse_infill_pattern": "grid",
            "top_shell_layers": "4",
            "bottom_shell_layers": "3",
            "outer_wall_speed": "200",
            "sparse_infill_speed": "270",
            "travel_speed": "500",
            "initial_layer_speed": "50",
            "skirt_loops": "1",
            "skirt_distance": "2",
            "brim_width": "0",
            "seam_position": "aligned",
            "default_acceleration": "10000",
            "travel_acceleration": "12000",
            "enable_arc_fitting": "1",
            "adaptive_layer_height": "0",
            "wall_generator": "arachne"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        assert!((config.layer_height - 0.2).abs() < 1e-9);
        assert!((config.first_layer_height - 0.28).abs() < 1e-9);
        assert_eq!(config.wall_count, 2);
        assert!((config.infill_density - 0.15).abs() < 1e-9);
        assert_eq!(config.infill_pattern, InfillPattern::Grid);
        assert_eq!(config.top_solid_layers, 4);
        assert_eq!(config.bottom_solid_layers, 3);
        assert!((config.perimeter_speed - 200.0).abs() < 1e-9);
        assert!((config.infill_speed - 270.0).abs() < 1e-9);
        assert!((config.travel_speed - 500.0).abs() < 1e-9);
        assert!((config.first_layer_speed - 50.0).abs() < 1e-9);
        assert_eq!(config.skirt_loops, 1);
        assert!((config.skirt_distance - 2.0).abs() < 1e-9);
        assert!((config.brim_width - 0.0).abs() < 1e-9);
        assert_eq!(config.seam_position, SeamPosition::Aligned);
        assert!((config.print_acceleration - 10000.0).abs() < 1e-9);
        assert!((config.travel_acceleration - 12000.0).abs() < 1e-9);
        assert!(config.arc_fitting_enabled);
        assert!(!config.adaptive_layer_height);
        assert!(config.arachne_enabled);
    }
}
