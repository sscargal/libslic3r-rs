//! G-code template variable translation between upstream slicer formats and ours.
//!
//! Upstream slicers (OrcaSlicer, PrusaSlicer) embed variable placeholders in
//! start/end G-code templates using different naming conventions and bracket
//! styles. This module provides data-driven translation tables that map
//! upstream variable names to our canonical `{variable}` syntax, plus a
//! [`translate_gcode_template`] function that performs the replacement.
//!
//! # Variable Syntax
//!
//! - **OrcaSlicer**: `{variable_name}` (curly braces)
//! - **PrusaSlicer**: `[variable_name]` (square brackets)
//! - **Ours**: `{variable_name}` (curly braces)
//!
//! # Usage
//!
//! ```
//! use slicecore_engine::gcode_template::{
//!     build_orcaslicer_translation_table, translate_gcode_template,
//! };
//!
//! let table = build_orcaslicer_translation_table();
//! let translated = translate_gcode_template(
//!     "M104 S{nozzle_temperature} ; heat nozzle",
//!     &table,
//! );
//! assert_eq!(translated, "M104 S{nozzle_temp} ; heat nozzle");
//! ```

/// Build the OrcaSlicer variable translation table.
///
/// Maps OrcaSlicer `{variable}` names to our canonical variable names.
/// Sorted by key length (longest first) to prevent partial-match collisions
/// during sequential replacement.
///
/// Variables that are OrcaSlicer-specific (no equivalent in our system) are
/// left as identity mappings so they pass through unchanged.
#[must_use]
pub fn build_orcaslicer_translation_table() -> Vec<(&'static str, &'static str)> {
    let mut table = vec![
        // Temperature variables
        ("{nozzle_temperature_initial_layer_single}", "{first_layer_nozzle_temp}"),
        ("{bed_temperature_initial_layer_single}", "{first_layer_bed_temp}"),
        ("{nozzle_temperature_initial_layer}", "{first_layer_nozzle_temp}"),
        ("{bed_temperature_initial_layer}", "{first_layer_bed_temp}"),
        ("{overall_chamber_temperature}", "{chamber_temperature}"),
        ("{nozzle_temperature}", "{nozzle_temp}"),
        ("{bed_temperature}", "{bed_temp}"),
        ("{chamber_temperature}", "{chamber_temperature}"),
        // Layer info
        ("{initial_layer_print_height}", "{first_layer_height}"),
        ("{total_layer_count}", "{total_layers}"),
        ("{current_object_name}", "{object_name}"),
        ("{layer_num}", "{layer_num}"),
        ("{layer_z}", "{layer_z}"),
        // Machine/bed info
        ("{curr_bed_type}", "{bed_type}"),
        ("{machine_name}", "{printer_name}"),
        ("{printable_area}", "{bed_shape}"),
        ("{max_print_height}", "{max_z}"),
        // Flow/extrusion
        ("{filament_flow_ratio}", "{extrusion_multiplier}"),
        ("{initial_layer_speed}", "{first_layer_speed}"),
        // Filament info
        ("{filament_type}", "{filament_material}"),
        ("{filament_density}", "{filament_density}"),
        ("{filament_diameter}", "{filament_diameter}"),
        // Tool/extruder
        ("{initial_extruder}", "{initial_tool}"),
        ("{initial_tool}", "{initial_tool}"),
        ("{next_extruder}", "{next_extruder}"),
        ("{previous_extruder}", "{previous_extruder}"),
        // Tool-change retraction
        ("{retraction_distance_when_cut}", "{retraction_distance_when_cut}"),
    ];
    // Sort by key length descending to prevent partial-match replacement.
    table.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    table
}

/// Build the PrusaSlicer variable translation table.
///
/// Maps PrusaSlicer `[variable]` names to our canonical `{variable}` names.
/// This converts bracket syntax to brace syntax and renames variables to
/// match our internal naming convention.
///
/// Sorted by key length (longest first) to prevent partial-match collisions.
#[must_use]
pub fn build_prusaslicer_translation_table() -> Vec<(&'static str, &'static str)> {
    let mut table = vec![
        // Temperature variables
        ("[first_layer_temperature]", "{first_layer_nozzle_temp}"),
        ("[first_layer_bed_temperature]", "{first_layer_bed_temp}"),
        ("[temperature]", "{nozzle_temp}"),
        ("[bed_temperature]", "{bed_temp}"),
        ("[chamber_temperature]", "{chamber_temperature}"),
        // Layer info
        ("[first_layer_height]", "{first_layer_height}"),
        ("[layer_num]", "{layer_num}"),
        ("[layer_z]", "{layer_z}"),
        ("[layer_height]", "{layer_height}"),
        ("[total_layer_count]", "{total_layers}"),
        // Filament info
        ("[filament_type]", "{filament_material}"),
        ("[filament_diameter]", "{filament_diameter}"),
        ("[nozzle_diameter]", "{nozzle_diameter}"),
        // Extruder/tool
        ("[current_extruder]", "{current_tool}"),
        ("[initial_extruder]", "{initial_tool}"),
        ("[initial_tool]", "{initial_tool}"),
        ("[next_extruder]", "{next_extruder}"),
        ("[previous_extruder]", "{previous_extruder}"),
        // Acceleration
        ("[default_acceleration]", "{default_acceleration}"),
        ("[first_layer_acceleration]", "{initial_layer_acceleration}"),
        // Extrusion
        ("[extrusion_width]", "{line_width}"),
        ("[retract_length]", "{retraction_length}"),
        // Machine info
        ("[printer_model]", "{printer_model}"),
        // Machine limits
        ("[machine_max_acceleration_x]", "{machine_max_acceleration_x}"),
        ("[machine_max_acceleration_y]", "{machine_max_acceleration_y}"),
        ("[machine_max_acceleration_z]", "{machine_max_acceleration_z}"),
        ("[machine_max_acceleration_e]", "{machine_max_acceleration_e}"),
        ("[machine_max_acceleration_extruding]", "{machine_max_acceleration_extruding}"),
        ("[machine_max_acceleration_retracting]", "{machine_max_acceleration_retracting}"),
        ("[machine_max_feedrate_x]", "{machine_max_speed_x}"),
        ("[machine_max_feedrate_y]", "{machine_max_speed_y}"),
        ("[machine_max_feedrate_z]", "{machine_max_speed_z}"),
        ("[machine_max_feedrate_e]", "{machine_max_speed_e}"),
        ("[machine_max_jerk_x]", "{machine_max_jerk_x}"),
        ("[machine_max_jerk_y]", "{machine_max_jerk_y}"),
        ("[machine_max_jerk_z]", "{machine_max_jerk_z}"),
        ("[machine_max_jerk_e]", "{machine_max_jerk_e}"),
    ];
    // Sort by key length descending to prevent partial-match replacement.
    table.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    table
}

/// Translate a G-code template string by replacing upstream variable names
/// with our canonical variable names.
///
/// Uses sequential replacement with longest-first ordering to avoid
/// partial-match collisions. For example, `{bed_temperature_initial_layer}`
/// is replaced before `{bed_temperature}`.
///
/// # Arguments
///
/// * `template` - The G-code template string with upstream variable names.
/// * `table` - Translation table from [`build_orcaslicer_translation_table`]
///   or [`build_prusaslicer_translation_table`].
///
/// # Returns
///
/// A new string with all recognized upstream variables replaced by our names.
/// Unrecognized variables are left unchanged.
pub fn translate_gcode_template(template: &str, table: &[(&str, &str)]) -> String {
    let mut result = template.to_string();
    for &(from, to) in table {
        result = result.replace(from, to);
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orcaslicer_table_has_at_least_15_entries() {
        let table = build_orcaslicer_translation_table();
        assert!(
            table.len() >= 15,
            "OrcaSlicer table should have at least 15 entries, got {}",
            table.len()
        );
    }

    #[test]
    fn prusaslicer_table_has_at_least_15_entries() {
        let table = build_prusaslicer_translation_table();
        assert!(
            table.len() >= 15,
            "PrusaSlicer table should have at least 15 entries, got {}",
            table.len()
        );
    }

    #[test]
    fn orcaslicer_tables_sorted_by_length_descending() {
        let table = build_orcaslicer_translation_table();
        for window in table.windows(2) {
            assert!(
                window[0].0.len() >= window[1].0.len(),
                "Table not sorted by length: '{}' ({}) before '{}' ({})",
                window[0].0,
                window[0].0.len(),
                window[1].0,
                window[1].0.len()
            );
        }
    }

    #[test]
    fn prusaslicer_tables_sorted_by_length_descending() {
        let table = build_prusaslicer_translation_table();
        for window in table.windows(2) {
            assert!(
                window[0].0.len() >= window[1].0.len(),
                "Table not sorted by length: '{}' ({}) before '{}' ({})",
                window[0].0,
                window[0].0.len(),
                window[1].0,
                window[1].0.len()
            );
        }
    }

    #[test]
    fn translate_orcaslicer_temperature_variables() {
        let table = build_orcaslicer_translation_table();
        let input = "M104 S{nozzle_temperature} ; set nozzle temp\nM140 S{bed_temperature}";
        let result = translate_gcode_template(input, &table);
        assert_eq!(
            result,
            "M104 S{nozzle_temp} ; set nozzle temp\nM140 S{bed_temp}"
        );
    }

    #[test]
    fn translate_orcaslicer_initial_layer_variables() {
        let table = build_orcaslicer_translation_table();
        let input = "M104 S{nozzle_temperature_initial_layer} ; first layer\nM140 S{bed_temperature_initial_layer}";
        let result = translate_gcode_template(input, &table);
        assert_eq!(
            result,
            "M104 S{first_layer_nozzle_temp} ; first layer\nM140 S{first_layer_bed_temp}"
        );
    }

    #[test]
    fn translate_prusaslicer_bracket_to_brace() {
        let table = build_prusaslicer_translation_table();
        let input = "M104 S[first_layer_temperature]\nM140 S[first_layer_bed_temperature]";
        let result = translate_gcode_template(input, &table);
        assert_eq!(
            result,
            "M104 S{first_layer_nozzle_temp}\nM140 S{first_layer_bed_temp}"
        );
    }

    #[test]
    fn translate_prusaslicer_layer_variables() {
        let table = build_prusaslicer_translation_table();
        let input = ";LAYER:[layer_num] Z=[layer_z]";
        let result = translate_gcode_template(input, &table);
        assert_eq!(result, ";LAYER:{layer_num} Z={layer_z}");
    }

    #[test]
    fn translate_preserves_unrecognized_variables() {
        let table = build_orcaslicer_translation_table();
        let input = "M104 S{some_unknown_var}";
        let result = translate_gcode_template(input, &table);
        assert_eq!(result, "M104 S{some_unknown_var}");
    }

    #[test]
    fn translate_empty_string() {
        let table = build_orcaslicer_translation_table();
        let result = translate_gcode_template("", &table);
        assert_eq!(result, "");
    }

    #[test]
    fn translate_no_variables() {
        let table = build_orcaslicer_translation_table();
        let input = "G28 ; home all\nG1 Z5 F3000";
        let result = translate_gcode_template(input, &table);
        assert_eq!(result, "G28 ; home all\nG1 Z5 F3000");
    }

    #[test]
    fn longest_match_prevents_partial_replacement() {
        // Ensure {bed_temperature_initial_layer} is replaced before {bed_temperature}.
        let table = build_orcaslicer_translation_table();
        let input = "M140 S{bed_temperature_initial_layer}";
        let result = translate_gcode_template(input, &table);
        assert_eq!(result, "M140 S{first_layer_bed_temp}");
        // And standalone {bed_temperature} still works.
        let input2 = "M140 S{bed_temperature}";
        let result2 = translate_gcode_template(input2, &table);
        assert_eq!(result2, "M140 S{bed_temp}");
    }
}
