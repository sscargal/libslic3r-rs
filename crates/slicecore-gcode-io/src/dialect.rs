//! G-code dialect definitions and configuration.
//!
//! Different 3D printer firmware flavors require different start/end sequences
//! and may interpret certain commands differently. The [`GcodeDialect`] enum
//! selects which firmware-specific behavior the writer should use.

use serde::{Deserialize, Serialize};

/// Supported G-code firmware dialects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GcodeDialect {
    /// Marlin firmware (most common for FDM printers).
    Marlin,
    /// Klipper firmware (uses extended commands for some operations).
    Klipper,
    /// RepRapFirmware (Duet boards, RRF3+).
    RepRapFirmware,
    /// Bambu Lab firmware (simplified sequences, built-in calibration).
    Bambu,
}

/// Configuration for generating start G-code sequences.
#[derive(Debug, Clone)]
pub struct StartConfig {
    /// Target bed temperature in degrees Celsius.
    pub bed_temp: f64,
    /// Target nozzle (extruder) temperature in degrees Celsius.
    pub nozzle_temp: f64,
    /// Bed width in millimeters (X axis).
    pub bed_x: f64,
    /// Bed depth in millimeters (Y axis).
    pub bed_y: f64,
}

/// Configuration for generating end G-code sequences.
#[derive(Debug, Clone)]
pub struct EndConfig {
    /// Retraction distance in millimeters for the final retract.
    pub retract_distance: f64,
}

/// Format an acceleration command for the given dialect.
///
/// Returns dialect-specific G-code text:
/// - Marlin: `M204 P{print} T{travel}`
/// - Klipper: `SET_VELOCITY_LIMIT ACCEL={print}`
/// - RepRapFirmware: `M204 S{print}` (single value, not P/T split)
/// - Bambu: `M204 P{print} T{travel}` (same as Marlin)
pub fn format_acceleration(dialect: GcodeDialect, print_accel: f64, travel_accel: f64) -> String {
    match dialect {
        GcodeDialect::Marlin | GcodeDialect::Bambu => {
            format!("M204 P{print_accel:.0} T{travel_accel:.0}")
        }
        GcodeDialect::Klipper => {
            format!("SET_VELOCITY_LIMIT ACCEL={print_accel:.0}")
        }
        GcodeDialect::RepRapFirmware => {
            format!("M204 S{print_accel:.0}")
        }
    }
}

/// Format a pressure advance command for the given dialect.
///
/// Returns dialect-specific G-code text:
/// - Marlin: `M900 K{value}`
/// - Klipper: `SET_PRESSURE_ADVANCE ADVANCE={value}`
/// - RepRapFirmware: `M572 D0 S{value}`
/// - Bambu: `M900 K{value}` (same as Marlin)
pub fn format_pressure_advance(dialect: GcodeDialect, value: f64) -> String {
    match dialect {
        GcodeDialect::Marlin | GcodeDialect::Bambu => {
            format!("M900 K{value:.4}")
        }
        GcodeDialect::Klipper => {
            format!("SET_PRESSURE_ADVANCE ADVANCE={value:.4}")
        }
        GcodeDialect::RepRapFirmware => {
            format!("M572 D0 S{value:.4}")
        }
    }
}

/// Format a jerk command for the given dialect.
///
/// Returns dialect-specific G-code text:
/// - Marlin: `M205 X{x} Y{y} Z{z}`
/// - Klipper: `SET_VELOCITY_LIMIT SQUARE_CORNER_VELOCITY={x}`
/// - RepRapFirmware: `M566 X{x*60} Y{y*60} Z{z*60}` (RepRap uses mm/min for jerk)
/// - Bambu: `M205 X{x} Y{y} Z{z}` (same as Marlin)
pub fn format_jerk(dialect: GcodeDialect, x: f64, y: f64, z: f64) -> String {
    match dialect {
        GcodeDialect::Marlin | GcodeDialect::Bambu => {
            format!("M205 X{x:.1} Y{y:.1} Z{z:.1}")
        }
        GcodeDialect::Klipper => {
            format!("SET_VELOCITY_LIMIT SQUARE_CORNER_VELOCITY={x:.1}")
        }
        GcodeDialect::RepRapFirmware => {
            // RepRap jerk is in mm/min, so multiply by 60.
            let x_min = x * 60.0;
            let y_min = y * 60.0;
            let z_min = z * 60.0;
            format!("M566 X{x_min:.1} Y{y_min:.1} Z{z_min:.1}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acceleration_marlin() {
        let s = format_acceleration(GcodeDialect::Marlin, 1000.0, 1500.0);
        assert_eq!(s, "M204 P1000 T1500");
    }

    #[test]
    fn acceleration_klipper() {
        let s = format_acceleration(GcodeDialect::Klipper, 1000.0, 1500.0);
        assert_eq!(s, "SET_VELOCITY_LIMIT ACCEL=1000");
    }

    #[test]
    fn acceleration_reprap() {
        let s = format_acceleration(GcodeDialect::RepRapFirmware, 1000.0, 1500.0);
        assert_eq!(s, "M204 S1000");
    }

    #[test]
    fn acceleration_bambu() {
        let s = format_acceleration(GcodeDialect::Bambu, 1000.0, 1500.0);
        assert_eq!(s, "M204 P1000 T1500");
    }

    #[test]
    fn pressure_advance_marlin() {
        let s = format_pressure_advance(GcodeDialect::Marlin, 0.05);
        assert_eq!(s, "M900 K0.0500");
    }

    #[test]
    fn pressure_advance_klipper() {
        let s = format_pressure_advance(GcodeDialect::Klipper, 0.05);
        assert_eq!(s, "SET_PRESSURE_ADVANCE ADVANCE=0.0500");
    }

    #[test]
    fn pressure_advance_reprap() {
        let s = format_pressure_advance(GcodeDialect::RepRapFirmware, 0.05);
        assert_eq!(s, "M572 D0 S0.0500");
    }

    #[test]
    fn pressure_advance_bambu() {
        let s = format_pressure_advance(GcodeDialect::Bambu, 0.05);
        assert_eq!(s, "M900 K0.0500");
    }

    #[test]
    fn jerk_marlin() {
        let s = format_jerk(GcodeDialect::Marlin, 8.0, 8.0, 0.4);
        assert_eq!(s, "M205 X8.0 Y8.0 Z0.4");
    }

    #[test]
    fn jerk_klipper() {
        let s = format_jerk(GcodeDialect::Klipper, 8.0, 8.0, 0.4);
        assert_eq!(s, "SET_VELOCITY_LIMIT SQUARE_CORNER_VELOCITY=8.0");
    }

    #[test]
    fn jerk_reprap() {
        let s = format_jerk(GcodeDialect::RepRapFirmware, 8.0, 8.0, 0.4);
        assert_eq!(s, "M566 X480.0 Y480.0 Z24.0");
    }

    #[test]
    fn jerk_bambu() {
        let s = format_jerk(GcodeDialect::Bambu, 8.0, 8.0, 0.4);
        assert_eq!(s, "M205 X8.0 Y8.0 Z0.4");
    }
}
