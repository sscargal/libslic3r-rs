//! Configuration validation and G-code template variable resolution.
//!
//! [`validate_config`] checks a [`PrintConfig`] for dangerous or suspicious
//! values before slicing begins. Issues are categorized by
//! [`ValidationSeverity`] so callers can decide whether to abort (errors) or
//! warn (warnings).
//!
//! [`resolve_template_variables`] replaces `{nozzle_temp}` and similar
//! placeholders in start/end G-code templates with actual config values.

use crate::config::PrintConfig;

/// Severity level for a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationSeverity {
    /// Suspicious but not necessarily dangerous.
    Warning,
    /// Dangerous value that should prevent slicing.
    Error,
}

/// A single validation issue found in the config.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// Config field name that triggered the issue.
    pub field: String,
    /// Human-readable description of the problem.
    pub message: String,
    /// Severity level.
    pub severity: ValidationSeverity,
    /// String representation of the problematic value.
    pub value: String,
}

/// Absolute safety limit for nozzle temperature (degrees C).
const MAX_NOZZLE_TEMP: f64 = 350.0;

/// Absolute safety limit for bed temperature (degrees C).
const MAX_BED_TEMP: f64 = 150.0;

/// Speed threshold above which a warning is emitted (mm/s).
const EXTREME_SPEED_THRESHOLD: f64 = 500.0;

/// Validates a [`PrintConfig`] and returns any issues found.
///
/// Returns an empty list if the config is safe. Issues are sorted with
/// errors before warnings.
///
/// # Examples
///
/// ```
/// use slicecore_engine::config::PrintConfig;
/// use slicecore_engine::config_validate::validate_config;
///
/// let config = PrintConfig::default();
/// assert!(validate_config(&config).is_empty());
/// ```
pub fn validate_config(config: &PrintConfig) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    let nozzle_diam = config.machine.nozzle_diameter();

    // Error: layer_height <= 0
    if config.layer_height <= 0.0 {
        issues.push(ValidationIssue {
            field: "layer_height".into(),
            message: "layer height must be positive".into(),
            severity: ValidationSeverity::Error,
            value: format!("{}", config.layer_height),
        });
    }

    // Error: nozzle_diameter <= 0
    if nozzle_diam <= 0.0 {
        issues.push(ValidationIssue {
            field: "nozzle_diameter".into(),
            message: "nozzle diameter must be positive".into(),
            severity: ValidationSeverity::Error,
            value: format!("{nozzle_diam}"),
        });
    }

    // Warning: layer_height > nozzle_diameter
    if config.layer_height > nozzle_diam && nozzle_diam > 0.0 {
        issues.push(ValidationIssue {
            field: "layer_height".into(),
            message: format!(
                "layer height ({:.2} mm) exceeds nozzle diameter ({:.2} mm)",
                config.layer_height, nozzle_diam
            ),
            severity: ValidationSeverity::Warning,
            value: format!("{}", config.layer_height),
        });
    } else if config.layer_height > nozzle_diam * 0.75 && config.layer_height <= nozzle_diam && nozzle_diam > 0.0 {
        issues.push(ValidationIssue {
            field: "layer_height".into(),
            message: format!(
                "layer height ({:.2} mm) is very thick relative to nozzle ({:.2} mm)",
                config.layer_height, nozzle_diam
            ),
            severity: ValidationSeverity::Warning,
            value: format!("{}", config.layer_height),
        });
    }

    // Warning: infill_density out of range
    if config.infill_density > 1.0 {
        issues.push(ValidationIssue {
            field: "infill_density".into(),
            message: "infill density exceeds 100%".into(),
            severity: ValidationSeverity::Warning,
            value: format!("{}", config.infill_density),
        });
    } else if config.infill_density < 0.0 {
        issues.push(ValidationIssue {
            field: "infill_density".into(),
            message: "infill density is negative".into(),
            severity: ValidationSeverity::Warning,
            value: format!("{}", config.infill_density),
        });
    }

    // Check speed fields for extreme values
    let speed_fields = [
        ("speeds.perimeter", config.speeds.perimeter),
        ("speeds.infill", config.speeds.infill),
        ("speeds.travel", config.speeds.travel),
        ("speeds.first_layer", config.speeds.first_layer),
        ("speeds.bridge", config.speeds.bridge),
        ("speeds.inner_wall", config.speeds.inner_wall),
        ("speeds.gap_fill", config.speeds.gap_fill),
        ("speeds.top_surface", config.speeds.top_surface),
        ("speeds.support", config.speeds.support),
    ];
    for (name, value) in speed_fields {
        if value > EXTREME_SPEED_THRESHOLD {
            issues.push(ValidationIssue {
                field: name.into(),
                message: format!(
                    "speed {value:.0} mm/s exceeds {EXTREME_SPEED_THRESHOLD:.0} mm/s threshold"
                ),
                severity: ValidationSeverity::Warning,
                value: format!("{value}"),
            });
        }
    }

    // Error: nozzle temp > 350 (absolute safety limit)
    let nozzle_temp = config.filament.nozzle_temp();
    if nozzle_temp > MAX_NOZZLE_TEMP {
        issues.push(ValidationIssue {
            field: "nozzle_temp".into(),
            message: format!(
                "nozzle temperature ({nozzle_temp:.0} C) exceeds absolute safety limit ({MAX_NOZZLE_TEMP:.0} C)"
            ),
            severity: ValidationSeverity::Error,
            value: format!("{nozzle_temp}"),
        });
    }

    // Error: bed temp > 150 (absolute safety limit)
    let bed_temp = config.filament.bed_temp();
    if bed_temp > MAX_BED_TEMP {
        issues.push(ValidationIssue {
            field: "bed_temp".into(),
            message: format!(
                "bed temperature ({bed_temp:.0} C) exceeds absolute safety limit ({MAX_BED_TEMP:.0} C)"
            ),
            severity: ValidationSeverity::Error,
            value: format!("{bed_temp}"),
        });
    }

    // Dimensional compensation range checks
    if config.dimensional_compensation.xy_hole_compensation.abs() > 2.0 {
        issues.push(ValidationIssue {
            field: "dimensional_compensation.xy_hole_compensation".into(),
            message: format!(
                "XY hole compensation {:.2} mm is extreme (range: -2.0 to 2.0)",
                config.dimensional_compensation.xy_hole_compensation
            ),
            severity: ValidationSeverity::Warning,
            value: format!("{}", config.dimensional_compensation.xy_hole_compensation),
        });
    }
    if config.dimensional_compensation.xy_contour_compensation.abs() > 2.0 {
        issues.push(ValidationIssue {
            field: "dimensional_compensation.xy_contour_compensation".into(),
            message: format!(
                "XY contour compensation {:.2} mm is extreme (range: -2.0 to 2.0)",
                config.dimensional_compensation.xy_contour_compensation
            ),
            severity: ValidationSeverity::Warning,
            value: format!("{}", config.dimensional_compensation.xy_contour_compensation),
        });
    }
    if config.dimensional_compensation.elephant_foot_compensation < 0.0
        || config.dimensional_compensation.elephant_foot_compensation > 2.0
    {
        issues.push(ValidationIssue {
            field: "dimensional_compensation.elephant_foot_compensation".into(),
            message: format!(
                "Elephant foot compensation {:.2} mm out of range (0.0 to 2.0)",
                config.dimensional_compensation.elephant_foot_compensation
            ),
            severity: ValidationSeverity::Warning,
            value: format!("{}", config.dimensional_compensation.elephant_foot_compensation),
        });
    }

    // Chamber temperature validation
    if config.filament.chamber_temperature > 0.0
        && config.machine.chamber_temperature > 0.0
        && config.filament.chamber_temperature > config.machine.chamber_temperature
    {
        issues.push(ValidationIssue {
            field: "filament.chamber_temperature".into(),
            message: format!(
                "Filament requests {}C chamber but machine max is {}C",
                config.filament.chamber_temperature,
                config.machine.chamber_temperature
            ),
            severity: ValidationSeverity::Warning,
            value: format!("{}", config.filament.chamber_temperature),
        });
    }
    if config.filament.chamber_temperature > 80.0 {
        issues.push(ValidationIssue {
            field: "filament.chamber_temperature".into(),
            message: format!(
                "Chamber temperature {}C exceeds safety limit (80C)",
                config.filament.chamber_temperature
            ),
            severity: ValidationSeverity::Error,
            value: format!("{}", config.filament.chamber_temperature),
        });
    }

    // Z offset range check
    let total_z_offset = config.z_offset + config.filament.z_offset;
    if total_z_offset.abs() > 5.0 {
        issues.push(ValidationIssue {
            field: "z_offset".into(),
            message: format!(
                "Total Z offset {:.2} mm (global {:.2} + filament {:.2}) exceeds safety limit (+/-5.0)",
                total_z_offset, config.z_offset, config.filament.z_offset
            ),
            severity: ValidationSeverity::Error,
            value: format!("{total_z_offset}"),
        });
    }

    // Filament shrink range check
    if config.filament.filament_shrink < 90.0 || config.filament.filament_shrink > 110.0 {
        issues.push(ValidationIssue {
            field: "filament.filament_shrink".into(),
            message: format!(
                "Filament shrink {}% is unusual (typical range: 90-110%)",
                config.filament.filament_shrink
            ),
            severity: ValidationSeverity::Warning,
            value: format!("{}", config.filament.filament_shrink),
        });
    }

    // --- P1 field range validations ---

    // Fuzzy skin validation
    if config.fuzzy_skin.enabled {
        if config.fuzzy_skin.thickness > 1.0 {
            issues.push(ValidationIssue {
                field: "fuzzy_skin.thickness".into(),
                message: format!(
                    "Fuzzy skin thickness ({:.2} mm) exceeds typical range (0.0-1.0)",
                    config.fuzzy_skin.thickness
                ),
                severity: ValidationSeverity::Warning,
                value: format!("{}", config.fuzzy_skin.thickness),
            });
        }
        if config.fuzzy_skin.point_distance < 0.1 || config.fuzzy_skin.point_distance > 5.0 {
            issues.push(ValidationIssue {
                field: "fuzzy_skin.point_distance".into(),
                message: format!(
                    "Fuzzy skin point distance ({:.2} mm) outside typical range (0.1-5.0)",
                    config.fuzzy_skin.point_distance
                ),
                severity: ValidationSeverity::Warning,
                value: format!("{}", config.fuzzy_skin.point_distance),
            });
        }
    }

    // Brim ears angle validation
    if config.brim_skirt.brim_ears && config.brim_skirt.brim_ears_max_angle > 180.0 {
        issues.push(ValidationIssue {
            field: "brim_skirt.brim_ears_max_angle".into(),
            message: format!(
                "Brim ears max angle ({:.1}) exceeds 180 degrees",
                config.brim_skirt.brim_ears_max_angle
            ),
            severity: ValidationSeverity::Warning,
            value: format!("{}", config.brim_skirt.brim_ears_max_angle),
        });
    }

    // Input shaping factor validation
    if config.input_shaping.accel_to_decel_enable
        && (config.input_shaping.accel_to_decel_factor < 0.0
            || config.input_shaping.accel_to_decel_factor > 1.0)
    {
        issues.push(ValidationIssue {
            field: "input_shaping.accel_to_decel_factor".into(),
            message: format!(
                "Accel-to-decel factor ({:.2}) outside range (0.0-1.0)",
                config.input_shaping.accel_to_decel_factor
            ),
            severity: ValidationSeverity::Warning,
            value: format!("{}", config.input_shaping.accel_to_decel_factor),
        });
    }

    // Infill combination validation
    if config.infill_combination > 10 {
        issues.push(ValidationIssue {
            field: "infill_combination".into(),
            message: format!(
                "Infill combination every {} layers is unusually high (typical: 0-10)",
                config.infill_combination
            ),
            severity: ValidationSeverity::Warning,
            value: format!("{}", config.infill_combination),
        });
    }

    // Infill anchor max validation
    if config.infill_anchor_max > 50.0 {
        issues.push(ValidationIssue {
            field: "infill_anchor_max".into(),
            message: format!(
                "Infill anchor max ({:.1} mm) exceeds typical range (0-50)",
                config.infill_anchor_max
            ),
            severity: ValidationSeverity::Warning,
            value: format!("{}", config.infill_anchor_max),
        });
    }

    // Arachne parameter validation
    if config.arachne_enabled {
        if config.min_bead_width <= 0.0 || config.min_bead_width > 1.0 {
            issues.push(ValidationIssue {
                field: "min_bead_width".into(),
                message: format!(
                    "Min bead width ({:.3} mm) outside typical range (0.0-1.0)",
                    config.min_bead_width
                ),
                severity: ValidationSeverity::Warning,
                value: format!("{}", config.min_bead_width),
            });
        }
        if config.min_feature_size <= 0.0 || config.min_feature_size > 1.0 {
            issues.push(ValidationIssue {
                field: "min_feature_size".into(),
                message: format!(
                    "Min feature size ({:.3} mm) outside typical range (0.0-1.0)",
                    config.min_feature_size
                ),
                severity: ValidationSeverity::Warning,
                value: format!("{}", config.min_feature_size),
            });
        }
    }

    // Tool change retraction validation
    if config.multi_material.tool_change_retraction.retraction_distance_when_cut > 50.0 {
        issues.push(ValidationIssue {
            field: "multi_material.tool_change_retraction.retraction_distance_when_cut".into(),
            message: format!(
                "Tool change retraction distance ({:.1} mm) unusually large (>50)",
                config.multi_material.tool_change_retraction.retraction_distance_when_cut
            ),
            severity: ValidationSeverity::Warning,
            value: format!(
                "{}",
                config.multi_material.tool_change_retraction.retraction_distance_when_cut
            ),
        });
    }

    // Additional cooling fan speed validation
    if config.cooling.additional_cooling_fan_speed > 100.0 {
        issues.push(ValidationIssue {
            field: "cooling.additional_cooling_fan_speed".into(),
            message: format!(
                "Additional cooling fan speed ({:.0}%) exceeds 100%",
                config.cooling.additional_cooling_fan_speed
            ),
            severity: ValidationSeverity::Warning,
            value: format!("{}", config.cooling.additional_cooling_fan_speed),
        });
    }

    // Internal bridge speed range
    if config.speeds.internal_bridge_speed > 300.0 {
        issues.push(ValidationIssue {
            field: "speeds.internal_bridge_speed".into(),
            message: format!(
                "Internal bridge speed {} mm/s exceeds 300 mm/s",
                config.speeds.internal_bridge_speed
            ),
            severity: ValidationSeverity::Warning,
            value: format!("{}", config.speeds.internal_bridge_speed),
        });
    }

    issues
}

/// Resolves template variables in a G-code string using config values.
///
/// Supported variables: `{nozzle_temp}`, `{bed_temp}`,
/// `{first_layer_nozzle_temp}`, `{first_layer_bed_temp}`,
/// `{layer_height}`, `{nozzle_diameter}`.
///
/// Unrecognized `{variables}` are left unchanged.
///
/// # Examples
///
/// ```
/// use slicecore_engine::config::PrintConfig;
/// use slicecore_engine::config_validate::resolve_template_variables;
///
/// let config = PrintConfig::default();
/// let result = resolve_template_variables("M104 S{nozzle_temp}", &config);
/// assert!(result.starts_with("M104 S"));
/// ```
pub fn resolve_template_variables(gcode: &str, config: &PrintConfig) -> String {
    let mut result = String::with_capacity(gcode.len());
    let mut chars = gcode.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            // Collect the variable name
            let mut var_name = String::new();
            let mut found_close = false;
            for next_ch in chars.by_ref() {
                if next_ch == '}' {
                    found_close = true;
                    break;
                }
                var_name.push(next_ch);
            }

            if !found_close {
                // Unclosed brace -- emit as-is
                result.push('{');
                result.push_str(&var_name);
            } else if let Some(replacement) = resolve_variable(&var_name, config) {
                result.push_str(&replacement);
            } else {
                // Unknown variable -- leave as-is
                result.push('{');
                result.push_str(&var_name);
                result.push('}');
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Resolves a single template variable name to its config value.
fn resolve_variable(name: &str, config: &PrintConfig) -> Option<String> {
    match name {
        "nozzle_temp" => Some(format!("{}", config.filament.nozzle_temp())),
        "bed_temp" => Some(format!("{}", config.filament.bed_temp())),
        "first_layer_nozzle_temp" => Some(format!("{}", config.filament.first_layer_nozzle_temp())),
        "first_layer_bed_temp" => Some(format!("{}", config.filament.first_layer_bed_temp())),
        "layer_height" => Some(format!("{}", config.layer_height)),
        "nozzle_diameter" => Some(format!("{}", config.machine.nozzle_diameter())),

        // Dimensional compensation
        "xy_hole_compensation" => Some(format!("{}", config.dimensional_compensation.xy_hole_compensation)),
        "xy_contour_compensation" => Some(format!("{}", config.dimensional_compensation.xy_contour_compensation)),
        "elephant_foot_compensation" => Some(format!("{}", config.dimensional_compensation.elephant_foot_compensation)),

        // Surface patterns (serialize as lowercase string for use in G-code comments)
        "top_surface_pattern" => Some(format!("{:?}", config.top_surface_pattern).to_lowercase()),
        "bottom_surface_pattern" => Some(format!("{:?}", config.bottom_surface_pattern).to_lowercase()),
        "solid_infill_pattern" => Some(format!("{:?}", config.solid_infill_pattern).to_lowercase()),

        // Overhangs
        "extra_perimeters_on_overhangs" => Some(format!("{}", u8::from(config.extra_perimeters_on_overhangs))),

        // Bridges
        "internal_bridge_speed" => Some(format!("{}", config.speeds.internal_bridge_speed)),
        "internal_bridge_support" => Some(format!("{:?}", config.internal_bridge_support).to_lowercase()),

        // Filament
        "chamber_temperature" => Some(format!("{}", config.filament.chamber_temperature)),
        "filament_shrink" => Some(format!("{}", config.filament.filament_shrink)),

        // Z offset (combined: global + per-filament)
        "z_offset" => Some(format!("{}", config.z_offset + config.filament.z_offset)),

        // Bed type
        "curr_bed_type" => Some(format!("{:?}", config.machine.curr_bed_type).to_lowercase()),

        // Acceleration
        "min_length_factor" => Some(format!("{}", config.accel.min_length_factor)),

        // Precise Z
        "precise_z_height" => Some(format!("{}", u8::from(config.precise_z_height))),

        // Fuzzy skin
        "fuzzy_skin" => Some(format!("{}", u8::from(config.fuzzy_skin.enabled))),
        "fuzzy_skin_thickness" => Some(format!("{}", config.fuzzy_skin.thickness)),
        "fuzzy_skin_point_dist" | "fuzzy_skin_point_distance" => {
            Some(format!("{}", config.fuzzy_skin.point_distance))
        }

        // Brim/skirt
        "brim_type" => Some(format!("{:?}", config.brim_skirt.brim_type).to_lowercase()),
        "brim_ears" => Some(format!("{}", u8::from(config.brim_skirt.brim_ears))),
        "brim_ears_max_angle" => Some(format!("{}", config.brim_skirt.brim_ears_max_angle)),
        "skirt_height" => Some(format!("{}", config.brim_skirt.skirt_height)),

        // Input shaping
        "accel_to_decel_enable" => {
            Some(format!("{}", u8::from(config.input_shaping.accel_to_decel_enable)))
        }
        "accel_to_decel_factor" => {
            Some(format!("{}", config.input_shaping.accel_to_decel_factor))
        }

        // Tool-change retraction
        "retraction_distances_when_cut" => Some(format!(
            "{}",
            config.multi_material.tool_change_retraction.retraction_distance_when_cut
        )),
        "long_retractions_when_cut" => Some(format!(
            "{}",
            u8::from(config.multi_material.tool_change_retraction.long_retraction_when_cut)
        )),

        // Acceleration extensions
        "internal_solid_infill_acceleration" => {
            Some(format!("{}", config.accel.internal_solid_infill_acceleration))
        }
        "support_acceleration" => Some(format!("{}", config.accel.support_acceleration)),
        "support_interface_acceleration" => {
            Some(format!("{}", config.accel.support_interface_acceleration))
        }

        // Cooling extensions
        "additional_cooling_fan_speed" => {
            Some(format!("{}", config.cooling.additional_cooling_fan_speed))
        }
        "auxiliary_fan" => Some(format!("{}", u8::from(config.cooling.auxiliary_fan))),

        // Speed extension
        "enable_overhang_speed" => {
            Some(format!("{}", u8::from(config.speeds.enable_overhang_speed)))
        }

        // Filament colour
        "filament_colour" => Some(config.filament.filament_colour.clone()),

        // Multi-material filament indices (1-based for G-code compatibility, 0 = default)
        "wall_filament" => {
            Some(format!("{}", config.multi_material.wall_filament.map_or(0, |v| v + 1)))
        }
        "solid_infill_filament" => Some(format!(
            "{}",
            config.multi_material.solid_infill_filament.map_or(0, |v| v + 1)
        )),
        "support_filament" => Some(format!(
            "{}",
            config.multi_material.support_filament.map_or(0, |v| v + 1)
        )),
        "support_interface_filament" => Some(format!(
            "{}",
            config.multi_material.support_interface_filament.map_or(0, |v| v + 1)
        )),

        // Top-level PrintConfig P1 fields
        "precise_outer_wall" => Some(format!("{}", u8::from(config.precise_outer_wall))),
        "draft_shield" => Some(format!("{}", u8::from(config.draft_shield))),
        "ooze_prevention" => Some(format!("{}", u8::from(config.ooze_prevention))),
        "infill_combination" | "infill_every_layers" => {
            Some(format!("{}", config.infill_combination))
        }
        "infill_anchor_max" => Some(format!("{}", config.infill_anchor_max)),
        "min_bead_width" => Some(format!("{}", config.min_bead_width)),
        "min_feature_size" => Some(format!("{}", config.min_feature_size)),

        // Support
        "support_bottom_interface_layers" | "support_material_bottom_interface_layers" => {
            Some(format!("{}", config.support.support_bottom_interface_layers))
        }

        // Passthrough fallback
        _ => config.passthrough.get(name).cloned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PrintConfig;

    #[test]
    fn default_config_has_no_issues() {
        let config = PrintConfig::default();
        let issues = validate_config(&config);
        assert!(
            issues.is_empty(),
            "default config should have no issues, got: {:?}",
            issues.iter().map(|i| &i.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn warns_layer_height_exceeds_nozzle_diameter() {
        let mut config = PrintConfig::default();
        config.layer_height = 0.5; // > 0.4mm nozzle
        let issues = validate_config(&config);
        let warn = issues
            .iter()
            .find(|i| i.field == "layer_height" && i.severity == ValidationSeverity::Warning);
        assert!(warn.is_some(), "should warn about layer_height > nozzle_diameter");
    }

    #[test]
    fn warns_extreme_speed() {
        let mut config = PrintConfig::default();
        config.speeds.travel = 600.0; // > 500 mm/s
        let issues = validate_config(&config);
        let warn = issues
            .iter()
            .find(|i| i.severity == ValidationSeverity::Warning && i.message.contains("speed"));
        assert!(warn.is_some(), "should warn about extreme speed");
    }

    #[test]
    fn errors_nozzle_temp_exceeds_350() {
        let mut config = PrintConfig::default();
        config.filament.nozzle_temperatures = vec![400.0];
        let issues = validate_config(&config);
        let err = issues
            .iter()
            .find(|i| i.severity == ValidationSeverity::Error && i.field.contains("nozzle_temp"));
        assert!(err.is_some(), "should error on nozzle temp > 350");
    }

    #[test]
    fn resolve_nozzle_temp_variable() {
        let config = PrintConfig::default();
        let gcode = "M104 S{nozzle_temp}";
        let result = resolve_template_variables(gcode, &config);
        let expected_temp = config.filament.nozzle_temp();
        assert_eq!(
            result,
            format!("M104 S{expected_temp}"),
            "should replace {{nozzle_temp}}"
        );
    }

    #[test]
    fn resolve_bed_temp_variable() {
        let config = PrintConfig::default();
        let gcode = "M140 S{bed_temp}";
        let result = resolve_template_variables(gcode, &config);
        let expected_temp = config.filament.bed_temp();
        assert_eq!(result, format!("M140 S{expected_temp}"));
    }

    #[test]
    fn unrecognized_variables_left_as_is() {
        let config = PrintConfig::default();
        let gcode = "G28 ; {unknown_var} stuff";
        let result = resolve_template_variables(gcode, &config);
        assert_eq!(result, gcode, "unrecognized variables should be left unchanged");
    }

    #[test]
    fn resolve_p0_template_variables() {
        let mut config = PrintConfig::default();
        config.dimensional_compensation.xy_hole_compensation = 0.1;
        config.filament.chamber_temperature = 45.0;
        config.z_offset = 0.05;
        config.filament.z_offset = 0.02;

        // Dimensional compensation
        let result = resolve_template_variables("{xy_hole_compensation}", &config);
        assert_eq!(result, "0.1");

        // Chamber temperature
        let result = resolve_template_variables("M141 S{chamber_temperature}", &config);
        assert_eq!(result, "M141 S45");

        // Combined z_offset (global + filament)
        let result = resolve_template_variables("{z_offset}", &config);
        assert!(result.contains("0.07"), "z_offset should be 0.05 + 0.02 = 0.07, got {result}");

        // Surface pattern
        let result = resolve_template_variables("{top_surface_pattern}", &config);
        assert_eq!(result, "monotonic");

        // Bool as u8
        let result = resolve_template_variables("{precise_z_height}", &config);
        assert_eq!(result, "0");

        // Bed type
        let result = resolve_template_variables("{curr_bed_type}", &config);
        assert_eq!(result, "texturedpei");

        // Filament shrink
        let result = resolve_template_variables("{filament_shrink}", &config);
        assert_eq!(result, "100");

        // Min length factor
        let result = resolve_template_variables("{min_length_factor}", &config);
        assert_eq!(result, "0");
    }

    #[test]
    fn passthrough_fallback_resolves() {
        let mut config = PrintConfig::default();
        config.passthrough.insert("custom_key".to_string(), "custom_value".to_string());
        let result = resolve_template_variables("{custom_key}", &config);
        assert_eq!(result, "custom_value");
    }

    #[test]
    fn warns_extreme_xy_hole_compensation() {
        let mut config = PrintConfig::default();
        config.dimensional_compensation.xy_hole_compensation = 3.0;
        let issues = validate_config(&config);
        let warn = issues
            .iter()
            .find(|i| i.field.contains("xy_hole_compensation") && i.severity == ValidationSeverity::Warning);
        assert!(warn.is_some(), "should warn about extreme xy_hole_compensation");
    }

    #[test]
    fn errors_chamber_temp_exceeds_80() {
        let mut config = PrintConfig::default();
        config.filament.chamber_temperature = 90.0;
        let issues = validate_config(&config);
        let err = issues
            .iter()
            .find(|i| i.field.contains("chamber_temperature") && i.severity == ValidationSeverity::Error);
        assert!(err.is_some(), "should error on chamber temp > 80C");
    }

    #[test]
    fn errors_z_offset_exceeds_5() {
        let mut config = PrintConfig::default();
        config.z_offset = 3.0;
        config.filament.z_offset = 3.0;
        let issues = validate_config(&config);
        let err = issues
            .iter()
            .find(|i| i.field == "z_offset" && i.severity == ValidationSeverity::Error);
        assert!(err.is_some(), "should error on total z_offset > 5.0");
    }

    #[test]
    fn warns_unusual_filament_shrink() {
        let mut config = PrintConfig::default();
        config.filament.filament_shrink = 85.0;
        let issues = validate_config(&config);
        let warn = issues
            .iter()
            .find(|i| i.field.contains("filament_shrink") && i.severity == ValidationSeverity::Warning);
        assert!(warn.is_some(), "should warn about unusual filament shrink");
    }

    #[test]
    fn warns_internal_bridge_speed_over_300() {
        let mut config = PrintConfig::default();
        config.speeds.internal_bridge_speed = 400.0;
        let issues = validate_config(&config);
        let warn = issues
            .iter()
            .find(|i| i.field.contains("internal_bridge_speed") && i.severity == ValidationSeverity::Warning);
        assert!(warn.is_some(), "should warn about internal bridge speed > 300");
    }
}
