//! Schema-driven configuration validation.
//!
//! Validates config values against the constraints defined in setting
//! definitions, replacing ad-hoc hardcoded range checks with a data-driven
//! approach.

use serde::{Deserialize, Serialize};

use crate::registry::SettingRegistry;
use crate::types::Constraint;

/// Severity level for a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationSeverity {
    /// Informational: depends_on condition not met.
    Info,
    /// Suspicious but not necessarily dangerous.
    Warning,
    /// Dangerous value that should prevent slicing.
    Error,
}

/// A single validation issue found in the config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    /// Config field key that triggered the issue.
    pub key: String,
    /// Human-readable description of the problem.
    pub message: String,
    /// Severity level.
    pub severity: ValidationSeverity,
}

/// Resolves a dotted key path (e.g., `"speed.perimeter"`) within a JSON value.
fn resolve_json_path<'a>(json: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    let mut current = json;
    for part in key.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

impl SettingRegistry {
    /// Validates a config JSON object against all registered setting constraints.
    ///
    /// For each setting definition, checks:
    /// - Range constraints: value must be within `[min, max]`
    /// - `DependsOn` constraints: warns if a dependent setting is configured but
    ///   its dependency condition is unmet
    /// - Deprecated settings: warns if a deprecated field differs from default
    #[must_use]
    pub fn validate_config(&self, config_json: &serde_json::Value) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        for def in self.all() {
            let value = resolve_json_path(config_json, &def.key.0);

            // Check range constraints
            for constraint in &def.constraints {
                match constraint {
                    Constraint::Range { min, max } => {
                        if let Some(val) = value.and_then(serde_json::Value::as_f64) {
                            if val < *min {
                                issues.push(ValidationIssue {
                                    key: def.key.0.clone(),
                                    message: format!(
                                        "{} value {val} is below minimum {min}",
                                        def.key
                                    ),
                                    severity: ValidationSeverity::Error,
                                });
                            } else if val > *max {
                                issues.push(ValidationIssue {
                                    key: def.key.0.clone(),
                                    message: format!(
                                        "{} value {val} exceeds maximum {max}",
                                        def.key
                                    ),
                                    severity: ValidationSeverity::Error,
                                });
                            } else if (val - min).abs() < f64::EPSILON * 10.0
                                || (val - max).abs() < f64::EPSILON * 10.0
                            {
                                // Very close to boundary -- informational only,
                                // not flagged as a warning since boundary values
                                // are typically valid.
                            }
                        }
                    }
                    Constraint::DependsOn { key, condition } => {
                        // Check if the dependency is unmet and this field has
                        // a non-default value.
                        if let Some(dep_val) = resolve_json_path(config_json, &key.0) {
                            let dep_unmet = is_dependency_unmet(dep_val, condition);
                            if dep_unmet {
                                // Only emit Info if this field has a non-default value
                                if let Some(val) = value {
                                    if *val != def.default_value && !val.is_null() {
                                        issues.push(ValidationIssue {
                                            key: def.key.0.clone(),
                                            message: format!(
                                                "{} is configured but {} condition '{}' is not met",
                                                def.key, key, condition
                                            ),
                                            severity: ValidationSeverity::Info,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Check deprecated fields
            if let Some(ref deprecation_msg) = def.deprecated {
                if let Some(val) = value {
                    if *val != def.default_value && !val.is_null() {
                        issues.push(ValidationIssue {
                            key: def.key.0.clone(),
                            message: format!(
                                "{} is deprecated: {deprecation_msg}",
                                def.key
                            ),
                            severity: ValidationSeverity::Warning,
                        });
                    }
                }
            }
        }

        issues
    }
}

/// Checks whether a dependency condition is unmet.
///
/// Supports simple conditions: `"== true"`, `"== false"`, `"> 0"`.
fn is_dependency_unmet(dep_val: &serde_json::Value, condition: &str) -> bool {
    let trimmed = condition.trim();

    if trimmed == "== true" {
        // Dependency is unmet if value is false (or not true)
        return dep_val != &serde_json::Value::Bool(true)
            && dep_val.as_f64().map_or(true, |v| v == 0.0);
    }
    if trimmed == "== false" {
        return dep_val != &serde_json::Value::Bool(false)
            && dep_val.as_f64().map_or(true, |v| v != 0.0);
    }
    if trimmed == "> 0" {
        return dep_val.as_f64().map_or(true, |v| v <= 0.0);
    }

    // Unknown condition format -- don't flag
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{SettingCategory, SettingDefinition, SettingKey, Tier, ValueType};
    use serde_json::json;

    fn make_def(key: &str) -> SettingDefinition {
        SettingDefinition {
            key: SettingKey::new(key),
            display_name: key.to_owned(),
            description: String::new(),
            tier: Tier::Simple,
            category: SettingCategory::Quality,
            value_type: ValueType::Float,
            default_value: json!(0.2),
            constraints: Vec::new(),
            affects: Vec::new(),
            affected_by: Vec::new(),
            units: None,
            tags: Vec::new(),
            since_version: "0.1.0".to_owned(),
            deprecated: None,
        }
    }

    #[test]
    fn range_validation_catches_out_of_bounds() {
        let mut reg = SettingRegistry::new();
        let mut def = make_def("layer_height");
        def.constraints = vec![Constraint::Range {
            min: 0.05,
            max: 0.6,
        }];
        reg.register(def);

        // Value too high
        let config = json!({ "layer_height": 1.0 });
        let issues = reg.validate_config(&config);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, ValidationSeverity::Error);
        assert!(issues[0].message.contains("exceeds maximum"));

        // Value too low
        let config = json!({ "layer_height": 0.01 });
        let issues = reg.validate_config(&config);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, ValidationSeverity::Error);
        assert!(issues[0].message.contains("below minimum"));

        // Valid value
        let config = json!({ "layer_height": 0.2 });
        let issues = reg.validate_config(&config);
        assert!(issues.is_empty());
    }

    #[test]
    fn depends_on_produces_info_for_unmet_condition() {
        let mut reg = SettingRegistry::new();

        // Register the dependency target
        let mut enable_def = make_def("support.enable");
        enable_def.value_type = ValueType::Bool;
        enable_def.default_value = json!(false);
        reg.register(enable_def);

        // Register a field that depends on support.enable == true
        let mut dep_def = make_def("support.density");
        dep_def.default_value = json!(0.5);
        dep_def.constraints = vec![Constraint::DependsOn {
            key: SettingKey::new("support.enable"),
            condition: "== true".to_owned(),
        }];
        reg.register(dep_def);

        // support.enable is false but density is set to non-default
        let config = json!({
            "support": {
                "enable": false,
                "density": 0.8
            }
        });
        let issues = reg.validate_config(&config);
        let info = issues
            .iter()
            .find(|i| i.severity == ValidationSeverity::Info);
        assert!(info.is_some(), "should produce Info for unmet dependency");
        assert!(info.unwrap().message.contains("not met"));
    }

    #[test]
    fn deprecated_field_warning() {
        let mut reg = SettingRegistry::new();
        let mut def = make_def("old_setting");
        def.default_value = json!(0.0);
        def.deprecated = Some("Use new_setting instead".to_owned());
        reg.register(def);

        // Non-default value on deprecated field
        let config = json!({ "old_setting": 1.5 });
        let issues = reg.validate_config(&config);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, ValidationSeverity::Warning);
        assert!(issues[0].message.contains("deprecated"));

        // Default value on deprecated field -- no warning
        let config = json!({ "old_setting": 0.0 });
        let issues = reg.validate_config(&config);
        assert!(issues.is_empty());
    }

    #[test]
    fn nested_key_resolution() {
        let mut reg = SettingRegistry::new();
        let mut def = make_def("speed.perimeter");
        def.constraints = vec![Constraint::Range {
            min: 1.0,
            max: 500.0,
        }];
        reg.register(def);

        let config = json!({ "speed": { "perimeter": 600.0 } });
        let issues = reg.validate_config(&config);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("exceeds maximum"));
    }
}
