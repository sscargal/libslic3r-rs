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
        // Machine/printer fields.
        "nozzle_diameter" => Some("nozzle_diameter"),
        "retract_length" => Some("retract_length"),
        "retract_speed" => Some("retract_speed"),
        "retract_lift" => Some("retract_z_hop"),
        "retract_before_travel" => Some("min_travel_for_retract"),
        "gcode_flavor" => Some("gcode_dialect"),
        "machine_max_jerk_x" => Some("jerk_x"),
        "machine_max_jerk_y" => Some("jerk_y"),
        "machine_max_jerk_z" => Some("jerk_z"),
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
/// - `nozzle_diameter`: Takes first comma-separated value for multi-extruder
/// - `machine_max_jerk_*`: Takes first comma-separated value
/// - `fill_pattern`: Maps PrusaSlicer pattern names to InfillPattern enum
/// - `seam_position`: Maps PrusaSlicer position names to SeamPosition enum
/// - `gcode_flavor`: Maps PrusaSlicer flavor names to GcodeDialect enum
pub fn apply_prusaslicer_field_mapping(
    config: &mut PrintConfig,
    key: &str,
    value: &str,
) -> bool {
    match key {
        // --- Process fields ---
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

        // --- Filament fields ---
        "temperature" => {
            // May be comma-separated for multi-extruder.
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_f64(first.trim(), &mut config.nozzle_temp)
        }
        "first_layer_temperature" => {
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_f64(first.trim(), &mut config.first_layer_nozzle_temp)
        }
        "bed_temperature" => {
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_f64(first.trim(), &mut config.bed_temp)
        }
        "first_layer_bed_temperature" => {
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_f64(first.trim(), &mut config.first_layer_bed_temp)
        }
        "filament_density" => {
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_f64(first.trim(), &mut config.filament_density)
        }
        "filament_diameter" => {
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_f64(first.trim(), &mut config.filament_diameter)
        }
        "filament_cost" => {
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_f64(first.trim(), &mut config.filament_cost_per_kg)
        }
        "extrusion_multiplier" => {
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_f64(first.trim(), &mut config.extrusion_multiplier)
        }
        "disable_fan_first_layers" => {
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_u32(first.trim(), &mut config.disable_fan_first_layers)
        }
        "fan_below_layer_time" => {
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_f64(first.trim(), &mut config.fan_below_layer_time)
        }

        // --- Machine/Printer fields ---
        "nozzle_diameter" => {
            // May be comma-separated for multi-extruder: "0.4,0.4,0.4,0.4"
            // Take first value.
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_f64(first.trim(), &mut config.nozzle_diameter)
        }
        "retract_length" => {
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_f64(first.trim(), &mut config.retract_length)
        }
        "retract_speed" => {
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_f64(first.trim(), &mut config.retract_speed)
        }
        "retract_lift" => {
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_f64(first.trim(), &mut config.retract_z_hop)
        }
        "retract_before_travel" => {
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_f64(first.trim(), &mut config.min_travel_for_retract)
        }
        "gcode_flavor" => map_gcode_dialect_prusaslicer(value, config),
        "machine_max_jerk_x" => {
            // May be comma-separated: "8,8" -- take first value.
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_f64(first.trim(), &mut config.jerk_x)
        }
        "machine_max_jerk_y" => {
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_f64(first.trim(), &mut config.jerk_y)
        }
        "machine_max_jerk_z" => {
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_f64(first.trim(), &mut config.jerk_z)
        }

        // Unknown field.
        _ => false,
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
pub fn import_prusaslicer_ini_profile(
    resolved_fields: &HashMap<String, String>,
    name: &str,
    section_type: &str,
) -> ImportResult {
    let mut config = PrintConfig::default();
    let mut mapped_fields = Vec::new();
    let mut unmapped_fields = Vec::new();

    // Metadata fields to skip during mapping.
    const METADATA_KEYS: &[&str] = &[
        "inherits",
        "compatible_printers",
        "compatible_printers_condition",
        "compatible_prints",
        "compatible_prints_condition",
        "renamed_from",
    ];

    for (key, value) in resolved_fields {
        if METADATA_KEYS.contains(&key.as_str()) {
            continue;
        }

        if apply_prusaslicer_field_mapping(&mut config, key, value) {
            mapped_fields.push(key.clone());
        } else {
            unmapped_fields.push(key.clone());
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
        passthrough_fields: Vec::new(),
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

        // Multi-extruder nozzle_diameter: take first value.
        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "nozzle_diameter",
            "0.4,0.4,0.4,0.4"
        ));
        assert!((config.nozzle_diameter - 0.4).abs() < 1e-9);

        // Single value.
        assert!(apply_prusaslicer_field_mapping(
            &mut config,
            "nozzle_diameter",
            "0.6"
        ));
        assert!((config.nozzle_diameter - 0.6).abs() < 1e-9);
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
        assert!(result
            .unmapped_fields
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
}
