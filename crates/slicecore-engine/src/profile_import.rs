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

use crate::config::{BedType, BrimType, InternalBridgeMode, PrintConfig, SurfacePattern};
use crate::error::EngineError;
use crate::infill::InfillPattern;
use crate::seam::SeamPosition;
use crate::support::config::{InterfacePattern, SupportPattern, SupportType};

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
    /// Fields from the source that were successfully mapped to PrintConfig typed fields.
    pub mapped_fields: Vec<String>,
    /// Fields from the source that have no typed PrintConfig equivalent.
    ///
    /// With passthrough storage, these fields are also stored in `config.passthrough`,
    /// so they are preserved for round-trip fidelity. This list is kept for backward
    /// compatibility with the convert pipeline (TOML comments) and CLI reporting.
    pub unmapped_fields: Vec<String>,
    /// Fields stored in passthrough (no typed mapping, but preserved for round-trip).
    pub passthrough_fields: Vec<String>,
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
    let mut passthrough_fields = Vec::new();

    // Extract metadata.
    let metadata = ProfileMetadata {
        name: obj.get("name").and_then(|v| v.as_str()).map(String::from),
        profile_type: obj.get("type").and_then(|v| v.as_str()).map(String::from),
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

        // Try array field mapping first (for Vec<f64> fields that need raw JSON value).
        if apply_array_field_mapping(&mut config, key, val) {
            mapped_fields.push(key.clone());
            continue;
        }

        // Extract string value (handles scalar, array-wrapped, and nil sentinel).
        let string_val = match extract_string_value(val) {
            Some(s) => s,
            None => continue, // nil or unextractable -- skip silently
        };

        // Apply field mapping.
        match apply_field_mapping(&mut config, key, &string_val) {
            FieldMappingResult::Mapped => {
                mapped_fields.push(key.clone());
            }
            FieldMappingResult::Passthrough => {
                passthrough_fields.push(key.clone());
                // Also track in unmapped for backward compat with convert pipeline.
                unmapped_fields.push(key.clone());
            }
            FieldMappingResult::Failed => {
                unmapped_fields.push(key.clone());
            }
        }
    }

    Ok(ImportResult {
        config,
        mapped_fields,
        unmapped_fields,
        passthrough_fields,
        metadata,
    })
}

// ---------------------------------------------------------------------------
// Field mapping result
// ---------------------------------------------------------------------------

/// Result of applying a single field mapping.
enum FieldMappingResult {
    /// Field was mapped to a typed PrintConfig field.
    Mapped,
    /// Field was stored in passthrough (no typed mapping).
    Passthrough,
    /// Field mapping failed (parse error, unrecognized value).
    Failed,
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

/// Extract all values from a JSON array as f64 Vec.
///
/// Handles:
/// - Array of strings: `["0.4", "0.6"]` -> `vec![0.4, 0.6]`
/// - Array of numbers: `[0.4, 0.6]` -> `vec![0.4, 0.6]`
/// - Single string: `"0.4"` -> `vec![0.4]`
/// - Single number: `0.4` -> `vec![0.4]`
/// - Nil sentinel: `"nil"` or `["nil"]` -> empty vec
fn extract_array_f64(value: &serde_json::Value) -> Vec<f64> {
    match value {
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|v| match v {
                serde_json::Value::String(s) if s != "nil" => s.parse::<f64>().ok(),
                serde_json::Value::Number(n) => n.as_f64(),
                _ => None,
            })
            .collect(),
        serde_json::Value::String(s) if s != "nil" => s.parse::<f64>().ok().into_iter().collect(),
        serde_json::Value::Number(n) => n.as_f64().into_iter().collect(),
        _ => Vec::new(),
    }
}

/// Parse a percentage string value, stripping `%` suffix if present.
///
/// `"50%"` -> `Some(50.0)`, `"80"` -> `Some(80.0)`, `"invalid"` -> `None`
fn parse_percentage_or_f64(value: &str) -> Option<f64> {
    let cleaned = value.trim_end_matches('%');
    cleaned.parse::<f64>().ok()
}

// ---------------------------------------------------------------------------
// Field mapping
// ---------------------------------------------------------------------------

/// Apply array field mapping for Vec<f64> multi-extruder fields.
///
/// These fields need the raw JSON value (not the extracted string) to preserve
/// all array elements. Returns `true` if the field was handled.
fn apply_array_field_mapping(
    config: &mut PrintConfig,
    key: &str,
    value: &serde_json::Value,
) -> bool {
    match key {
        // --- Multi-extruder Vec<f64> array fields ---
        "nozzle_diameter" => {
            config.machine.nozzle_diameters = extract_array_f64(value);
            true
        }
        "machine_max_jerk_x" => {
            config.machine.jerk_values_x = extract_array_f64(value);
            true
        }
        "machine_max_jerk_y" => {
            config.machine.jerk_values_y = extract_array_f64(value);
            true
        }
        "machine_max_jerk_z" => {
            config.machine.jerk_values_z = extract_array_f64(value);
            true
        }
        "machine_max_jerk_e" => {
            config.machine.jerk_values_e = extract_array_f64(value);
            true
        }
        "nozzle_temperature" | "temperature" => {
            config.filament.nozzle_temperatures = extract_array_f64(value);
            true
        }
        "bed_temperature" => {
            config.filament.bed_temperatures = extract_array_f64(value);
            true
        }
        "hot_plate_temp" => {
            let temps = extract_array_f64(value);
            config.filament.bed_temperatures = temps.clone();
            config.filament.hot_plate_temp = temps;
            true
        }
        "nozzle_temperature_initial_layer" | "first_layer_temperature" => {
            config.filament.first_layer_nozzle_temperatures = extract_array_f64(value);
            true
        }
        "bed_temperature_initial_layer" | "first_layer_bed_temperature" => {
            config.filament.first_layer_bed_temperatures = extract_array_f64(value);
            true
        }
        "hot_plate_temp_initial_layer" => {
            let temps = extract_array_f64(value);
            config.filament.first_layer_bed_temperatures = temps.clone();
            config.filament.hot_plate_temp_initial_layer = temps;
            true
        }

        // --- Per-bed-type temperature arrays (OrcaSlicer-specific) ---
        "cool_plate_temp" => {
            config.filament.cool_plate_temp = extract_array_f64(value);
            true
        }
        "eng_plate_temp" => {
            config.filament.eng_plate_temp = extract_array_f64(value);
            true
        }
        "textured_plate_temp" => {
            config.filament.textured_plate_temp = extract_array_f64(value);
            true
        }
        "cool_plate_temp_initial_layer" => {
            config.filament.cool_plate_temp_initial_layer = extract_array_f64(value);
            true
        }
        "eng_plate_temp_initial_layer" => {
            config.filament.eng_plate_temp_initial_layer = extract_array_f64(value);
            true
        }
        "textured_plate_temp_initial_layer" => {
            config.filament.textured_plate_temp_initial_layer = extract_array_f64(value);
            true
        }
        _ => false,
    }
}

/// Map an upstream JSON key name to the corresponding PrintConfig field name.
///
/// Returns `None` for keys that don't map to a simple top-level field (e.g.,
/// ironing sub-fields, enum mappings with complex logic).
pub(crate) fn upstream_key_to_config_field(key: &str) -> Option<&'static str> {
    match key {
        // --- Original process fields ---
        "layer_height" => Some("layer_height"),
        "initial_layer_print_height" => Some("first_layer_height"),
        "wall_loops" => Some("wall_count"),
        "sparse_infill_density" => Some("infill_density"),
        "sparse_infill_pattern" => Some("infill_pattern"),
        "top_shell_layers" => Some("top_solid_layers"),
        "bottom_shell_layers" => Some("bottom_solid_layers"),
        "outer_wall_speed" => Some("speeds.perimeter"),
        "sparse_infill_speed" => Some("speeds.infill"),
        "travel_speed" => Some("speeds.travel"),
        "initial_layer_speed" => Some("speeds.first_layer"),
        "skirt_loops" => Some("skirt_loops"),
        "skirt_distance" => Some("skirt_distance"),
        "brim_width" => Some("brim_width"),
        "default_acceleration" => Some("accel.print"),
        "travel_acceleration" => Some("accel.travel"),
        "enable_arc_fitting" => Some("arc_fitting_enabled"),
        "adaptive_layer_height" => Some("adaptive_layer_height"),
        "wall_generator" => Some("arachne_enabled"),
        "seam_position" => Some("seam_position"),

        // --- Speed sub-config fields ---
        "bridge_speed" => Some("speeds.bridge"),
        "inner_wall_speed" => Some("speeds.inner_wall"),
        "gap_infill_speed" => Some("speeds.gap_fill"),
        "top_surface_speed" => Some("speeds.top_surface"),
        "internal_solid_infill_speed" => Some("speeds.internal_solid_infill"),
        "initial_layer_infill_speed" => Some("speeds.initial_layer_infill"),
        "support_speed" => Some("speeds.support"),
        "support_interface_speed" => Some("speeds.support_interface"),
        "small_perimeter_speed" => Some("speeds.small_perimeter"),
        "solid_infill_speed" => Some("speeds.solid_infill"),
        "overhang_1_4_speed" | "overhang_speed_0" => Some("speeds.overhang_1_4"),
        "overhang_2_4_speed" | "overhang_speed_1" => Some("speeds.overhang_2_4"),
        "overhang_3_4_speed" | "overhang_speed_2" => Some("speeds.overhang_3_4"),
        "overhang_4_4_speed" | "overhang_speed_3" => Some("speeds.overhang_4_4"),
        "travel_speed_z" => Some("speeds.travel_z"),

        // --- Line width sub-config fields ---
        "line_width" | "extrusion_width" => Some("line_widths.outer_wall"),
        "outer_wall_line_width" => Some("line_widths.outer_wall"),
        "inner_wall_line_width" => Some("line_widths.inner_wall"),
        "sparse_infill_line_width" => Some("line_widths.infill"),
        "top_surface_line_width" => Some("line_widths.top_surface"),
        "initial_layer_line_width" => Some("line_widths.initial_layer"),
        "internal_solid_infill_line_width" => Some("line_widths.internal_solid_infill"),
        "support_line_width" => Some("line_widths.support"),

        // --- Cooling sub-config fields ---
        "fan_max_speed" => Some("cooling.fan_max_speed"),
        "fan_min_speed" => Some("cooling.fan_min_speed"),
        "slow_down_layer_time" | "slowdown_below_layer_time" => {
            Some("cooling.slow_down_layer_time")
        }
        "slow_down_min_speed" | "min_print_speed" => Some("cooling.slow_down_min_speed"),
        "overhang_fan_speed" => Some("cooling.overhang_fan_speed"),
        "overhang_fan_threshold" => Some("cooling.overhang_fan_threshold"),
        "full_fan_speed_layer" | "disable_fan_first_layers" => Some("cooling.full_fan_speed_layer"),
        "slow_down_for_layer_cooling" => Some("cooling.slow_down_for_layer_cooling"),

        // --- Retraction sub-config fields ---
        "deretraction_speed" => Some("retraction.deretraction_speed"),
        "retract_before_wipe" => Some("retraction.retract_before_wipe"),
        "retract_when_changing_layer" => Some("retraction.retract_when_changing_layer"),
        "wipe" => Some("retraction.wipe"),
        "wipe_distance" => Some("retraction.wipe_distance"),

        // --- Machine sub-config fields ---
        "machine_start_gcode" | "start_gcode" => Some("machine.start_gcode"),
        "machine_end_gcode" | "end_gcode" => Some("machine.end_gcode"),
        "layer_change_gcode" | "layer_gcode" => Some("machine.layer_change_gcode"),
        "printable_height" | "max_print_height" => Some("machine.printable_height"),
        "machine_max_acceleration_x" => Some("machine.max_acceleration_x"),
        "machine_max_acceleration_y" => Some("machine.max_acceleration_y"),
        "machine_max_acceleration_z" => Some("machine.max_acceleration_z"),
        "machine_max_acceleration_e" => Some("machine.max_acceleration_e"),
        "machine_max_acceleration_extruding" => Some("machine.max_acceleration_extruding"),
        "machine_max_acceleration_retracting" => Some("machine.max_acceleration_retracting"),
        "machine_max_acceleration_travel" => Some("machine.max_acceleration_travel"),
        "machine_max_speed_x" => Some("machine.max_speed_x"),
        "machine_max_speed_y" => Some("machine.max_speed_y"),
        "machine_max_speed_z" => Some("machine.max_speed_z"),
        "machine_max_speed_e" => Some("machine.max_speed_e"),
        "nozzle_type" => Some("machine.nozzle_type"),
        "printer_model" | "printer_model_id" => Some("machine.printer_model"),
        "bed_shape" | "printable_area" => Some("machine.bed_shape"),
        "min_layer_height" => Some("machine.min_layer_height"),
        "max_layer_height" => Some("machine.max_layer_height"),

        // --- Sequential/gantry clearance fields ---
        "extruder_clearance_radius" | "extruder_clearance_max_radius" => {
            Some("sequential.extruder_clearance_radius")
        }
        "extruder_clearance_height_to_rod"
        | "extruder_clearance_height_to_lid"
        | "extruder_clearance_height" => Some("sequential.extruder_clearance_height"),
        "gantry_width" => Some("sequential.gantry_width"),

        // --- Acceleration sub-config fields ---
        "outer_wall_acceleration" => Some("accel.outer_wall"),
        "inner_wall_acceleration" => Some("accel.inner_wall"),
        "initial_layer_acceleration" => Some("accel.initial_layer"),
        "initial_layer_travel_acceleration" | "initial_layer_travel_speed" => {
            Some("accel.initial_layer_travel")
        }
        "top_surface_acceleration" => Some("accel.top_surface"),
        "sparse_infill_acceleration" => Some("accel.sparse_infill"),
        "bridge_acceleration" => Some("accel.bridge"),

        // --- Filament temperature fields (migrated to sub-config) ---
        "nozzle_temperature" | "temperature" => Some("filament.nozzle_temperatures"),
        "nozzle_temperature_initial_layer" | "first_layer_temperature" => {
            Some("filament.first_layer_nozzle_temperatures")
        }
        "hot_plate_temp" | "bed_temperature" => Some("filament.bed_temperatures"),
        "hot_plate_temp_initial_layer"
        | "bed_temperature_initial_layer"
        | "first_layer_bed_temperature" => Some("filament.first_layer_bed_temperatures"),
        "filament_density" => Some("filament.density"),
        "filament_diameter" => Some("filament.diameter"),
        "filament_cost" => Some("filament.cost_per_kg"),
        "filament_flow_ratio" => Some("extrusion_multiplier"),
        "close_fan_the_first_x_layers" => Some("cooling.disable_fan_first_layers"),
        "fan_cooling_layer_time" => Some("cooling.fan_below_layer_time"),

        // --- Filament sub-config fields ---
        "filament_type" => Some("filament.filament_type"),
        "filament_vendor" => Some("filament.filament_vendor"),
        "filament_max_volumetric_speed" => Some("filament.max_volumetric_speed"),
        "nozzle_temperature_range_low" => Some("filament.nozzle_temperature_range_low"),
        "nozzle_temperature_range_high" => Some("filament.nozzle_temperature_range_high"),
        "filament_retraction_length" => Some("filament.filament_retraction_length"),
        "filament_retraction_speed" => Some("filament.filament_retraction_speed"),
        "filament_start_gcode" => Some("filament.filament_start_gcode"),
        "filament_end_gcode" => Some("filament.filament_end_gcode"),

        // --- Machine fields (migrated to sub-configs) ---
        "nozzle_diameter" => Some("machine.nozzle_diameters"),
        "retraction_length" => Some("retraction.length"),
        "retraction_speed" => Some("retraction.speed"),
        "z_hop" => Some("retraction.z_hop"),
        "retraction_minimum_travel" => Some("retraction.min_travel"),
        "gcode_flavor" => Some("gcode_dialect"),
        "machine_max_jerk_x" => Some("machine.jerk_values_x"),
        "machine_max_jerk_y" => Some("machine.jerk_values_y"),
        "machine_max_jerk_z" => Some("machine.jerk_values_z"),
        "machine_max_jerk_e" => Some("machine.jerk_values_e"),

        // --- Process misc fields ---
        "bridge_flow" | "bridge_flow_ratio" => Some("bridge_flow"),
        "elefant_foot_compensation" => Some("elefant_foot_compensation"),
        "infill_direction" => Some("infill_direction"),
        "infill_wall_overlap" | "infill_overlap" => Some("infill_wall_overlap"),
        "spiral_mode" | "spiral_vase" => Some("spiral_mode"),
        "only_one_wall_top" => Some("only_one_wall_top"),
        "resolution" => Some("resolution"),
        "raft_layers" => Some("raft_layers"),
        "detect_thin_wall" | "thin_walls" => Some("detect_thin_wall"),

        // --- P0 config gap closure fields ---
        "xy_hole_compensation" => Some("dimensional_compensation.xy_hole_compensation"),
        "xy_contour_compensation" => Some("dimensional_compensation.xy_contour_compensation"),
        "top_surface_pattern" => Some("top_surface_pattern"),
        "bottom_surface_pattern" => Some("bottom_surface_pattern"),
        "internal_solid_infill_pattern" => Some("solid_infill_pattern"),
        "extra_perimeters_on_overhangs" => Some("extra_perimeters_on_overhangs"),
        "internal_bridge_speed" => Some("speeds.internal_bridge_speed"),
        "internal_bridge_support_enabled" => Some("internal_bridge_support"),
        "filament_shrinkage_compensation" => Some("filament.filament_shrink"),
        "z_offset" => Some("z_offset"),
        "precise_z_height" => Some("precise_z_height"),
        "min_length_factor" => Some("accel.min_length_factor"),
        "chamber_temperature" => Some("filament.chamber_temperature"),
        "curr_bed_type" => Some("machine.curr_bed_type"),
        "cool_plate_temp" => Some("filament.cool_plate_temp"),
        "eng_plate_temp" => Some("filament.eng_plate_temp"),
        "textured_plate_temp" => Some("filament.textured_plate_temp"),
        "cool_plate_temp_initial_layer" => Some("filament.cool_plate_temp_initial_layer"),
        "eng_plate_temp_initial_layer" => Some("filament.eng_plate_temp_initial_layer"),
        "textured_plate_temp_initial_layer" => {
            Some("filament.textured_plate_temp_initial_layer")
        }

        // --- P1 config gap closure fields ---
        "fuzzy_skin" => Some("fuzzy_skin.enabled"),
        "fuzzy_skin_thickness" => Some("fuzzy_skin.thickness"),
        "fuzzy_skin_point_dist" => Some("fuzzy_skin.point_distance"),
        "brim_type" => Some("brim_skirt.brim_type"),
        "brim_ears" => Some("brim_skirt.brim_ears"),
        "brim_ears_max_angle" => Some("brim_skirt.brim_ears_max_angle"),
        "skirt_height" => Some("brim_skirt.skirt_height"),
        "accel_to_decel_enable" => Some("input_shaping.accel_to_decel_enable"),
        "accel_to_decel_factor" => Some("input_shaping.accel_to_decel_factor"),
        "retraction_distances_when_cut" => {
            Some("multi_material.tool_change_retraction.retraction_distance_when_cut")
        }
        "long_retractions_when_cut" => {
            Some("multi_material.tool_change_retraction.long_retraction_when_cut")
        }
        "internal_solid_infill_acceleration" => {
            Some("accel.internal_solid_infill_acceleration")
        }
        "support_acceleration" => Some("accel.support_acceleration"),
        "support_interface_acceleration" => Some("accel.support_interface_acceleration"),
        "additional_cooling_fan_speed" => Some("cooling.additional_cooling_fan_speed"),
        "auxiliary_fan" => Some("cooling.auxiliary_fan"),
        "enable_overhang_speed" => Some("speeds.enable_overhang_speed"),
        "filament_colour" => Some("filament.filament_colour"),
        "wall_filament" => Some("multi_material.wall_filament"),
        "solid_infill_filament" => Some("multi_material.solid_infill_filament"),
        "support_filament" => Some("multi_material.support_filament"),
        "support_interface_filament" => Some("multi_material.support_interface_filament"),
        "precise_outer_wall" => Some("precise_outer_wall"),
        "draft_shield" => Some("draft_shield"),
        "ooze_prevention" => Some("ooze_prevention"),
        "infill_combination" | "infill_every_layers" => Some("infill_combination"),
        "infill_anchor_max" => Some("infill_anchor_max"),
        "min_bead_width" => Some("min_bead_width"),
        "min_feature_size" => Some("min_feature_size"),
        "support_bottom_interface_layers" => Some("support.support_bottom_interface_layers"),

        // --- Support config fields ---
        "enable_support" | "support_material" => Some("support.enabled"),
        "support_type" | "support_material_type" | "support_style"
        | "support_material_style" => Some("support.support_type"),
        "support_threshold_angle" | "support_angle"
        | "support_material_threshold" => Some("support.overhang_angle"),
        "support_base_pattern" | "support_material_pattern" => Some("support.support_pattern"),
        "support_on_build_plate_only" | "support_material_buildplate_only" => {
            Some("support.build_plate_only")
        }
        "support_top_z_distance" | "support_material_contact_distance" => Some("support.z_gap"),
        "support_bottom_z_distance" | "support_material_bottom_contact_distance" => {
            Some("support.bottom_z_gap")
        }
        "support_object_xy_distance" | "support_material_xy_spacing" => Some("support.xy_gap"),
        "support_interface_top_layers" | "support_material_interface_layers" => {
            Some("support.interface_layers")
        }
        "support_interface_bottom_layers"
        | "support_material_bottom_interface_layers" => {
            Some("support.support_bottom_interface_layers")
        }
        "support_interface_pattern" | "support_material_interface_pattern" => {
            Some("support.interface_pattern")
        }
        "support_base_pattern_spacing" | "support_material_spacing" => {
            Some("support.support_density")
        }
        "support_interface_spacing" | "support_material_interface_spacing" => {
            Some("support.interface_density")
        }
        "support_expansion" => Some("support.expansion"),
        "support_critical_regions_only" => Some("support.critical_regions_only"),
        "support_remove_small_overhang" => Some("support.remove_small_overhang"),
        "support_flow_ratio" | "support_material_flow" => Some("support.flow_ratio"),
        "support_interface_flow_ratio" | "support_material_interface_flow" => {
            Some("support.interface_flow_ratio")
        }
        "support_material_synchronize_layers" => Some("support.synchronize_layers"),
        "enforce_support_layers" | "support_material_enforce_layers" => {
            Some("support.enforce_layers")
        }
        "support_closing_radius" | "support_material_closing_radius" => {
            Some("support.closing_radius")
        }
        "support_material_auto" => Some("support.support_type"),
        "raft_first_layer_expansion" => Some("support.raft_expansion"),
        "bridge_angle" => Some("support.bridge.angle"),
        "bridge_density" => Some("support.bridge.density"),
        "thick_bridges" => Some("support.bridge.thick_bridges"),
        "bridge_no_support" => Some("support.bridge.no_support"),
        "bridge_fan_speed" => Some("support.bridge.fan_speed"),
        "tree_support_branch_angle" | "support_tree_angle" => {
            Some("support.tree.branch_angle")
        }
        "tree_support_branch_diameter" | "support_tree_branch_diameter" => {
            Some("support.tree.max_trunk_diameter")
        }
        "tree_support_tip_diameter" => Some("support.tree.tip_diameter"),
        "tree_support_branch_distance" | "support_tree_top_rate" => {
            Some("support.tree.branch_distance")
        }
        "tree_support_branch_diameter_angle" => Some("support.tree.branch_diameter_angle"),
        "tree_support_wall_count" => Some("support.tree.wall_count"),
        "tree_support_auto_brim" => Some("support.tree.auto_brim"),
        "tree_support_brim_width" => Some("support.tree.brim_width"),
        "tree_support_adaptive_layer_height" => Some("support.tree.adaptive_layer_height"),
        "tree_support_angle_slow" => Some("support.tree.angle_slow"),
        "tree_support_top_rate" => Some("support.tree.top_rate"),
        "tree_support_with_infill" => Some("support.tree.with_infill"),

        // Ironing sub-fields don't map to simple top-level fields.
        _ => None,
    }
}

/// Apply a single field mapping from an upstream JSON key/value to PrintConfig.
///
/// The `value` parameter is the already-extracted plain string (scalar or
/// array-unwrapped). Returns a `FieldMappingResult` indicating success, passthrough,
/// or failure.
///
/// Note: Vec<f64> array fields (nozzle_diameter, jerk, temperatures) are handled
/// by `apply_array_field_mapping` which is called first with the raw JSON value.
fn apply_field_mapping(config: &mut PrintConfig, key: &str, value: &str) -> FieldMappingResult {
    let mapped = match key {
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
        "outer_wall_speed" => parse_and_set_f64(value, &mut config.speeds.perimeter),
        "sparse_infill_speed" => parse_and_set_f64(value, &mut config.speeds.infill),
        "travel_speed" => parse_and_set_f64(value, &mut config.speeds.travel),
        "initial_layer_speed" => parse_and_set_f64(value, &mut config.speeds.first_layer),
        "skirt_loops" => parse_and_set_u32(value, &mut config.skirt_loops),
        "skirt_distance" => parse_and_set_f64(value, &mut config.skirt_distance),
        "brim_width" => parse_and_set_f64(value, &mut config.brim_width),
        "default_acceleration" => parse_and_set_f64(value, &mut config.accel.print),
        "travel_acceleration" => parse_and_set_f64(value, &mut config.accel.travel),
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

        // --- Speed sub-config fields ---
        "bridge_speed" => parse_and_set_f64(value, &mut config.speeds.bridge),
        "inner_wall_speed" => parse_and_set_f64(value, &mut config.speeds.inner_wall),
        "gap_infill_speed" => parse_and_set_f64(value, &mut config.speeds.gap_fill),
        "top_surface_speed" => parse_and_set_f64(value, &mut config.speeds.top_surface),
        "internal_solid_infill_speed" => {
            parse_and_set_f64(value, &mut config.speeds.internal_solid_infill)
        }
        "initial_layer_infill_speed" => {
            parse_and_set_f64(value, &mut config.speeds.initial_layer_infill)
        }
        "support_speed" => parse_and_set_f64(value, &mut config.speeds.support),
        "support_interface_speed" => parse_and_set_f64(value, &mut config.speeds.support_interface),
        "small_perimeter_speed" => {
            // Handle percentage format: strip % if present, parse as raw mm/s.
            if let Some(v) = parse_percentage_or_f64(value) {
                config.speeds.small_perimeter = v;
                true
            } else {
                false
            }
        }
        "solid_infill_speed" => parse_and_set_f64(value, &mut config.speeds.solid_infill),
        "overhang_1_4_speed" | "overhang_speed_0" => {
            parse_and_set_f64(value, &mut config.speeds.overhang_1_4)
        }
        "overhang_2_4_speed" | "overhang_speed_1" => {
            parse_and_set_f64(value, &mut config.speeds.overhang_2_4)
        }
        "overhang_3_4_speed" | "overhang_speed_2" => {
            parse_and_set_f64(value, &mut config.speeds.overhang_3_4)
        }
        "overhang_4_4_speed" | "overhang_speed_3" => {
            parse_and_set_f64(value, &mut config.speeds.overhang_4_4)
        }
        "travel_speed_z" => parse_and_set_f64(value, &mut config.speeds.travel_z),

        // --- Line width sub-config fields ---
        "line_width" | "extrusion_width" => {
            // Base line width: store in passthrough for reference.
            config
                .passthrough
                .insert(key.to_string(), value.to_string());
            true
        }
        "outer_wall_line_width" => parse_and_set_f64(value, &mut config.line_widths.outer_wall),
        "inner_wall_line_width" => parse_and_set_f64(value, &mut config.line_widths.inner_wall),
        "sparse_infill_line_width" => parse_and_set_f64(value, &mut config.line_widths.infill),
        "top_surface_line_width" => parse_and_set_f64(value, &mut config.line_widths.top_surface),
        "initial_layer_line_width" => {
            parse_and_set_f64(value, &mut config.line_widths.initial_layer)
        }
        "internal_solid_infill_line_width" => {
            parse_and_set_f64(value, &mut config.line_widths.internal_solid_infill)
        }
        "support_line_width" => parse_and_set_f64(value, &mut config.line_widths.support),

        // --- Cooling sub-config fields ---
        "fan_max_speed" => {
            // Percentage value (0-100), strip % if present.
            if let Some(v) = parse_percentage_or_f64(value) {
                config.cooling.fan_max_speed = v;
                true
            } else {
                false
            }
        }
        "fan_min_speed" => {
            if let Some(v) = parse_percentage_or_f64(value) {
                config.cooling.fan_min_speed = v;
                true
            } else {
                false
            }
        }
        "slow_down_layer_time" | "slowdown_below_layer_time" => {
            parse_and_set_f64(value, &mut config.cooling.slow_down_layer_time)
        }
        "slow_down_min_speed" | "min_print_speed" => {
            parse_and_set_f64(value, &mut config.cooling.slow_down_min_speed)
        }
        "overhang_fan_speed" => {
            if let Some(v) = parse_percentage_or_f64(value) {
                config.cooling.overhang_fan_speed = v;
                true
            } else {
                false
            }
        }
        "overhang_fan_threshold" => {
            parse_and_set_f64(value, &mut config.cooling.overhang_fan_threshold)
        }
        "full_fan_speed_layer" => {
            parse_and_set_u32(value, &mut config.cooling.full_fan_speed_layer)
        }
        "slow_down_for_layer_cooling" => {
            config.cooling.slow_down_for_layer_cooling = value == "1" || value == "true";
            true
        }

        // --- Retraction sub-config fields ---
        "deretraction_speed" => parse_and_set_f64(value, &mut config.retraction.deretraction_speed),
        "retract_before_wipe" => {
            // Percentage value.
            if let Some(v) = parse_percentage_or_f64(value) {
                config.retraction.retract_before_wipe = v;
                true
            } else {
                false
            }
        }
        "retract_when_changing_layer" => {
            config.retraction.retract_when_changing_layer = value == "1" || value == "true";
            true
        }
        "wipe" => {
            config.retraction.wipe = value == "1" || value == "true";
            true
        }
        "wipe_distance" => parse_and_set_f64(value, &mut config.retraction.wipe_distance),

        // --- Machine sub-config fields ---
        "machine_start_gcode" | "start_gcode" => {
            config.machine.start_gcode = value.to_string();
            true
        }
        "machine_end_gcode" | "end_gcode" => {
            config.machine.end_gcode = value.to_string();
            true
        }
        "layer_change_gcode" | "layer_gcode" => {
            config.machine.layer_change_gcode = value.to_string();
            true
        }
        "printable_height" | "max_print_height" => {
            parse_and_set_f64(value, &mut config.machine.printable_height)
        }
        "machine_max_acceleration_x" => {
            parse_and_set_f64(value, &mut config.machine.max_acceleration_x)
        }
        "machine_max_acceleration_y" => {
            parse_and_set_f64(value, &mut config.machine.max_acceleration_y)
        }
        "machine_max_acceleration_z" => {
            parse_and_set_f64(value, &mut config.machine.max_acceleration_z)
        }
        "machine_max_acceleration_e" => {
            parse_and_set_f64(value, &mut config.machine.max_acceleration_e)
        }
        "machine_max_acceleration_extruding" => {
            parse_and_set_f64(value, &mut config.machine.max_acceleration_extruding)
        }
        "machine_max_acceleration_retracting" => {
            parse_and_set_f64(value, &mut config.machine.max_acceleration_retracting)
        }
        "machine_max_acceleration_travel" => {
            parse_and_set_f64(value, &mut config.machine.max_acceleration_travel)
        }
        "machine_max_speed_x" => parse_and_set_f64(value, &mut config.machine.max_speed_x),
        "machine_max_speed_y" => parse_and_set_f64(value, &mut config.machine.max_speed_y),
        "machine_max_speed_z" => parse_and_set_f64(value, &mut config.machine.max_speed_z),
        "machine_max_speed_e" => parse_and_set_f64(value, &mut config.machine.max_speed_e),
        "nozzle_type" => {
            config.machine.nozzle_type = value.to_string();
            true
        }
        "printer_model" | "printer_model_id" => {
            config.machine.printer_model = value.to_string();
            true
        }
        "bed_shape" | "printable_area" => {
            config.machine.bed_shape = value.to_string();
            true
        }
        "min_layer_height" => parse_and_set_f64(value, &mut config.machine.min_layer_height),
        "max_layer_height" => parse_and_set_f64(value, &mut config.machine.max_layer_height),

        // --- Sequential/gantry clearance fields ---
        "extruder_clearance_radius" | "extruder_clearance_max_radius" => {
            parse_and_set_f64(value, &mut config.sequential.extruder_clearance_radius)
        }
        "extruder_clearance_height_to_rod" | "extruder_clearance_height_to_lid" => {
            // OrcaSlicer has two height fields; take the max of both.
            if let Ok(v) = value.parse::<f64>() {
                if v > config.sequential.extruder_clearance_height {
                    config.sequential.extruder_clearance_height = v;
                }
                true
            } else {
                false
            }
        }
        "extruder_clearance_height" => {
            parse_and_set_f64(value, &mut config.sequential.extruder_clearance_height)
        }
        "gantry_width" => parse_and_set_f64(value, &mut config.sequential.gantry_width),

        // --- Acceleration sub-config fields ---
        "outer_wall_acceleration" => parse_and_set_f64(value, &mut config.accel.outer_wall),
        "inner_wall_acceleration" => parse_and_set_f64(value, &mut config.accel.inner_wall),
        "initial_layer_acceleration" => parse_and_set_f64(value, &mut config.accel.initial_layer),
        "initial_layer_travel_acceleration" | "initial_layer_travel_speed" => {
            parse_and_set_f64(value, &mut config.accel.initial_layer_travel)
        }
        "top_surface_acceleration" => parse_and_set_f64(value, &mut config.accel.top_surface),
        "sparse_infill_acceleration" => parse_and_set_f64(value, &mut config.accel.sparse_infill),
        "bridge_acceleration" => parse_and_set_f64(value, &mut config.accel.bridge),

        // --- Filament fields (original flat) ---
        // Note: temperature array fields are handled by apply_array_field_mapping.
        // These scalar fallbacks handle the case when apply_array_field_mapping
        // already consumed them (won't reach here), but we keep them for safety.
        "filament_density" => parse_and_set_f64(value, &mut config.filament.density),
        "filament_diameter" => parse_and_set_f64(value, &mut config.filament.diameter),
        "filament_cost" => parse_and_set_f64(value, &mut config.filament.cost_per_kg),
        "filament_flow_ratio" => parse_and_set_f64(value, &mut config.extrusion_multiplier),
        "close_fan_the_first_x_layers" => {
            parse_and_set_u32(value, &mut config.cooling.disable_fan_first_layers)
        }
        "fan_cooling_layer_time" => {
            parse_and_set_f64(value, &mut config.cooling.fan_below_layer_time)
        }

        // --- Filament sub-config fields ---
        "filament_type" => {
            config.filament.filament_type = value.to_string();
            true
        }
        "filament_vendor" => {
            config.filament.filament_vendor = value.to_string();
            true
        }
        "filament_max_volumetric_speed" => {
            parse_and_set_f64(value, &mut config.filament.max_volumetric_speed)
        }
        "nozzle_temperature_range_low" => {
            parse_and_set_f64(value, &mut config.filament.nozzle_temperature_range_low)
        }
        "nozzle_temperature_range_high" => {
            parse_and_set_f64(value, &mut config.filament.nozzle_temperature_range_high)
        }
        "filament_retraction_length" => {
            if let Ok(v) = value.parse::<f64>() {
                config.filament.filament_retraction_length = Some(v);
                true
            } else {
                false
            }
        }
        "filament_retraction_speed" => {
            if let Ok(v) = value.parse::<f64>() {
                config.filament.filament_retraction_speed = Some(v);
                true
            } else {
                false
            }
        }
        "filament_start_gcode" => {
            config.filament.filament_start_gcode = value.to_string();
            true
        }
        "filament_end_gcode" => {
            config.filament.filament_end_gcode = value.to_string();
            true
        }

        // --- Machine fields (original flat) ---
        // Note: nozzle_diameter and jerk array fields are handled by apply_array_field_mapping.
        "retraction_length" => parse_and_set_f64(value, &mut config.retraction.length),
        "retraction_speed" => parse_and_set_f64(value, &mut config.retraction.speed),
        "z_hop" => parse_and_set_f64(value, &mut config.retraction.z_hop),
        "retraction_minimum_travel" => parse_and_set_f64(value, &mut config.retraction.min_travel),
        "gcode_flavor" => {
            if let Some(dialect) = map_gcode_dialect(value) {
                config.gcode_dialect = dialect;
                true
            } else {
                false
            }
        }

        // --- Process misc flat fields ---
        "bridge_flow" | "bridge_flow_ratio" => parse_and_set_f64(value, &mut config.bridge_flow),
        "elefant_foot_compensation" => {
            parse_and_set_f64(value, &mut config.dimensional_compensation.elephant_foot_compensation)
        }
        "infill_direction" => parse_and_set_f64(value, &mut config.infill_direction),
        "infill_wall_overlap" | "infill_overlap" => {
            // Handle percentage format: strip %, divide by 100.
            let cleaned = value.trim_end_matches('%');
            if let Ok(v) = cleaned.parse::<f64>() {
                config.infill_wall_overlap = if value.contains('%') { v / 100.0 } else { v };
                true
            } else {
                false
            }
        }
        "spiral_mode" | "spiral_vase" => {
            config.spiral_mode = value == "1" || value == "true";
            true
        }
        "only_one_wall_top" => {
            config.only_one_wall_top = value == "1" || value == "true";
            true
        }
        "resolution" => parse_and_set_f64(value, &mut config.resolution),
        "raft_layers" => parse_and_set_u32(value, &mut config.raft_layers),
        "detect_thin_wall" | "thin_walls" => {
            config.detect_thin_wall = value == "1" || value == "true";
            true
        }

        // --- P0 config gap closure: dimensional compensation ---
        "xy_hole_compensation" => {
            parse_and_set_f64(value, &mut config.dimensional_compensation.xy_hole_compensation)
        }
        "xy_contour_compensation" => {
            parse_and_set_f64(value, &mut config.dimensional_compensation.xy_contour_compensation)
        }

        // --- P0 config gap closure: surface patterns ---
        "top_surface_pattern" => {
            if let Some(p) = map_surface_pattern(value) {
                config.top_surface_pattern = p;
                true
            } else {
                false
            }
        }
        "bottom_surface_pattern" => {
            if let Some(p) = map_surface_pattern(value) {
                config.bottom_surface_pattern = p;
                true
            } else {
                false
            }
        }
        "internal_solid_infill_pattern" => {
            if let Some(p) = map_surface_pattern(value) {
                config.solid_infill_pattern = p;
                true
            } else {
                false
            }
        }

        // --- P0 config gap closure: overhang perimeters ---
        "extra_perimeters_on_overhangs" => {
            config.extra_perimeters_on_overhangs =
                value == "1" || value.eq_ignore_ascii_case("true");
            true
        }

        // --- P0 config gap closure: bridge settings ---
        "internal_bridge_speed" => {
            parse_and_set_f64(value, &mut config.speeds.internal_bridge_speed)
        }
        "internal_bridge_support_enabled" => {
            if let Some(mode) = map_internal_bridge_mode(value) {
                config.internal_bridge_support = mode;
                true
            } else {
                false
            }
        }

        // --- P0 config gap closure: filament shrink ---
        "filament_shrinkage_compensation" => {
            parse_and_set_f64(value, &mut config.filament.filament_shrink)
        }

        // --- P0 config gap closure: z offset (global) ---
        "z_offset" => parse_and_set_f64(value, &mut config.z_offset),

        // --- P0 config gap closure: precise Z ---
        "precise_z_height" => {
            config.precise_z_height = value == "1" || value.eq_ignore_ascii_case("true");
            true
        }

        // --- P0 config gap closure: acceleration min_length_factor ---
        "min_length_factor" => parse_and_set_f64(value, &mut config.accel.min_length_factor),

        // --- P0 config gap closure: chamber temperature ---
        // OrcaSlicer uses same key in both machine and filament contexts.
        // Import as filament by default; machine profiles use separate mapping.
        "chamber_temperature" => {
            parse_and_set_f64(value, &mut config.filament.chamber_temperature)
        }

        // --- P0 config gap closure: bed type ---
        "curr_bed_type" => {
            if let Some(bt) = map_bed_type(value) {
                config.machine.curr_bed_type = bt;
                true
            } else {
                false
            }
        }

        // --- P1 config gap closure: fuzzy skin ---
        "fuzzy_skin" => {
            config.fuzzy_skin.enabled =
                value == "1" || value.eq_ignore_ascii_case("true");
            true
        }
        "fuzzy_skin_thickness" => parse_and_set_f64(value, &mut config.fuzzy_skin.thickness),
        "fuzzy_skin_point_dist" => {
            parse_and_set_f64(value, &mut config.fuzzy_skin.point_distance)
        }

        // --- P1 config gap closure: brim/skirt ---
        "brim_type" => {
            if let Some(bt) = map_brim_type(value) {
                config.brim_skirt.brim_type = bt;
                true
            } else {
                false
            }
        }
        "brim_ears" => {
            config.brim_skirt.brim_ears =
                value == "1" || value.eq_ignore_ascii_case("true");
            true
        }
        "brim_ears_max_angle" => {
            parse_and_set_f64(value, &mut config.brim_skirt.brim_ears_max_angle)
        }
        "skirt_height" => parse_and_set_u32(value, &mut config.brim_skirt.skirt_height),

        // --- P1 config gap closure: input shaping ---
        "accel_to_decel_enable" => {
            config.input_shaping.accel_to_decel_enable =
                value == "1" || value.eq_ignore_ascii_case("true");
            true
        }
        "accel_to_decel_factor" => {
            parse_and_set_f64(value, &mut config.input_shaping.accel_to_decel_factor)
        }

        // --- P1 config gap closure: tool change retraction ---
        "retraction_distances_when_cut" => {
            // OrcaSlicer uses plural "distances" and may be an array; take first value.
            let first = value.split(',').next().unwrap_or(value).trim();
            parse_and_set_f64(
                first,
                &mut config.multi_material.tool_change_retraction.retraction_distance_when_cut,
            )
        }
        "long_retractions_when_cut" => {
            // OrcaSlicer uses plural "retractions" and may be an array; take first value.
            let first = value.split(',').next().unwrap_or(value).trim();
            config
                .multi_material
                .tool_change_retraction
                .long_retraction_when_cut =
                first == "1" || first.eq_ignore_ascii_case("true");
            true
        }

        // --- P1 config gap closure: acceleration ---
        "internal_solid_infill_acceleration" => {
            parse_and_set_f64(value, &mut config.accel.internal_solid_infill_acceleration)
        }
        "support_acceleration" => {
            parse_and_set_f64(value, &mut config.accel.support_acceleration)
        }
        "support_interface_acceleration" => {
            parse_and_set_f64(value, &mut config.accel.support_interface_acceleration)
        }

        // --- P1 config gap closure: cooling ---
        "additional_cooling_fan_speed" => {
            parse_and_set_f64(value, &mut config.cooling.additional_cooling_fan_speed)
        }
        "auxiliary_fan" => {
            config.cooling.auxiliary_fan =
                value == "1" || value.eq_ignore_ascii_case("true");
            true
        }

        // --- P1 config gap closure: speed ---
        "enable_overhang_speed" => {
            config.speeds.enable_overhang_speed =
                value == "1" || value.eq_ignore_ascii_case("true");
            true
        }

        // --- P1 config gap closure: filament ---
        "filament_colour" => {
            // May be array for multi-extruder; take first value.
            let first = value.split(';').next().unwrap_or(value).trim();
            config.filament.filament_colour = first.to_string();
            true
        }

        // --- P1 config gap closure: multi-material filament indices ---
        "wall_filament" => {
            if let Ok(v) = value.parse::<usize>() {
                config.multi_material.wall_filament = if v > 0 { Some(v - 1) } else { None };
                true
            } else {
                false
            }
        }
        "solid_infill_filament" => {
            if let Ok(v) = value.parse::<usize>() {
                config.multi_material.solid_infill_filament =
                    if v > 0 { Some(v - 1) } else { None };
                true
            } else {
                false
            }
        }
        "support_filament" => {
            if let Ok(v) = value.parse::<usize>() {
                config.multi_material.support_filament =
                    if v > 0 { Some(v - 1) } else { None };
                true
            } else {
                false
            }
        }
        "support_interface_filament" => {
            if let Ok(v) = value.parse::<usize>() {
                config.multi_material.support_interface_filament =
                    if v > 0 { Some(v - 1) } else { None };
                true
            } else {
                false
            }
        }

        // --- P1 config gap closure: top-level fields ---
        "precise_outer_wall" => {
            config.precise_outer_wall =
                value == "1" || value.eq_ignore_ascii_case("true");
            true
        }
        "draft_shield" => {
            config.draft_shield = value == "1" || value.eq_ignore_ascii_case("true");
            true
        }
        "ooze_prevention" => {
            config.ooze_prevention =
                value == "1" || value.eq_ignore_ascii_case("true");
            true
        }
        "infill_combination" | "infill_every_layers" => {
            parse_and_set_u32(value, &mut config.infill_combination)
        }
        "infill_anchor_max" => parse_and_set_f64(value, &mut config.infill_anchor_max),
        "min_bead_width" => parse_and_set_f64(value, &mut config.min_bead_width),
        "min_feature_size" => parse_and_set_f64(value, &mut config.min_feature_size),

        // --- P1 config gap closure: support ---
        "support_bottom_interface_layers" => {
            parse_and_set_u32(value, &mut config.support.support_bottom_interface_layers)
        }

        // --- Support config fields (OrcaSlicer + shared keys) ---
        "enable_support" | "support_material" => {
            config.support.enabled =
                value == "1" || value.eq_ignore_ascii_case("true");
            true
        }
        "support_type" | "support_material_type" | "support_style"
        | "support_material_style" => {
            if let Some(st) = map_support_type(value) {
                config.support.support_type = st;
                true
            } else {
                false
            }
        }
        "support_threshold_angle" | "support_angle"
        | "support_material_threshold" => {
            parse_and_set_f64(value, &mut config.support.overhang_angle)
        }
        "support_base_pattern" | "support_material_pattern" => {
            if let Some(p) = map_support_pattern(value) {
                config.support.support_pattern = p;
                true
            } else {
                false
            }
        }
        "support_on_build_plate_only" | "support_material_buildplate_only" => {
            config.support.build_plate_only =
                value == "1" || value.eq_ignore_ascii_case("true");
            true
        }
        "support_top_z_distance" | "support_material_contact_distance" => {
            parse_and_set_f64(value, &mut config.support.z_gap)
        }
        "support_bottom_z_distance" | "support_material_bottom_contact_distance" => {
            if let Ok(v) = value.parse::<f64>() {
                config.support.bottom_z_gap = Some(v);
                true
            } else {
                false
            }
        }
        "support_object_xy_distance" | "support_material_xy_spacing" => {
            parse_and_set_f64(value, &mut config.support.xy_gap)
        }
        "support_interface_top_layers" | "support_material_interface_layers" => {
            parse_and_set_u32(value, &mut config.support.interface_layers)
        }
        "support_interface_bottom_layers"
        | "support_material_bottom_interface_layers" => {
            parse_and_set_u32(
                value,
                &mut config.support.support_bottom_interface_layers,
            )
        }
        "support_interface_pattern" | "support_material_interface_pattern" => {
            if let Some(p) = map_interface_pattern(value) {
                config.support.interface_pattern = p;
                true
            } else {
                false
            }
        }
        "support_base_pattern_spacing" | "support_material_spacing" => {
            // Convert spacing to density: density = line_width / spacing.
            if let Ok(spacing) = value.parse::<f64>() {
                if spacing > 0.0 {
                    let line_width = config
                        .passthrough
                        .get("line_width")
                        .or_else(|| config.passthrough.get("extrusion_width"))
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.4);
                    config.support.support_density =
                        (line_width / spacing).clamp(0.0, 1.0);
                }
                true
            } else {
                false
            }
        }
        "support_interface_spacing" | "support_material_interface_spacing" => {
            // Convert spacing to density: density = line_width / spacing.
            if let Ok(spacing) = value.parse::<f64>() {
                if spacing > 0.0 {
                    let line_width = config
                        .passthrough
                        .get("line_width")
                        .or_else(|| config.passthrough.get("extrusion_width"))
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.4);
                    config.support.interface_density =
                        (line_width / spacing).clamp(0.0, 1.0);
                } else {
                    // spacing == 0 means 100% density.
                    config.support.interface_density = 1.0;
                }
                true
            } else {
                false
            }
        }
        "support_expansion" => {
            parse_and_set_f64(value, &mut config.support.expansion)
        }
        "support_critical_regions_only" => {
            config.support.critical_regions_only =
                value == "1" || value.eq_ignore_ascii_case("true");
            true
        }
        "support_remove_small_overhang" => {
            config.support.remove_small_overhang =
                value == "1" || value.eq_ignore_ascii_case("true");
            true
        }
        "support_flow_ratio" | "support_material_flow" => {
            parse_and_set_f64(value, &mut config.support.flow_ratio)
        }
        "support_interface_flow_ratio" | "support_material_interface_flow" => {
            parse_and_set_f64(value, &mut config.support.interface_flow_ratio)
        }
        "support_material_synchronize_layers" => {
            config.support.synchronize_layers =
                value == "1" || value.eq_ignore_ascii_case("true");
            true
        }
        "enforce_support_layers" | "support_material_enforce_layers" => {
            parse_and_set_u32(value, &mut config.support.enforce_layers)
        }
        "support_closing_radius" | "support_material_closing_radius" => {
            parse_and_set_f64(value, &mut config.support.closing_radius)
        }
        "support_material_auto" => {
            // PrusaSlicer: "1" means auto-detect support type.
            if value == "1" || value.eq_ignore_ascii_case("true") {
                config.support.support_type = SupportType::Auto;
            }
            true
        }
        "raft_first_layer_expansion" => {
            parse_and_set_f64(value, &mut config.support.raft_expansion)
        }
        "support_material_with_sheath" => {
            // Store as passthrough (no direct equivalent field needed).
            config
                .passthrough
                .insert(key.to_string(), value.to_string());
            true
        }

        // --- Bridge config fields (support sub-struct) ---
        "bridge_angle" => {
            parse_and_set_f64(value, &mut config.support.bridge.angle)
        }
        "bridge_density" => {
            parse_and_set_f64(value, &mut config.support.bridge.density)
        }
        "thick_bridges" => {
            config.support.bridge.thick_bridges =
                value == "1" || value.eq_ignore_ascii_case("true");
            true
        }
        "bridge_no_support" => {
            config.support.bridge.no_support =
                value == "1" || value.eq_ignore_ascii_case("true");
            true
        }
        "bridge_fan_speed" => {
            if let Ok(v) = value.parse::<f64>() {
                config.support.bridge.fan_speed = (v.clamp(0.0, 255.0)) as u8;
                true
            } else {
                false
            }
        }

        // --- Tree support fields ---
        "tree_support_branch_angle" | "support_tree_angle" => {
            parse_and_set_f64(value, &mut config.support.tree.branch_angle)
        }
        "tree_support_branch_diameter" | "support_tree_branch_diameter" => {
            parse_and_set_f64(value, &mut config.support.tree.max_trunk_diameter)
        }
        "tree_support_tip_diameter" => {
            parse_and_set_f64(value, &mut config.support.tree.tip_diameter)
        }
        "tree_support_branch_distance" | "support_tree_top_rate" => {
            parse_and_set_f64(value, &mut config.support.tree.branch_distance)
        }
        "tree_support_branch_diameter_angle" => {
            parse_and_set_f64(value, &mut config.support.tree.branch_diameter_angle)
        }
        "tree_support_wall_count" => {
            parse_and_set_u32(value, &mut config.support.tree.wall_count)
        }
        "tree_support_auto_brim" => {
            config.support.tree.auto_brim =
                value == "1" || value.eq_ignore_ascii_case("true");
            true
        }
        "tree_support_brim_width" => {
            parse_and_set_f64(value, &mut config.support.tree.brim_width)
        }
        "tree_support_adaptive_layer_height" => {
            config.support.tree.adaptive_layer_height =
                value == "1" || value.eq_ignore_ascii_case("true");
            true
        }
        "tree_support_angle_slow" => {
            parse_and_set_f64(value, &mut config.support.tree.angle_slow)
        }
        "tree_support_top_rate" => {
            parse_and_set_f64(value, &mut config.support.tree.top_rate)
        }
        "tree_support_with_infill" => {
            config.support.tree.with_infill =
                value == "1" || value.eq_ignore_ascii_case("true");
            true
        }

        // --- Default: store unmapped fields in passthrough ---
        _ => {
            config
                .passthrough
                .insert(key.to_string(), value.to_string());
            return FieldMappingResult::Passthrough;
        }
    };

    if mapped {
        FieldMappingResult::Mapped
    } else {
        FieldMappingResult::Failed
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

/// Map an upstream surface pattern name to our `SurfacePattern` enum.
///
/// Handles OrcaSlicer and PrusaSlicer naming conventions.
pub(crate) fn map_surface_pattern(value: &str) -> Option<SurfacePattern> {
    match value.to_lowercase().as_str() {
        "rectilinear" | "zig-zag" | "line" => Some(SurfacePattern::Rectilinear),
        "monotonic" => Some(SurfacePattern::Monotonic),
        "monotonicline" | "monotonic_line" => Some(SurfacePattern::MonotonicLine),
        "concentric" => Some(SurfacePattern::Concentric),
        "hilbertcurve" | "hilbert" => Some(SurfacePattern::Hilbert),
        "archimedeanchords" | "archimedean" => Some(SurfacePattern::Archimedean),
        _ => None,
    }
}

/// Map an upstream bed type name to our `BedType` enum.
pub(crate) fn map_bed_type(value: &str) -> Option<BedType> {
    match value.to_lowercase().replace(' ', "").as_str() {
        "coolplate" | "cool_plate" => Some(BedType::CoolPlate),
        "engineeringplate" | "engineering_plate" | "epplate" => Some(BedType::EngineeringPlate),
        "hightempplate" | "high_temp_plate" | "hotplate" => Some(BedType::HighTempPlate),
        "texturedpeiplate" | "textured_pei" | "texturedpei" => Some(BedType::TexturedPei),
        "smoothpeiplate" | "smooth_pei" | "smoothpei" => Some(BedType::SmoothPei),
        "satinpeiplate" | "satin_pei" | "satinpei" => Some(BedType::SatinPei),
        _ => None,
    }
}

/// Map an upstream internal bridge mode value to our `InternalBridgeMode` enum.
pub(crate) fn map_internal_bridge_mode(value: &str) -> Option<InternalBridgeMode> {
    match value.to_lowercase().as_str() {
        "0" | "false" | "off" | "disabled" => Some(InternalBridgeMode::Off),
        "1" | "true" | "auto" => Some(InternalBridgeMode::Auto),
        "2" | "always" => Some(InternalBridgeMode::Always),
        _ => None,
    }
}

/// Map an upstream brim type name to our `BrimType` enum.
///
/// Handles OrcaSlicer and PrusaSlicer naming conventions including underscore
/// and space-separated variants.
pub(crate) fn map_brim_type(value: &str) -> Option<BrimType> {
    match value.to_lowercase().replace(' ', "").as_str() {
        "no_brim" | "nobrim" | "none" => Some(BrimType::None),
        "outer_only" | "outeronly" | "outer" => Some(BrimType::Outer),
        "inner_only" | "inneronly" | "inner" => Some(BrimType::Inner),
        "outer_and_inner" | "outerandinner" | "both" => Some(BrimType::Both),
        _ => None,
    }
}

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

/// Map an upstream support type value to our `SupportType` enum.
///
/// Handles both OrcaSlicer and PrusaSlicer naming conventions:
/// - OrcaSlicer: "none"/"disable", "normal(auto)"/"normal(manual)"/"grid", "tree(auto)"/"organic"/"tree_slim"
/// - PrusaSlicer: "0" (none), "1" (auto), "snug", "grid", "organic"
pub(crate) fn map_support_type(value: &str) -> Option<SupportType> {
    match value.to_lowercase().as_str() {
        "none" | "disable" | "0" => Some(SupportType::None),
        "auto" | "default" | "1" => Some(SupportType::Auto),
        "normal" | "normal(auto)" | "normal(manual)" | "grid" | "snug"
        | "traditional" => Some(SupportType::Traditional),
        "tree" | "tree(auto)" | "tree_slim" | "organic" => Some(SupportType::Tree),
        _ => None,
    }
}

/// Map an upstream support pattern value to our `SupportPattern` enum.
///
/// Handles both OrcaSlicer and PrusaSlicer pattern names.
pub(crate) fn map_support_pattern(value: &str) -> Option<SupportPattern> {
    match value.to_lowercase().as_str() {
        "default" | "line" => Some(SupportPattern::Line),
        "rectilinear" => Some(SupportPattern::Rectilinear),
        "grid" => Some(SupportPattern::Grid),
        "honeycomb" => Some(SupportPattern::Honeycomb),
        "lightning" => Some(SupportPattern::Lightning),
        _ => None,
    }
}

/// Map an upstream support interface pattern value to our `InterfacePattern` enum.
///
/// Handles both OrcaSlicer and PrusaSlicer interface pattern names.
pub(crate) fn map_interface_pattern(value: &str) -> Option<InterfacePattern> {
    match value.to_lowercase().as_str() {
        "default" | "rectilinear" | "auto" => Some(InterfacePattern::Rectilinear),
        "grid" => Some(InterfacePattern::Grid),
        "concentric" => Some(InterfacePattern::Concentric),
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
        assert_eq!(detect_config_format(b"\n\t  {\"a\":1}"), ConfigFormat::Json);
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
        assert!((config.speeds.perimeter - 200.0).abs() < 1e-9);
        assert!((config.speeds.travel - 500.0).abs() < 1e-9);
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

        assert!((config.filament.nozzle_temp() - 220.0).abs() < 1e-9);
        assert!((config.filament.bed_temp() - 55.0).abs() < 1e-9);
        assert!((config.filament.density - 1.24).abs() < 1e-9);
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

        assert!((config.machine.nozzle_diameter() - 0.4).abs() < 1e-9);
        assert!((config.retraction.length - 0.8).abs() < 1e-9);
        assert_eq!(config.gcode_dialect, GcodeDialect::Klipper);
    }

    #[test]
    fn test_native_json_format() {
        // Native JSON format with PrintConfig-matching field names and numeric values.
        // nozzle_diameter is now in machine sub-config.
        let json_str = r#"{
            "layer_height": 0.15,
            "machine": { "nozzle_diameters": [0.6] },
            "wall_count": 4,
            "infill_density": 0.3
        }"#;

        let config = PrintConfig::from_json(json_str).unwrap();
        assert!((config.layer_height - 0.15).abs() < 1e-9);
        assert!((config.machine.nozzle_diameter() - 0.6).abs() < 1e-9);
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
        assert_eq!(map_infill_pattern("line"), Some(InfillPattern::Rectilinear));
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
        assert!((result.config.machine.jerk_x() - 10.0).abs() < 1e-9);
        assert!((result.config.machine.jerk_y() - 10.0).abs() < 1e-9);
        assert!((result.config.machine.jerk_z() - 0.5).abs() < 1e-9);
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

        assert!((config.filament.nozzle_temp() - 220.0).abs() < 1e-9);
        assert!((config.filament.first_layer_nozzle_temp() - 225.0).abs() < 1e-9);
        assert!((config.filament.density - 1.24).abs() < 1e-9);
        assert!((config.filament.diameter - 1.75).abs() < 1e-9);
        assert!((config.extrusion_multiplier - 0.98).abs() < 1e-9);
        assert!((config.filament.cost_per_kg - 20.0).abs() < 1e-9);
        assert_eq!(config.cooling.disable_fan_first_layers, 1);
        assert!((config.filament.bed_temp() - 55.0).abs() < 1e-9);
        assert!((config.filament.first_layer_bed_temp() - 60.0).abs() < 1e-9);
        assert!((config.cooling.fan_below_layer_time - 30.0).abs() < 1e-9);
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
        assert!((config.speeds.perimeter - 200.0).abs() < 1e-9);
        assert!((config.speeds.infill - 270.0).abs() < 1e-9);
        assert!((config.speeds.travel - 500.0).abs() < 1e-9);
        assert!((config.speeds.first_layer - 50.0).abs() < 1e-9);
        assert_eq!(config.skirt_loops, 1);
        assert!((config.skirt_distance - 2.0).abs() < 1e-9);
        assert!((config.brim_width - 0.0).abs() < 1e-9);
        assert_eq!(config.seam_position, SeamPosition::Aligned);
        assert!((config.accel.print - 10000.0).abs() < 1e-9);
        assert!((config.accel.travel - 12000.0).abs() < 1e-9);
        assert!(config.arc_fitting_enabled);
        assert!(!config.adaptive_layer_height);
        assert!(config.arachne_enabled);
    }

    // ========================================================================
    // Phase 20 Plan 02: Expanded field mapping tests
    // ========================================================================

    #[test]
    fn test_extract_array_f64_string_array() {
        let val = json!(["0.4", "0.6"]);
        assert_eq!(extract_array_f64(&val), vec![0.4, 0.6]);
    }

    #[test]
    fn test_extract_array_f64_number_array() {
        let val = json!([0.4, 0.6]);
        assert_eq!(extract_array_f64(&val), vec![0.4, 0.6]);
    }

    #[test]
    fn test_extract_array_f64_single_string() {
        let val = json!("0.4");
        assert_eq!(extract_array_f64(&val), vec![0.4]);
    }

    #[test]
    fn test_extract_array_f64_single_number() {
        let val = json!(0.4);
        assert_eq!(extract_array_f64(&val), vec![0.4]);
    }

    #[test]
    fn test_extract_array_f64_nil() {
        let val = json!("nil");
        assert!(extract_array_f64(&val).is_empty());

        let val = json!(["nil"]);
        assert!(extract_array_f64(&val).is_empty());
    }

    #[test]
    fn test_extract_array_f64_mixed_with_nil() {
        // Array with some nil entries -- only valid values extracted.
        let val = json!(["0.4", "nil", "0.6"]);
        assert_eq!(extract_array_f64(&val), vec![0.4, 0.6]);
    }

    #[test]
    fn test_extract_array_f64_null() {
        let val = json!(null);
        assert!(extract_array_f64(&val).is_empty());
    }

    #[test]
    fn test_speed_fields_mapping() {
        let json_val = json!({
            "type": "process",
            "name": "Speed Test",
            "bridge_speed": "50",
            "inner_wall_speed": "300",
            "gap_infill_speed": "200",
            "top_surface_speed": "100",
            "internal_solid_infill_speed": "250",
            "initial_layer_infill_speed": "40",
            "support_speed": "150",
            "support_interface_speed": "80",
            "small_perimeter_speed": "50%",
            "solid_infill_speed": "200",
            "overhang_1_4_speed": "60",
            "overhang_2_4_speed": "40",
            "overhang_3_4_speed": "25",
            "overhang_4_4_speed": "15",
            "travel_speed_z": "12"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        assert!((config.speeds.bridge - 50.0).abs() < 1e-9);
        assert!((config.speeds.inner_wall - 300.0).abs() < 1e-9);
        assert!((config.speeds.gap_fill - 200.0).abs() < 1e-9);
        assert!((config.speeds.top_surface - 100.0).abs() < 1e-9);
        assert!((config.speeds.internal_solid_infill - 250.0).abs() < 1e-9);
        assert!((config.speeds.initial_layer_infill - 40.0).abs() < 1e-9);
        assert!((config.speeds.support - 150.0).abs() < 1e-9);
        assert!((config.speeds.support_interface - 80.0).abs() < 1e-9);
        assert!((config.speeds.small_perimeter - 50.0).abs() < 1e-9);
        assert!((config.speeds.solid_infill - 200.0).abs() < 1e-9);
        assert!((config.speeds.overhang_1_4 - 60.0).abs() < 1e-9);
        assert!((config.speeds.overhang_2_4 - 40.0).abs() < 1e-9);
        assert!((config.speeds.overhang_3_4 - 25.0).abs() < 1e-9);
        assert!((config.speeds.overhang_4_4 - 15.0).abs() < 1e-9);
        assert!((config.speeds.travel_z - 12.0).abs() < 1e-9);

        // All should be mapped.
        assert!(result.mapped_fields.contains(&"bridge_speed".to_string()));
        assert!(result
            .mapped_fields
            .contains(&"gap_infill_speed".to_string()));
    }

    #[test]
    fn test_line_width_fields_mapping() {
        let json_val = json!({
            "type": "process",
            "name": "Width Test",
            "outer_wall_line_width": "0.42",
            "inner_wall_line_width": "0.45",
            "sparse_infill_line_width": "0.50",
            "top_surface_line_width": "0.40",
            "initial_layer_line_width": "0.55",
            "internal_solid_infill_line_width": "0.42",
            "support_line_width": "0.38"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        assert!((config.line_widths.outer_wall - 0.42).abs() < 1e-9);
        assert!((config.line_widths.inner_wall - 0.45).abs() < 1e-9);
        assert!((config.line_widths.infill - 0.50).abs() < 1e-9);
        assert!((config.line_widths.top_surface - 0.40).abs() < 1e-9);
        assert!((config.line_widths.initial_layer - 0.55).abs() < 1e-9);
        assert!((config.line_widths.internal_solid_infill - 0.42).abs() < 1e-9);
        assert!((config.line_widths.support - 0.38).abs() < 1e-9);
    }

    #[test]
    fn test_machine_gcode_string_fields() {
        let json_val = json!({
            "type": "machine",
            "name": "GCode Test",
            "machine_start_gcode": "G28 ; home all\\nG1 Z5 F3000",
            "machine_end_gcode": "M104 S0\\nM140 S0",
            "layer_change_gcode": ";LAYER_CHANGE",
            "nozzle_type": "hardened_steel",
            "printer_model": "X1Carbon",
            "bed_shape": "0x0,256x0,256x256,0x256"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        assert!(config.machine.start_gcode.contains("G28"));
        assert!(config.machine.end_gcode.contains("M104"));
        assert!(config.machine.layer_change_gcode.contains("LAYER_CHANGE"));
        assert_eq!(config.machine.nozzle_type, "hardened_steel");
        assert_eq!(config.machine.printer_model, "X1Carbon");
        assert!(config.machine.bed_shape.contains("256"));
    }

    #[test]
    fn test_cooling_fields_mapping() {
        let json_val = json!({
            "type": "process",
            "name": "Cooling Test",
            "fan_max_speed": "80%",
            "fan_min_speed": "20%",
            "slow_down_layer_time": "10",
            "slow_down_min_speed": "15",
            "overhang_fan_speed": "100%",
            "overhang_fan_threshold": "30",
            "full_fan_speed_layer": "3",
            "slow_down_for_layer_cooling": "1"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        assert!((config.cooling.fan_max_speed - 80.0).abs() < 1e-9);
        assert!((config.cooling.fan_min_speed - 20.0).abs() < 1e-9);
        assert!((config.cooling.slow_down_layer_time - 10.0).abs() < 1e-9);
        assert!((config.cooling.slow_down_min_speed - 15.0).abs() < 1e-9);
        assert!((config.cooling.overhang_fan_speed - 100.0).abs() < 1e-9);
        assert!((config.cooling.overhang_fan_threshold - 30.0).abs() < 1e-9);
        assert_eq!(config.cooling.full_fan_speed_layer, 3);
        assert!(config.cooling.slow_down_for_layer_cooling);
    }

    #[test]
    fn test_retraction_fields_mapping() {
        let json_val = json!({
            "type": "machine",
            "name": "Retraction Test",
            "deretraction_speed": "30",
            "retract_before_wipe": "70%",
            "retract_when_changing_layer": "1",
            "wipe": "1",
            "wipe_distance": "2.0"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        assert!((config.retraction.deretraction_speed - 30.0).abs() < 1e-9);
        assert!((config.retraction.retract_before_wipe - 70.0).abs() < 1e-9);
        assert!(config.retraction.retract_when_changing_layer);
        assert!(config.retraction.wipe);
        assert!((config.retraction.wipe_distance - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_acceleration_fields_mapping() {
        let json_val = json!({
            "type": "process",
            "name": "Accel Test",
            "outer_wall_acceleration": "5000",
            "inner_wall_acceleration": "10000",
            "initial_layer_acceleration": "500",
            "initial_layer_travel_acceleration": "1000",
            "top_surface_acceleration": "2000",
            "sparse_infill_acceleration": "10000",
            "bridge_acceleration": "1000"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        assert!((config.accel.outer_wall - 5000.0).abs() < 1e-9);
        assert!((config.accel.inner_wall - 10000.0).abs() < 1e-9);
        assert!((config.accel.initial_layer - 500.0).abs() < 1e-9);
        assert!((config.accel.initial_layer_travel - 1000.0).abs() < 1e-9);
        assert!((config.accel.top_surface - 2000.0).abs() < 1e-9);
        assert!((config.accel.sparse_infill - 10000.0).abs() < 1e-9);
        assert!((config.accel.bridge - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn test_machine_acceleration_and_speed_fields() {
        let json_val = json!({
            "type": "machine",
            "name": "Machine Limits Test",
            "printable_height": "300",
            "machine_max_acceleration_x": "10000",
            "machine_max_acceleration_y": "10000",
            "machine_max_acceleration_z": "200",
            "machine_max_acceleration_e": "5000",
            "machine_max_acceleration_extruding": "20000",
            "machine_max_acceleration_retracting": "5000",
            "machine_max_acceleration_travel": "12000",
            "machine_max_speed_x": "500",
            "machine_max_speed_y": "500",
            "machine_max_speed_z": "20",
            "machine_max_speed_e": "30",
            "min_layer_height": "0.04",
            "max_layer_height": "0.32"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        assert!((config.machine.printable_height - 300.0).abs() < 1e-9);
        assert!((config.machine.max_acceleration_x - 10000.0).abs() < 1e-9);
        assert!((config.machine.max_acceleration_y - 10000.0).abs() < 1e-9);
        assert!((config.machine.max_acceleration_z - 200.0).abs() < 1e-9);
        assert!((config.machine.max_acceleration_e - 5000.0).abs() < 1e-9);
        assert!((config.machine.max_acceleration_extruding - 20000.0).abs() < 1e-9);
        assert!((config.machine.max_acceleration_retracting - 5000.0).abs() < 1e-9);
        assert!((config.machine.max_acceleration_travel - 12000.0).abs() < 1e-9);
        assert!((config.machine.max_speed_x - 500.0).abs() < 1e-9);
        assert!((config.machine.max_speed_y - 500.0).abs() < 1e-9);
        assert!((config.machine.max_speed_z - 20.0).abs() < 1e-9);
        assert!((config.machine.max_speed_e - 30.0).abs() < 1e-9);
        assert!((config.machine.min_layer_height - 0.04).abs() < 1e-9);
        assert!((config.machine.max_layer_height - 0.32).abs() < 1e-9);
    }

    #[test]
    fn test_filament_sub_config_fields() {
        let json_val = json!({
            "type": "filament",
            "name": "PLA Test",
            "filament_type": "PLA",
            "filament_vendor": "eSUN",
            "filament_max_volumetric_speed": "15",
            "nozzle_temperature_range_low": "190",
            "nozzle_temperature_range_high": "230",
            "filament_retraction_length": "0.8",
            "filament_retraction_speed": "30",
            "filament_start_gcode": "M900 K0.04",
            "filament_end_gcode": "M900 K0"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        assert_eq!(config.filament.filament_type, "PLA");
        assert_eq!(config.filament.filament_vendor, "eSUN");
        assert!((config.filament.max_volumetric_speed - 15.0).abs() < 1e-9);
        assert!((config.filament.nozzle_temperature_range_low - 190.0).abs() < 1e-9);
        assert!((config.filament.nozzle_temperature_range_high - 230.0).abs() < 1e-9);
        assert_eq!(config.filament.filament_retraction_length, Some(0.8));
        assert_eq!(config.filament.filament_retraction_speed, Some(30.0));
        assert_eq!(config.filament.filament_start_gcode, "M900 K0.04");
        assert_eq!(config.filament.filament_end_gcode, "M900 K0");
    }

    #[test]
    fn test_process_misc_fields() {
        let json_val = json!({
            "type": "process",
            "name": "Misc Test",
            "bridge_flow": "0.95",
            "elefant_foot_compensation": "0.1",
            "infill_direction": "45",
            "infill_wall_overlap": "15%",
            "spiral_mode": "1",
            "only_one_wall_top": "1",
            "resolution": "0.015",
            "raft_layers": "2",
            "detect_thin_wall": "1"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        assert!((config.bridge_flow - 0.95).abs() < 1e-9);
        assert!((config.dimensional_compensation.elephant_foot_compensation - 0.1).abs() < 1e-9);
        assert!((config.infill_direction - 45.0).abs() < 1e-9);
        assert!((config.infill_wall_overlap - 0.15).abs() < 1e-9);
        assert!(config.spiral_mode);
        assert!(config.only_one_wall_top);
        assert!((config.resolution - 0.015).abs() < 1e-9);
        assert_eq!(config.raft_layers, 2);
        assert!(config.detect_thin_wall);
    }

    #[test]
    fn test_unknown_fields_go_to_passthrough() {
        let json_val = json!({
            "type": "process",
            "name": "Passthrough Test",
            "layer_height": "0.2",
            "ams_drying_temperature": "55",
            "scan_first_layer": "1",
            "timelapse_gcode": "M400\nM971"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        // Known field mapped normally.
        assert!((config.layer_height - 0.2).abs() < 1e-9);
        assert!(result.mapped_fields.contains(&"layer_height".to_string()));

        // Unknown fields stored in passthrough.
        assert_eq!(
            config.passthrough.get("ams_drying_temperature").unwrap(),
            "55"
        );
        assert_eq!(config.passthrough.get("scan_first_layer").unwrap(), "1");
        assert!(config
            .passthrough
            .get("timelapse_gcode")
            .unwrap()
            .contains("M400"));

        // Passthrough fields tracked in passthrough_fields.
        assert!(result
            .passthrough_fields
            .contains(&"ams_drying_temperature".to_string()));
        assert!(result
            .passthrough_fields
            .contains(&"scan_first_layer".to_string()));
        assert!(result
            .passthrough_fields
            .contains(&"timelapse_gcode".to_string()));
    }

    #[test]
    fn test_nozzle_diameter_array_populates_vec() {
        let json_val = json!({
            "type": "machine",
            "name": "Dual Extruder",
            "nozzle_diameter": ["0.4", "0.6"]
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        assert_eq!(config.machine.nozzle_diameters, vec![0.4, 0.6]);
        // Accessor returns first element.
        assert!((config.machine.nozzle_diameter() - 0.4).abs() < 1e-9);
    }

    #[test]
    fn test_temperature_arrays_populate_vecs() {
        let json_val = json!({
            "type": "filament",
            "name": "Temp Array Test",
            "nozzle_temperature": ["220", "230"],
            "bed_temperature": ["60", "70"],
            "nozzle_temperature_initial_layer": ["225", "235"],
            "bed_temperature_initial_layer": ["65", "75"]
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        assert_eq!(config.filament.nozzle_temperatures, vec![220.0, 230.0]);
        assert_eq!(config.filament.bed_temperatures, vec![60.0, 70.0]);
        assert_eq!(
            config.filament.first_layer_nozzle_temperatures,
            vec![225.0, 235.0]
        );
        assert_eq!(
            config.filament.first_layer_bed_temperatures,
            vec![65.0, 75.0]
        );

        // Accessor returns first element.
        assert!((config.filament.nozzle_temp() - 220.0).abs() < 1e-9);
        assert!((config.filament.bed_temp() - 60.0).abs() < 1e-9);
        assert!((config.filament.first_layer_nozzle_temp() - 225.0).abs() < 1e-9);
        assert!((config.filament.first_layer_bed_temp() - 65.0).abs() < 1e-9);
    }

    #[test]
    fn test_jerk_arrays_populate_vecs() {
        let json_val = json!({
            "type": "machine",
            "name": "Jerk Array Test",
            "machine_max_jerk_x": ["9", "7"],
            "machine_max_jerk_y": ["9", "7"],
            "machine_max_jerk_z": ["0.4", "0.3"],
            "machine_max_jerk_e": ["2.5", "2.0"]
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        assert_eq!(config.machine.jerk_values_x, vec![9.0, 7.0]);
        assert_eq!(config.machine.jerk_values_y, vec![9.0, 7.0]);
        assert_eq!(config.machine.jerk_values_z, vec![0.4, 0.3]);
        assert_eq!(config.machine.jerk_values_e, vec![2.5, 2.0]);

        // Accessor returns first element.
        assert!((config.machine.jerk_x() - 9.0).abs() < 1e-9);
        assert!((config.machine.jerk_y() - 9.0).abs() < 1e-9);
        assert!((config.machine.jerk_z() - 0.4).abs() < 1e-9);
    }

    #[test]
    fn test_overhang_speed_aliases() {
        // Test alternative key names (overhang_speed_0/1/2/3).
        let json_val = json!({
            "type": "process",
            "name": "Overhang Alias Test",
            "overhang_speed_0": "60",
            "overhang_speed_1": "40",
            "overhang_speed_2": "25",
            "overhang_speed_3": "15"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let config = &result.config;

        assert!((config.speeds.overhang_1_4 - 60.0).abs() < 1e-9);
        assert!((config.speeds.overhang_2_4 - 40.0).abs() < 1e-9);
        assert!((config.speeds.overhang_3_4 - 25.0).abs() < 1e-9);
        assert!((config.speeds.overhang_4_4 - 15.0).abs() < 1e-9);
    }

    #[test]
    fn test_match_arm_count_exceeds_100() {
        // Build a large JSON profile programmatically to avoid macro recursion limit.
        use serde_json::Map;
        let mut obj = Map::new();
        obj.insert("type".into(), json!("process"));
        obj.insert("name".into(), json!("100+ Fields Test"));

        // Process basics (19)
        for key in &[
            "layer_height",
            "initial_layer_print_height",
            "wall_loops",
            "sparse_infill_density",
            "top_shell_layers",
            "bottom_shell_layers",
            "outer_wall_speed",
            "sparse_infill_speed",
            "travel_speed",
            "initial_layer_speed",
            "skirt_loops",
            "skirt_distance",
            "brim_width",
            "default_acceleration",
            "travel_acceleration",
            "enable_arc_fitting",
            "adaptive_layer_height",
            "wall_generator",
            "seam_position",
        ] {
            obj.insert((*key).into(), json!("10"));
        }
        // Speed sub-config (15)
        for key in &[
            "bridge_speed",
            "inner_wall_speed",
            "gap_infill_speed",
            "top_surface_speed",
            "internal_solid_infill_speed",
            "initial_layer_infill_speed",
            "support_speed",
            "support_interface_speed",
            "small_perimeter_speed",
            "solid_infill_speed",
            "overhang_1_4_speed",
            "overhang_2_4_speed",
            "overhang_3_4_speed",
            "overhang_4_4_speed",
            "travel_speed_z",
        ] {
            obj.insert((*key).into(), json!("50"));
        }
        // Line widths (7)
        for key in &[
            "outer_wall_line_width",
            "inner_wall_line_width",
            "sparse_infill_line_width",
            "top_surface_line_width",
            "initial_layer_line_width",
            "internal_solid_infill_line_width",
            "support_line_width",
        ] {
            obj.insert((*key).into(), json!("0.45"));
        }
        // Cooling (8)
        for key in &[
            "fan_max_speed",
            "fan_min_speed",
            "slow_down_layer_time",
            "slow_down_min_speed",
            "overhang_fan_speed",
            "overhang_fan_threshold",
            "full_fan_speed_layer",
            "slow_down_for_layer_cooling",
        ] {
            obj.insert((*key).into(), json!("10"));
        }
        // Retraction (5)
        for key in &[
            "deretraction_speed",
            "retract_before_wipe",
            "retract_when_changing_layer",
            "wipe",
            "wipe_distance",
        ] {
            obj.insert((*key).into(), json!("1"));
        }
        // Machine limits (14)
        for key in &[
            "printable_height",
            "machine_max_acceleration_x",
            "machine_max_acceleration_y",
            "machine_max_acceleration_z",
            "machine_max_acceleration_e",
            "machine_max_acceleration_extruding",
            "machine_max_acceleration_retracting",
            "machine_max_acceleration_travel",
            "machine_max_speed_x",
            "machine_max_speed_y",
            "machine_max_speed_z",
            "machine_max_speed_e",
            "min_layer_height",
            "max_layer_height",
        ] {
            obj.insert((*key).into(), json!("100"));
        }
        // Acceleration (7)
        for key in &[
            "outer_wall_acceleration",
            "inner_wall_acceleration",
            "initial_layer_acceleration",
            "initial_layer_travel_acceleration",
            "top_surface_acceleration",
            "sparse_infill_acceleration",
            "bridge_acceleration",
        ] {
            obj.insert((*key).into(), json!("5000"));
        }
        // Process misc (9)
        for key in &[
            "bridge_flow",
            "elefant_foot_compensation",
            "infill_direction",
            "infill_wall_overlap",
            "resolution",
            "spiral_mode",
            "only_one_wall_top",
            "raft_layers",
            "detect_thin_wall",
        ] {
            obj.insert((*key).into(), json!("0"));
        }
        // Ironing (4)
        obj.insert("ironing_type".into(), json!("top"));
        obj.insert("ironing_flow".into(), json!("15%"));
        obj.insert("ironing_speed".into(), json!("20"));
        obj.insert("ironing_spacing".into(), json!("0.1"));
        // Machine strings (4)
        obj.insert("machine_start_gcode".into(), json!("G28"));
        obj.insert("machine_end_gcode".into(), json!("M84"));
        obj.insert("nozzle_type".into(), json!("brass"));
        obj.insert("printer_model".into(), json!("TestPrinter"));
        // Retraction + dialect (5)
        obj.insert("retraction_length".into(), json!("0.8"));
        obj.insert("retraction_speed".into(), json!("45"));
        obj.insert("z_hop".into(), json!("0.3"));
        obj.insert("retraction_minimum_travel".into(), json!("1.5"));
        obj.insert("gcode_flavor".into(), json!("klipper"));
        // Filament (9)
        obj.insert("filament_type".into(), json!("PLA"));
        obj.insert("filament_vendor".into(), json!("eSUN"));
        obj.insert("filament_density".into(), json!("1.24"));
        obj.insert("filament_diameter".into(), json!("1.75"));
        obj.insert("filament_cost".into(), json!("20"));
        obj.insert("filament_flow_ratio".into(), json!("0.98"));
        obj.insert("filament_max_volumetric_speed".into(), json!("15"));
        obj.insert("filament_retraction_length".into(), json!("0.8"));
        obj.insert("filament_retraction_speed".into(), json!("30"));
        // Array fields (7)
        obj.insert("nozzle_diameter".into(), json!(["0.4"]));
        obj.insert("nozzle_temperature".into(), json!(["220"]));
        obj.insert("hot_plate_temp".into(), json!(["60"]));
        obj.insert("machine_max_jerk_x".into(), json!(["9"]));
        obj.insert("machine_max_jerk_y".into(), json!(["9"]));
        obj.insert("machine_max_jerk_z".into(), json!(["0.4"]));
        obj.insert("machine_max_jerk_e".into(), json!(["2.5"]));

        let json_val = serde_json::Value::Object(obj);
        let result = import_upstream_profile(&json_val).unwrap();

        // Should have at least 100 mapped fields.
        assert!(
            result.mapped_fields.len() >= 100,
            "Expected at least 100 mapped fields, got {}: {:?}",
            result.mapped_fields.len(),
            result.mapped_fields
        );
    }
}
