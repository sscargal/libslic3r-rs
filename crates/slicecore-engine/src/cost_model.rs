//! Cost estimation model for 3D printing.
//!
//! Provides progressive-disclosure cost estimation: only cost components with
//! sufficient input data are computed. Missing inputs produce helpful hints
//! guiding the user to provide the data needed for a more complete estimate.
//!
//! # Components
//!
//! - **Filament cost**: weight * price per kg
//! - **Electricity cost**: print hours * watts/1000 * rate
//! - **Depreciation cost**: printer cost / expected lifetime hours * print hours
//! - **Labor cost**: labor rate * setup time
//!
//! # Example
//!
//! ```
//! use slicecore_engine::cost_model::{CostInputs, compute_cost};
//!
//! let inputs = CostInputs {
//!     filament_weight_g: 50.0,
//!     print_time_seconds: 3600.0,
//!     filament_price_per_kg: Some(25.0),
//!     ..CostInputs::default()
//! };
//! let estimate = compute_cost(&inputs);
//! assert!(estimate.filament_cost.is_some());
//! ```

use serde::{Deserialize, Serialize};

use crate::statistics::filament_mm_to_grams;

/// Inputs for cost estimation.
///
/// Only `filament_weight_g` and `print_time_seconds` are required.
/// All other fields are optional; when absent, the corresponding cost
/// component is skipped and a hint is generated instead.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostInputs {
    /// Weight of filament consumed in grams.
    pub filament_weight_g: f64,
    /// Total print time in seconds.
    pub print_time_seconds: f64,
    /// Filament price per kilogram (currency units).
    pub filament_price_per_kg: Option<f64>,
    /// Electricity rate per kWh (currency units).
    pub electricity_rate: Option<f64>,
    /// Printer power consumption in watts.
    pub printer_watts: Option<f64>,
    /// Printer purchase cost (currency units).
    pub printer_cost: Option<f64>,
    /// Expected printer lifetime in hours.
    pub expected_hours: Option<f64>,
    /// Labor rate per hour (currency units).
    pub labor_rate: Option<f64>,
    /// Setup/post-processing time in minutes.
    pub setup_time_minutes: Option<f64>,
}

impl Default for CostInputs {
    fn default() -> Self {
        Self {
            filament_weight_g: 0.0,
            print_time_seconds: 0.0,
            filament_price_per_kg: None,
            electricity_rate: None,
            printer_watts: None,
            printer_cost: None,
            expected_hours: None,
            labor_rate: None,
            setup_time_minutes: None,
        }
    }
}

/// Estimated cost breakdown with progressive disclosure.
///
/// Each cost component is `Some` only when sufficient inputs were provided.
/// `missing_hints` contains user-facing guidance for inputs that were absent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    /// Filament material cost (weight * price/kg).
    pub filament_cost: Option<f64>,
    /// Electricity cost (hours * watts/1000 * rate).
    pub electricity_cost: Option<f64>,
    /// Printer depreciation cost (printer_cost / lifetime_hours * hours).
    pub depreciation_cost: Option<f64>,
    /// Labor/setup cost (labor_rate * setup_minutes / 60).
    pub labor_cost: Option<f64>,
    /// Sum of all available cost components (`None` if all are `None`).
    pub total_cost: Option<f64>,
    /// Hints describing missing inputs that would enable additional cost components.
    pub missing_hints: Vec<String>,
}

/// Rough estimate derived from mesh volume alone.
///
/// Accuracy is limited (+/- 30-50%) because infill density, shell thickness,
/// and support material are approximated with a single fill factor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeEstimate {
    /// Estimated filament length in mm.
    pub filament_length_mm: f64,
    /// Estimated filament weight in grams.
    pub filament_weight_g: f64,
    /// Very rough print time estimate in seconds.
    pub rough_time_seconds: f64,
    /// Accuracy disclaimer.
    pub disclaimer: String,
}

/// Computes a cost estimate from the given inputs using progressive disclosure.
///
/// Each cost component is computed independently when its required inputs are
/// present and non-zero. Missing inputs generate user-facing hints.
///
/// # Formulas
///
/// - Filament: `weight_g * price_per_kg / 1000`
/// - Electricity: `hours * (watts / 1000) * rate`
/// - Depreciation: `(printer_cost / expected_hours) * hours`
/// - Labor: `labor_rate * (setup_minutes / 60)`
///
/// # Examples
///
/// ```
/// use slicecore_engine::cost_model::{CostInputs, compute_cost};
///
/// let inputs = CostInputs {
///     filament_weight_g: 100.0,
///     print_time_seconds: 7200.0,
///     filament_price_per_kg: Some(25.0),
///     electricity_rate: Some(0.12),
///     printer_watts: Some(200.0),
///     printer_cost: Some(500.0),
///     expected_hours: Some(2000.0),
///     labor_rate: Some(15.0),
///     setup_time_minutes: Some(10.0),
/// };
/// let est = compute_cost(&inputs);
/// assert!(est.filament_cost.is_some());
/// assert!(est.electricity_cost.is_some());
/// assert!(est.depreciation_cost.is_some());
/// assert!(est.labor_cost.is_some());
/// assert!(est.total_cost.is_some());
/// assert!(est.missing_hints.is_empty());
/// ```
pub fn compute_cost(inputs: &CostInputs) -> CostEstimate {
    let hours = inputs.print_time_seconds / 3600.0;
    let mut missing_hints = Vec::new();

    // Filament cost
    let filament_cost = match inputs.filament_price_per_kg {
        Some(price) if price > 0.0 => Some(inputs.filament_weight_g * price / 1000.0),
        _ => {
            missing_hints.push("provide --filament-price to calculate filament cost".to_string());
            None
        }
    };

    // Electricity cost (needs both watts and rate)
    let electricity_cost = match (inputs.printer_watts, inputs.electricity_rate) {
        (Some(watts), Some(rate)) if watts > 0.0 && rate > 0.0 => {
            Some(hours * (watts / 1000.0) * rate)
        }
        _ => {
            if inputs.printer_watts.map_or(true, |w| w <= 0.0) {
                missing_hints
                    .push("provide --printer-watts to calculate electricity cost".to_string());
            }
            if inputs.electricity_rate.map_or(true, |r| r <= 0.0) {
                missing_hints
                    .push("provide --electricity-rate to calculate electricity cost".to_string());
            }
            None
        }
    };

    // Depreciation cost (needs printer_cost and expected_hours > 0)
    let depreciation_cost = match (inputs.printer_cost, inputs.expected_hours) {
        (Some(cost), Some(hours_life)) if cost > 0.0 && hours_life > 0.0 => {
            Some((cost / hours_life) * hours)
        }
        _ => {
            if inputs.printer_cost.map_or(true, |c| c <= 0.0) {
                missing_hints
                    .push("provide --printer-cost to calculate depreciation cost".to_string());
            }
            if inputs.expected_hours.map_or(true, |h| h <= 0.0) {
                missing_hints
                    .push("provide --expected-hours to calculate depreciation cost".to_string());
            }
            None
        }
    };

    // Labor cost
    let labor_cost = match (inputs.labor_rate, inputs.setup_time_minutes) {
        (Some(rate), Some(mins)) if rate > 0.0 && mins > 0.0 => Some(rate * mins / 60.0),
        _ => {
            if inputs.labor_rate.map_or(true, |r| r <= 0.0) {
                missing_hints.push("provide --labor-rate to calculate labor cost".to_string());
            }
            if inputs.setup_time_minutes.map_or(true, |m| m <= 0.0) {
                missing_hints.push("provide --setup-time to calculate labor cost".to_string());
            }
            None
        }
    };

    // Total: sum of available components
    let components = [
        filament_cost,
        electricity_cost,
        depreciation_cost,
        labor_cost,
    ];
    let total_cost = {
        let sum: f64 = components.iter().filter_map(|c| *c).sum();
        if components.iter().any(|c| c.is_some()) {
            Some(sum)
        } else {
            None
        }
    };

    CostEstimate {
        filament_cost,
        electricity_cost,
        depreciation_cost,
        labor_cost,
        total_cost,
        missing_hints,
    }
}

/// Computes a rough filament and time estimate from mesh volume alone.
///
/// Uses a combined infill+shell factor of 0.50 to approximate the fraction
/// of the bounding volume that becomes actual printed material.
///
/// # Examples
///
/// ```
/// use slicecore_engine::cost_model::volume_estimate;
///
/// // A 20mm cube has volume 8000 mm^3
/// let est = volume_estimate(8000.0, 1.75, 1.24);
/// assert!(est.filament_length_mm > 0.0);
/// assert!(est.filament_weight_g > 0.0);
/// ```
pub fn volume_estimate(
    volume_mm3: f64,
    filament_diameter: f64,
    filament_density: f64,
) -> VolumeEstimate {
    let effective_volume = volume_mm3 * 0.50; // combined infill+shell factor
    let radius = filament_diameter / 2.0;
    let cross_section = std::f64::consts::PI * radius * radius;
    let filament_length_mm = if cross_section > 0.0 {
        effective_volume / cross_section
    } else {
        0.0
    };
    let filament_weight_g =
        filament_mm_to_grams(filament_length_mm, filament_diameter, filament_density);
    let rough_time_seconds = filament_length_mm / 40.0; // 40 mm/s average extrusion rate

    VolumeEstimate {
        filament_length_mm,
        filament_weight_g,
        rough_time_seconds,
        disclaimer: "+/- 30-50% accuracy".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_cost_all_inputs() {
        let inputs = CostInputs {
            filament_weight_g: 100.0,
            print_time_seconds: 7200.0, // 2 hours
            filament_price_per_kg: Some(25.0),
            electricity_rate: Some(0.12),
            printer_watts: Some(200.0),
            printer_cost: Some(500.0),
            expected_hours: Some(2000.0),
            labor_rate: Some(15.0),
            setup_time_minutes: Some(10.0),
        };
        let est = compute_cost(&inputs);

        // filament: 100 * 25 / 1000 = 2.50
        let fc = est.filament_cost.unwrap();
        assert!((fc - 2.50).abs() < 0.01, "filament_cost={fc}");

        // electricity: 2h * 200/1000 * 0.12 = 0.048
        let ec = est.electricity_cost.unwrap();
        assert!((ec - 0.048).abs() < 0.001, "electricity_cost={ec}");

        // depreciation: 500/2000 * 2 = 0.50
        let dc = est.depreciation_cost.unwrap();
        assert!((dc - 0.50).abs() < 0.01, "depreciation_cost={dc}");

        // labor: 15 * 10/60 = 2.50
        let lc = est.labor_cost.unwrap();
        assert!((lc - 2.50).abs() < 0.01, "labor_cost={lc}");

        // total: 2.50 + 0.048 + 0.50 + 2.50 = 5.548
        let total = est.total_cost.unwrap();
        assert!((total - 5.548).abs() < 0.01, "total={total}");

        assert!(est.missing_hints.is_empty());
    }

    #[test]
    fn test_compute_cost_filament_only() {
        let inputs = CostInputs {
            filament_weight_g: 50.0,
            print_time_seconds: 3600.0,
            filament_price_per_kg: Some(25.0),
            ..CostInputs::default()
        };
        let est = compute_cost(&inputs);

        assert!(est.filament_cost.is_some());
        assert!(est.electricity_cost.is_none());
        assert!(est.depreciation_cost.is_none());
        assert!(est.labor_cost.is_none());
        // Total should equal filament cost alone
        assert_eq!(est.total_cost, est.filament_cost);
    }

    #[test]
    fn test_compute_cost_zero_expected_hours_no_div_by_zero() {
        let inputs = CostInputs {
            filament_weight_g: 50.0,
            print_time_seconds: 3600.0,
            printer_cost: Some(500.0),
            expected_hours: Some(0.0), // zero => should not divide
            ..CostInputs::default()
        };
        let est = compute_cost(&inputs);
        assert!(est.depreciation_cost.is_none());
    }

    #[test]
    fn test_compute_cost_no_optional_inputs() {
        let inputs = CostInputs {
            filament_weight_g: 50.0,
            print_time_seconds: 3600.0,
            ..CostInputs::default()
        };
        let est = compute_cost(&inputs);

        assert!(est.filament_cost.is_none());
        assert!(est.electricity_cost.is_none());
        assert!(est.depreciation_cost.is_none());
        assert!(est.labor_cost.is_none());
        assert!(est.total_cost.is_none());
        // Should have hints for filament, electricity (watts + rate), depreciation (cost + hours), labor (rate + time)
        assert!(
            est.missing_hints.len() >= 4,
            "expected at least 4 hints, got {}",
            est.missing_hints.len()
        );
    }

    #[test]
    fn test_volume_estimate_20mm_cube() {
        // 20mm cube = 8000 mm^3
        let est = volume_estimate(8000.0, 1.75, 1.24);
        assert!(
            est.filament_length_mm > 100.0,
            "length={}",
            est.filament_length_mm
        );
        assert!(
            est.filament_length_mm < 10000.0,
            "length={}",
            est.filament_length_mm
        );
        assert!(
            est.filament_weight_g > 0.5,
            "weight={}",
            est.filament_weight_g
        );
        assert!(
            est.filament_weight_g < 50.0,
            "weight={}",
            est.filament_weight_g
        );
        assert!(est.rough_time_seconds > 0.0);
        assert!(est.disclaimer.contains("accuracy"));
    }

    #[test]
    fn test_machine_config_watts_default() {
        let config = crate::config::MachineConfig::default();
        assert!((config.watts - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_machine_config_watts_toml_roundtrip() {
        let config = crate::config::MachineConfig::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: crate::config::MachineConfig = toml::from_str(&toml_str).unwrap();
        assert!((parsed.watts - 0.0).abs() < f64::EPSILON);
    }
}
