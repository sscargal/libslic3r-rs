//! G-code output validator.
//!
//! Validates emitted G-code for syntax correctness, coordinate finiteness,
//! feedrate positivity, and temperature range. Produces a structured
//! [`ValidationResult`] with errors (fatal) and warnings (informational).

use serde::{Deserialize, Serialize};

/// Result of G-code validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the G-code is valid (no errors).
    pub valid: bool,
    /// Fatal errors that indicate invalid G-code.
    pub errors: Vec<String>,
    /// Non-fatal warnings (informational).
    pub warnings: Vec<String>,
    /// Total number of lines in the input.
    pub line_count: usize,
}

/// Validate a G-code string for syntax and basic semantic correctness.
///
/// Checks:
/// - Each non-empty, non-comment line starts with a known command prefix (G, M, T)
///   or a known extended command
/// - All numeric parameters are finite (not NaN, not Inf)
/// - F (feedrate) parameter is positive when present
/// - S (temperature) parameter for M104/M109/M140/M190 is in range 0-400
/// - Lines longer than 256 characters produce a warning
pub fn validate_gcode(gcode: &str) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut line_count = 0;

    for (line_idx, line) in gcode.lines().enumerate() {
        line_count += 1;
        let line_num = line_idx + 1;
        let trimmed = line.trim();

        // Empty lines are OK
        if trimmed.is_empty() {
            continue;
        }

        // Comment lines are OK
        if trimmed.starts_with(';') {
            continue;
        }

        // Line length warning
        if trimmed.len() > 256 {
            warnings.push(format!(
                "line {line_num}: excessively long ({} chars)",
                trimmed.len()
            ));
        }

        // Strip inline comments for analysis
        let code_part = if let Some(pos) = trimmed.find(';') {
            trimmed[..pos].trim()
        } else {
            trimmed
        };

        if code_part.is_empty() {
            continue;
        }

        // Check line starts with known command prefix
        let first_char = code_part.chars().next().unwrap();
        let is_known_prefix = matches!(first_char, 'G' | 'M' | 'T');
        // Also allow known Klipper/RepRap extended commands (uppercase words)
        let is_extended_cmd = code_part
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_uppercase())
            && code_part
                .split_whitespace()
                .next()
                .is_some_and(|word| word.chars().all(|c| c.is_ascii_uppercase() || c == '_'));

        if !is_known_prefix && !is_extended_cmd {
            errors.push(format!(
                "line {line_num}: unknown command prefix: {code_part}"
            ));
            continue;
        }

        // Parse parameters and check for numeric validity
        let parts: Vec<&str> = code_part.split_whitespace().collect();
        let cmd = parts[0];

        for &param in &parts[1..] {
            if param.is_empty() {
                continue;
            }

            let param_letter = param.chars().next().unwrap();
            let param_value = &param[1..];

            // Only validate numeric parameters
            if param_value.is_empty() {
                continue;
            }

            // Try to parse as a number
            if let Ok(val) = param_value.parse::<f64>() {
                // Check finiteness
                if !val.is_finite() {
                    errors.push(format!(
                        "line {line_num}: non-finite value in parameter {param_letter}: {param_value}"
                    ));
                    continue;
                }

                // Check feedrate is positive
                if param_letter == 'F' && val <= 0.0 {
                    errors.push(format!(
                        "line {line_num}: feedrate must be positive, got {val}"
                    ));
                }

                // Check temperature range for temperature commands
                if param_letter == 'S'
                    && (cmd == "M104" || cmd == "M109" || cmd == "M140" || cmd == "M190")
                    && !(0.0..=400.0).contains(&val)
                {
                    errors.push(format!(
                        "line {line_num}: temperature out of range: {val} (must be 0-400)"
                    ));
                }
            } else if param_value.contains("NaN")
                || param_value.contains("nan")
                || param_value.contains("inf")
                || param_value.contains("Inf")
            {
                errors.push(format!(
                    "line {line_num}: non-finite value in parameter {param_letter}: {param_value}"
                ));
            }
            // Non-numeric parameter values are allowed (e.g., T0, etc.)
        }
    }

    ValidationResult {
        valid: errors.is_empty(),
        errors,
        warnings,
        line_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_marlin_gcode_passes() {
        let gcode = "\
; Generated by slicecore
M83
M140 S60
M104 S200
M190 S60
M109 S200
G28
G92 E0
G1 X100.000 Y100.000 Z0.300 E0.50000 F1800.0
G1 X150.000 Y100.000 E0.80000
M104 S0
M140 S0
M107
G28 X Y
M84
";
        let result = validate_gcode(gcode);
        assert!(
            result.valid,
            "valid Marlin G-code should pass: {:?}",
            result.errors
        );
        assert_eq!(result.errors.len(), 0);
    }

    #[test]
    fn nan_coordinate_produces_error() {
        let gcode = "G1 XNaN Y10.000\n";
        let result = validate_gcode(gcode);
        assert!(!result.valid);
        assert!(
            result.errors.iter().any(|e| e.contains("non-finite")),
            "should detect NaN: {:?}",
            result.errors
        );
    }

    #[test]
    fn negative_feedrate_produces_error() {
        let gcode = "G1 X10.000 F-100.0\n";
        let result = validate_gcode(gcode);
        assert!(!result.valid);
        assert!(
            result.errors.iter().any(|e| e.contains("feedrate")),
            "should detect negative feedrate: {:?}",
            result.errors
        );
    }

    #[test]
    fn temperature_out_of_range_produces_error() {
        let gcode = "M109 S500\n";
        let result = validate_gcode(gcode);
        assert!(!result.valid);
        assert!(
            result.errors.iter().any(|e| e.contains("temperature")),
            "should detect out-of-range temperature: {:?}",
            result.errors
        );
    }

    #[test]
    fn comment_only_lines_pass() {
        let gcode = "; this is a comment\n; another comment\n";
        let result = validate_gcode(gcode);
        assert!(result.valid);
        assert_eq!(result.errors.len(), 0);
    }

    #[test]
    fn empty_lines_pass() {
        let gcode = "\n\n\n";
        let result = validate_gcode(gcode);
        assert!(result.valid);
        assert_eq!(result.errors.len(), 0);
    }

    #[test]
    fn zero_feedrate_produces_error() {
        let gcode = "G1 X10.000 F0.0\n";
        let result = validate_gcode(gcode);
        assert!(!result.valid);
        assert!(
            result.errors.iter().any(|e| e.contains("feedrate")),
            "should detect zero feedrate: {:?}",
            result.errors
        );
    }

    #[test]
    fn temperature_at_boundary_passes() {
        let gcode_zero = "M104 S0\n";
        let gcode_max = "M140 S400\n";
        assert!(validate_gcode(gcode_zero).valid, "S0 should be valid");
        assert!(validate_gcode(gcode_max).valid, "S400 should be valid");
    }

    #[test]
    fn klipper_extended_commands_pass() {
        let gcode = "TURN_OFF_HEATERS\nBED_MESH_CALIBRATE\n";
        let result = validate_gcode(gcode);
        assert!(
            result.valid,
            "Klipper extended commands should pass: {:?}",
            result.errors
        );
    }

    #[test]
    fn arc_commands_pass_validation() {
        let gcode = "G2 X10.000 Y20.000 I5.000 J0.000 E0.50000 F1800.0\n\
                      G3 X5.000 Y10.000 I-2.000 J3.000\n";
        let result = validate_gcode(gcode);
        assert!(
            result.valid,
            "G2/G3 arc commands should pass: {:?}",
            result.errors
        );
    }

    #[test]
    fn tool_change_commands_pass_validation() {
        let gcode = "T0\nT1\nT3\n";
        let result = validate_gcode(gcode);
        assert!(
            result.valid,
            "T commands should pass: {:?}",
            result.errors
        );
    }

    #[test]
    fn klipper_pressure_advance_commands_pass() {
        let gcode = "SET_PRESSURE_ADVANCE ADVANCE=0.0500\n\
                      SET_VELOCITY_LIMIT ACCEL=1000\n\
                      SET_VELOCITY_LIMIT SQUARE_CORNER_VELOCITY=8.0\n";
        let result = validate_gcode(gcode);
        assert!(
            result.valid,
            "Klipper extended PA/velocity commands should pass: {:?}",
            result.errors
        );
    }

    #[test]
    fn reprap_pressure_advance_commands_pass() {
        let gcode = "M572 D0 S0.0500\nM566 X480.0 Y480.0 Z24.0\n";
        let result = validate_gcode(gcode);
        assert!(
            result.valid,
            "RepRap M572/M566 commands should pass: {:?}",
            result.errors
        );
    }

    #[test]
    fn bambu_ams_commands_pass_validation() {
        let gcode = "M620 S0\nM621 S0\n";
        let result = validate_gcode(gcode);
        assert!(
            result.valid,
            "Bambu M620/M621 AMS commands should pass: {:?}",
            result.errors
        );
    }

    #[test]
    fn acceleration_commands_pass_validation() {
        let gcode = "M204 P1000 T1500\nM204 S1000\nM205 X8.0 Y8.0 Z0.4\nM900 K0.0500\n";
        let result = validate_gcode(gcode);
        assert!(
            result.valid,
            "Acceleration/jerk/PA commands should pass: {:?}",
            result.errors
        );
    }

    #[test]
    fn long_line_produces_warning() {
        let long_line = format!("G1 X{}", "0".repeat(260));
        let result = validate_gcode(&long_line);
        assert!(
            result.warnings.iter().any(|w| w.contains("long")),
            "should warn about long lines: {:?}",
            result.warnings
        );
    }

    #[test]
    fn infinity_coordinate_produces_error() {
        let gcode = "G1 Xinf Y10.000\n";
        let result = validate_gcode(gcode);
        assert!(!result.valid);
        assert!(
            result.errors.iter().any(|e| e.contains("non-finite")),
            "should detect infinity: {:?}",
            result.errors
        );
    }
}
