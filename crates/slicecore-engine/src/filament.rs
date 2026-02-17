//! Filament usage estimation.
//!
//! Computes filament consumption from a G-code command stream as length (mm/m),
//! weight (grams), and cost (currency units). This enables material planning
//! and cost tracking for print jobs.
//!
//! # Computation
//!
//! - **Length**: Sum of all positive E-values from extrusion moves (LinearMove,
//!   ArcMoveCW, ArcMoveCCW). Negative E-values (retractions) are excluded.
//! - **Weight**: `length_mm * cross_section_area_mm2 * density_g_per_mm3`
//!   where cross-section is `PI * (diameter/2)^2`.
//! - **Cost**: `(weight_g / 1000.0) * cost_per_kg`.

use serde::{Deserialize, Serialize};
use slicecore_gcode_io::GcodeCommand;

/// Filament usage breakdown for a print job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilamentUsage {
    /// Total filament length consumed in mm.
    pub length_mm: f64,
    /// Total filament length consumed in meters (convenience).
    pub length_m: f64,
    /// Total filament weight in grams.
    pub weight_g: f64,
    /// Total filament cost in configured currency units.
    pub cost: f64,
}

/// Estimates filament usage from a G-code command stream.
///
/// Sums all positive E-values from extrusion moves to compute total filament
/// length, then derives weight and cost from physical properties.
///
/// # Parameters
///
/// - `commands`: The G-code command stream.
/// - `filament_diameter`: Filament diameter in mm (typically 1.75 or 2.85).
/// - `filament_density`: Filament density in g/cm^3 (PLA ~1.24, ABS ~1.04).
/// - `filament_cost_per_kg`: Cost per kilogram in currency units.
///
/// # Returns
///
/// A [`FilamentUsage`] with length, weight, and cost.
pub fn estimate_filament_usage(
    commands: &[GcodeCommand],
    filament_diameter: f64,
    filament_density: f64,
    filament_cost_per_kg: f64,
) -> FilamentUsage {
    let mut total_e_mm: f64 = 0.0;

    for cmd in commands {
        match cmd {
            GcodeCommand::LinearMove { e: Some(e), .. } if *e > 0.0 => {
                total_e_mm += e;
            }
            GcodeCommand::ArcMoveCW { e: Some(e), .. } if *e > 0.0 => {
                total_e_mm += e;
            }
            GcodeCommand::ArcMoveCCW { e: Some(e), .. } if *e > 0.0 => {
                total_e_mm += e;
            }
            _ => {}
        }
    }

    let length_mm = total_e_mm;
    let length_m = length_mm / 1000.0;

    // Cross-section area of filament in mm^2.
    let radius = filament_diameter / 2.0;
    let cross_section_mm2 = std::f64::consts::PI * radius * radius;

    // Volume in mm^3.
    let volume_mm3 = length_mm * cross_section_mm2;

    // Convert density from g/cm^3 to g/mm^3: 1 cm^3 = 1000 mm^3.
    let density_g_per_mm3 = filament_density / 1000.0;

    // Weight in grams.
    let weight_g = volume_mm3 * density_g_per_mm3;

    // Cost = (weight in kg) * cost_per_kg.
    let cost = (weight_g / 1000.0) * filament_cost_per_kg;

    FilamentUsage {
        length_mm,
        length_m,
        weight_g,
        cost,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filament_usage_length_from_e_values() {
        let commands = vec![
            GcodeCommand::LinearMove {
                x: Some(10.0),
                y: Some(0.0),
                z: None,
                e: Some(1.5),
                f: Some(3000.0),
            },
            GcodeCommand::LinearMove {
                x: Some(20.0),
                y: Some(0.0),
                z: None,
                e: Some(2.0),
                f: None,
            },
            // Retraction should be excluded (negative E).
            GcodeCommand::Retract {
                distance: 0.8,
                feedrate: 2700.0,
            },
            GcodeCommand::LinearMove {
                x: Some(30.0),
                y: Some(0.0),
                z: None,
                e: Some(1.0),
                f: None,
            },
        ];

        let usage = estimate_filament_usage(&commands, 1.75, 1.24, 25.0);
        assert!(
            (usage.length_mm - 4.5).abs() < 1e-6,
            "Length should be 1.5+2.0+1.0=4.5mm, got {}",
            usage.length_mm
        );
        assert!(
            (usage.length_m - 0.0045).abs() < 1e-9,
            "Length in meters should be 0.0045, got {}",
            usage.length_m
        );
    }

    #[test]
    fn filament_usage_weight_cross_section() {
        // 1000mm of 1.75mm filament at 1.24 g/cm^3
        let commands = vec![GcodeCommand::LinearMove {
            x: Some(100.0),
            y: Some(0.0),
            z: None,
            e: Some(1000.0),
            f: Some(3000.0),
        }];

        let usage = estimate_filament_usage(&commands, 1.75, 1.24, 25.0);

        // Cross-section = PI * (0.875)^2 = ~2.405 mm^2
        let expected_cross_section = std::f64::consts::PI * (1.75_f64 / 2.0).powi(2);
        // Volume = 1000 * 2.405 = 2405.28 mm^3
        let expected_volume = 1000.0 * expected_cross_section;
        // Weight = 2405.28 * (1.24/1000) = 2.983 g
        let expected_weight = expected_volume * (1.24 / 1000.0);

        assert!(
            (usage.weight_g - expected_weight).abs() < 0.01,
            "Weight should be ~{:.3}g, got {:.3}",
            expected_weight,
            usage.weight_g
        );
    }

    #[test]
    fn filament_usage_cost_from_weight() {
        let commands = vec![GcodeCommand::LinearMove {
            x: Some(100.0),
            y: Some(0.0),
            z: None,
            e: Some(1000.0),
            f: Some(3000.0),
        }];

        let cost_per_kg = 25.0;
        let usage = estimate_filament_usage(&commands, 1.75, 1.24, cost_per_kg);

        // Cost = (weight_g / 1000) * cost_per_kg
        let expected_cost = (usage.weight_g / 1000.0) * cost_per_kg;
        assert!(
            (usage.cost - expected_cost).abs() < 1e-9,
            "Cost should be {:.6}, got {:.6}",
            expected_cost,
            usage.cost
        );
    }

    #[test]
    fn filament_usage_excludes_negative_e() {
        // Only positive E values should be counted (retractions are negative).
        let commands = vec![
            GcodeCommand::LinearMove {
                x: Some(10.0),
                y: Some(0.0),
                z: None,
                e: Some(2.0),
                f: Some(3000.0),
            },
            // This is a retraction emitted as LinearMove with negative E.
            GcodeCommand::LinearMove {
                x: None,
                y: None,
                z: None,
                e: Some(-0.8),
                f: Some(2700.0),
            },
            GcodeCommand::LinearMove {
                x: Some(20.0),
                y: Some(0.0),
                z: None,
                e: Some(3.0),
                f: Some(3000.0),
            },
        ];

        let usage = estimate_filament_usage(&commands, 1.75, 1.24, 25.0);
        assert!(
            (usage.length_mm - 5.0).abs() < 1e-6,
            "Length should be 2.0+3.0=5.0mm (excluding -0.8), got {}",
            usage.length_mm
        );
    }

    #[test]
    fn filament_usage_arc_moves_counted() {
        let commands = vec![
            GcodeCommand::ArcMoveCW {
                x: Some(10.0),
                y: Some(10.0),
                i: 5.0,
                j: 0.0,
                e: Some(1.5),
                f: Some(3000.0),
            },
            GcodeCommand::ArcMoveCCW {
                x: Some(20.0),
                y: Some(0.0),
                i: -5.0,
                j: 0.0,
                e: Some(2.5),
                f: Some(3000.0),
            },
        ];

        let usage = estimate_filament_usage(&commands, 1.75, 1.24, 25.0);
        assert!(
            (usage.length_mm - 4.0).abs() < 1e-6,
            "Arc move E-values should be summed: 1.5+2.5=4.0, got {}",
            usage.length_mm
        );
    }
}
