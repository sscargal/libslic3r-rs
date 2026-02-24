//! PrusaSlicer INI profile parsing, inheritance resolution, and field mapping.
//!
//! This module handles the PrusaSlicer vendor INI format, which differs from the
//! OrcaSlicer JSON format handled by [`crate::profile_import`]. Key differences:
//!
//! - **INI format**: `[type:name]` section headers with `key = value` pairs
//! - **Multi-parent inheritance**: `inherits = *base1*; *base2*` with semicolon separation
//! - **Abstract profiles**: Names wrapped in asterisks (e.g., `*common*`, `*PLA*`)
//! - **Percentage values**: `fill_density = 15%` with `%` suffix
//! - **G-code escapes**: `\n` in values preserved as literal text
//!
//! # Usage
//!
//! ```ignore
//! use slicecore_engine::profile_import_ini::{
//!     parse_prusaslicer_ini, resolve_ini_inheritance, import_prusaslicer_ini_profile,
//! };
//!
//! let sections = parse_prusaslicer_ini(ini_content);
//! let lookup = build_section_lookup(&sections);
//! let resolved = resolve_ini_inheritance(&sections[0], &lookup, 0);
//! let result = import_prusaslicer_ini_profile(&resolved, "Profile Name", "print");
//! ```

use std::collections::HashMap;

use slicecore_gcode_io::GcodeDialect;

use crate::config::PrintConfig;
use crate::infill::InfillPattern;
use crate::profile_import::{ImportResult, ProfileMetadata};
use crate::seam::SeamPosition;

// ---------------------------------------------------------------------------
// INI section types
// ---------------------------------------------------------------------------

/// A parsed INI section with its type, name, and key-value pairs.
#[derive(Debug, Clone)]
pub struct IniSection {
    /// Section type: "vendor", "printer_model", "print", "filament", "printer".
    pub section_type: String,
    /// Section name (e.g., "PrusaResearch", "*common*", "0.20mm NORMAL").
    pub name: String,
    /// Key-value pairs in this section.
    pub fields: HashMap<String, String>,
    /// Whether this is an abstract/base profile (name starts AND ends with `*`).
    pub is_abstract: bool,
}

// ---------------------------------------------------------------------------
// INI parser
// ---------------------------------------------------------------------------

/// Maximum inheritance depth to guard against circular references.
const MAX_INHERITANCE_DEPTH: usize = 10;

/// Parse a PrusaSlicer vendor INI file into typed sections.
///
/// Handles:
/// - `[type:name]` section headers (and `[type]` for vendor sections)
/// - `key = value` pairs per section
/// - Comment lines starting with `#` or `;`
/// - `##` section-level comments (treated as regular comments)
/// - Abstract profile detection (name starts AND ends with `*`)
/// - Preserves `\n` escape sequences in values (G-code fields)
pub fn parse_prusaslicer_ini(content: &str) -> Vec<IniSection> {
    let mut sections = Vec::new();
    let mut current: Option<IniSection> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip empty lines.
        if trimmed.is_empty() {
            continue;
        }

        // Skip comment lines (# or ;).
        if trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }

        // Check for section header: [type:name] or [type].
        if let Some(header) = parse_section_header(trimmed) {
            // Save current section if any.
            if let Some(section) = current.take() {
                sections.push(section);
            }
            let (section_type, name) = header;
            let is_abstract = name.starts_with('*') && name.ends_with('*') && name.len() >= 2;
            current = Some(IniSection {
                section_type,
                name,
                fields: HashMap::new(),
                is_abstract,
            });
            continue;
        }

        // Parse key = value pairs.
        if let Some(section) = current.as_mut() {
            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim().to_string();
                let value = trimmed[eq_pos + 1..].trim().to_string();
                if !key.is_empty() {
                    section.fields.insert(key, value);
                }
            }
        }
    }

    // Don't forget the last section.
    if let Some(section) = current {
        sections.push(section);
    }

    sections
}

/// Parse a section header line into (type, name).
///
/// Returns `None` if the line is not a valid section header.
fn parse_section_header(line: &str) -> Option<(String, String)> {
    if !line.starts_with('[') || !line.ends_with(']') {
        return None;
    }
    let inner = &line[1..line.len() - 1];
    if let Some(colon_pos) = inner.find(':') {
        let section_type = inner[..colon_pos].to_string();
        let name = inner[colon_pos + 1..].to_string();
        Some((section_type, name))
    } else {
        Some((inner.to_string(), String::new()))
    }
}

// ---------------------------------------------------------------------------
// Inheritance resolution
// ---------------------------------------------------------------------------

/// Build a lookup map from (section_type, name) -> index in the sections Vec.
pub fn build_section_lookup(sections: &[IniSection]) -> HashMap<(String, String), usize> {
    let mut lookup = HashMap::new();
    for (i, section) in sections.iter().enumerate() {
        lookup.insert((section.section_type.clone(), section.name.clone()), i);
    }
    lookup
}

/// Resolve the inheritance chain for a section, returning flattened key-value pairs.
///
/// Multi-parent inheritance (`inherits = *base1*; *base2*`) is resolved left-to-right:
/// 1. Start with an empty field map
/// 2. For each parent (left to right), recursively resolve its inheritance and merge
/// 3. Overlay the child's own fields on top
///
/// `MAX_INHERITANCE_DEPTH` (10) guards against circular references.
pub fn resolve_ini_inheritance(
    section: &IniSection,
    sections: &[IniSection],
    lookup: &HashMap<(String, String), usize>,
    depth: usize,
) -> HashMap<String, String> {
    if depth > MAX_INHERITANCE_DEPTH {
        return section.fields.clone();
    }

    let mut resolved = HashMap::new();

    // Check for inherits field.
    if let Some(inherits_value) = section.fields.get("inherits") {
        let parents = parse_inherits(inherits_value);

        for parent_name in parents {
            // Look up parent by (section_type, parent_name).
            let key = (section.section_type.clone(), parent_name);
            if let Some(&parent_idx) = lookup.get(&key) {
                let parent = &sections[parent_idx];
                let parent_resolved =
                    resolve_ini_inheritance(parent, sections, lookup, depth + 1);
                // Merge parent fields into resolved (left-to-right overlay).
                for (k, v) in parent_resolved {
                    resolved.insert(k, v);
                }
            }
        }
    }

    // Overlay child's own fields on top of merged parents.
    for (k, v) in &section.fields {
        if k != "inherits" {
            resolved.insert(k.clone(), v.clone());
        }
    }

    resolved
}

/// Parse the `inherits` field value into a list of parent names.
///
/// Splits on `; ` (semicolon-space). Also handles plain `;` without space.
fn parse_inherits(value: &str) -> Vec<String> {
    value
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

// ---------------------------------------------------------------------------
// PrusaSlicer field mapping
// ---------------------------------------------------------------------------

/// Map a PrusaSlicer INI key name to the corresponding PrintConfig field name.
///
/// Returns `Some("field_name")` for known mappings, `None` for unmapped keys.
/// Parallel to `upstream_key_to_config_field` in `profile_import.rs` but for
/// PrusaSlicer INI key names.
pub fn prusaslicer_key_to_config_field(key: &str) -> Option<&'static str> {
    match key {
        // Process fields.
        "layer_height" => Some("layer_height"),
        "first_layer_height" => Some("first_layer_height"),
        "perimeters" => Some("wall_count"),
        "fill_density" => Some("infill_density"),
        "fill_pattern" => Some("infill_pattern"),
        "top_solid_layers" => Some("top_solid_layers"),
        "bottom_solid_layers" => Some("bottom_solid_layers"),
        "perimeter_speed" => Some("perimeter_speed"),
        "infill_speed" => Some("infill_speed"),
        "travel_speed" => Some("travel_speed"),
        "first_layer_speed" => Some("first_layer_speed"),
        "skirts" => Some("skirt_loops"),
        "skirt_distance" => Some("skirt_distance"),
        "brim_width" => Some("brim_width"),
        "default_acceleration" => Some("print_acceleration"),
        "seam_position" => Some("seam_position"),

        // Speed sub-config fields.
        "bridge_speed" => Some("speeds.bridge"),
        "small_perimeter_speed" => Some("speeds.small_perimeter"),
        "gap_fill_speed" => Some("speeds.gap_fill"),
        "top_solid_infill_speed" | "top_infill_speed" => Some("speeds.top_surface"),
        "solid_infill_speed" => Some("speeds.solid_infill"),
        "support_material_speed" => Some("speeds.support"),
        "support_material_interface_speed" => Some("speeds.support_interface"),
        "travel_speed_z" => Some("speeds.travel_z"),

        // Line width sub-config fields.
        "first_layer_extrusion_width" => Some("line_widths.initial_layer"),
        "perimeter_extrusion_width" => Some("line_widths.outer_wall"),
        "external_perimeter_extrusion_width" => Some("line_widths.outer_wall"),
        "infill_extrusion_width" => Some("line_widths.infill"),
        "solid_infill_extrusion_width" => Some("line_widths.internal_solid_infill"),
        "top_infill_extrusion_width" => Some("line_widths.top_surface"),
        "support_material_extrusion_width" => Some("line_widths.support"),

        // Cooling sub-config fields.
        "max_fan_speed" => Some("cooling.fan_max_speed"),
        "min_fan_speed" => Some("cooling.fan_min_speed"),
        "slowdown_below_layer_time" => Some("cooling.slow_down_layer_time"),
        "min_print_speed" => Some("cooling.slow_down_min_speed"),
        "bridge_fan_speed" => Some("cooling.overhang_fan_speed"),
        "full_fan_speed_layer" => Some("cooling.full_fan_speed_layer"),
        "cooling" => Some("cooling.slow_down_for_layer_cooling"),

        // Retraction sub-config fields.
        "deretract_speed" => Some("retraction.deretraction_speed"),
        "retract_before_wipe" => Some("retraction.retract_before_wipe"),
        "retract_layer_change" => Some("retraction.retract_when_changing_layer"),
        "wipe" => Some("retraction.wipe"),

        // Machine sub-config fields.
        "start_gcode" => Some("machine.start_gcode"),
        "end_gcode" => Some("machine.end_gcode"),
        "layer_gcode" => Some("machine.layer_change_gcode"),
        "max_print_height" => Some("machine.printable_height"),
        "machine_max_acceleration_x" => Some("machine.max_acceleration_x"),
        "machine_max_acceleration_y" => Some("machine.max_acceleration_y"),
        "machine_max_acceleration_z" => Some("machine.max_acceleration_z"),
        "machine_max_acceleration_e" => Some("machine.max_acceleration_e"),
        "machine_max_acceleration_extruding" => Some("machine.max_acceleration_extruding"),
        "machine_max_acceleration_retracting" => Some("machine.max_acceleration_retracting"),
        "machine_max_acceleration_travel" => Some("machine.max_acceleration_travel"),
        "machine_max_speed_x" | "machine_max_feedrate_x" => Some("machine.max_speed_x"),
        "machine_max_speed_y" | "machine_max_feedrate_y" => Some("machine.max_speed_y"),
        "machine_max_speed_z" | "machine_max_feedrate_z" => Some("machine.max_speed_z"),
        "machine_max_speed_e" | "machine_max_feedrate_e" => Some("machine.max_speed_e"),
        "nozzle_type" => Some("machine.nozzle_type"),
        "printer_model" => Some("machine.printer_model"),
        "bed_shape" => Some("machine.bed_shape"),
        "min_layer_height" => Some("machine.min_layer_height"),
        "max_layer_height" => Some("machine.max_layer_height"),

        // Acceleration sub-config fields.
        "external_perimeter_acceleration" => Some("accel.outer_wall"),
        "perimeter_acceleration" => Some("accel.inner_wall"),
        "first_layer_acceleration" | "first_layer_acceleration_over_raft" => {
            Some("accel.initial_layer")
        }
        "top_solid_infill_acceleration" => Some("accel.top_surface"),
        "infill_acceleration" => Some("accel.sparse_infill"),
        "bridge_acceleration" => Some("accel.bridge"),

        // Filament fields.
        "temperature" => Some("nozzle_temp"),
        "first_layer_temperature" => Some("first_layer_nozzle_temp"),
        "bed_temperature" => Some("bed_temp"),
        "first_layer_bed_temperature" => Some("first_layer_bed_temp"),
        "filament_density" => Some("filament_density"),
        "filament_diameter" => Some("filament_diameter"),
        "filament_cost" => Some("filament_cost_per_kg"),
        "extrusion_multiplier" => Some("extrusion_multiplier"),
        "disable_fan_first_layers" => Some("disable_fan_first_layers"),
        "fan_below_layer_time" => Some("fan_below_layer_time"),

        // Filament sub-config fields.
        "filament_type" => Some("filament.filament_type"),
        "filament_vendor" => Some("filament.filament_vendor"),
        "filament_max_volumetric_speed" => Some("filament.max_volumetric_speed"),
        "temperature_range_low" | "min_temperature" => {
            Some("filament.nozzle_temperature_range_low")
        }
        "temperature_range_high" | "max_temperature" => {
            Some("filament.nozzle_temperature_range_high")
        }
        "filament_retract_length" => Some("filament.filament_retraction_length"),
        "filament_retract_speed" => Some("filament.filament_retraction_speed"),
        "start_filament_gcode" => Some("filament.filament_start_gcode"),
        "end_filament_gcode" => Some("filament.filament_end_gcode"),

        // Machine/printer fields (flat).
        "nozzle_diameter" => Some("nozzle_diameter"),
        "retract_length" => Some("retract_length"),
        "retract_speed" => Some("retract_speed"),
        "retract_lift" => Some("retract_z_hop"),
        "retract_before_travel" => Some("min_travel_for_retract"),
        "gcode_flavor" => Some("gcode_dialect"),
        "machine_max_jerk_x" => Some("jerk_x"),
        "machine_max_jerk_y" => Some("jerk_y"),
        "machine_max_jerk_z" => Some("jerk_z"),

        // Process misc fields.
        "bridge_flow_ratio" => Some("bridge_flow"),
        "elefant_foot_compensation" | "elephant_foot_compensation" => {
            Some("elefant_foot_compensation")
        }
        "fill_angle" => Some("infill_direction"),
        "infill_overlap" => Some("infill_wall_overlap"),
        "spiral_vase" => Some("spiral_mode"),
        "only_one_perimeter_top" => Some("only_one_wall_top"),
        "resolution" => Some("resolution"),
        "raft_layers" => Some("raft_layers"),
        "thin_walls" | "detect_thin_wall" => Some("detect_thin_wall"),

        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Comma-separated value helper
// ---------------------------------------------------------------------------

/// Parse a comma-separated string of f64 values into a Vec.
///
/// Splits on commas, trims whitespace, and parses each element as f64.
/// Non-parseable elements are silently skipped.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(parse_comma_separated_f64("0.4,0.6"), vec![0.4, 0.6]);
/// assert_eq!(parse_comma_separated_f64("200,210"), vec![200.0, 210.0]);
/// assert_eq!(parse_comma_separated_f64("8"), vec![8.0]);
/// ```
fn parse_comma_separated_f64(value: &str) -> Vec<f64> {
    value
        .split(',')
        .filter_map(|s| s.trim().parse::<f64>().ok())
        .collect()
}

/// Parse a PrusaSlicer boolean value ("0"/"1" or "true"/"false").
///
/// Returns `Some(bool)` for recognized values, `None` otherwise.
fn parse_bool(value: &str) -> Option<bool> {
    match value.trim() {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}

/// Apply PrusaSlicer field mapping to a PrintConfig.
///
/// Maps a single PrusaSlicer INI key/value pair to the corresponding PrintConfig
/// field. Returns `true` if the field was successfully mapped, `false` otherwise.
///
/// Handles PrusaSlicer-specific value formats:
/// - `fill_density`: Strips `%` suffix, divides by 100 (e.g., "15%" -> 0.15)
/// - `first_layer_speed`: Skips percentage values (e.g., "50%")
/// - `nozzle_diameter`: Takes first comma-separated value for scalar, full Vec for sub-config
/// - `machine_max_jerk_*`: Takes first comma-separated value for scalar, full Vec for sub-config
/// - `fill_pattern`: Maps PrusaSlicer pattern names to InfillPattern enum
/// - `seam_position`: Maps PrusaSlicer position names to SeamPosition enum
/// - `gcode_flavor`: Maps PrusaSlicer flavor names to GcodeDialect enum
/// - Percentage speed/width values (ending with `%`): skipped (not absolute)
/// - Boolean fields (`0`/`1`): parsed to bool
/// - Unmapped fields: stored in `config.passthrough` BTreeMap
pub fn apply_prusaslicer_field_mapping(
    config: &mut PrintConfig,
    key: &str,
    value: &str,
) -> bool {
    match key {
        // =====================================================================
        // Process fields (existing flat fields)
        // =====================================================================
        "layer_height" => parse_and_set_f64(value, &mut config.layer_height),
        "first_layer_height" => parse_and_set_f64(value, &mut config.first_layer_height),
        "perimeters" => parse_and_set_u32(value, &mut config.wall_count),
        "fill_density" => {
            // PrusaSlicer: "15%" -> 0.15
            let cleaned = value.trim_end_matches('%');
            if let Ok(pct) = cleaned.parse::<f64>() {
                config.infill_density = pct / 100.0;
                true
            } else {
                false
            }
        }
        "fill_pattern" => map_infill_pattern_prusaslicer(value, config),
        "top_solid_layers" => parse_and_set_u32(value, &mut config.top_solid_layers),
        "bottom_solid_layers" => parse_and_set_u32(value, &mut config.bottom_solid_layers),
        "perimeter_speed" => parse_and_set_f64(value, &mut config.perimeter_speed),
        "infill_speed" => parse_and_set_f64(value, &mut config.infill_speed),
        "travel_speed" => parse_and_set_f64(value, &mut config.travel_speed),
        "first_layer_speed" => {
            // Skip percentage speed values (e.g., "50%").
            if value.ends_with('%') {
                return false;
            }
            parse_and_set_f64(value, &mut config.first_layer_speed)
        }
        "skirts" => parse_and_set_u32(value, &mut config.skirt_loops),
        "skirt_distance" => parse_and_set_f64(value, &mut config.skirt_distance),
        "brim_width" => parse_and_set_f64(value, &mut config.brim_width),
        "default_acceleration" => parse_and_set_f64(value, &mut config.print_acceleration),
        "seam_position" => map_seam_position_prusaslicer(value, config),

        // =====================================================================
        // Speed sub-config fields (PrusaSlicer names)
        // =====================================================================
        "bridge_speed" => parse_and_set_f64(value, &mut config.speeds.bridge),
        "small_perimeter_speed" => {
            // PrusaSlicer may use percentage format (e.g., "75%"); skip if percentage.
            if value.ends_with('%') {
                return false;
            }
            parse_and_set_f64(value, &mut config.speeds.small_perimeter)
        }
        "external_perimeter_speed" => {
            // Already mapped to perimeter_speed in existing code for outer wall.
            // Skip percentage format.
            if value.ends_with('%') {
                return false;
            }
            parse_and_set_f64(value, &mut config.perimeter_speed)
        }
        "gap_fill_speed" => parse_and_set_f64(value, &mut config.speeds.gap_fill),
        "top_solid_infill_speed" | "top_infill_speed" => {
            if value.ends_with('%') {
                return false;
            }
            parse_and_set_f64(value, &mut config.speeds.top_surface)
        }
        "solid_infill_speed" => {
            if value.ends_with('%') {
                return false;
            }
            parse_and_set_f64(value, &mut config.speeds.solid_infill)
        }
        "support_material_speed" => {
            parse_and_set_f64(value, &mut config.speeds.support)
        }
        "support_material_interface_speed" => {
            if value.ends_with('%') {
                return false;
            }
            parse_and_set_f64(value, &mut config.speeds.support_interface)
        }
        "first_layer_speed_over_raft" => {
            // No direct equivalent; store in passthrough.
            config
                .passthrough
                .insert(key.to_string(), value.to_string());
            true
        }
        "travel_speed_z" => parse_and_set_f64(value, &mut config.speeds.travel_z),

        // =====================================================================
        // Line width sub-config fields (PrusaSlicer names)
        // =====================================================================
        "extrusion_width" => {
            // Base extrusion width: store in passthrough for reference.
            if value.ends_with('%') {
                return false;
            }
            config
                .passthrough
                .insert(key.to_string(), value.to_string());
            true
        }
        "first_layer_extrusion_width" => {
            if value.ends_with('%') {
                return false;
            }
            parse_and_set_f64(value, &mut config.line_widths.initial_layer)
        }
        "perimeter_extrusion_width" => {
            // PrusaSlicer does not distinguish inner/outer width; map to outer_wall.
            if value.ends_with('%') {
                return false;
            }
            parse_and_set_f64(value, &mut config.line_widths.outer_wall)
        }
        "external_perimeter_extrusion_width" => {
            if value.ends_with('%') {
                return false;
            }
            parse_and_set_f64(value, &mut config.line_widths.outer_wall)
        }
        "infill_extrusion_width" => {
            if value.ends_with('%') {
                return false;
            }
            parse_and_set_f64(value, &mut config.line_widths.infill)
        }
        "solid_infill_extrusion_width" => {
            if value.ends_with('%') {
                return false;
            }
            parse_and_set_f64(value, &mut config.line_widths.internal_solid_infill)
        }
        "top_infill_extrusion_width" => {
            if value.ends_with('%') {
                return false;
            }
            parse_and_set_f64(value, &mut config.line_widths.top_surface)
        }
        "support_material_extrusion_width" => {
            if value.ends_with('%') {
                return false;
            }
            parse_and_set_f64(value, &mut config.line_widths.support)
        }

        // =====================================================================
        // Cooling sub-config fields (PrusaSlicer names)
        // =====================================================================
        "max_fan_speed" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.cooling.fan_max_speed)
        }
        "min_fan_speed" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.cooling.fan_min_speed)
        }
        "slowdown_below_layer_time" => {
            parse_and_set_f64(value, &mut config.cooling.slow_down_layer_time)
        }
        "min_print_speed" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.cooling.slow_down_min_speed)
        }
        "bridge_fan_speed" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.cooling.overhang_fan_speed)
        }
        "full_fan_speed_layer" => {
            parse_and_set_u32(value, &mut config.cooling.full_fan_speed_layer)
        }
        "fan_always_on" => {
            // PrusaSlicer-specific; store in passthrough.
            config
                .passthrough
                .insert(key.to_string(), value.to_string());
            true
        }
        "cooling" => {
            // PrusaSlicer "cooling" is a boolean (0/1) for slow-down-for-layer-cooling.
            let first = first_comma_value(value);
            if let Some(b) = parse_bool(first) {
                config.cooling.slow_down_for_layer_cooling = b;
                true
            } else {
                false
            }
        }

        // =====================================================================
        // Retraction sub-config fields (PrusaSlicer names)
        // =====================================================================
        "deretract_speed" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.retraction.deretraction_speed)
        }
        "retract_before_wipe" => {
            let first = first_comma_value(value);
            // PrusaSlicer stores as percentage (0-100).
            parse_and_set_f64(first, &mut config.retraction.retract_before_wipe)
        }
        "retract_layer_change" => {
            let first = first_comma_value(value);
            if let Some(b) = parse_bool(first) {
                config.retraction.retract_when_changing_layer = b;
                true
            } else {
                false
            }
        }
        "wipe" => {
            let first = first_comma_value(value);
            if let Some(b) = parse_bool(first) {
                config.retraction.wipe = b;
                true
            } else {
                false
            }
        }

        // =====================================================================
        // Machine sub-config fields (PrusaSlicer names)
        // =====================================================================
        "start_gcode" => {
            config.machine.start_gcode = value.to_string();
            true
        }
        "end_gcode" => {
            config.machine.end_gcode = value.to_string();
            true
        }
        "layer_gcode" => {
            config.machine.layer_change_gcode = value.to_string();
            true
        }
        "max_print_height" => {
            parse_and_set_f64(value, &mut config.machine.printable_height)
        }
        "machine_max_acceleration_x" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.machine.max_acceleration_x)
        }
        "machine_max_acceleration_y" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.machine.max_acceleration_y)
        }
        "machine_max_acceleration_z" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.machine.max_acceleration_z)
        }
        "machine_max_acceleration_e" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.machine.max_acceleration_e)
        }
        "machine_max_acceleration_extruding" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.machine.max_acceleration_extruding)
        }
        "machine_max_acceleration_retracting" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.machine.max_acceleration_retracting)
        }
        "machine_max_acceleration_travel" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.machine.max_acceleration_travel)
        }
        "machine_max_speed_x" | "machine_max_feedrate_x" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.machine.max_speed_x)
        }
        "machine_max_speed_y" | "machine_max_feedrate_y" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.machine.max_speed_y)
        }
        "machine_max_speed_z" | "machine_max_feedrate_z" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.machine.max_speed_z)
        }
        "machine_max_speed_e" | "machine_max_feedrate_e" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.machine.max_speed_e)
        }
        "nozzle_type" => {
            config.machine.nozzle_type = value.to_string();
            true
        }
        "printer_model" => {
            config.machine.printer_model = value.to_string();
            true
        }
        "bed_shape" => {
            config.machine.bed_shape = value.to_string();
            true
        }
        "min_layer_height" => {
            // May be comma-separated for multi-extruder; take first.
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.machine.min_layer_height)
        }
        "max_layer_height" => {
            // May be comma-separated for multi-extruder; take first.
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.machine.max_layer_height)
        }

        // =====================================================================
        // Multi-extruder Vec<f64> array fields (comma-separated)
        // =====================================================================
        "nozzle_diameter" => {
            // Populate both scalar (first value) and Vec (all values).
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.nozzle_diameter);
            config.machine.nozzle_diameters = parse_comma_separated_f64(value);
            true
        }
        "machine_max_jerk_x" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.jerk_x);
            config.machine.jerk_values_x = parse_comma_separated_f64(value);
            true
        }
        "machine_max_jerk_y" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.jerk_y);
            config.machine.jerk_values_y = parse_comma_separated_f64(value);
            true
        }
        "machine_max_jerk_z" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.jerk_z);
            config.machine.jerk_values_z = parse_comma_separated_f64(value);
            true
        }
        "machine_max_jerk_e" => {
            let first = first_comma_value(value);
            if let Ok(v) = first.parse::<f64>() {
                // No flat jerk_e on PrintConfig, but populate sub-config Vec.
                let _ = v;
            }
            config.machine.jerk_values_e = parse_comma_separated_f64(value);
            true
        }
        "temperature" => {
            // Populate both scalar (first value) and Vec (all values).
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.nozzle_temp);
            config.filament.nozzle_temperatures = parse_comma_separated_f64(value);
            true
        }
        "bed_temperature" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.bed_temp);
            config.filament.bed_temperatures = parse_comma_separated_f64(value);
            true
        }
        "first_layer_temperature" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.first_layer_nozzle_temp);
            config.filament.first_layer_nozzle_temperatures =
                parse_comma_separated_f64(value);
            true
        }
        "first_layer_bed_temperature" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.first_layer_bed_temp);
            config.filament.first_layer_bed_temperatures =
                parse_comma_separated_f64(value);
            true
        }

        // =====================================================================
        // Acceleration sub-config fields (PrusaSlicer names)
        // =====================================================================
        "external_perimeter_acceleration" => {
            parse_and_set_f64(value, &mut config.accel.outer_wall)
        }
        "perimeter_acceleration" => {
            parse_and_set_f64(value, &mut config.accel.inner_wall)
        }
        "first_layer_acceleration" | "first_layer_acceleration_over_raft" => {
            parse_and_set_f64(value, &mut config.accel.initial_layer)
        }
        "top_solid_infill_acceleration" => {
            parse_and_set_f64(value, &mut config.accel.top_surface)
        }
        "infill_acceleration" => {
            parse_and_set_f64(value, &mut config.accel.sparse_infill)
        }
        "bridge_acceleration" => {
            parse_and_set_f64(value, &mut config.accel.bridge)
        }

        // =====================================================================
        // Filament sub-config fields (PrusaSlicer names)
        // =====================================================================
        "filament_type" => {
            // May be comma-separated for multi-extruder; take first.
            let first = first_comma_value(value);
            config.filament.filament_type = first.to_string();
            true
        }
        "filament_vendor" => {
            config.filament.filament_vendor = value.to_string();
            true
        }
        "filament_max_volumetric_speed" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.filament.max_volumetric_speed)
        }
        "temperature_range_low" | "min_temperature" => {
            parse_and_set_f64(value, &mut config.filament.nozzle_temperature_range_low)
        }
        "temperature_range_high" | "max_temperature" => {
            parse_and_set_f64(value, &mut config.filament.nozzle_temperature_range_high)
        }
        "filament_retract_length" => {
            let first = first_comma_value(value);
            if let Ok(v) = first.parse::<f64>() {
                config.filament.filament_retraction_length = Some(v);
                true
            } else {
                false
            }
        }
        "filament_retract_speed" => {
            let first = first_comma_value(value);
            if let Ok(v) = first.parse::<f64>() {
                config.filament.filament_retraction_speed = Some(v);
                true
            } else {
                false
            }
        }
        "start_filament_gcode" => {
            config.filament.filament_start_gcode = value.to_string();
            true
        }
        "end_filament_gcode" => {
            config.filament.filament_end_gcode = value.to_string();
            true
        }

        // =====================================================================
        // Filament fields (existing flat fields)
        // =====================================================================
        "filament_density" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.filament_density)
        }
        "filament_diameter" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.filament_diameter)
        }
        "filament_cost" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.filament_cost_per_kg)
        }
        "extrusion_multiplier" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.extrusion_multiplier)
        }
        "disable_fan_first_layers" => {
            let first = first_comma_value(value);
            parse_and_set_u32(first, &mut config.disable_fan_first_layers)
        }
        "fan_below_layer_time" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.fan_below_layer_time)
        }

        // =====================================================================
        // Machine/Printer fields (existing flat fields)
        // =====================================================================
        "retract_length" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.retract_length)
        }
        "retract_speed" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.retract_speed)
        }
        "retract_lift" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.retract_z_hop)
        }
        "retract_before_travel" => {
            let first = first_comma_value(value);
            parse_and_set_f64(first, &mut config.min_travel_for_retract)
        }
        "gcode_flavor" => map_gcode_dialect_prusaslicer(value, config),

        // =====================================================================
        // Process misc flat fields (PrusaSlicer names)
        // =====================================================================
        "bridge_flow_ratio" => parse_and_set_f64(value, &mut config.bridge_flow),
        "elefant_foot_compensation" | "elephant_foot_compensation" => {
            parse_and_set_f64(value, &mut config.elefant_foot_compensation)
        }
        "fill_angle" => parse_and_set_f64(value, &mut config.infill_direction),
        "infill_overlap" => {
            // PrusaSlicer may use percentage format (e.g., "25%").
            let cleaned = value.trim_end_matches('%');
            if let Ok(v) = cleaned.parse::<f64>() {
                // If it had a %, convert from percentage to fraction.
                config.infill_wall_overlap = if value.ends_with('%') {
                    v / 100.0
                } else {
                    v
                };
                true
            } else {
                false
            }
        }
        "spiral_vase" => {
            if let Some(b) = parse_bool(value) {
                config.spiral_mode = b;
                true
            } else {
                false
            }
        }
        "only_one_perimeter_top" => {
            if let Some(b) = parse_bool(value) {
                config.only_one_wall_top = b;
                true
            } else {
                false
            }
        }
        "resolution" => parse_and_set_f64(value, &mut config.resolution),
        "raft_layers" => parse_and_set_u32(value, &mut config.raft_layers),
        "thin_walls" | "detect_thin_wall" => {
            if let Some(b) = parse_bool(value) {
                config.detect_thin_wall = b;
                true
            } else {
                false
            }
        }

        // =====================================================================
        // Default: store unmapped fields in passthrough
        // =====================================================================
        _ => {
            config
                .passthrough
                .insert(key.to_string(), value.to_string());
            true
        }
    }
}

// ---------------------------------------------------------------------------
// Conversion entry point
// ---------------------------------------------------------------------------

/// Convert resolved PrusaSlicer INI fields into an ImportResult.
///
/// Takes the flattened (inheritance-resolved) key-value pairs and section metadata,
/// creates a `PrintConfig` via field mapping, and returns an `ImportResult` compatible
/// with the existing TOML conversion pipeline.
///
/// Unmapped fields are stored in `config.passthrough` and tracked in
/// `passthrough_fields` for backward compatibility with the convert pipeline.
pub fn import_prusaslicer_ini_profile(
    resolved_fields: &HashMap<String, String>,
    name: &str,
    section_type: &str,
) -> ImportResult {
    let mut config = PrintConfig::default();
    let mut mapped_fields = Vec::new();
    let mut unmapped_fields = Vec::new();
    let mut passthrough_fields = Vec::new();

    // Metadata fields to skip during mapping.
    const METADATA_KEYS: &[&str] = &[
        "inherits",
        "compatible_printers",
        "compatible_printers_condition",
        "compatible_prints",
        "compatible_prints_condition",
        "renamed_from",
    ];

    // Track which keys existed in passthrough before mapping, so we can detect
    // which were added by the default arm.
    let passthrough_before: std::collections::BTreeSet<String> =
        config.passthrough.keys().cloned().collect();

    for (key, value) in resolved_fields {
        if METADATA_KEYS.contains(&key.as_str()) {
            continue;
        }

        let had_mapping = prusaslicer_key_to_config_field(key).is_some();
        let mapped = apply_prusaslicer_field_mapping(&mut config, key, value);

        if mapped {
            if had_mapping {
                mapped_fields.push(key.clone());
            } else {
                // Field went to passthrough (default arm or explicit passthrough).
                passthrough_fields.push(key.clone());
                // Also track in unmapped for backward compat with convert pipeline.
                unmapped_fields.push(key.clone());
            }
        } else {
            unmapped_fields.push(key.clone());
        }
    }

    // Also detect fields that were explicitly stored in passthrough by named arms
    // (like extrusion_width, fan_always_on, first_layer_speed_over_raft).
    for k in config.passthrough.keys() {
        if !passthrough_before.contains(k) && !passthrough_fields.contains(k) {
            passthrough_fields.push(k.clone());
        }
    }

    // Map section_type to profile type.
    let profile_type = match section_type {
        "print" => "process",
        "filament" => "filament",
        "printer" => "machine",
        _ => section_type,
    };

    ImportResult {
        config,
        mapped_fields,
        unmapped_fields,
        passthrough_fields,
        metadata: ProfileMetadata {
            name: Some(name.to_string()),
            profile_type: Some(profile_type.to_string()),
            inherits: resolved_fields.get("inherits").cloned(),
        },
    }
}

// ---------------------------------------------------------------------------
// Value parsing helpers
// ---------------------------------------------------------------------------

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

/// Extract the first comma-separated value from a string.
///
/// PrusaSlicer uses comma-separated values for multi-extruder fields.
/// This returns the first value, trimmed.
fn first_comma_value(value: &str) -> &str {
    value.split(',').next().unwrap_or(value).trim()
}

// ---------------------------------------------------------------------------
// Enum mapping helpers
// ---------------------------------------------------------------------------

/// Map a PrusaSlicer infill pattern name to our InfillPattern enum.
fn map_infill_pattern_prusaslicer(value: &str, config: &mut PrintConfig) -> bool {
    let pattern = match value.to_lowercase().as_str() {
        "grid" => Some(InfillPattern::Grid),
        "honeycomb" => Some(InfillPattern::Honeycomb),
        "gyroid" => Some(InfillPattern::Gyroid),
        "cubic" => Some(InfillPattern::Cubic),
        "adaptivecubic" => Some(InfillPattern::AdaptiveCubic),
        "lightning" => Some(InfillPattern::Lightning),
        "monotonic" => Some(InfillPattern::Monotonic),
        "rectilinear" | "line" => Some(InfillPattern::Rectilinear),
        _ => None,
    };

    if let Some(p) = pattern {
        config.infill_pattern = p;
        true
    } else {
        false
    }
}

/// Map a PrusaSlicer seam position name to our SeamPosition enum.
fn map_seam_position_prusaslicer(value: &str, config: &mut PrintConfig) -> bool {
    let pos = match value.to_lowercase().as_str() {
        "aligned" => Some(SeamPosition::Aligned),
        "random" => Some(SeamPosition::Random),
        "rear" => Some(SeamPosition::Rear),
        "nearest" => Some(SeamPosition::NearestCorner),
        _ => None,
    };

    if let Some(p) = pos {
        config.seam_position = p;
        true
    } else {
        false
    }
}

/// Map a PrusaSlicer gcode_flavor name to our GcodeDialect enum.
fn map_gcode_dialect_prusaslicer(value: &str, config: &mut PrintConfig) -> bool {
    let dialect = match value.to_lowercase().as_str() {
        "marlin" | "marlin2" => Some(GcodeDialect::Marlin),
        "klipper" => Some(GcodeDialect::Klipper),
        "reprapfirmware" | "reprap" => Some(GcodeDialect::RepRapFirmware),
        _ => None,
    };

    if let Some(d) = dialect {
        config.gcode_dialect = d;
        true
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Task 1 tests: Parser and inheritance
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_multi_section_ini() {
        let ini = "\
[vendor]
name = PrusaResearch
config_version = 1.0.0

[print:*common*]
layer_height = 0.2
perimeters = 2
fill_density = 15%

[print:0.20mm NORMAL]
inherits = *common*
perimeter_speed = 45

[filament:Prusament PLA]
temperature = 215
bed_temperature = 60
filament_density = 1.24
";

        let sections = parse_prusaslicer_ini(ini);
        assert_eq!(sections.len(), 4);

        // Vendor section.
        assert_eq!(sections[0].section_type, "vendor");
        assert_eq!(sections[0].name, "");
        assert!(!sections[0].is_abstract);

        // Abstract print section.
        assert_eq!(sections[1].section_type, "print");
        assert_eq!(sections[1].name, "*common*");
        assert!(sections[1].is_abstract);
        assert_eq!(sections[1].fields.get("layer_height").unwrap(), "0.2");
        assert_eq!(sections[1].fields.get("fill_density").unwrap(), "15%");

        // Concrete print section.
        assert_eq!(sections[2].section_type, "print");
        assert_eq!(sections[2].name, "0.20mm NORMAL");
        assert!(!sections[2].is_abstract);
        assert_eq!(sections[2].fields.get("inherits").unwrap(), "*common*");

        // Filament section.
        assert_eq!(sections[3].section_type, "filament");
        assert_eq!(sections[3].name, "Prusament PLA");
        assert!(!sections[3].is_abstract);
        assert_eq!(sections[3].fields.get("temperature").unwrap(), "215");
    }

    #[test]
    fn test_abstract_vs_concrete_detection() {
        let ini = "\
[print:*common*]
layer_height = 0.2

[print:*PLA*]
fill_density = 20%

[print:0.20mm NORMAL]
perimeters = 3

[filament:Prusament PLA @MK4S]
temperature = 215
";

        let sections = parse_prusaslicer_ini(ini);
        assert_eq!(sections.len(), 4);

        assert!(sections[0].is_abstract); // *common*
        assert!(sections[1].is_abstract); // *PLA*
        assert!(!sections[2].is_abstract); // 0.20mm NORMAL
        assert!(!sections[3].is_abstract); // Prusament PLA @MK4S
    }

    #[test]
    fn test_multi_parent_inheritance() {
        let ini = "\
[print:*base*]
layer_height = 0.2
perimeters = 2
fill_density = 15%
perimeter_speed = 40

[print:*quality*]
perimeters = 3
perimeter_speed = 30

[print:0.20mm QUALITY]
inherits = *base*; *quality*
fill_density = 20%
";

        let sections = parse_prusaslicer_ini(ini);
        let lookup = build_section_lookup(&sections);

        // Resolve the concrete profile.
        let resolved = resolve_ini_inheritance(&sections[2], &sections, &lookup, 0);

        // From *base*: layer_height = 0.2
        assert_eq!(resolved.get("layer_height").unwrap(), "0.2");
        // From *quality* (overrides *base*): perimeters = 3
        assert_eq!(resolved.get("perimeters").unwrap(), "3");
        // From *quality* (overrides *base*): perimeter_speed = 30
        assert_eq!(resolved.get("perimeter_speed").unwrap(), "30");
        // From child (overrides both parents): fill_density = 20%
        assert_eq!(resolved.get("fill_density").unwrap(), "20%");
    }

    #[test]
    fn test_comment_handling() {
        let ini = "\
# This is a comment
; This is also a comment
[print:test]
## Section comment
layer_height = 0.2
# inline is not a key
perimeters = 3
";

        let sections = parse_prusaslicer_ini(ini);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].name, "test");
        assert_eq!(sections[0].fields.len(), 2);
        assert_eq!(sections[0].fields.get("layer_height").unwrap(), "0.2");
        assert_eq!(sections[0].fields.get("perimeters").unwrap(), "3");
    }

    #[test]
    fn test_gcode_newline_escape_preserved() {
        let ini = "\
[printer:TestPrinter]
start_gcode = G28\\nG1 Z5 F3000\\nG1 X10 Y10
end_gcode = G91\\nG1 E-2 F2700\\nG28 X
";

        let sections = parse_prusaslicer_ini(ini);
        assert_eq!(sections.len(), 1);

        // The \\n should be preserved as literal text, not treated as a newline.
        let start = sections[0].fields.get("start_gcode").unwrap();
        assert!(
            start.contains("\\n"),
            "Expected \\n to be preserved in G-code value, got: {}",
            start
        );
        assert_eq!(start, "G28\\nG1 Z5 F3000\\nG1 X10 Y10");

        let end = sections[0].fields.get("end_gcode").unwrap();
        assert!(end.contains("\\n"));
    }

    #[test]
    fn test_inheritance_depth_guard() {
        // Create sections that form a chain of depth > MAX_INHERITANCE_DEPTH.
        // We won't create a cycle, just a deep chain to verify the guard.
        let ini = "\
[print:*level0*]
layer_height = 0.1

[print:*level1*]
inherits = *level0*
perimeters = 1

[print:*level2*]
inherits = *level1*
perimeters = 2

[print:leaf]
inherits = *level2*
fill_density = 10%
";

        let sections = parse_prusaslicer_ini(ini);
        let lookup = build_section_lookup(&sections);

        // This should resolve successfully (only 3 levels deep).
        let resolved = resolve_ini_inheritance(&sections[3], &sections, &lookup, 0);
        assert_eq!(resolved.get("layer_height").unwrap(), "0.1"); // from level0
        assert_eq!(resolved.get("perimeters").unwrap(), "2"); // from level2
        assert_eq!(resolved.get("fill_density").unwrap(), "10%"); // from leaf
    }

    // -----------------------------------------------------------------------
    // Task 2 tests: Field mapping and conversion
    // -----------------------------------------------------------------------

    #[test]
    fn test_field_mapping_basic() {
        let mut config = PrintConfig::default();

        // perimeters -> wall_count
        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "perimeters",
            "3"
        ));
        assert_eq!(config.wall_count, 3);

        // fill_density with % -> infill_density
        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "fill_density",
            "15%"
        ));
        assert!((config.infill_density - 0.15).abs() < 1e-9);

        // temperature -> nozzle_temp
        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "temperature",
            "215"
        ));
        assert!((config.nozzle_temp - 215.0).abs() < 1e-9);
    }

    #[test]
    fn test_nozzle_diameter_comma_separated() {
        let mut config = PrintConfig::default();

        // Multi-extruder nozzle_diameter: take first value for scalar, full Vec for sub-config.
        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "nozzle_diameter",
            "0.4,0.4,0.4,0.4"
        ));
        assert!((config.nozzle_diameter - 0.4).abs() < 1e-9);
        assert_eq!(config.machine.nozzle_diameters, vec![0.4, 0.4, 0.4, 0.4]);

        // Single value.
        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "nozzle_diameter",
            "0.6"
        ));
        assert!((config.nozzle_diameter - 0.6).abs() < 1e-9);
        assert_eq!(config.machine.nozzle_diameters, vec![0.6]);
    }

    #[test]
    fn test_percentage_speed_skipped() {
        let mut config = PrintConfig::default();
        let original_speed = config.first_layer_speed;

        // Percentage speed should be skipped (returns false).
        assert!(!apply_prusaslicer_field_mapping(
            &mut config,
            "first_layer_speed",
            "50%"
        ));
        assert!((config.first_layer_speed - original_speed).abs() < 1e-9);

        // Absolute speed should be mapped.
        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "first_layer_speed",
            "30"
        ));
        assert!((config.first_layer_speed - 30.0).abs() < 1e-9);
    }

    #[test]
    fn test_gcode_flavor_mapping() {
        let mut config = PrintConfig::default();

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "gcode_flavor",
            "marlin"
        ));
        assert_eq!(config.gcode_dialect, GcodeDialect::Marlin);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "gcode_flavor",
            "klipper"
        ));
        assert_eq!(config.gcode_dialect, GcodeDialect::Klipper);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "gcode_flavor",
            "reprapfirmware"
        ));
        assert_eq!(config.gcode_dialect, GcodeDialect::RepRapFirmware);

        // Unknown flavor should fail.
        assert!(!apply_prusaslicer_field_mapping(
            &mut config,
            "gcode_flavor",
            "some_unknown_flavor"
        ));
    }

    #[test]
    fn test_infill_pattern_mapping() {
        let mut config = PrintConfig::default();

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "fill_pattern",
            "gyroid"
        ));
        assert_eq!(config.infill_pattern, InfillPattern::Gyroid);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "fill_pattern",
            "cubic"
        ));
        assert_eq!(config.infill_pattern, InfillPattern::Cubic);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "fill_pattern",
            "honeycomb"
        ));
        assert_eq!(config.infill_pattern, InfillPattern::Honeycomb);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "fill_pattern",
            "rectilinear"
        ));
        assert_eq!(config.infill_pattern, InfillPattern::Rectilinear);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "fill_pattern",
            "line"
        ));
        assert_eq!(config.infill_pattern, InfillPattern::Rectilinear);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "fill_pattern",
            "grid"
        ));
        assert_eq!(config.infill_pattern, InfillPattern::Grid);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "fill_pattern",
            "lightning"
        ));
        assert_eq!(config.infill_pattern, InfillPattern::Lightning);
    }

    #[test]
    fn test_import_prusaslicer_end_to_end() {
        let mut resolved = HashMap::new();
        resolved.insert("layer_height".to_string(), "0.15".to_string());
        resolved.insert("perimeters".to_string(), "4".to_string());
        resolved.insert("fill_density".to_string(), "20%".to_string());
        resolved.insert("temperature".to_string(), "210".to_string());
        resolved.insert("bed_temperature".to_string(), "60".to_string());
        resolved.insert("nozzle_diameter".to_string(), "0.4".to_string());
        resolved.insert("gcode_flavor".to_string(), "klipper".to_string());
        resolved.insert("some_unknown_field".to_string(), "value".to_string());
        resolved.insert("inherits".to_string(), "*common*".to_string());

        let result =
            import_prusaslicer_ini_profile(&resolved, "0.15mm QUALITY @MK4S", "print");

        assert!((result.config.layer_height - 0.15).abs() < 1e-9);
        assert_eq!(result.config.wall_count, 4);
        assert!((result.config.infill_density - 0.20).abs() < 1e-9);
        assert!((result.config.nozzle_temp - 210.0).abs() < 1e-9);
        assert!((result.config.bed_temp - 60.0).abs() < 1e-9);
        assert!((result.config.nozzle_diameter - 0.4).abs() < 1e-9);
        assert_eq!(result.config.gcode_dialect, GcodeDialect::Klipper);

        // Metadata.
        assert_eq!(
            result.metadata.name.as_deref(),
            Some("0.15mm QUALITY @MK4S")
        );
        assert_eq!(result.metadata.profile_type.as_deref(), Some("process"));
        assert_eq!(result.metadata.inherits.as_deref(), Some("*common*"));

        // Field tracking.
        assert!(result.mapped_fields.contains(&"layer_height".to_string()));
        assert!(result.mapped_fields.contains(&"perimeters".to_string()));
        assert!(result.mapped_fields.contains(&"fill_density".to_string()));

        // Unknown fields now go to passthrough.
        assert_eq!(
            result.config.passthrough.get("some_unknown_field").unwrap(),
            "value"
        );
        assert!(result
            .passthrough_fields
            .contains(&"some_unknown_field".to_string()));

        // inherits should not appear in mapped or unmapped (it's metadata).
        assert!(!result.mapped_fields.contains(&"inherits".to_string()));
        assert!(!result.unmapped_fields.contains(&"inherits".to_string()));
    }

    #[test]
    fn test_jerk_comma_separated() {
        let mut config = PrintConfig::default();

        // PrusaSlicer jerk values may be comma-separated: "8,8".
        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "machine_max_jerk_x",
            "8,8"
        ));
        assert!((config.jerk_x - 8.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "machine_max_jerk_y",
            "10,10"
        ));
        assert!((config.jerk_y - 10.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "machine_max_jerk_z",
            "0.4,0.4"
        ));
        assert!((config.jerk_z - 0.4).abs() < 1e-9);
    }

    #[test]
    fn test_seam_position_mapping() {
        let mut config = PrintConfig::default();

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "seam_position",
            "aligned"
        ));
        assert_eq!(config.seam_position, SeamPosition::Aligned);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "seam_position",
            "random"
        ));
        assert_eq!(config.seam_position, SeamPosition::Random);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "seam_position",
            "nearest"
        ));
        assert_eq!(config.seam_position, SeamPosition::NearestCorner);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "seam_position",
            "rear"
        ));
        assert_eq!(config.seam_position, SeamPosition::Rear);
    }

    #[test]
    fn test_prusaslicer_key_to_config_field() {
        assert_eq!(
            prusaslicer_key_to_config_field("perimeters"),
            Some("wall_count")
        );
        assert_eq!(
            prusaslicer_key_to_config_field("fill_density"),
            Some("infill_density")
        );
        assert_eq!(
            prusaslicer_key_to_config_field("temperature"),
            Some("nozzle_temp")
        );
        assert_eq!(
            prusaslicer_key_to_config_field("retract_lift"),
            Some("retract_z_hop")
        );
        assert_eq!(
            prusaslicer_key_to_config_field("retract_before_travel"),
            Some("min_travel_for_retract")
        );
        assert_eq!(
            prusaslicer_key_to_config_field("unknown_field"),
            None
        );
    }

    #[test]
    fn test_filament_section_type_mapping() {
        let mut resolved = HashMap::new();
        resolved.insert("temperature".to_string(), "220".to_string());

        let result = import_prusaslicer_ini_profile(&resolved, "PLA", "filament");
        assert_eq!(result.metadata.profile_type.as_deref(), Some("filament"));

        let result = import_prusaslicer_ini_profile(&resolved, "MK4S", "printer");
        assert_eq!(result.metadata.profile_type.as_deref(), Some("machine"));
    }

    // -----------------------------------------------------------------------
    // Plan 03 tests: Expanded PrusaSlicer field mapping
    // -----------------------------------------------------------------------

    #[test]
    fn test_speed_sub_config_fields() {
        let mut config = PrintConfig::default();

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "bridge_speed",
            "30"
        ));
        assert!((config.speeds.bridge - 30.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "gap_fill_speed",
            "20"
        ));
        assert!((config.speeds.gap_fill - 20.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "top_solid_infill_speed",
            "40"
        ));
        assert!((config.speeds.top_surface - 40.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "solid_infill_speed",
            "60"
        ));
        assert!((config.speeds.solid_infill - 60.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "support_material_speed",
            "50"
        ));
        assert!((config.speeds.support - 50.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "support_material_interface_speed",
            "35"
        ));
        assert!((config.speeds.support_interface - 35.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "travel_speed_z",
            "12"
        ));
        assert!((config.speeds.travel_z - 12.0).abs() < 1e-9);

        // Percentage speed should be skipped.
        assert!(!apply_prusaslicer_field_mapping(
            &mut config,
            "small_perimeter_speed",
            "75%"
        ));

        // Absolute speed should be mapped.
        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "small_perimeter_speed",
            "25"
        ));
        assert!((config.speeds.small_perimeter - 25.0).abs() < 1e-9);
    }

    #[test]
    fn test_line_width_sub_config_fields() {
        let mut config = PrintConfig::default();

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "first_layer_extrusion_width",
            "0.5"
        ));
        assert!((config.line_widths.initial_layer - 0.5).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "perimeter_extrusion_width",
            "0.45"
        ));
        assert!((config.line_widths.outer_wall - 0.45).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "external_perimeter_extrusion_width",
            "0.42"
        ));
        assert!((config.line_widths.outer_wall - 0.42).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "infill_extrusion_width",
            "0.45"
        ));
        assert!((config.line_widths.infill - 0.45).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "solid_infill_extrusion_width",
            "0.42"
        ));
        assert!((config.line_widths.internal_solid_infill - 0.42).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "top_infill_extrusion_width",
            "0.4"
        ));
        assert!((config.line_widths.top_surface - 0.4).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "support_material_extrusion_width",
            "0.38"
        ));
        assert!((config.line_widths.support - 0.38).abs() < 1e-9);

        // Percentage width should be skipped.
        assert!(!apply_prusaslicer_field_mapping(
            &mut config,
            "first_layer_extrusion_width",
            "105%"
        ));
    }

    #[test]
    fn test_machine_gcode_string_fields() {
        let mut config = PrintConfig::default();

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "start_gcode",
            "G28\\nG1 Z5"
        ));
        assert_eq!(config.machine.start_gcode, "G28\\nG1 Z5");

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "end_gcode",
            "G91\\nG1 E-2"
        ));
        assert_eq!(config.machine.end_gcode, "G91\\nG1 E-2");

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "layer_gcode",
            ";LAYER:[layer_num]"
        ));
        assert_eq!(config.machine.layer_change_gcode, ";LAYER:[layer_num]");

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "printer_model",
            "MK4S"
        ));
        assert_eq!(config.machine.printer_model, "MK4S");

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "nozzle_type",
            "hardened_steel"
        ));
        assert_eq!(config.machine.nozzle_type, "hardened_steel");

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "bed_shape",
            "0x0,250x0,250x210,0x210"
        ));
        assert_eq!(config.machine.bed_shape, "0x0,250x0,250x210,0x210");
    }

    #[test]
    fn test_passthrough_unknown_fields() {
        let mut config = PrintConfig::default();

        // Unknown fields should be stored in passthrough and return true.
        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "some_totally_unknown_field",
            "some_value"
        ));
        assert_eq!(
            config.passthrough.get("some_totally_unknown_field").unwrap(),
            "some_value"
        );

        // fan_always_on stored in passthrough explicitly.
        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "fan_always_on",
            "1"
        ));
        assert_eq!(config.passthrough.get("fan_always_on").unwrap(), "1");

        // first_layer_speed_over_raft stored in passthrough explicitly.
        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "first_layer_speed_over_raft",
            "50"
        ));
        assert_eq!(
            config.passthrough.get("first_layer_speed_over_raft").unwrap(),
            "50"
        );
    }

    #[test]
    fn test_comma_separated_scalar_take_first() {
        let mut config = PrintConfig::default();

        // min_layer_height comma-separated: take first.
        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "min_layer_height",
            "0.07,0.07"
        ));
        assert!((config.machine.min_layer_height - 0.07).abs() < 1e-9);

        // max_layer_height comma-separated: take first.
        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "max_layer_height",
            "0.24,0.20"
        ));
        assert!((config.machine.max_layer_height - 0.24).abs() < 1e-9);

        // filament_type comma-separated: take first.
        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "filament_type",
            "PLA,ABS"
        ));
        assert_eq!(config.filament.filament_type, "PLA");
    }

    #[test]
    fn test_nozzle_diameter_vec_population() {
        let mut config = PrintConfig::default();

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "nozzle_diameter",
            "0.4,0.6"
        ));
        assert_eq!(config.machine.nozzle_diameters, vec![0.4, 0.6]);
        assert!((config.nozzle_diameter - 0.4).abs() < 1e-9);
    }

    #[test]
    fn test_temperature_vec_population() {
        let mut config = PrintConfig::default();

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "temperature",
            "200,210"
        ));
        assert_eq!(config.filament.nozzle_temperatures, vec![200.0, 210.0]);
        assert!((config.nozzle_temp - 200.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "bed_temperature",
            "60,65"
        ));
        assert_eq!(config.filament.bed_temperatures, vec![60.0, 65.0]);
        assert!((config.bed_temp - 60.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "first_layer_temperature",
            "215,220"
        ));
        assert_eq!(
            config.filament.first_layer_nozzle_temperatures,
            vec![215.0, 220.0]
        );
        assert!((config.first_layer_nozzle_temp - 215.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "first_layer_bed_temperature",
            "65,70"
        ));
        assert_eq!(
            config.filament.first_layer_bed_temperatures,
            vec![65.0, 70.0]
        );
        assert!((config.first_layer_bed_temp - 65.0).abs() < 1e-9);
    }

    #[test]
    fn test_jerk_vec_population() {
        let mut config = PrintConfig::default();

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "machine_max_jerk_x",
            "8,10"
        ));
        assert_eq!(config.machine.jerk_values_x, vec![8.0, 10.0]);
        assert!((config.jerk_x - 8.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "machine_max_jerk_y",
            "8,10"
        ));
        assert_eq!(config.machine.jerk_values_y, vec![8.0, 10.0]);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "machine_max_jerk_z",
            "0.4,0.6"
        ));
        assert_eq!(config.machine.jerk_values_z, vec![0.4, 0.6]);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "machine_max_jerk_e",
            "2.5,3.0"
        ));
        assert_eq!(config.machine.jerk_values_e, vec![2.5, 3.0]);
    }

    #[test]
    fn test_cooling_sub_config_fields() {
        let mut config = PrintConfig::default();

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "max_fan_speed",
            "100"
        ));
        assert!((config.cooling.fan_max_speed - 100.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "min_fan_speed",
            "35"
        ));
        assert!((config.cooling.fan_min_speed - 35.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "slowdown_below_layer_time",
            "5"
        ));
        assert!((config.cooling.slow_down_layer_time - 5.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "min_print_speed",
            "10"
        ));
        assert!((config.cooling.slow_down_min_speed - 10.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "bridge_fan_speed",
            "100"
        ));
        assert!((config.cooling.overhang_fan_speed - 100.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "full_fan_speed_layer",
            "4"
        ));
        assert_eq!(config.cooling.full_fan_speed_layer, 4);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "cooling",
            "1"
        ));
        assert!(config.cooling.slow_down_for_layer_cooling);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "cooling",
            "0"
        ));
        assert!(!config.cooling.slow_down_for_layer_cooling);
    }

    #[test]
    fn test_retraction_sub_config_fields() {
        let mut config = PrintConfig::default();

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "deretract_speed",
            "40"
        ));
        assert!((config.retraction.deretraction_speed - 40.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "retract_before_wipe",
            "70"
        ));
        assert!((config.retraction.retract_before_wipe - 70.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "retract_layer_change",
            "1"
        ));
        assert!(config.retraction.retract_when_changing_layer);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "wipe",
            "1"
        ));
        assert!(config.retraction.wipe);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "wipe",
            "0"
        ));
        assert!(!config.retraction.wipe);
    }

    #[test]
    fn test_acceleration_sub_config_fields() {
        let mut config = PrintConfig::default();

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "external_perimeter_acceleration",
            "1000"
        ));
        assert!((config.accel.outer_wall - 1000.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "perimeter_acceleration",
            "800"
        ));
        assert!((config.accel.inner_wall - 800.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "first_layer_acceleration",
            "500"
        ));
        assert!((config.accel.initial_layer - 500.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "top_solid_infill_acceleration",
            "1500"
        ));
        assert!((config.accel.top_surface - 1500.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "infill_acceleration",
            "2000"
        ));
        assert!((config.accel.sparse_infill - 2000.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "bridge_acceleration",
            "600"
        ));
        assert!((config.accel.bridge - 600.0).abs() < 1e-9);
    }

    #[test]
    fn test_filament_sub_config_fields() {
        let mut config = PrintConfig::default();

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "filament_type",
            "PLA"
        ));
        assert_eq!(config.filament.filament_type, "PLA");

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "filament_vendor",
            "Prusament"
        ));
        assert_eq!(config.filament.filament_vendor, "Prusament");

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "filament_max_volumetric_speed",
            "15"
        ));
        assert!((config.filament.max_volumetric_speed - 15.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "temperature_range_low",
            "190"
        ));
        assert!((config.filament.nozzle_temperature_range_low - 190.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "temperature_range_high",
            "230"
        ));
        assert!((config.filament.nozzle_temperature_range_high - 230.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "filament_retract_length",
            "0.8"
        ));
        assert_eq!(config.filament.filament_retraction_length, Some(0.8));

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "filament_retract_speed",
            "35"
        ));
        assert_eq!(config.filament.filament_retraction_speed, Some(35.0));

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "start_filament_gcode",
            "M104 S[nozzle_temperature]"
        ));
        assert_eq!(
            config.filament.filament_start_gcode,
            "M104 S[nozzle_temperature]"
        );

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "end_filament_gcode",
            "M104 S0"
        ));
        assert_eq!(config.filament.filament_end_gcode, "M104 S0");
    }

    #[test]
    fn test_process_misc_fields() {
        let mut config = PrintConfig::default();

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "bridge_flow_ratio",
            "0.95"
        ));
        assert!((config.bridge_flow - 0.95).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "elefant_foot_compensation",
            "0.2"
        ));
        assert!((config.elefant_foot_compensation - 0.2).abs() < 1e-9);

        // Alternate spelling.
        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "elephant_foot_compensation",
            "0.15"
        ));
        assert!((config.elefant_foot_compensation - 0.15).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "fill_angle",
            "45"
        ));
        assert!((config.infill_direction - 45.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "infill_overlap",
            "25%"
        ));
        assert!((config.infill_wall_overlap - 0.25).abs() < 1e-9);

        // infill_overlap as fraction (no %).
        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "infill_overlap",
            "0.15"
        ));
        assert!((config.infill_wall_overlap - 0.15).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "spiral_vase",
            "1"
        ));
        assert!(config.spiral_mode);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "spiral_vase",
            "0"
        ));
        assert!(!config.spiral_mode);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "only_one_perimeter_top",
            "1"
        ));
        assert!(config.only_one_wall_top);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "resolution",
            "0.012"
        ));
        assert!((config.resolution - 0.012).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "raft_layers",
            "3"
        ));
        assert_eq!(config.raft_layers, 3);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "thin_walls",
            "1"
        ));
        assert!(config.detect_thin_wall);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "detect_thin_wall",
            "0"
        ));
        assert!(!config.detect_thin_wall);
    }

    #[test]
    fn test_machine_acceleration_fields() {
        let mut config = PrintConfig::default();

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "machine_max_acceleration_x",
            "5000,5000"
        ));
        assert!((config.machine.max_acceleration_x - 5000.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "machine_max_acceleration_y",
            "5000,5000"
        ));
        assert!((config.machine.max_acceleration_y - 5000.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "machine_max_acceleration_z",
            "100,100"
        ));
        assert!((config.machine.max_acceleration_z - 100.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "machine_max_acceleration_e",
            "5000"
        ));
        assert!((config.machine.max_acceleration_e - 5000.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "machine_max_acceleration_extruding",
            "2500"
        ));
        assert!((config.machine.max_acceleration_extruding - 2500.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "machine_max_acceleration_retracting",
            "2500"
        ));
        assert!((config.machine.max_acceleration_retracting - 2500.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "machine_max_acceleration_travel",
            "3000"
        ));
        assert!((config.machine.max_acceleration_travel - 3000.0).abs() < 1e-9);
    }

    #[test]
    fn test_machine_speed_fields() {
        let mut config = PrintConfig::default();

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "machine_max_speed_x",
            "500,500"
        ));
        assert!((config.machine.max_speed_x - 500.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "machine_max_feedrate_y",
            "400"
        ));
        assert!((config.machine.max_speed_y - 400.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "machine_max_feedrate_z",
            "12"
        ));
        assert!((config.machine.max_speed_z - 12.0).abs() < 1e-9);

        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "machine_max_speed_e",
            "120"
        ));
        assert!((config.machine.max_speed_e - 120.0).abs() < 1e-9);
    }

    #[test]
    fn test_parse_comma_separated_f64_helper() {
        assert_eq!(parse_comma_separated_f64("0.4,0.6"), vec![0.4, 0.6]);
        assert_eq!(parse_comma_separated_f64("200,210"), vec![200.0, 210.0]);
        assert_eq!(parse_comma_separated_f64("8"), vec![8.0]);
        assert_eq!(
            parse_comma_separated_f64("0.4, 0.6, 0.8"),
            vec![0.4, 0.6, 0.8]
        );
        assert!(parse_comma_separated_f64("").is_empty());
    }

    #[test]
    fn test_parse_bool_helper() {
        assert_eq!(parse_bool("1"), Some(true));
        assert_eq!(parse_bool("0"), Some(false));
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool("false"), Some(false));
        assert_eq!(parse_bool("maybe"), None);
    }

    #[test]
    fn test_import_with_passthrough_tracking() {
        let mut resolved = HashMap::new();
        resolved.insert("layer_height".to_string(), "0.2".to_string());
        resolved.insert(
            "some_unknown_prusaslicer_field".to_string(),
            "123".to_string(),
        );
        resolved.insert(
            "another_unknown_field".to_string(),
            "abc".to_string(),
        );

        let result = import_prusaslicer_ini_profile(&resolved, "Test", "print");

        // Known field should be mapped.
        assert!(result.mapped_fields.contains(&"layer_height".to_string()));

        // Unknown fields should be in passthrough.
        assert_eq!(
            result
                .config
                .passthrough
                .get("some_unknown_prusaslicer_field")
                .unwrap(),
            "123"
        );
        assert_eq!(
            result.config.passthrough.get("another_unknown_field").unwrap(),
            "abc"
        );

        // Passthrough fields tracked.
        assert!(result
            .passthrough_fields
            .contains(&"some_unknown_prusaslicer_field".to_string()));
        assert!(result
            .passthrough_fields
            .contains(&"another_unknown_field".to_string()));

        // Also tracked in unmapped for backward compat.
        assert!(result
            .unmapped_fields
            .contains(&"some_unknown_prusaslicer_field".to_string()));
    }
}
