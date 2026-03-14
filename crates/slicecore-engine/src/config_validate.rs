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

/// Validates a [`PrintConfig`] and returns any issues found.
pub fn validate_config(_config: &PrintConfig) -> Vec<ValidationIssue> {
    todo!()
}

/// Resolves template variables in a G-code string using config values.
///
/// Supported variables: `{nozzle_temp}`, `{bed_temp}`,
/// `{first_layer_nozzle_temp}`, `{first_layer_bed_temp}`,
/// `{layer_height}`, `{nozzle_diameter}`.
///
/// Unrecognized `{variables}` are left unchanged.
pub fn resolve_template_variables(_gcode: &str, _config: &PrintConfig) -> String {
    todo!()
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
