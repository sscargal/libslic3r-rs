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
        _ => None,
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
}
