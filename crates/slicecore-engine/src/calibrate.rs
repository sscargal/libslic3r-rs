//! Core calibration types for test print generation.
//!
//! Provides parameter structs and utility functions shared across all
//! calibration test generators: temperature tower, retraction, flow rate,
//! and first layer adhesion tests.
//!
//! # Components
//!
//! - [`TempTowerParams`]: Temperature tower test parameters
//! - [`RetractionParams`]: Retraction distance/speed test parameters
//! - [`FlowParams`]: Flow rate multiplier test parameters
//! - [`FirstLayerParams`]: First layer adhesion test parameters
//! - [`validate_bed_fit`]: Checks whether a model fits the printer bed
//! - [`inject_temp_changes`]: Inserts temperature changes at Z boundaries
//! - [`temp_schedule`]: Generates Z-height to temperature mappings

use serde::{Deserialize, Serialize};
use slicecore_gcode_io::GcodeCommand;

use crate::config::{FilamentPropsConfig, MachineConfig, PrintConfig};

/// Parameters for a temperature tower calibration test.
///
/// A temperature tower prints identical blocks stacked vertically, each at a
/// different nozzle temperature. The user inspects print quality per block to
/// find the optimal temperature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempTowerParams {
    /// Starting (lowest) temperature in degrees C.
    pub start_temp: f64,
    /// Ending (highest) temperature in degrees C.
    pub end_temp: f64,
    /// Temperature step between blocks in degrees C.
    pub step: f64,
    /// Height of each temperature block in mm.
    pub block_height: f64,
    /// Width of the tower base in mm.
    pub base_width: f64,
    /// Depth of the tower base in mm.
    pub base_depth: f64,
}

impl TempTowerParams {
    /// Creates temperature tower parameters derived from filament config.
    ///
    /// Uses the filament's temperature range to set start/end temps,
    /// with 5-degree steps and 10mm blocks.
    ///
    /// # Examples
    ///
    /// ```
    /// use slicecore_engine::config::FilamentPropsConfig;
    /// use slicecore_engine::calibrate::TempTowerParams;
    ///
    /// let filament = FilamentPropsConfig::default();
    /// let params = TempTowerParams::from_filament(&filament);
    /// assert!(params.start_temp < params.end_temp);
    /// ```
    #[must_use]
    pub fn from_filament(filament: &FilamentPropsConfig) -> Self {
        Self {
            start_temp: filament.nozzle_temperature_range_low,
            end_temp: filament.nozzle_temperature_range_high,
            step: 5.0,
            block_height: 10.0,
            base_width: 30.0,
            base_depth: 30.0,
        }
    }
}

/// Parameters for a retraction calibration test.
///
/// Prints test patterns with varying retraction distances and speeds to
/// find optimal retraction settings that minimize stringing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetractionParams {
    /// Starting retraction distance in mm.
    pub start_distance: f64,
    /// Ending retraction distance in mm.
    pub end_distance: f64,
    /// Distance step between test sections in mm.
    pub step: f64,
    /// Starting retraction speed in mm/s.
    pub start_speed: f64,
    /// Ending retraction speed in mm/s.
    pub end_speed: f64,
}

impl RetractionParams {
    /// Creates retraction parameters derived from filament config.
    ///
    /// Centers the test range around the filament's retraction length
    /// (or 1.0mm default), spanning +/- 2mm.
    ///
    /// # Examples
    ///
    /// ```
    /// use slicecore_engine::config::FilamentPropsConfig;
    /// use slicecore_engine::calibrate::RetractionParams;
    ///
    /// let filament = FilamentPropsConfig::default();
    /// let params = RetractionParams::from_filament(&filament);
    /// assert!(params.start_distance < params.end_distance);
    /// ```
    #[must_use]
    pub fn from_filament(filament: &FilamentPropsConfig) -> Self {
        let base = filament.filament_retraction_length.unwrap_or(1.0);
        let start = (base - 2.0).max(0.0);
        let end = base + 2.0;
        Self {
            start_distance: start,
            end_distance: end,
            step: 0.5,
            start_speed: 25.0,
            end_speed: 60.0,
        }
    }
}

/// Parameters for a flow rate calibration test.
///
/// Prints test walls with varying extrusion multipliers to find the
/// optimal flow rate that produces correct wall thickness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowParams {
    /// Baseline extrusion multiplier (typically from config).
    pub baseline_multiplier: f64,
    /// Step size for multiplier variation.
    pub step: f64,
    /// Number of test steps (both above and below baseline).
    pub steps: usize,
}

impl FlowParams {
    /// Creates flow parameters derived from print config.
    ///
    /// Centers on the config's extrusion multiplier with +/- 5% range.
    ///
    /// # Examples
    ///
    /// ```
    /// use slicecore_engine::config::PrintConfig;
    /// use slicecore_engine::calibrate::FlowParams;
    ///
    /// let config = PrintConfig::default();
    /// let params = FlowParams::from_config(&config);
    /// assert!(params.steps > 0);
    /// ```
    #[must_use]
    pub fn from_config(config: &PrintConfig) -> Self {
        Self {
            baseline_multiplier: config.extrusion_multiplier,
            step: 0.01,
            steps: 5,
        }
    }
}

/// Pattern type for first layer calibration tests.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FirstLayerPattern {
    /// Grid pattern (perpendicular lines).
    Grid,
    /// Parallel lines.
    Lines,
    /// Concentric pattern from outside in.
    Concentric,
}

/// Parameters for a first layer adhesion calibration test.
///
/// Prints a thin pattern covering a portion of the bed to help dial in
/// first layer height and flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirstLayerParams {
    /// Pattern type for the first layer test.
    pub pattern: FirstLayerPattern,
    /// Percentage of bed to cover (0.0-100.0).
    pub coverage_percent: f64,
}

/// Validates that a model of given dimensions fits the printer bed.
///
/// Requires a 10mm margin on all sides and checks height against
/// `printable_height`.
///
/// # Errors
///
/// Returns a descriptive error string if the model does not fit.
///
/// # Examples
///
/// ```
/// use slicecore_engine::config::MachineConfig;
/// use slicecore_engine::calibrate::validate_bed_fit;
///
/// let machine = MachineConfig::default(); // 220x220x250
/// assert!(validate_bed_fit(200.0, 200.0, 200.0, &machine).is_ok());
/// assert!(validate_bed_fit(220.0, 220.0, 200.0, &machine).is_err());
/// ```
pub fn validate_bed_fit(
    width: f64,
    depth: f64,
    height: f64,
    machine: &MachineConfig,
) -> Result<(), String> {
    let margin = 10.0;
    let max_w = machine.bed_x - 2.0 * margin;
    let max_d = machine.bed_y - 2.0 * margin;
    let max_h = machine.printable_height;

    if width > max_w || depth > max_d {
        return Err(format!(
            "model ({width:.1}x{depth:.1} mm) exceeds bed ({:.1}x{:.1} mm with {margin}mm margin)",
            machine.bed_x, machine.bed_y
        ));
    }
    if height > max_h {
        return Err(format!(
            "model height ({height:.1} mm) exceeds printable height ({max_h:.1} mm)"
        ));
    }
    Ok(())
}

/// Generates a temperature schedule from temp tower parameters.
///
/// Returns a list of `(z_height, temperature)` pairs, one per block.
/// The first block starts at Z=0.
///
/// # Examples
///
/// ```
/// use slicecore_engine::calibrate::{TempTowerParams, temp_schedule};
///
/// let params = TempTowerParams {
///     start_temp: 190.0, end_temp: 220.0, step: 10.0,
///     block_height: 10.0, base_width: 30.0, base_depth: 30.0,
/// };
/// let schedule = temp_schedule(&params);
/// assert_eq!(schedule.len(), 4); // 190, 200, 210, 220
/// assert!((schedule[0].0 - 0.0).abs() < f64::EPSILON);
/// assert!((schedule[0].1 - 190.0).abs() < f64::EPSILON);
/// ```
#[must_use]
pub fn temp_schedule(params: &TempTowerParams) -> Vec<(f64, f64)> {
    let mut schedule = Vec::new();
    let mut temp = params.start_temp;
    let mut z = 0.0_f64;
    while temp <= params.end_temp + f64::EPSILON {
        schedule.push((z, temp));
        temp += params.step;
        z += params.block_height;
    }
    schedule
}

/// Injects temperature change commands at Z height boundaries.
///
/// Scans through G-code commands tracking the current Z position from
/// `LinearMove` and `RapidMove` commands. When Z crosses a boundary in
/// the schedule, inserts a `Comment` and `SetExtruderTemp` command.
///
/// # Examples
///
/// ```
/// use slicecore_gcode_io::GcodeCommand;
/// use slicecore_engine::calibrate::inject_temp_changes;
///
/// let commands = vec![
///     GcodeCommand::LinearMove { x: Some(10.0), y: Some(10.0), z: Some(5.0), e: Some(1.0), f: Some(600.0) },
///     GcodeCommand::LinearMove { x: Some(20.0), y: Some(20.0), z: Some(15.0), e: Some(2.0), f: Some(600.0) },
/// ];
/// let schedule = vec![(0.0, 200.0), (10.0, 210.0)];
/// let result = inject_temp_changes(commands, &schedule);
/// // Should have injected a temp change before the z=15 move
/// assert!(result.len() > 2);
/// ```
pub fn inject_temp_changes(
    commands: Vec<GcodeCommand>,
    schedule: &[(f64, f64)],
) -> Vec<GcodeCommand> {
    if schedule.is_empty() {
        return commands;
    }

    let mut result = Vec::with_capacity(commands.len() + schedule.len() * 2);
    let mut current_z = 0.0_f64;
    let mut current_schedule_idx = 0_usize;

    // Find initial schedule entry
    for (i, &(z, _)) in schedule.iter().enumerate() {
        if z <= current_z + f64::EPSILON {
            current_schedule_idx = i;
        } else {
            break;
        }
    }

    for cmd in commands {
        // Extract Z from move commands
        let new_z = match &cmd {
            GcodeCommand::LinearMove { z: Some(z), .. }
            | GcodeCommand::RapidMove { z: Some(z), .. } => Some(*z),
            _ => None,
        };

        if let Some(z) = new_z {
            if z > current_z {
                // Check if we crossed a schedule boundary
                for (i, &(sched_z, temp)) in schedule.iter().enumerate() {
                    if i > current_schedule_idx && sched_z <= z + f64::EPSILON {
                        result.push(GcodeCommand::Comment(format!(
                            "Temperature change to {temp:.0}C at Z={sched_z:.1}"
                        )));
                        result.push(GcodeCommand::SetExtruderTemp {
                            temp,
                            wait: false,
                        });
                        current_schedule_idx = i;
                    }
                }
                current_z = z;
            }
        }

        result.push(cmd);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_bed_fit_within_bounds() {
        let machine = MachineConfig::default(); // 220x220x250
        assert!(validate_bed_fit(100.0, 100.0, 100.0, &machine).is_ok());
        assert!(validate_bed_fit(200.0, 200.0, 250.0, &machine).is_ok());
    }

    #[test]
    fn test_validate_bed_fit_rejects_oversized() {
        let machine = MachineConfig::default(); // 220x220x250
        // 210 > 220 - 20 = 200
        let result = validate_bed_fit(210.0, 100.0, 100.0, &machine);
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("220.0"), "error should mention bed size: {msg}");
        assert!(msg.contains("210.0"), "error should mention model size: {msg}");
    }

    #[test]
    fn test_validate_bed_fit_rejects_too_tall() {
        let machine = MachineConfig::default(); // 220x220x250
        let result = validate_bed_fit(100.0, 100.0, 300.0, &machine);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("300.0"));
    }

    #[test]
    fn test_temp_schedule_generates_correct_pairs() {
        let params = TempTowerParams {
            start_temp: 190.0,
            end_temp: 220.0,
            step: 10.0,
            block_height: 10.0,
            base_width: 30.0,
            base_depth: 30.0,
        };
        let schedule = temp_schedule(&params);
        assert_eq!(schedule.len(), 4);
        assert!((schedule[0].0 - 0.0).abs() < f64::EPSILON);
        assert!((schedule[0].1 - 190.0).abs() < f64::EPSILON);
        assert!((schedule[1].0 - 10.0).abs() < f64::EPSILON);
        assert!((schedule[1].1 - 200.0).abs() < f64::EPSILON);
        assert!((schedule[2].0 - 20.0).abs() < f64::EPSILON);
        assert!((schedule[2].1 - 210.0).abs() < f64::EPSILON);
        assert!((schedule[3].0 - 30.0).abs() < f64::EPSILON);
        assert!((schedule[3].1 - 220.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_inject_temp_changes_inserts_at_correct_z() {
        let commands = vec![
            GcodeCommand::Comment("start".to_string()),
            GcodeCommand::LinearMove {
                x: Some(10.0),
                y: Some(10.0),
                z: Some(5.0),
                e: Some(1.0),
                f: Some(600.0),
            },
            GcodeCommand::LinearMove {
                x: Some(20.0),
                y: Some(20.0),
                z: Some(12.0),
                e: Some(2.0),
                f: Some(600.0),
            },
            GcodeCommand::LinearMove {
                x: Some(30.0),
                y: Some(30.0),
                z: Some(25.0),
                e: Some(3.0),
                f: Some(600.0),
            },
        ];
        let schedule = vec![(0.0, 200.0), (10.0, 210.0), (20.0, 220.0)];
        let result = inject_temp_changes(commands, &schedule);

        // Should have: comment, move(z=5), temp_comment, temp_set, move(z=12),
        // temp_comment, temp_set, move(z=25) = 8 total
        assert_eq!(result.len(), 8, "got {} commands: {result:?}", result.len());

        // Check that the temp change at z=10 was inserted before the z=12 move
        let temp_changes: Vec<_> = result
            .iter()
            .filter_map(|c| match c {
                GcodeCommand::SetExtruderTemp { temp, .. } => Some(*temp),
                _ => None,
            })
            .collect();
        assert_eq!(temp_changes, vec![210.0, 220.0]);
    }

    #[test]
    fn test_temp_tower_from_filament() {
        let filament = FilamentPropsConfig::default();
        let params = TempTowerParams::from_filament(&filament);
        assert!((params.start_temp - 190.0).abs() < f64::EPSILON);
        assert!((params.end_temp - 240.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_retraction_from_filament() {
        let filament = FilamentPropsConfig::default();
        let params = RetractionParams::from_filament(&filament);
        assert!(params.start_distance < params.end_distance);
    }

    #[test]
    fn test_flow_from_config() {
        let config = PrintConfig::default();
        let params = FlowParams::from_config(&config);
        assert!(params.steps > 0);
        assert!((params.baseline_multiplier - config.extrusion_multiplier).abs() < f64::EPSILON);
    }
}
