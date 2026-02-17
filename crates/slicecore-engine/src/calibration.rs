//! Pressure advance calibration pattern generator.
//!
//! Generates a complete G-code file that prints a test pattern with varying
//! pressure advance (PA) values. Each line in the pattern alternates between
//! slow and fast extrusion speeds, revealing PA artifacts at the transitions.
//! By visually inspecting the printed pattern, users can identify the optimal
//! PA value for their printer/filament combination.
//!
//! This is a standalone G-code generator, not part of the normal slicing pipeline.

use std::f64::consts::PI;

use slicecore_gcode_io::{
    format_pressure_advance, EndConfig, GcodeCommand, GcodeDialect, GcodeWriter, StartConfig,
};

use crate::config::PaCalibrationConfig;
use crate::extrusion::extrusion_cross_section;

/// Compute the E-value for a linear move of the given length.
///
/// Uses the standard Slic3r cross-section model with the calibration
/// config's line_width, layer_height, and filament_diameter.
fn compute_e(move_length: f64, config: &PaCalibrationConfig) -> f64 {
    if move_length <= 0.0 {
        return 0.0;
    }
    let cross_section = extrusion_cross_section(config.line_width, config.layer_height);
    let volume = cross_section * move_length;
    let filament_area = PI * (config.filament_diameter / 2.0) * (config.filament_diameter / 2.0);
    volume / filament_area
}

/// Generate a pressure advance calibration pattern as raw bytes.
///
/// Produces a complete G-code file including start code, the calibration
/// pattern with varying PA values, and end code. The pattern uses
/// dialect-specific PA commands.
///
/// # Pattern layout
///
/// For each PA step from `pa_start` to `pa_end`:
/// - Emit a dialect-specific PA command
/// - Print a line with alternating slow/fast sections:
///   - 20mm at `slow_speed` (stable baseline)
///   - 40mm at `fast_speed` (reveals over/under extrusion from PA mismatch)
///   - 20mm at `slow_speed` (return to baseline)
/// - Each line is offset in Y by `line_width * 2` from the previous
/// - A label comment is added with the PA value
///
/// # Parameters
/// - `config`: Calibration pattern parameters
/// - `dialect`: Target firmware dialect for PA commands
pub fn generate_pa_calibration(config: &PaCalibrationConfig, dialect: GcodeDialect) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    let mut writer = GcodeWriter::new(&mut buf, dialect);

    // --- Start code ---
    writer
        .write_start_gcode(&StartConfig {
            bed_temp: config.bed_temp,
            nozzle_temp: config.nozzle_temp,
            bed_x: config.bed_center_x * 2.0,
            bed_y: config.bed_center_y * 2.0,
        })
        .expect("write start gcode");

    // Move to layer height
    writer
        .write_command(&GcodeCommand::LinearMove {
            x: None,
            y: None,
            z: Some(config.layer_height),
            e: None,
            f: Some(600.0),
        })
        .expect("write Z move");

    // --- Prime line ---
    let prime_x_start = config.bed_center_x - config.pattern_width / 2.0 - 10.0;
    let prime_y = config.bed_center_y - 10.0;
    writer
        .write_command(&GcodeCommand::Comment("Prime line".to_string()))
        .expect("write comment");
    writer
        .write_command(&GcodeCommand::RapidMove {
            x: Some(prime_x_start),
            y: Some(prime_y),
            z: None,
            f: Some(config.fast_speed * 60.0),
        })
        .expect("write rapid move");
    let prime_length = 30.0;
    let prime_e = compute_e(prime_length, config);
    writer
        .write_command(&GcodeCommand::LinearMove {
            x: Some(prime_x_start + prime_length),
            y: Some(prime_y),
            z: None,
            e: Some(prime_e),
            f: Some(config.slow_speed * 60.0),
        })
        .expect("write prime line");

    // --- Calibration pattern ---
    // Compute the number of PA steps
    let num_steps = ((config.pa_end - config.pa_start) / config.pa_step).round() as usize + 1;
    let y_spacing = config.line_width * 2.0;
    let x_start = config.bed_center_x - config.pattern_width / 2.0;

    // Section lengths: 20mm slow + 40mm fast + 20mm slow = 80mm total
    // But we need pattern_width to work, so we scale proportionally.
    // The plan specifies 20/40/20 for a total of 80mm.
    // If pattern_width != 80, we scale proportionally.
    let total_nominal = 80.0;
    let scale = config.pattern_width / total_nominal;
    let slow_len = 20.0 * scale;
    let fast_len = 40.0 * scale;

    writer
        .write_command(&GcodeCommand::Comment(
            "PA Calibration Pattern Start".to_string(),
        ))
        .expect("write comment");

    for i in 0..num_steps {
        let pa_value = config.pa_start + (i as f64) * config.pa_step;
        // Clamp to pa_end to avoid floating-point overshoot
        let pa_value = pa_value.min(config.pa_end);

        let y = config.bed_center_y + (i as f64) * y_spacing;
        let x0 = x_start;
        let x1 = x0 + slow_len;
        let x2 = x1 + fast_len;
        let x3 = x2 + slow_len;

        // PA label comment
        writer
            .write_command(&GcodeCommand::Comment(format!("PA = {pa_value:.3}")))
            .expect("write PA comment");

        // Set PA value using dialect-specific command
        let pa_cmd = format_pressure_advance(dialect, pa_value);
        writer
            .write_command(&GcodeCommand::Raw(pa_cmd))
            .expect("write PA command");

        // Travel to line start
        writer
            .write_command(&GcodeCommand::RapidMove {
                x: Some(x0),
                y: Some(y),
                z: None,
                f: Some(config.fast_speed * 60.0),
            })
            .expect("write travel");

        // Section 1: slow (20mm scaled)
        let e1 = compute_e(slow_len, config);
        writer
            .write_command(&GcodeCommand::LinearMove {
                x: Some(x1),
                y: Some(y),
                z: None,
                e: Some(e1),
                f: Some(config.slow_speed * 60.0),
            })
            .expect("write slow section 1");

        // Section 2: fast (40mm scaled)
        let e2 = compute_e(fast_len, config);
        writer
            .write_command(&GcodeCommand::LinearMove {
                x: Some(x2),
                y: Some(y),
                z: None,
                e: Some(e2),
                f: Some(config.fast_speed * 60.0),
            })
            .expect("write fast section");

        // Section 3: slow (20mm scaled)
        let e3 = compute_e(slow_len, config);
        writer
            .write_command(&GcodeCommand::LinearMove {
                x: Some(x3),
                y: Some(y),
                z: None,
                e: Some(e3),
                f: Some(config.slow_speed * 60.0),
            })
            .expect("write slow section 2");
    }

    writer
        .write_command(&GcodeCommand::Comment(
            "PA Calibration Pattern End".to_string(),
        ))
        .expect("write comment");

    // --- End code ---
    writer
        .write_end_gcode(&EndConfig {
            retract_distance: 5.0,
        })
        .expect("write end gcode");

    buf
}

/// Generate a pressure advance calibration pattern as a String.
///
/// Convenience wrapper around [`generate_pa_calibration`] that returns
/// a UTF-8 string instead of raw bytes.
pub fn generate_pa_calibration_gcode(
    config: &PaCalibrationConfig,
    dialect: GcodeDialect,
) -> String {
    let bytes = generate_pa_calibration(config, dialect);
    String::from_utf8(bytes).expect("G-code output should be valid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_pa_config_has_expected_values() {
        let config = PaCalibrationConfig::default();
        assert!((config.pa_start - 0.0).abs() < 1e-9);
        assert!((config.pa_end - 0.1).abs() < 1e-9);
        assert!((config.pa_step - 0.005).abs() < 1e-9);
        assert!((config.slow_speed - 20.0).abs() < 1e-9);
        assert!((config.fast_speed - 100.0).abs() < 1e-9);
        assert!((config.line_width - 0.5).abs() < 1e-9);
        assert!((config.layer_height - 0.2).abs() < 1e-9);
        assert!((config.bed_center_x - 110.0).abs() < 1e-9);
        assert!((config.bed_center_y - 110.0).abs() < 1e-9);
        assert!((config.pattern_width - 100.0).abs() < 1e-9);
        assert!((config.nozzle_temp - 200.0).abs() < 1e-9);
        assert!((config.bed_temp - 60.0).abs() < 1e-9);
        assert!((config.filament_diameter - 1.75).abs() < 1e-9);
    }

    #[test]
    fn generate_pa_calibration_produces_nonempty_output() {
        let config = PaCalibrationConfig::default();
        let output = generate_pa_calibration(&config, GcodeDialect::Marlin);
        assert!(!output.is_empty(), "Output should not be empty");
    }

    #[test]
    fn output_contains_pa_commands_at_each_step() {
        let config = PaCalibrationConfig::default();
        let gcode = generate_pa_calibration_gcode(&config, GcodeDialect::Marlin);

        // With default config: pa_start=0.0, pa_end=0.1, pa_step=0.005
        // That's (0.1-0.0)/0.005 + 1 = 21 steps
        let expected_steps =
            ((config.pa_end - config.pa_start) / config.pa_step).round() as usize + 1;

        // Check PA value labels exist
        for i in 0..expected_steps {
            let pa_value = config.pa_start + (i as f64) * config.pa_step;
            let pa_value = pa_value.min(config.pa_end);
            let label = format!("; PA = {pa_value:.3}");
            assert!(
                gcode.contains(&label),
                "Output should contain label '{}' for step {}",
                label,
                i
            );
        }
    }

    #[test]
    fn klipper_output_contains_set_pressure_advance() {
        let config = PaCalibrationConfig::default();
        let gcode = generate_pa_calibration_gcode(&config, GcodeDialect::Klipper);
        assert!(
            gcode.contains("SET_PRESSURE_ADVANCE"),
            "Klipper output should contain SET_PRESSURE_ADVANCE"
        );
    }

    #[test]
    fn marlin_output_contains_m900_k() {
        let config = PaCalibrationConfig::default();
        let gcode = generate_pa_calibration_gcode(&config, GcodeDialect::Marlin);
        assert!(
            gcode.contains("M900 K"),
            "Marlin output should contain M900 K"
        );
    }

    #[test]
    fn reprap_output_contains_m572() {
        let config = PaCalibrationConfig::default();
        let gcode = generate_pa_calibration_gcode(&config, GcodeDialect::RepRapFirmware);
        assert!(
            gcode.contains("M572 D0 S"),
            "RepRap output should contain M572 D0 S"
        );
    }

    #[test]
    fn pa_command_count_matches_expected_steps() {
        let config = PaCalibrationConfig::default();
        let gcode = generate_pa_calibration_gcode(&config, GcodeDialect::Marlin);

        let expected_steps =
            ((config.pa_end - config.pa_start) / config.pa_step).round() as usize + 1;

        // Count M900 K lines (Marlin PA command)
        let pa_count = gcode.lines().filter(|l| l.starts_with("M900 K")).count();
        assert_eq!(
            pa_count, expected_steps,
            "Expected {} PA commands, found {}",
            expected_steps, pa_count
        );
    }

    #[test]
    fn output_starts_with_start_gcode_and_ends_with_end_gcode() {
        let config = PaCalibrationConfig::default();
        let gcode = generate_pa_calibration_gcode(&config, GcodeDialect::Marlin);

        // Start G-code contains G28 (home) early
        let lines: Vec<&str> = gcode.lines().collect();
        let has_home_early = lines.iter().take(20).any(|l| l.contains("G28"));
        assert!(has_home_early, "Start G-code should contain G28 (home)");

        // End G-code contains M84 (disable steppers)
        let has_disable_steppers = lines.iter().rev().take(10).any(|l| l.contains("M84"));
        assert!(
            has_disable_steppers,
            "End G-code should contain M84 (disable steppers)"
        );
    }

    #[test]
    fn e_values_are_positive_and_nonzero() {
        let config = PaCalibrationConfig::default();
        let gcode = generate_pa_calibration_gcode(&config, GcodeDialect::Marlin);

        // Check that G1 lines with E values have positive E
        for line in gcode.lines() {
            if line.starts_with("G1") {
                if let Some(e_pos) = line.find(" E") {
                    let e_str = &line[e_pos + 2..];
                    let e_end = e_str.find(' ').unwrap_or(e_str.len());
                    let e_val: f64 = e_str[..e_end].parse().unwrap_or(0.0);
                    // E values in calibration should be positive (relative mode)
                    // Skip retraction commands (negative E)
                    if !line.contains("E-") {
                        assert!(
                            e_val > 0.0,
                            "E value should be positive, got {} in line: {}",
                            e_val,
                            line
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn alternating_speeds_visible_in_output() {
        let config = PaCalibrationConfig::default();
        let gcode = generate_pa_calibration_gcode(&config, GcodeDialect::Marlin);

        let slow_f = format!("F{:.1}", config.slow_speed * 60.0);
        let fast_f = format!("F{:.1}", config.fast_speed * 60.0);

        assert!(
            gcode.contains(&slow_f),
            "Output should contain slow speed feedrate {}",
            slow_f
        );
        assert!(
            gcode.contains(&fast_f),
            "Output should contain fast speed feedrate {}",
            fast_f
        );
    }

    #[test]
    fn custom_config_respects_parameters() {
        let config = PaCalibrationConfig {
            pa_start: 0.0,
            pa_end: 0.02,
            pa_step: 0.01,
            ..Default::default()
        };
        let gcode = generate_pa_calibration_gcode(&config, GcodeDialect::Marlin);

        // Should have 3 steps: 0.0, 0.01, 0.02
        let pa_count = gcode.lines().filter(|l| l.starts_with("M900 K")).count();
        assert_eq!(
            pa_count, 3,
            "Expected 3 PA commands for 0.0/0.02/0.01, found {}",
            pa_count
        );
    }

    #[test]
    fn string_convenience_wrapper_matches_bytes() {
        let config = PaCalibrationConfig::default();
        let dialect = GcodeDialect::Marlin;

        let bytes = generate_pa_calibration(&config, dialect);
        let string = generate_pa_calibration_gcode(&config, dialect);

        assert_eq!(
            bytes,
            string.as_bytes(),
            "String wrapper should produce same content as bytes"
        );
    }
}
