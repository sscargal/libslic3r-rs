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
use slicecore_mesh::TriangleMesh;

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
                        result.push(GcodeCommand::SetExtruderTemp { temp, wait: false });
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

/// Generates a temperature tower mesh as stacked boxes.
///
/// Creates a 1mm base plate at the full width/depth, then stacks N blocks
/// (one per temperature step) on top. The number of blocks is determined by
/// the temperature range and step size.
///
/// # Examples
///
/// ```
/// use slicecore_engine::calibrate::{TempTowerParams, generate_temp_tower_mesh};
///
/// let params = TempTowerParams {
///     start_temp: 190.0, end_temp: 220.0, step: 10.0,
///     block_height: 10.0, base_width: 30.0, base_depth: 30.0,
/// };
/// let mesh = generate_temp_tower_mesh(&params);
/// assert!(mesh.triangle_count() > 0);
/// ```
#[must_use]
pub fn generate_temp_tower_mesh(params: &TempTowerParams) -> TriangleMesh {
    let num_blocks = ((params.end_temp - params.start_temp) / params.step).abs() as usize + 1;
    let total_height = 1.0 + num_blocks as f64 * params.block_height;
    build_stacked_tower(
        params.base_width,
        params.base_depth,
        params.block_height,
        num_blocks,
        total_height,
    )
}

/// Generates a retraction test mesh as stacked boxes.
///
/// Uses the same stacked-box structure as the temperature tower. Each block
/// corresponds to a retraction distance step, with fixed 8mm block height
/// and 30mm x 30mm base plus 1mm base plate.
///
/// # Examples
///
/// ```
/// use slicecore_engine::calibrate::{RetractionParams, generate_retraction_mesh};
///
/// let params = RetractionParams {
///     start_distance: 0.5, end_distance: 3.0, step: 0.5,
///     start_speed: 25.0, end_speed: 60.0,
/// };
/// let mesh = generate_retraction_mesh(&params);
/// assert!(mesh.triangle_count() > 0);
/// ```
#[must_use]
pub fn generate_retraction_mesh(params: &RetractionParams) -> TriangleMesh {
    let num_blocks =
        ((params.end_distance - params.start_distance) / params.step).abs() as usize + 1;
    let block_height = 8.0;
    let base_width = 30.0;
    let base_depth = 30.0;
    let total_height = 1.0 + num_blocks as f64 * block_height;
    build_stacked_tower(
        base_width,
        base_depth,
        block_height,
        num_blocks,
        total_height,
    )
}

/// Generates a retraction distance schedule mapping Z heights to retraction values.
///
/// Returns `(z_height, retraction_distance)` pairs. The base plate occupies
/// Z=0..1, then each block starts at Z = 1.0 + i * 8.0.
///
/// # Examples
///
/// ```
/// use slicecore_engine::calibrate::{RetractionParams, retraction_schedule};
///
/// let params = RetractionParams {
///     start_distance: 0.5, end_distance: 2.0, step: 0.5,
///     start_speed: 25.0, end_speed: 60.0,
/// };
/// let schedule = retraction_schedule(&params);
/// assert_eq!(schedule.len(), 4);
/// ```
#[must_use]
pub fn retraction_schedule(params: &RetractionParams) -> Vec<(f64, f64)> {
    let block_height = 8.0;
    let mut schedule = Vec::new();
    let mut dist = params.start_distance;
    let mut z = 1.0_f64; // after base plate
    while dist <= params.end_distance + f64::EPSILON {
        schedule.push((z, dist));
        dist += params.step;
        z += block_height;
    }
    schedule
}

/// Injects retraction distance comments at Z height boundaries.
///
/// At each Z boundary in the schedule, inserts a G-code comment indicating
/// the retraction distance that section documents. Does NOT modify actual
/// retraction settings -- the entire tower is sliced with the profile's
/// single retraction setting.
///
/// # Examples
///
/// ```
/// use slicecore_gcode_io::GcodeCommand;
/// use slicecore_engine::calibrate::inject_retraction_comments;
///
/// let commands = vec![
///     GcodeCommand::LinearMove { x: Some(10.0), y: None, z: Some(2.0), e: None, f: Some(600.0) },
///     GcodeCommand::LinearMove { x: Some(20.0), y: None, z: Some(10.0), e: None, f: Some(600.0) },
/// ];
/// let schedule = vec![(1.0, 1.0), (9.0, 1.5)];
/// let result = inject_retraction_comments(commands, &schedule);
/// assert!(result.len() > 2);
/// ```
pub fn inject_retraction_comments(
    commands: Vec<GcodeCommand>,
    schedule: &[(f64, f64)],
) -> Vec<GcodeCommand> {
    if schedule.is_empty() {
        return commands;
    }

    let mut result = Vec::with_capacity(commands.len() + schedule.len());
    let mut current_z = 0.0_f64;
    // Start before the first entry so it can be triggered
    let mut next_idx = 0_usize;

    for cmd in commands {
        let new_z = match &cmd {
            GcodeCommand::LinearMove { z: Some(z), .. }
            | GcodeCommand::RapidMove { z: Some(z), .. } => Some(*z),
            _ => None,
        };

        if let Some(z) = new_z {
            if z > current_z {
                while next_idx < schedule.len() && schedule[next_idx].0 <= z + f64::EPSILON {
                    let dist = schedule[next_idx].1;
                    result.push(GcodeCommand::Comment(format!(
                        "=== RETRACTION SECTION: {dist:.1}mm (print this block, evaluate stringing, adjust, reprint) ==="
                    )));
                    next_idx += 1;
                }
                current_z = z;
            }
        }

        result.push(cmd);
    }

    result
}

/// Generates a flow rate schedule mapping Z heights to flow percentages.
///
/// Returns `(z_height, flow_percent)` pairs. The base plate occupies
/// Z=0..1, then each block starts at Z = 1.0 + i * 5.0 (5mm per block).
/// Flow percentages are centered on the baseline multiplier.
///
/// # Examples
///
/// ```
/// use slicecore_engine::calibrate::{FlowParams, flow_schedule};
///
/// let params = FlowParams { baseline_multiplier: 1.0, step: 0.02, steps: 5 };
/// let schedule = flow_schedule(&params);
/// assert_eq!(schedule.len(), 5);
/// // First section should be lowest multiplier: 1.0 + (0 - 2) * 0.02 = 0.96 → 96%
/// assert!((schedule[0].1 - 96.0).abs() < 0.1);
/// ```
#[must_use]
pub fn flow_schedule(params: &FlowParams) -> Vec<(f64, f64)> {
    let block_height = 5.0;
    let mut schedule = Vec::new();
    let half = params.steps / 2;
    for i in 0..params.steps {
        let z = 1.0 + i as f64 * block_height;
        let offset = i as f64 - half as f64;
        let multiplier = params.baseline_multiplier + offset * params.step;
        let flow_percent = multiplier * 100.0;
        schedule.push((z, flow_percent));
    }
    schedule
}

/// Generates a flow calibration tower mesh.
///
/// Creates a stacked-box tower where each 5mm-tall block represents a
/// different flow rate section. The tower has 30mm x 30mm base with a
/// 1mm base plate, followed by `params.steps` blocks.
///
/// # Examples
///
/// ```
/// use slicecore_engine::calibrate::{FlowParams, generate_flow_mesh};
///
/// let params = FlowParams { baseline_multiplier: 1.0, step: 0.02, steps: 5 };
/// let mesh = generate_flow_mesh(&params);
/// assert!(mesh.triangle_count() > 0);
/// ```
#[must_use]
pub fn generate_flow_mesh(params: &FlowParams) -> TriangleMesh {
    let block_height = 5.0;
    let base_width = 30.0;
    let base_depth = 30.0;
    let total_height = 1.0 + params.steps as f64 * block_height;
    build_stacked_tower(
        base_width,
        base_depth,
        block_height,
        params.steps,
        total_height,
    )
}

/// Injects M221 flow rate override commands into G-code text at Z boundaries.
///
/// At each Z boundary in the schedule, inserts an `M221 S{percent}` command
/// to change the flow rate percentage and a comment labelling the section.
///
/// # Examples
///
/// ```
/// use slicecore_engine::calibrate::inject_flow_changes_text;
///
/// let gcode = "G1 Z2.0 E1.0\nG1 X10 Y10\nG1 Z7.0 E2.0\n";
/// let schedule = vec![(1.0, 95.0), (6.0, 100.0)];
/// let result = inject_flow_changes_text(gcode, &schedule, "");
/// assert!(result.contains("M221 S95"));
/// assert!(result.contains("M221 S100"));
/// ```
pub fn inject_flow_changes_text(gcode: &str, schedule: &[(f64, f64)], header: &str) -> String {
    let mut output = String::with_capacity(gcode.len() + header.len() + schedule.len() * 80);
    output.push_str(header);
    if !header.is_empty() {
        output.push('\n');
    }

    let mut current_z = 0.0_f64;
    let mut next_idx = 0_usize;

    for line in gcode.lines() {
        if let Some(z) = extract_z_from_gcode_line(line) {
            if z > current_z {
                while next_idx < schedule.len() && schedule[next_idx].0 <= z + f64::EPSILON {
                    let (sched_z, flow_pct) = schedule[next_idx];
                    let multiplier = flow_pct / 100.0;
                    output.push_str(&format!(
                        "; === FLOW RATE: {flow_pct:.0}% (multiplier: {multiplier:.2}) at Z={sched_z:.1}mm ===\n"
                    ));
                    output.push_str(&format!("M221 S{flow_pct:.0} ; set flow rate\n"));
                    next_idx += 1;
                }
                current_z = z;
            }
        }
        output.push_str(line);
        output.push('\n');
    }

    output
}

/// Generates a first layer calibration mesh as a flat plate.
///
/// Creates a thin box covering `coverage_percent` of the bed area, centered
/// on the bed. The plate height is set to `first_layer_height` (0.3mm) so
/// the slicer produces exactly one layer.
///
/// # Examples
///
/// ```
/// use slicecore_engine::calibrate::{FirstLayerParams, FirstLayerPattern, generate_first_layer_mesh};
///
/// let params = FirstLayerParams { pattern: FirstLayerPattern::Grid, coverage_percent: 80.0 };
/// let mesh = generate_first_layer_mesh(&params, 220.0, 220.0);
/// assert!(mesh.triangle_count() > 0);
/// let aabb = mesh.aabb();
/// let height = aabb.max.z - aabb.min.z;
/// assert!((height - 0.3).abs() < 0.01);
/// ```
#[must_use]
pub fn generate_first_layer_mesh(
    params: &FirstLayerParams,
    bed_x: f64,
    bed_y: f64,
) -> TriangleMesh {
    use slicecore_math::Point3;

    let coverage = params.coverage_percent / 100.0;
    let plate_x = bed_x * coverage;
    let plate_y = bed_y * coverage;
    let plate_z = 0.3; // single layer height

    let hx = plate_x / 2.0;
    let hy = plate_y / 2.0;

    let vertices = vec![
        Point3::new(-hx, -hy, 0.0),
        Point3::new(hx, -hy, 0.0),
        Point3::new(hx, hy, 0.0),
        Point3::new(-hx, hy, 0.0),
        Point3::new(-hx, -hy, plate_z),
        Point3::new(hx, -hy, plate_z),
        Point3::new(hx, hy, plate_z),
        Point3::new(-hx, hy, plate_z),
    ];

    let indices = vec![
        // Front (z=plate_z)
        [4_u32, 5, 6],
        [4, 6, 7],
        // Back (z=0)
        [1, 0, 3],
        [1, 3, 2],
        // Right (x=hx)
        [5, 1, 2],
        [5, 2, 6],
        // Left (x=-hx)
        [0, 4, 7],
        [0, 7, 3],
        // Top (y=hy)
        [3, 7, 6],
        [3, 6, 2],
        // Bottom (y=-hy)
        [4, 0, 1],
        [4, 1, 5],
    ];

    TriangleMesh::new(vertices, indices).expect("first layer plate mesh should be valid")
}

/// Extracts Z value from a G0/G1 G-code line.
fn extract_z_from_gcode_line(line: &str) -> Option<f64> {
    let trimmed = line.trim();
    if !trimmed.starts_with("G0 ") && !trimmed.starts_with("G1 ") {
        return None;
    }
    for token in trimmed.split_whitespace() {
        if let Some(z_str) = token.strip_prefix('Z') {
            return z_str.parse::<f64>().ok();
        }
    }
    None
}

/// Builds a stacked tower mesh (base plate + N blocks) directly from geometry.
///
/// Creates a single watertight mesh consisting of a 1mm base plate and
/// `num_blocks` stacked boxes of `block_height` each. Each box is added
/// as a separate set of vertices and triangles (the slicer handles
/// coincident faces correctly).
fn build_stacked_tower(
    width: f64,
    depth: f64,
    block_height: f64,
    num_blocks: usize,
    _total_height: f64,
) -> TriangleMesh {
    use slicecore_math::Point3;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Helper: add an axis-aligned box from (x0,y0,z0) to (x1,y1,z1)
    let mut add_box = |x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64| {
        let base_idx = vertices.len() as u32;
        vertices.extend_from_slice(&[
            Point3::new(x0, y0, z0), // 0: left-bottom-back
            Point3::new(x1, y0, z0), // 1: right-bottom-back
            Point3::new(x1, y1, z0), // 2: right-top-back
            Point3::new(x0, y1, z0), // 3: left-top-back
            Point3::new(x0, y0, z1), // 4: left-bottom-front
            Point3::new(x1, y0, z1), // 5: right-bottom-front
            Point3::new(x1, y1, z1), // 6: right-top-front
            Point3::new(x0, y1, z1), // 7: left-top-front
        ]);
        // 12 triangles, CCW winding from outside
        let b = base_idx;
        indices.extend_from_slice(&[
            // Front (z=z1)
            [b + 4, b + 5, b + 6],
            [b + 4, b + 6, b + 7],
            // Back (z=z0)
            [b + 1, b, b + 3],
            [b + 1, b + 3, b + 2],
            // Right (x=x1)
            [b + 5, b + 1, b + 2],
            [b + 5, b + 2, b + 6],
            // Left (x=x0)
            [b, b + 4, b + 7],
            [b, b + 7, b + 3],
            // Top (y=y1)
            [b + 3, b + 7, b + 6],
            [b + 3, b + 6, b + 2],
            // Bottom (y=y0)
            [b + 4, b, b + 1],
            [b + 4, b + 1, b + 5],
        ]);
    };

    let hw = width / 2.0;
    let hd = depth / 2.0;

    // Base plate: 1mm tall
    add_box(-hw, -hd, 0.0, hw, hd, 1.0);

    // Stacked blocks
    for i in 0..num_blocks {
        let z_bottom = 1.0 + i as f64 * block_height;
        let z_top = z_bottom + block_height;
        add_box(-hw, -hd, z_bottom, hw, hd, z_top);
    }

    TriangleMesh::new(vertices, indices).expect("stacked tower mesh should be valid")
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
        assert!(
            msg.contains("220.0"),
            "error should mention bed size: {msg}"
        );
        assert!(
            msg.contains("210.0"),
            "error should mention model size: {msg}"
        );
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

    #[test]
    fn test_generate_temp_tower_mesh() {
        let params = TempTowerParams {
            start_temp: 190.0,
            end_temp: 220.0,
            step: 10.0,
            block_height: 10.0,
            base_width: 30.0,
            base_depth: 30.0,
        };
        let mesh = generate_temp_tower_mesh(&params);
        assert!(mesh.triangle_count() > 0, "mesh should have triangles");
        assert!(mesh.vertex_count() > 0, "mesh should have vertices");
        // 4 blocks + 1 base = 5 boxes * 12 triangles = 60
        assert_eq!(mesh.triangle_count(), 60);
        // Check approximate height: 1mm base + 4 * 10mm = 41mm
        let aabb = mesh.aabb();
        let height = aabb.max.z - aabb.min.z;
        assert!(
            (height - 41.0).abs() < 0.1,
            "height should be ~41mm, got {height}"
        );
    }

    #[test]
    fn test_generate_retraction_mesh() {
        let params = RetractionParams {
            start_distance: 0.5,
            end_distance: 3.0,
            step: 0.5,
            start_speed: 25.0,
            end_speed: 60.0,
        };
        let mesh = generate_retraction_mesh(&params);
        assert!(mesh.triangle_count() > 0, "mesh should have triangles");
        assert!(mesh.vertex_count() > 0, "mesh should have vertices");
        // 6 blocks + 1 base = 7 boxes * 12 = 84 triangles
        assert_eq!(mesh.triangle_count(), 84);
    }

    #[test]
    fn test_retraction_schedule() {
        let params = RetractionParams {
            start_distance: 0.5,
            end_distance: 2.0,
            step: 0.5,
            start_speed: 25.0,
            end_speed: 60.0,
        };
        let schedule = retraction_schedule(&params);
        assert_eq!(schedule.len(), 4);
        assert!(
            (schedule[0].0 - 1.0).abs() < f64::EPSILON,
            "first section at Z=1.0"
        );
        assert!((schedule[0].1 - 0.5).abs() < f64::EPSILON);
        assert!(
            (schedule[1].0 - 9.0).abs() < f64::EPSILON,
            "second section at Z=9.0"
        );
        assert!((schedule[1].1 - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_inject_retraction_comments() {
        let commands = vec![
            GcodeCommand::LinearMove {
                x: Some(10.0),
                y: None,
                z: Some(2.0),
                e: None,
                f: Some(600.0),
            },
            GcodeCommand::LinearMove {
                x: Some(20.0),
                y: None,
                z: Some(10.0),
                e: None,
                f: Some(600.0),
            },
        ];
        let schedule = vec![(1.0, 1.0), (9.0, 1.5)];
        let result = inject_retraction_comments(commands, &schedule);
        // Should insert 2 comments (one at z>=1.0, one at z>=9.0)
        let comments: Vec<_> = result
            .iter()
            .filter(|c| matches!(c, GcodeCommand::Comment(_)))
            .collect();
        assert_eq!(comments.len(), 2, "should insert 2 retraction comments");
    }

    #[test]
    fn test_generate_flow_mesh() {
        let params = FlowParams {
            baseline_multiplier: 1.0,
            step: 0.02,
            steps: 5,
        };
        let mesh = generate_flow_mesh(&params);
        assert!(mesh.triangle_count() > 0, "mesh should have triangles");
        // 5 blocks + 1 base = 6 boxes * 12 = 72 triangles
        assert_eq!(mesh.triangle_count(), 72);
        let aabb = mesh.aabb();
        let height = aabb.max.z - aabb.min.z;
        // 1mm base + 5 * 5mm = 26mm
        assert!(
            (height - 26.0).abs() < 0.1,
            "height should be ~26mm, got {height}"
        );
    }

    #[test]
    fn test_flow_schedule() {
        let params = FlowParams {
            baseline_multiplier: 1.0,
            step: 0.02,
            steps: 5,
        };
        let schedule = flow_schedule(&params);
        assert_eq!(schedule.len(), 5);
        // With half=2, offsets are -2, -1, 0, 1, 2
        // flow_pct: 96, 98, 100, 102, 104
        assert!(
            (schedule[0].1 - 96.0).abs() < 0.1,
            "first should be 96%, got {}",
            schedule[0].1
        );
        assert!(
            (schedule[2].1 - 100.0).abs() < 0.1,
            "middle should be 100%, got {}",
            schedule[2].1
        );
        assert!(
            (schedule[4].1 - 104.0).abs() < 0.1,
            "last should be 104%, got {}",
            schedule[4].1
        );
    }

    #[test]
    fn test_inject_flow_changes_text() {
        let gcode = "G1 Z2.0 E1.0\nG1 X10 Y10\nG1 Z7.0 E2.0\n";
        let schedule = vec![(1.0, 95.0), (6.0, 100.0)];
        let result = inject_flow_changes_text(gcode, &schedule, "");
        assert!(
            result.contains("M221 S95"),
            "should contain M221 S95: {result}"
        );
        assert!(
            result.contains("M221 S100"),
            "should contain M221 S100: {result}"
        );
        assert!(
            result.contains("FLOW RATE"),
            "should contain flow rate comment: {result}"
        );
    }

    #[test]
    fn test_generate_first_layer_mesh() {
        let params = FirstLayerParams {
            pattern: FirstLayerPattern::Grid,
            coverage_percent: 80.0,
        };
        let mesh = generate_first_layer_mesh(&params, 220.0, 220.0);
        assert!(mesh.triangle_count() > 0, "mesh should have triangles");
        assert_eq!(mesh.triangle_count(), 12, "single box = 12 triangles");
        let aabb = mesh.aabb();
        let height = aabb.max.z - aabb.min.z;
        assert!(
            (height - 0.3).abs() < 0.01,
            "height should be ~0.3mm, got {height}"
        );
        let width = aabb.max.x - aabb.min.x;
        assert!(
            (width - 176.0).abs() < 0.1,
            "width should be ~176mm (80% of 220), got {width}"
        );
    }
}
