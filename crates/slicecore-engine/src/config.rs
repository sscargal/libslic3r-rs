//! Print configuration for the slicing pipeline.
//!
//! [`PrintConfig`] contains all parameters that control the slicing and
//! G-code generation pipeline. It is designed for TOML deserialization with
//! `#[serde(default)]`, so any field not specified in the TOML input will
//! use sensible FDM defaults.
//!
//! [`WallOrder`] controls whether perimeters are printed inside-out or
//! outside-in.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use slicecore_gcode_io::GcodeDialect;

use crate::custom_gcode::CustomGcodeHooks;
use crate::error::EngineError;
use crate::flow_control::PerFeatureFlow;
use crate::infill::InfillPattern;
use crate::ironing::IroningConfig;
use crate::seam::SeamPosition;
use crate::support::config::SupportConfig;

/// Controls the order in which perimeter walls are printed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallOrder {
    /// Print inner walls first, then outer wall.
    InnerFirst,
    /// Print outer wall first, then inner walls.
    #[default]
    OuterFirst,
}

// ============================================================================
// Sub-config structs for organized field grouping
// ============================================================================

/// Per-feature line width configuration.
///
/// Controls the extrusion width for different feature types. A value of 0.0
/// typically means "auto from nozzle diameter". Defaults are BambuStudio
/// reference values for a 0.4mm nozzle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LineWidthConfig {
    /// Outer wall line width in mm.
    pub outer_wall: f64,
    /// Inner wall line width in mm.
    pub inner_wall: f64,
    /// Sparse infill line width in mm.
    pub infill: f64,
    /// Top surface line width in mm.
    pub top_surface: f64,
    /// Initial (first) layer line width in mm.
    pub initial_layer: f64,
    /// Internal solid infill line width in mm.
    pub internal_solid_infill: f64,
    /// Support structure line width in mm.
    pub support: f64,
}

impl Default for LineWidthConfig {
    fn default() -> Self {
        Self {
            outer_wall: 0.42,
            inner_wall: 0.45,
            infill: 0.45,
            top_surface: 0.42,
            initial_layer: 0.5,
            internal_solid_infill: 0.42,
            support: 0.42,
        }
    }
}

/// Per-feature speed configuration (mm/s).
///
/// A value of 0.0 means "inherit from the parent speed" (e.g., inner_wall
/// inherits from perimeter_speed). This matches upstream slicer behavior
/// where 0 indicates automatic/inherited speed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SpeedConfig {
    /// Bridge print speed (mm/s).
    pub bridge: f64,
    /// Inner wall speed (mm/s, 0 = inherit from perimeter_speed).
    pub inner_wall: f64,
    /// Gap fill speed (mm/s, 0 = inherit from perimeter_speed).
    pub gap_fill: f64,
    /// Top surface speed (mm/s, 0 = inherit from perimeter_speed).
    pub top_surface: f64,
    /// Internal solid infill speed (mm/s, 0 = inherit).
    pub internal_solid_infill: f64,
    /// Initial layer infill speed (mm/s, 0 = inherit).
    pub initial_layer_infill: f64,
    /// Support structure speed (mm/s, 0 = inherit).
    pub support: f64,
    /// Support interface speed (mm/s, 0 = inherit).
    pub support_interface: f64,
    /// Small perimeter speed (mm/s, 0 = inherit from perimeter_speed).
    pub small_perimeter: f64,
    /// Solid infill speed (mm/s, 0 = inherit).
    pub solid_infill: f64,
    /// Overhang speed for 0-25% overhang (mm/s, 0 = inherit).
    pub overhang_1_4: f64,
    /// Overhang speed for 25-50% overhang (mm/s, 0 = inherit).
    pub overhang_2_4: f64,
    /// Overhang speed for 50-75% overhang (mm/s, 0 = inherit).
    pub overhang_3_4: f64,
    /// Overhang speed for 75-100% overhang (mm/s, 0 = inherit).
    pub overhang_4_4: f64,
    /// Z-axis travel speed (mm/s, 0 = use travel_speed).
    pub travel_z: f64,
}

impl Default for SpeedConfig {
    fn default() -> Self {
        Self {
            bridge: 25.0,
            inner_wall: 0.0,
            gap_fill: 0.0,
            top_surface: 0.0,
            internal_solid_infill: 0.0,
            initial_layer_infill: 0.0,
            support: 0.0,
            support_interface: 0.0,
            small_perimeter: 0.0,
            solid_infill: 0.0,
            overhang_1_4: 0.0,
            overhang_2_4: 0.0,
            overhang_3_4: 0.0,
            overhang_4_4: 0.0,
            travel_z: 0.0,
        }
    }
}

/// Cooling and fan configuration.
///
/// Controls fan speeds, layer-time-based slowdown, and overhang cooling.
/// Percentage values are 0-100 (not 0-1 fraction).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CoolingConfig {
    /// Maximum fan speed (percentage, 0-100).
    pub fan_max_speed: f64,
    /// Minimum fan speed (percentage, 0-100).
    pub fan_min_speed: f64,
    /// Slow down if layer time falls below this value (seconds).
    pub slow_down_layer_time: f64,
    /// Minimum speed when slowing down for layer cooling (mm/s).
    pub slow_down_min_speed: f64,
    /// Fan speed for overhang regions (percentage, 0-100).
    pub overhang_fan_speed: f64,
    /// Overhang angle threshold for fan override (degrees).
    pub overhang_fan_threshold: f64,
    /// Layer number at which fan reaches full speed (0 = immediate).
    pub full_fan_speed_layer: u32,
    /// Enable automatic slowdown for layer cooling.
    pub slow_down_for_layer_cooling: bool,
}

impl Default for CoolingConfig {
    fn default() -> Self {
        Self {
            fan_max_speed: 100.0,
            fan_min_speed: 35.0,
            slow_down_layer_time: 5.0,
            slow_down_min_speed: 10.0,
            overhang_fan_speed: 100.0,
            overhang_fan_threshold: 25.0,
            full_fan_speed_layer: 0,
            slow_down_for_layer_cooling: true,
        }
    }
}

/// Additional retraction configuration.
///
/// Note: The existing flat `retract_length`, `retract_speed`, `retract_z_hop`,
/// and `min_travel_for_retract` fields on `PrintConfig` are NOT moved here yet
/// (migration happens in Plan 04). These are ADDITIONAL retraction fields not
/// previously in `PrintConfig`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RetractionConfig {
    /// Deretraction (unretract) speed in mm/s (0 = same as retraction speed).
    pub deretraction_speed: f64,
    /// Percentage of retraction to perform before wipe (0-100).
    pub retract_before_wipe: f64,
    /// Whether to retract when changing layers.
    pub retract_when_changing_layer: bool,
    /// Enable wipe move during retraction.
    pub wipe: bool,
    /// Wipe distance in mm.
    pub wipe_distance: f64,
}

impl Default for RetractionConfig {
    fn default() -> Self {
        Self {
            deretraction_speed: 0.0,
            retract_before_wipe: 0.0,
            retract_when_changing_layer: false,
            wipe: false,
            wipe_distance: 0.0,
        }
    }
}

/// Machine/printer hardware configuration.
///
/// Contains printer capabilities, motion limits, G-code templates, and
/// multi-extruder array fields. Vec fields use single-element defaults
/// for single-extruder printers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MachineConfig {
    /// Maximum printable height in mm.
    pub printable_height: f64,
    /// Maximum X acceleration (mm/s^2).
    pub max_acceleration_x: f64,
    /// Maximum Y acceleration (mm/s^2).
    pub max_acceleration_y: f64,
    /// Maximum Z acceleration (mm/s^2).
    pub max_acceleration_z: f64,
    /// Maximum E (extruder) acceleration (mm/s^2).
    pub max_acceleration_e: f64,
    /// Maximum acceleration while extruding (mm/s^2).
    pub max_acceleration_extruding: f64,
    /// Maximum acceleration while retracting (mm/s^2).
    pub max_acceleration_retracting: f64,
    /// Maximum acceleration during travel moves (mm/s^2).
    pub max_acceleration_travel: f64,
    /// Maximum X speed (mm/s).
    pub max_speed_x: f64,
    /// Maximum Y speed (mm/s).
    pub max_speed_y: f64,
    /// Maximum Z speed (mm/s).
    pub max_speed_z: f64,
    /// Maximum E (extruder) speed (mm/s).
    pub max_speed_e: f64,
    /// Nozzle diameters per extruder (mm). Multi-extruder array.
    pub nozzle_diameters: Vec<f64>,
    /// Jerk values for X axis per extruder (mm/s). Multi-extruder array.
    pub jerk_values_x: Vec<f64>,
    /// Jerk values for Y axis per extruder (mm/s). Multi-extruder array.
    pub jerk_values_y: Vec<f64>,
    /// Jerk values for Z axis per extruder (mm/s). Multi-extruder array.
    pub jerk_values_z: Vec<f64>,
    /// Jerk values for E axis per extruder (mm/s). Multi-extruder array.
    pub jerk_values_e: Vec<f64>,
    /// Machine start G-code template.
    pub start_gcode: String,
    /// Machine end G-code template.
    pub end_gcode: String,
    /// G-code inserted at every layer change.
    pub layer_change_gcode: String,
    /// Nozzle type descriptor (e.g., "hardened_steel", "brass").
    pub nozzle_type: String,
    /// Printer model identifier.
    pub printer_model: String,
    /// Bed shape descriptor (serialized polygon or rectangle).
    pub bed_shape: String,
    /// Minimum layer height the printer can handle (mm).
    pub min_layer_height: f64,
    /// Maximum layer height (mm, 0 = auto from nozzle diameter).
    pub max_layer_height: f64,
}

impl Default for MachineConfig {
    fn default() -> Self {
        Self {
            printable_height: 250.0,
            max_acceleration_x: 5000.0,
            max_acceleration_y: 5000.0,
            max_acceleration_z: 100.0,
            max_acceleration_e: 5000.0,
            max_acceleration_extruding: 5000.0,
            max_acceleration_retracting: 5000.0,
            max_acceleration_travel: 5000.0,
            max_speed_x: 500.0,
            max_speed_y: 500.0,
            max_speed_z: 12.0,
            max_speed_e: 120.0,
            nozzle_diameters: vec![0.4],
            jerk_values_x: vec![8.0],
            jerk_values_y: vec![8.0],
            jerk_values_z: vec![0.4],
            jerk_values_e: vec![2.5],
            start_gcode: String::new(),
            end_gcode: String::new(),
            layer_change_gcode: String::new(),
            nozzle_type: String::new(),
            printer_model: String::new(),
            bed_shape: String::new(),
            min_layer_height: 0.07,
            max_layer_height: 0.0,
        }
    }
}

impl MachineConfig {
    /// Returns the primary nozzle diameter (first extruder), or 0.4 if empty.
    pub fn nozzle_diameter(&self) -> f64 {
        self.nozzle_diameters.first().copied().unwrap_or(0.4)
    }

    /// Returns the primary X jerk value (first extruder), or 8.0 if empty.
    pub fn jerk_x(&self) -> f64 {
        self.jerk_values_x.first().copied().unwrap_or(8.0)
    }

    /// Returns the primary Y jerk value (first extruder), or 8.0 if empty.
    pub fn jerk_y(&self) -> f64 {
        self.jerk_values_y.first().copied().unwrap_or(8.0)
    }

    /// Returns the primary Z jerk value (first extruder), or 0.4 if empty.
    pub fn jerk_z(&self) -> f64 {
        self.jerk_values_z.first().copied().unwrap_or(0.4)
    }

    /// Returns the primary E jerk value (first extruder), or 2.5 if empty.
    pub fn jerk_e(&self) -> f64 {
        self.jerk_values_e.first().copied().unwrap_or(2.5)
    }
}

/// Per-feature acceleration configuration (mm/s^2).
///
/// A value of 0.0 means "use the base print_acceleration". These are
/// ADDITIONAL per-feature acceleration fields; the existing flat
/// `print_acceleration` and `travel_acceleration` on `PrintConfig` are
/// NOT moved here yet (migration happens in Plan 04).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AccelerationConfig {
    /// Outer wall acceleration (mm/s^2, 0 = use print_acceleration).
    pub outer_wall: f64,
    /// Inner wall acceleration (mm/s^2, 0 = use print_acceleration).
    pub inner_wall: f64,
    /// Initial layer acceleration (mm/s^2, 0 = use print_acceleration).
    pub initial_layer: f64,
    /// Initial layer travel acceleration (mm/s^2, 0 = use travel_acceleration).
    pub initial_layer_travel: f64,
    /// Top surface acceleration (mm/s^2, 0 = use print_acceleration).
    pub top_surface: f64,
    /// Sparse infill acceleration (mm/s^2, 0 = use print_acceleration).
    pub sparse_infill: f64,
    /// Bridge acceleration (mm/s^2, 0 = use print_acceleration).
    pub bridge: f64,
}

impl Default for AccelerationConfig {
    fn default() -> Self {
        Self {
            outer_wall: 0.0,
            inner_wall: 0.0,
            initial_layer: 0.0,
            initial_layer_travel: 0.0,
            top_surface: 0.0,
            sparse_infill: 0.0,
            bridge: 0.0,
        }
    }
}

/// Filament properties configuration.
///
/// Contains filament metadata, temperature ranges, per-extruder temperature
/// arrays, and filament-specific retraction overrides. The existing flat
/// `filament_density`, `filament_cost_per_kg`, and `filament_diameter` fields
/// on `PrintConfig` are NOT moved here yet (migration happens in Plan 04).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FilamentPropsConfig {
    /// Filament material type (e.g., "PLA", "ABS", "PETG").
    pub filament_type: String,
    /// Filament vendor/manufacturer name.
    pub filament_vendor: String,
    /// Maximum volumetric speed (mm^3/s, 0 = unlimited).
    pub max_volumetric_speed: f64,
    /// Low end of recommended nozzle temperature range (degrees C).
    pub nozzle_temperature_range_low: f64,
    /// High end of recommended nozzle temperature range (degrees C).
    pub nozzle_temperature_range_high: f64,
    /// Per-extruder nozzle temperatures (degrees C). Multi-extruder array.
    pub nozzle_temperatures: Vec<f64>,
    /// Per-extruder bed temperatures (degrees C). Multi-extruder array.
    pub bed_temperatures: Vec<f64>,
    /// Per-extruder first layer nozzle temperatures (degrees C). Multi-extruder array.
    pub first_layer_nozzle_temperatures: Vec<f64>,
    /// Per-extruder first layer bed temperatures (degrees C). Multi-extruder array.
    pub first_layer_bed_temperatures: Vec<f64>,
    /// Filament-specific retraction length override (mm, None = use global).
    pub filament_retraction_length: Option<f64>,
    /// Filament-specific retraction speed override (mm/s, None = use global).
    pub filament_retraction_speed: Option<f64>,
    /// Filament start G-code (run once when filament loaded).
    pub filament_start_gcode: String,
    /// Filament end G-code (run once when filament unloaded).
    pub filament_end_gcode: String,
}

impl Default for FilamentPropsConfig {
    fn default() -> Self {
        Self {
            filament_type: String::new(),
            filament_vendor: String::new(),
            max_volumetric_speed: 0.0,
            nozzle_temperature_range_low: 190.0,
            nozzle_temperature_range_high: 240.0,
            nozzle_temperatures: vec![200.0],
            bed_temperatures: vec![60.0],
            first_layer_nozzle_temperatures: vec![210.0],
            first_layer_bed_temperatures: vec![65.0],
            filament_retraction_length: None,
            filament_retraction_speed: None,
            filament_start_gcode: String::new(),
            filament_end_gcode: String::new(),
        }
    }
}

impl FilamentPropsConfig {
    /// Returns the primary nozzle temperature (first extruder), or 200.0 if empty.
    pub fn nozzle_temp(&self) -> f64 {
        self.nozzle_temperatures.first().copied().unwrap_or(200.0)
    }

    /// Returns the primary bed temperature (first extruder), or 60.0 if empty.
    pub fn bed_temp(&self) -> f64 {
        self.bed_temperatures.first().copied().unwrap_or(60.0)
    }

    /// Returns the primary first layer nozzle temperature, or 210.0 if empty.
    pub fn first_layer_nozzle_temp(&self) -> f64 {
        self.first_layer_nozzle_temperatures
            .first()
            .copied()
            .unwrap_or(210.0)
    }

    /// Returns the primary first layer bed temperature, or 65.0 if empty.
    pub fn first_layer_bed_temp(&self) -> f64 {
        self.first_layer_bed_temperatures
            .first()
            .copied()
            .unwrap_or(65.0)
    }
}

/// Print configuration controlling the entire slicing pipeline.
///
/// All fields have sensible FDM defaults. Use [`PrintConfig::from_toml`] to
/// parse from a TOML string, [`PrintConfig::from_json`] to parse from a JSON
/// string (native or OrcaSlicer/BambuStudio format), or [`PrintConfig::from_file`]
/// to auto-detect the format and load from a file. Fields not specified in the
/// input use defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrintConfig {
    // --- Layer geometry ---
    /// Standard layer height in mm.
    pub layer_height: f64,
    /// First layer height in mm (typically thicker for bed adhesion).
    pub first_layer_height: f64,
    /// Nozzle diameter in mm.
    pub nozzle_diameter: f64,

    // --- Walls ---
    /// Number of perimeter walls.
    pub wall_count: u32,
    /// Order in which walls are printed.
    pub wall_order: WallOrder,
    /// Seam placement strategy for perimeter loops.
    pub seam_position: SeamPosition,

    // --- Infill ---
    /// Infill pattern to use for sparse infill regions.
    pub infill_pattern: InfillPattern,
    /// Infill density as a fraction (0.0 = hollow, 1.0 = solid).
    pub infill_density: f64,
    /// Number of solid top layers.
    pub top_solid_layers: u32,
    /// Number of solid bottom layers.
    pub bottom_solid_layers: u32,

    // --- Speeds (mm/s) ---
    /// Perimeter print speed.
    pub perimeter_speed: f64,
    /// Infill print speed.
    pub infill_speed: f64,
    /// Travel (non-extrusion) speed.
    pub travel_speed: f64,
    /// First layer print speed.
    pub first_layer_speed: f64,

    // --- Retraction ---
    /// Retraction distance in mm.
    pub retract_length: f64,
    /// Retraction speed in mm/s.
    pub retract_speed: f64,
    /// Z-hop height during retraction in mm.
    pub retract_z_hop: f64,
    /// Minimum travel distance to trigger retraction in mm.
    pub min_travel_for_retract: f64,

    // --- Temperature ---
    /// Nozzle temperature in degrees Celsius.
    pub nozzle_temp: f64,
    /// Bed temperature in degrees Celsius.
    pub bed_temp: f64,
    /// Nozzle temperature for the first layer.
    pub first_layer_nozzle_temp: f64,
    /// Bed temperature for the first layer.
    pub first_layer_bed_temp: f64,

    // --- Fan ---
    /// Fan speed (0-255).
    pub fan_speed: u8,
    /// Enable fan when layer time falls below this value (seconds).
    pub fan_below_layer_time: f64,
    /// Number of initial layers with fan disabled.
    pub disable_fan_first_layers: u32,

    // --- Skirt/Brim ---
    /// Number of skirt loops.
    pub skirt_loops: u32,
    /// Distance of skirt from object in mm.
    pub skirt_distance: f64,
    /// Brim width in mm (0.0 = disabled).
    pub brim_width: f64,

    // --- Bed ---
    /// Bed X dimension in mm.
    pub bed_x: f64,
    /// Bed Y dimension in mm.
    pub bed_y: f64,

    // --- Extrusion ---
    /// Extrusion multiplier (flow rate factor).
    pub extrusion_multiplier: f64,
    /// Filament diameter in mm.
    pub filament_diameter: f64,
    /// Filament density in g/cm^3 (PLA ~1.24, ABS ~1.04, PETG ~1.27).
    pub filament_density: f64,
    /// Filament cost per kilogram in currency units (e.g., USD/kg).
    pub filament_cost_per_kg: f64,

    // --- Adaptive Layer Heights ---
    /// Enable adaptive layer heights based on surface curvature.
    pub adaptive_layer_height: bool,
    /// Minimum layer height for adaptive layers (mm).
    pub adaptive_min_layer_height: f64,
    /// Maximum layer height for adaptive layers (mm).
    pub adaptive_max_layer_height: f64,
    /// Adaptive layer quality (0.0 = speed, 1.0 = quality).
    pub adaptive_layer_quality: f64,

    // --- Gap Fill ---
    /// Enable gap fill between perimeters.
    pub gap_fill_enabled: bool,
    /// Minimum gap width to fill (mm).
    pub gap_fill_min_width: f64,

    // --- Polyhole Conversion ---
    /// Enable polyhole conversion for circular holes (dimensional accuracy).
    pub polyhole_enabled: bool,
    /// Minimum hole diameter (mm) for polyhole conversion (skip very small holes).
    pub polyhole_min_diameter: f64,

    // --- Arachne Variable-Width Perimeters ---
    /// Enable Arachne variable-width perimeters for thin walls.
    pub arachne_enabled: bool,

    // --- Scarf Joint Seam ---
    /// Scarf joint seam configuration.
    pub scarf_joint: ScarfJointConfig,

    // --- Support Structures ---
    /// Support structure generation configuration.
    pub support: SupportConfig,

    // --- Ironing ---
    /// Ironing pass configuration for smooth top surfaces.
    pub ironing: IroningConfig,

    // --- Per-Feature Flow ---
    /// Per-feature flow multipliers for fine-tuning extrusion per feature type.
    pub per_feature_flow: PerFeatureFlow,

    // --- Custom G-code Injection ---
    /// Custom G-code hooks for injection at layer transitions and specific Z heights.
    pub custom_gcode: CustomGcodeHooks,

    // --- G-code Dialect ---
    /// G-code firmware dialect (Marlin, Klipper, RepRapFirmware, Bambu).
    pub gcode_dialect: GcodeDialect,

    // --- Arc Fitting ---
    /// Enable arc fitting post-processing (G1 -> G2/G3 conversion).
    pub arc_fitting_enabled: bool,
    /// Maximum deviation (mm) for arc fitting tolerance.
    pub arc_fitting_tolerance: f64,
    /// Minimum number of consecutive G1 moves to consider for arc fitting.
    pub arc_fitting_min_points: usize,

    // --- Acceleration / Jerk / Pressure Advance ---
    /// Print acceleration in mm/s^2.
    pub print_acceleration: f64,
    /// Travel acceleration in mm/s^2.
    pub travel_acceleration: f64,
    /// Jerk X in mm/s.
    pub jerk_x: f64,
    /// Jerk Y in mm/s.
    pub jerk_y: f64,
    /// Jerk Z in mm/s.
    pub jerk_z: f64,
    /// Pressure advance value (0.0 = disabled).
    pub pressure_advance: f64,
    /// Enable acceleration command emission at feature transitions.
    pub acceleration_enabled: bool,

    // --- Multi-Material ---
    /// Multi-material printing configuration (MMU tool changes and purge tower).
    pub multi_material: MultiMaterialConfig,

    // --- Sequential Printing ---
    /// Sequential (object-by-object) printing configuration.
    pub sequential: SequentialConfig,

    // --- Plugins ---
    /// Directory to scan for plugins (optional).
    ///
    /// When set, the engine can discover and load infill plugins from this
    /// directory. Each plugin should be in its own subdirectory containing
    /// a `plugin.toml` manifest.
    #[serde(default)]
    pub plugin_dir: Option<String>,

    // --- Sub-config structs (Phase 20) ---
    /// Per-feature line width configuration.
    pub line_widths: LineWidthConfig,
    /// Per-feature speed configuration.
    pub speeds: SpeedConfig,
    /// Cooling and fan configuration.
    pub cooling: CoolingConfig,
    /// Additional retraction configuration.
    pub retraction: RetractionConfig,
    /// Machine/printer hardware configuration.
    pub machine: MachineConfig,
    /// Per-feature acceleration configuration.
    pub accel: AccelerationConfig,
    /// Filament properties configuration.
    pub filament: FilamentPropsConfig,

    /// Passthrough fields from upstream profiles that have no engine equivalent.
    /// Preserved for round-trip fidelity and G-code template variable access.
    /// Uses `BTreeMap` for deterministic serialization order.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub passthrough: BTreeMap<String, String>,

    // --- Process misc fields (Phase 20) ---
    /// Bridge flow ratio (1.0 = normal flow).
    pub bridge_flow: f64,
    /// Elephant foot compensation in mm.
    pub elefant_foot_compensation: f64,
    /// Infill line direction in degrees.
    pub infill_direction: f64,
    /// Infill-wall overlap as a fraction (0-1).
    pub infill_wall_overlap: f64,
    /// Enable spiral (vase) mode.
    pub spiral_mode: bool,
    /// Use only one wall on top surfaces.
    pub only_one_wall_top: bool,
    /// G-code resolution in mm.
    pub resolution: f64,
    /// Number of raft layers (0 = disabled).
    pub raft_layers: u32,
    /// Enable thin wall detection.
    pub detect_thin_wall: bool,
}

/// Scarf joint seam configuration.
///
/// The scarf joint gradually ramps Z height and flow rate at the perimeter
/// seam point, creating a smooth overlap instead of an abrupt start/end.
/// This makes seams nearly invisible on smooth surfaces.
///
/// All 12 parameters match OrcaSlicer's scarf joint specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScarfJointConfig {
    /// Enable scarf joint seam.
    pub enabled: bool,
    /// Apply to contours and/or holes.
    pub scarf_joint_type: ScarfJointType,
    /// Only apply on smooth perimeters (no sharp corners near seam).
    pub conditional_scarf: bool,
    /// Speed during scarf region (mm/s, 0 = use wall speed).
    pub scarf_speed: f64,
    /// Z offset at ramp start as fraction of layer height (0.0-1.0).
    pub scarf_start_height: f64,
    /// Apply scarf around entire wall (not just seam region).
    pub scarf_around_entire_wall: bool,
    /// Horizontal length of the scarf ramp in mm.
    pub scarf_length: f64,
    /// Number of discrete steps in the ramp.
    pub scarf_steps: u32,
    /// Extrusion flow ratio during scarf (1.0 = normal).
    pub scarf_flow_ratio: f64,
    /// Apply scarf to inner walls (not just outer).
    pub scarf_inner_walls: bool,
    /// Use role-based wipe speed at seam.
    pub role_based_wipe_speed: bool,
    /// Wipe speed at seam end (mm/s).
    pub wipe_speed: f64,
    /// Enable inward wipe at seam close.
    pub wipe_on_loop: bool,
}

impl Default for ScarfJointConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            scarf_joint_type: ScarfJointType::default(),
            conditional_scarf: false,
            scarf_speed: 0.0,
            scarf_start_height: 0.5,
            scarf_around_entire_wall: false,
            scarf_length: 10.0,
            scarf_steps: 10,
            scarf_flow_ratio: 1.0,
            scarf_inner_walls: false,
            role_based_wipe_speed: false,
            wipe_speed: 0.0,
            wipe_on_loop: false,
        }
    }
}

/// Controls which perimeter types receive scarf joint treatment.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScarfJointType {
    /// Apply scarf to contours (outer boundaries) only.
    #[default]
    Contour,
    /// Apply scarf to both contours and holes.
    ContourAndHole,
}

impl Default for PrintConfig {
    fn default() -> Self {
        Self {
            layer_height: 0.2,
            first_layer_height: 0.3,
            nozzle_diameter: 0.4,

            wall_count: 2,
            wall_order: WallOrder::default(),
            seam_position: SeamPosition::default(),

            infill_pattern: InfillPattern::default(),
            infill_density: 0.2,
            top_solid_layers: 3,
            bottom_solid_layers: 3,

            perimeter_speed: 45.0,
            infill_speed: 80.0,
            travel_speed: 150.0,
            first_layer_speed: 20.0,

            retract_length: 0.8,
            retract_speed: 45.0,
            retract_z_hop: 0.0,
            min_travel_for_retract: 1.5,

            nozzle_temp: 200.0,
            bed_temp: 60.0,
            first_layer_nozzle_temp: 210.0,
            first_layer_bed_temp: 65.0,

            fan_speed: 255,
            fan_below_layer_time: 60.0,
            disable_fan_first_layers: 1,

            skirt_loops: 1,
            skirt_distance: 6.0,
            brim_width: 0.0,

            bed_x: 220.0,
            bed_y: 220.0,

            extrusion_multiplier: 1.0,
            filament_diameter: 1.75,
            filament_density: 1.24,
            filament_cost_per_kg: 25.0,

            adaptive_layer_height: false,
            adaptive_min_layer_height: 0.05,
            adaptive_max_layer_height: 0.3,
            adaptive_layer_quality: 0.5,

            gap_fill_enabled: true,
            gap_fill_min_width: 0.1,

            polyhole_enabled: false,
            polyhole_min_diameter: 1.0,

            arachne_enabled: false,

            scarf_joint: ScarfJointConfig::default(),

            support: SupportConfig::default(),

            ironing: IroningConfig::default(),

            per_feature_flow: PerFeatureFlow::default(),

            custom_gcode: CustomGcodeHooks::default(),

            gcode_dialect: GcodeDialect::Marlin,

            arc_fitting_enabled: false,
            arc_fitting_tolerance: 0.05,
            arc_fitting_min_points: 3,

            print_acceleration: 1000.0,
            travel_acceleration: 1500.0,
            jerk_x: 8.0,
            jerk_y: 8.0,
            jerk_z: 0.4,
            pressure_advance: 0.0,
            acceleration_enabled: false,

            multi_material: MultiMaterialConfig::default(),
            sequential: SequentialConfig::default(),
            plugin_dir: None,

            line_widths: LineWidthConfig::default(),
            speeds: SpeedConfig::default(),
            cooling: CoolingConfig::default(),
            retraction: RetractionConfig::default(),
            machine: MachineConfig::default(),
            accel: AccelerationConfig::default(),
            filament: FilamentPropsConfig::default(),
            passthrough: BTreeMap::new(),

            bridge_flow: 1.0,
            elefant_foot_compensation: 0.0,
            infill_direction: 45.0,
            infill_wall_overlap: 0.15,
            spiral_mode: false,
            only_one_wall_top: false,
            resolution: 0.012,
            raft_layers: 0,
            detect_thin_wall: true,
        }
    }
}

impl PrintConfig {
    /// Parses a `PrintConfig` from a TOML string.
    ///
    /// Fields not present in the TOML string will use default values.
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }

    /// Loads a `PrintConfig` from a TOML file.
    ///
    /// Fields not present in the file will use default values.
    pub fn from_toml_file(path: &std::path::Path) -> Result<Self, EngineError> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| EngineError::ConfigIo(path.to_path_buf(), e))?;
        Self::from_toml(&contents).map_err(EngineError::ConfigParse)
    }

    /// Parses a `PrintConfig` from a JSON string.
    ///
    /// Supports two JSON variants:
    /// 1. **Native format** -- field names match `PrintConfig` with numeric values.
    ///    Deserialized directly via serde.
    /// 2. **OrcaSlicer/BambuStudio format** -- detected by the presence of a `"type"`
    ///    field. Uses [`import_upstream_profile`](crate::profile_import::import_upstream_profile)
    ///    for field mapping.
    pub fn from_json(json_str: &str) -> Result<Self, EngineError> {
        let value: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| EngineError::ConfigError(format!("JSON parse error: {}", e)))?;

        if value.get("type").is_some() {
            // Upstream slicer profile -- use mapping import.
            let result = crate::profile_import::import_upstream_profile(&value)?;
            Ok(result.config)
        } else {
            // Native JSON format -- direct deserialization.
            serde_json::from_str(json_str)
                .map_err(|e| EngineError::ConfigError(format!("JSON config error: {}", e)))
        }
    }

    /// Parses a `PrintConfig` from a JSON string, returning the full
    /// [`ImportResult`](crate::profile_import::ImportResult) with mapped/unmapped
    /// field reporting.
    ///
    /// For native JSON format (no `"type"` field), all fields are treated as mapped.
    pub fn from_json_with_details(
        json_str: &str,
    ) -> Result<crate::profile_import::ImportResult, EngineError> {
        let value: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| EngineError::ConfigError(format!("JSON parse error: {}", e)))?;

        if value.get("type").is_some() {
            crate::profile_import::import_upstream_profile(&value)
        } else {
            let config: PrintConfig = serde_json::from_str(json_str)
                .map_err(|e| EngineError::ConfigError(format!("JSON config error: {}", e)))?;
            Ok(crate::profile_import::ImportResult {
                config,
                mapped_fields: Vec::new(),
                unmapped_fields: Vec::new(),
                passthrough_fields: Vec::new(),
                metadata: crate::profile_import::ProfileMetadata::default(),
            })
        }
    }

    /// Loads a `PrintConfig` from a file, auto-detecting format (TOML or JSON).
    ///
    /// Format detection uses content sniffing via
    /// [`detect_config_format`](crate::profile_import::detect_config_format):
    /// - JSON files start with `{` (after optional whitespace/BOM)
    /// - Everything else is treated as TOML
    pub fn from_file(path: &std::path::Path) -> Result<Self, EngineError> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| EngineError::ConfigIo(path.to_path_buf(), e))?;

        match crate::profile_import::detect_config_format(contents.as_bytes()) {
            crate::profile_import::ConfigFormat::Toml => {
                Self::from_toml(&contents).map_err(EngineError::ConfigParse)
            }
            crate::profile_import::ConfigFormat::Json => Self::from_json(&contents),
        }
    }

    /// Returns the extrusion width in mm.
    ///
    /// Currently uses a simple heuristic of `nozzle_diameter * 1.1`.
    pub fn extrusion_width(&self) -> f64 {
        self.nozzle_diameter * 1.1
    }
}

/// Per-tool configuration for multi-material printing.
///
/// Each tool (extruder) can have independent temperature and retraction
/// settings for optimal tool-change sequences.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolConfig {
    /// Nozzle temperature for this tool in degrees Celsius.
    pub nozzle_temp: f64,
    /// Retraction length for this tool in mm.
    pub retract_length: f64,
    /// Retraction speed for this tool in mm/s.
    pub retract_speed: f64,
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            nozzle_temp: 200.0,
            retract_length: 0.8,
            retract_speed: 45.0,
        }
    }
}

/// Multi-material printing configuration.
///
/// Controls MMU (multi-material unit) tool change sequences and purge tower
/// generation. When enabled, the slicer generates retract-park-change-prime
/// sequences at tool transitions and maintains a purge tower on every layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MultiMaterialConfig {
    /// Enable multi-material printing.
    pub enabled: bool,
    /// Number of tools (extruders) available.
    pub tool_count: u8,
    /// Per-tool configuration.
    pub tools: Vec<ToolConfig>,
    /// Purge tower position [x, y] in mm.
    pub purge_tower_position: [f64; 2],
    /// Purge tower width in mm.
    pub purge_tower_width: f64,
    /// Purge volume per tool change in mm^3.
    pub purge_volume: f64,
    /// Wipe length across the purge tower in mm.
    pub wipe_length: f64,
}

impl Default for MultiMaterialConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            tool_count: 1,
            tools: Vec::new(),
            purge_tower_position: [200.0, 200.0],
            purge_tower_width: 15.0,
            purge_volume: 70.0,
            wipe_length: 2.0,
        }
    }
}

/// Sequential (object-by-object) printing configuration.
///
/// In sequential mode, each object is printed completely before moving to
/// the next. This requires collision detection to ensure the extruder
/// clearance envelope does not hit previously printed objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SequentialConfig {
    /// Enable sequential (object-by-object) printing.
    pub enabled: bool,
    /// Extruder clearance radius in mm (XY distance from nozzle to widest
    /// part of the print head assembly).
    pub extruder_clearance_radius: f64,
    /// Extruder clearance height in mm (height above nozzle tip to the
    /// bottom of the X carriage / gantry).
    pub extruder_clearance_height: f64,
}

impl Default for SequentialConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            extruder_clearance_radius: 35.0,
            extruder_clearance_height: 40.0,
        }
    }
}

/// Configuration for pressure advance calibration pattern generation.
///
/// This configures a standalone G-code generator that produces a test print
/// with varying pressure advance (PA) values. The pattern prints alternating
/// slow/fast extrusion sections at incrementally increasing PA values, allowing
/// users to visually identify the optimal PA setting for their printer/filament
/// combination.
///
/// All fields have sensible defaults via `#[serde(default)]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PaCalibrationConfig {
    /// Starting PA value.
    pub pa_start: f64,
    /// Ending PA value.
    pub pa_end: f64,
    /// PA increment per line.
    pub pa_step: f64,
    /// Slow extrusion speed in mm/s (reveals PA artifacts at transitions).
    pub slow_speed: f64,
    /// Fast extrusion speed in mm/s (reveals PA artifacts at transitions).
    pub fast_speed: f64,
    /// Extrusion line width in mm.
    pub line_width: f64,
    /// Layer height in mm.
    pub layer_height: f64,
    /// Bed center X coordinate in mm.
    pub bed_center_x: f64,
    /// Bed center Y coordinate in mm.
    pub bed_center_y: f64,
    /// Total pattern width in mm.
    pub pattern_width: f64,
    /// Nozzle temperature in degrees Celsius.
    pub nozzle_temp: f64,
    /// Bed temperature in degrees Celsius.
    pub bed_temp: f64,
    /// Filament diameter in mm.
    pub filament_diameter: f64,
}

impl Default for PaCalibrationConfig {
    fn default() -> Self {
        Self {
            pa_start: 0.0,
            pa_end: 0.1,
            pa_step: 0.005,
            slow_speed: 20.0,
            fast_speed: 100.0,
            line_width: 0.5,
            layer_height: 0.2,
            bed_center_x: 110.0,
            bed_center_y: 110.0,
            pattern_width: 100.0,
            nozzle_temp: 200.0,
            bed_temp: 60.0,
            filament_diameter: 1.75,
        }
    }
}

/// Per-region setting overrides for modifier meshes.
///
/// Each field is optional: `Some(value)` overrides the corresponding
/// [`PrintConfig`] field, `None` inherits the base config value.
/// Use [`merge_into`](SettingOverrides::merge_into) to produce an
/// effective config for a modifier region.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SettingOverrides {
    /// Override infill density (0.0-1.0).
    pub infill_density: Option<f64>,
    /// Override infill pattern.
    pub infill_pattern: Option<InfillPattern>,
    /// Override number of perimeter walls.
    pub wall_count: Option<u32>,
    /// Override perimeter speed (mm/s).
    pub perimeter_speed: Option<f64>,
    /// Override infill speed (mm/s).
    pub infill_speed: Option<f64>,
    /// Override number of top solid layers.
    pub top_solid_layers: Option<u32>,
    /// Override number of bottom solid layers.
    pub bottom_solid_layers: Option<u32>,
}

impl SettingOverrides {
    /// Produces an effective [`PrintConfig`] by cloning `base` and applying
    /// any `Some()` overrides from this struct.
    pub fn merge_into(&self, base: &PrintConfig) -> PrintConfig {
        let mut config = base.clone();
        if let Some(v) = self.infill_density {
            config.infill_density = v;
        }
        if let Some(ref v) = self.infill_pattern {
            config.infill_pattern = v.clone();
        }
        if let Some(v) = self.wall_count {
            config.wall_count = v;
        }
        if let Some(v) = self.perimeter_speed {
            config.perimeter_speed = v;
        }
        if let Some(v) = self.infill_speed {
            config.infill_speed = v;
        }
        if let Some(v) = self.top_solid_layers {
            config.top_solid_layers = v;
        }
        if let Some(v) = self.bottom_solid_layers {
            config.bottom_solid_layers = v;
        }
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_values() {
        let config = PrintConfig::default();
        assert!((config.layer_height - 0.2).abs() < 1e-9);
        assert!((config.first_layer_height - 0.3).abs() < 1e-9);
        assert!((config.nozzle_diameter - 0.4).abs() < 1e-9);
        assert_eq!(config.wall_count, 2);
        assert_eq!(config.wall_order, WallOrder::OuterFirst);
        assert!((config.infill_density - 0.2).abs() < 1e-9);
        assert_eq!(config.top_solid_layers, 3);
        assert_eq!(config.bottom_solid_layers, 3);
        assert!((config.perimeter_speed - 45.0).abs() < 1e-9);
        assert!((config.infill_speed - 80.0).abs() < 1e-9);
        assert!((config.travel_speed - 150.0).abs() < 1e-9);
        assert!((config.first_layer_speed - 20.0).abs() < 1e-9);
        assert!((config.retract_length - 0.8).abs() < 1e-9);
        assert!((config.retract_speed - 45.0).abs() < 1e-9);
        assert!((config.retract_z_hop - 0.0).abs() < 1e-9);
        assert!((config.min_travel_for_retract - 1.5).abs() < 1e-9);
        assert!((config.nozzle_temp - 200.0).abs() < 1e-9);
        assert!((config.bed_temp - 60.0).abs() < 1e-9);
        assert!((config.first_layer_nozzle_temp - 210.0).abs() < 1e-9);
        assert!((config.first_layer_bed_temp - 65.0).abs() < 1e-9);
        assert_eq!(config.fan_speed, 255);
        assert!((config.fan_below_layer_time - 60.0).abs() < 1e-9);
        assert_eq!(config.disable_fan_first_layers, 1);
        assert_eq!(config.skirt_loops, 1);
        assert!((config.skirt_distance - 6.0).abs() < 1e-9);
        assert!((config.brim_width - 0.0).abs() < 1e-9);
        assert!((config.bed_x - 220.0).abs() < 1e-9);
        assert!((config.bed_y - 220.0).abs() < 1e-9);
        assert!((config.extrusion_multiplier - 1.0).abs() < 1e-9);
        assert!((config.filament_diameter - 1.75).abs() < 1e-9);
    }

    #[test]
    fn from_toml_empty_produces_defaults() {
        let config = PrintConfig::from_toml("").unwrap();
        assert!((config.layer_height - 0.2).abs() < 1e-9);
        assert!((config.nozzle_diameter - 0.4).abs() < 1e-9);
        assert_eq!(config.wall_order, WallOrder::OuterFirst);
    }

    #[test]
    fn from_toml_partial_overrides() {
        let toml = "layer_height = 0.1\ninfill_density = 0.5";
        let config = PrintConfig::from_toml(toml).unwrap();
        assert!((config.layer_height - 0.1).abs() < 1e-9);
        assert!((config.infill_density - 0.5).abs() < 1e-9);
        assert!((config.nozzle_diameter - 0.4).abs() < 1e-9);
        assert_eq!(config.wall_count, 2);
        assert!((config.perimeter_speed - 45.0).abs() < 1e-9);
    }

    #[test]
    fn wall_order_serde_round_trip() {
        let outer_first = WallOrder::OuterFirst;
        let json = serde_json::to_string(&outer_first).unwrap();
        assert_eq!(json, "\"outer_first\"");
        let inner_first = WallOrder::InnerFirst;
        let json = serde_json::to_string(&inner_first).unwrap();
        assert_eq!(json, "\"inner_first\"");
        let deserialized: WallOrder = serde_json::from_str("\"outer_first\"").unwrap();
        assert_eq!(deserialized, WallOrder::OuterFirst);
        let deserialized: WallOrder = serde_json::from_str("\"inner_first\"").unwrap();
        assert_eq!(deserialized, WallOrder::InnerFirst);
    }

    #[test]
    fn wall_order_toml_round_trip() {
        let toml = "wall_order = \"inner_first\"";
        let config = PrintConfig::from_toml(toml).unwrap();
        assert_eq!(config.wall_order, WallOrder::InnerFirst);
    }

    #[test]
    fn extrusion_width_is_nozzle_times_1_1() {
        let config = PrintConfig::default();
        let expected = 0.4 * 1.1;
        assert!(
            (config.extrusion_width() - expected).abs() < 1e-9,
            "extrusion_width should be nozzle_diameter * 1.1 = {}, got {}",
            expected,
            config.extrusion_width()
        );
    }

    #[test]
    fn extrusion_width_with_custom_nozzle() {
        let mut config = PrintConfig::default();
        config.nozzle_diameter = 0.6;
        let expected = 0.6 * 1.1;
        assert!((config.extrusion_width() - expected).abs() < 1e-9);
    }

    #[test]
    fn adaptive_layer_defaults() {
        let config = PrintConfig::default();
        assert!(!config.adaptive_layer_height);
        assert!((config.adaptive_min_layer_height - 0.05).abs() < 1e-9);
        assert!((config.adaptive_max_layer_height - 0.3).abs() < 1e-9);
        assert!((config.adaptive_layer_quality - 0.5).abs() < 1e-9);
    }

    #[test]
    fn adaptive_fields_from_toml() {
        let toml = r#"
adaptive_layer_height = true
adaptive_min_layer_height = 0.04
adaptive_max_layer_height = 0.25
adaptive_layer_quality = 0.8
"#;
        let config = PrintConfig::from_toml(toml).unwrap();
        assert!(config.adaptive_layer_height);
        assert!((config.adaptive_min_layer_height - 0.04).abs() < 1e-9);
        assert!((config.adaptive_max_layer_height - 0.25).abs() < 1e-9);
        assert!((config.adaptive_layer_quality - 0.8).abs() < 1e-9);
    }

    #[test]
    fn scarf_joint_defaults() {
        let config = PrintConfig::default();
        assert!(!config.scarf_joint.enabled);
        assert_eq!(config.scarf_joint.scarf_joint_type, ScarfJointType::Contour);
        assert!((config.scarf_joint.scarf_length - 10.0).abs() < 1e-9);
        assert_eq!(config.scarf_joint.scarf_steps, 10);
        assert!((config.scarf_joint.scarf_flow_ratio - 1.0).abs() < 1e-9);
    }

    #[test]
    fn scarf_joint_from_toml() {
        let toml = r#"
[scarf_joint]
enabled = true
scarf_length = 15.0
scarf_steps = 20
scarf_flow_ratio = 0.9
scarf_inner_walls = true
scarf_joint_type = "contour_and_hole"
"#;
        let config = PrintConfig::from_toml(toml).unwrap();
        assert!(config.scarf_joint.enabled);
        assert!((config.scarf_joint.scarf_length - 15.0).abs() < 1e-9);
        assert_eq!(config.scarf_joint.scarf_steps, 20);
        assert!((config.scarf_joint.scarf_flow_ratio - 0.9).abs() < 1e-9);
        assert!(config.scarf_joint.scarf_inner_walls);
        assert_eq!(
            config.scarf_joint.scarf_joint_type,
            ScarfJointType::ContourAndHole
        );
    }

    #[test]
    fn scarf_joint_type_serde_round_trip() {
        let types = [ScarfJointType::Contour, ScarfJointType::ContourAndHole];
        for t in &types {
            let json = serde_json::to_string(t).unwrap();
            let deserialized: ScarfJointType = serde_json::from_str(&json).unwrap();
            assert_eq!(*t, deserialized, "Serde round-trip failed for {:?}", t);
        }
    }

    #[test]
    fn per_feature_flow_defaults() {
        let config = PrintConfig::default();
        assert!((config.per_feature_flow.outer_perimeter - 1.0).abs() < 1e-9);
        assert!((config.per_feature_flow.ironing - 1.0).abs() < 1e-9);
    }

    #[test]
    fn per_feature_flow_from_toml() {
        let toml = r#"
[per_feature_flow]
outer_perimeter = 0.95
bridge = 1.1
"#;
        let config = PrintConfig::from_toml(toml).unwrap();
        assert!((config.per_feature_flow.outer_perimeter - 0.95).abs() < 1e-9);
        assert!((config.per_feature_flow.bridge - 1.1).abs() < 1e-9);
        assert!((config.per_feature_flow.inner_perimeter - 1.0).abs() < 1e-9);
    }

    #[test]
    fn custom_gcode_defaults() {
        let config = PrintConfig::default();
        assert!(config.custom_gcode.before_layer_change.is_empty());
        assert!(config.custom_gcode.after_layer_change.is_empty());
        assert!(config.custom_gcode.custom_gcode_per_z.is_empty());
    }

    #[test]
    fn ironing_defaults() {
        let config = PrintConfig::default();
        assert!(!config.ironing.enabled);
        assert!((config.ironing.flow_rate - 0.1).abs() < 1e-9);
        assert!((config.ironing.speed - 15.0).abs() < 1e-9);
        assert!((config.ironing.spacing - 0.1).abs() < 1e-9);
        assert!((config.ironing.angle - 45.0).abs() < 1e-9);
    }

    #[test]
    fn ironing_from_toml() {
        let toml = r#"
[ironing]
enabled = true
flow_rate = 0.08
speed = 25.0
spacing = 0.15
angle = 60.0
"#;
        let config = PrintConfig::from_toml(toml).unwrap();
        assert!(config.ironing.enabled);
        assert!((config.ironing.flow_rate - 0.08).abs() < 1e-9);
        assert!((config.ironing.speed - 25.0).abs() < 1e-9);
        assert!((config.ironing.spacing - 0.15).abs() < 1e-9);
        assert!((config.ironing.angle - 60.0).abs() < 1e-9);
    }

    #[test]
    fn custom_gcode_from_toml() {
        let toml = r#"
[custom_gcode]
after_layer_change = "M117 Layer {layer_num}"
custom_gcode_per_z = [[5.0, "M600"]]
"#;
        let config = PrintConfig::from_toml(toml).unwrap();
        assert_eq!(config.custom_gcode.after_layer_change, "M117 Layer {layer_num}");
        assert_eq!(config.custom_gcode.custom_gcode_per_z.len(), 1);
    }

    #[test]
    fn setting_overrides_default_all_none() {
        let overrides = SettingOverrides::default();
        assert!(overrides.infill_density.is_none());
        assert!(overrides.infill_pattern.is_none());
        assert!(overrides.wall_count.is_none());
        assert!(overrides.perimeter_speed.is_none());
        assert!(overrides.infill_speed.is_none());
        assert!(overrides.top_solid_layers.is_none());
        assert!(overrides.bottom_solid_layers.is_none());
    }

    #[test]
    fn setting_overrides_merge_applies_some_fields() {
        let base = PrintConfig::default();
        let overrides = SettingOverrides {
            infill_density: Some(0.8),
            wall_count: Some(4),
            perimeter_speed: Some(30.0),
            ..Default::default()
        };
        let merged = overrides.merge_into(&base);
        // Overridden fields.
        assert!((merged.infill_density - 0.8).abs() < 1e-9);
        assert_eq!(merged.wall_count, 4);
        assert!((merged.perimeter_speed - 30.0).abs() < 1e-9);
        // Non-overridden fields preserved.
        assert!((merged.infill_speed - base.infill_speed).abs() < 1e-9);
        assert_eq!(merged.top_solid_layers, base.top_solid_layers);
        assert_eq!(merged.bottom_solid_layers, base.bottom_solid_layers);
        assert!((merged.layer_height - base.layer_height).abs() < 1e-9);
    }

    #[test]
    fn setting_overrides_merge_preserves_non_overridden() {
        let base = PrintConfig {
            infill_density: 0.3,
            wall_count: 3,
            perimeter_speed: 50.0,
            ..Default::default()
        };
        let overrides = SettingOverrides::default(); // all None
        let merged = overrides.merge_into(&base);
        assert!((merged.infill_density - 0.3).abs() < 1e-9);
        assert_eq!(merged.wall_count, 3);
        assert!((merged.perimeter_speed - 50.0).abs() < 1e-9);
    }

    #[test]
    fn arc_fitting_defaults() {
        let config = PrintConfig::default();
        assert!(!config.arc_fitting_enabled);
        assert!((config.arc_fitting_tolerance - 0.05).abs() < 1e-9);
        assert_eq!(config.arc_fitting_min_points, 3);
    }

    #[test]
    fn arc_fitting_from_toml() {
        let toml = r#"
arc_fitting_enabled = true
arc_fitting_tolerance = 0.1
arc_fitting_min_points = 5
"#;
        let config = PrintConfig::from_toml(toml).unwrap();
        assert!(config.arc_fitting_enabled);
        assert!((config.arc_fitting_tolerance - 0.1).abs() < 1e-9);
        assert_eq!(config.arc_fitting_min_points, 5);
    }

    #[test]
    fn filament_density_and_cost_defaults() {
        let config = PrintConfig::default();
        assert!(
            (config.filament_density - 1.24).abs() < 1e-9,
            "filament_density should default to 1.24 (PLA)"
        );
        assert!(
            (config.filament_cost_per_kg - 25.0).abs() < 1e-9,
            "filament_cost_per_kg should default to 25.0"
        );
    }

    #[test]
    fn filament_density_and_cost_from_toml() {
        let toml = r#"
filament_density = 1.04
filament_cost_per_kg = 30.0
"#;
        let config = PrintConfig::from_toml(toml).unwrap();
        assert!(
            (config.filament_density - 1.04).abs() < 1e-9,
            "filament_density should parse from TOML"
        );
        assert!(
            (config.filament_cost_per_kg - 30.0).abs() < 1e-9,
            "filament_cost_per_kg should parse from TOML"
        );
    }

    #[test]
    fn polyhole_defaults() {
        let config = PrintConfig::default();
        assert!(!config.polyhole_enabled);
        assert!((config.polyhole_min_diameter - 1.0).abs() < 1e-9);
    }

    #[test]
    fn polyhole_from_toml() {
        let toml = r#"
polyhole_enabled = true
polyhole_min_diameter = 0.5
"#;
        let config = PrintConfig::from_toml(toml).unwrap();
        assert!(config.polyhole_enabled);
        assert!((config.polyhole_min_diameter - 0.5).abs() < 1e-9);
    }

    // ========================================================================
    // Phase 20 sub-config tests
    // ========================================================================

    #[test]
    fn sub_config_defaults() {
        let config = PrintConfig::default();

        // LineWidthConfig
        assert!((config.line_widths.outer_wall - 0.42).abs() < 1e-9);
        assert!((config.line_widths.inner_wall - 0.45).abs() < 1e-9);
        assert!((config.line_widths.infill - 0.45).abs() < 1e-9);
        assert!((config.line_widths.top_surface - 0.42).abs() < 1e-9);
        assert!((config.line_widths.initial_layer - 0.5).abs() < 1e-9);
        assert!((config.line_widths.internal_solid_infill - 0.42).abs() < 1e-9);
        assert!((config.line_widths.support - 0.42).abs() < 1e-9);

        // SpeedConfig
        assert!((config.speeds.bridge - 25.0).abs() < 1e-9);
        assert!((config.speeds.inner_wall - 0.0).abs() < 1e-9);
        assert!((config.speeds.gap_fill - 0.0).abs() < 1e-9);
        assert!((config.speeds.top_surface - 0.0).abs() < 1e-9);
        assert!((config.speeds.overhang_1_4 - 0.0).abs() < 1e-9);
        assert!((config.speeds.travel_z - 0.0).abs() < 1e-9);

        // CoolingConfig
        assert!((config.cooling.fan_max_speed - 100.0).abs() < 1e-9);
        assert!((config.cooling.fan_min_speed - 35.0).abs() < 1e-9);
        assert!((config.cooling.slow_down_layer_time - 5.0).abs() < 1e-9);
        assert!((config.cooling.slow_down_min_speed - 10.0).abs() < 1e-9);
        assert!((config.cooling.overhang_fan_speed - 100.0).abs() < 1e-9);
        assert!((config.cooling.overhang_fan_threshold - 25.0).abs() < 1e-9);
        assert_eq!(config.cooling.full_fan_speed_layer, 0);
        assert!(config.cooling.slow_down_for_layer_cooling);

        // RetractionConfig
        assert!((config.retraction.deretraction_speed - 0.0).abs() < 1e-9);
        assert!((config.retraction.retract_before_wipe - 0.0).abs() < 1e-9);
        assert!(!config.retraction.retract_when_changing_layer);
        assert!(!config.retraction.wipe);
        assert!((config.retraction.wipe_distance - 0.0).abs() < 1e-9);

        // MachineConfig
        assert!((config.machine.printable_height - 250.0).abs() < 1e-9);
        assert!((config.machine.max_acceleration_x - 5000.0).abs() < 1e-9);
        assert!((config.machine.max_speed_z - 12.0).abs() < 1e-9);
        assert!((config.machine.min_layer_height - 0.07).abs() < 1e-9);
        assert!((config.machine.max_layer_height - 0.0).abs() < 1e-9);
        assert!(config.machine.start_gcode.is_empty());
        assert!(config.machine.printer_model.is_empty());

        // AccelerationConfig
        assert!((config.accel.outer_wall - 0.0).abs() < 1e-9);
        assert!((config.accel.inner_wall - 0.0).abs() < 1e-9);
        assert!((config.accel.bridge - 0.0).abs() < 1e-9);

        // FilamentPropsConfig
        assert!(config.filament.filament_type.is_empty());
        assert!((config.filament.max_volumetric_speed - 0.0).abs() < 1e-9);
        assert!((config.filament.nozzle_temperature_range_low - 190.0).abs() < 1e-9);
        assert!((config.filament.nozzle_temperature_range_high - 240.0).abs() < 1e-9);
        assert!(config.filament.filament_retraction_length.is_none());
        assert!(config.filament.filament_retraction_speed.is_none());

        // Passthrough
        assert!(config.passthrough.is_empty());

        // Process misc
        assert!((config.bridge_flow - 1.0).abs() < 1e-9);
        assert!((config.elefant_foot_compensation - 0.0).abs() < 1e-9);
        assert!((config.infill_direction - 45.0).abs() < 1e-9);
        assert!((config.infill_wall_overlap - 0.15).abs() < 1e-9);
        assert!(!config.spiral_mode);
        assert!(!config.only_one_wall_top);
        assert!((config.resolution - 0.012).abs() < 1e-9);
        assert_eq!(config.raft_layers, 0);
        assert!(config.detect_thin_wall);
    }

    #[test]
    fn sub_config_from_toml() {
        let toml = r#"
bridge_flow = 0.95
spiral_mode = true
infill_direction = 90.0

[line_widths]
outer_wall = 0.40
inner_wall = 0.50
infill = 0.55

[speeds]
bridge = 30.0
inner_wall = 40.0
overhang_1_4 = 15.0
travel_z = 5.0

[cooling]
fan_max_speed = 80.0
fan_min_speed = 20.0
slow_down_for_layer_cooling = false
full_fan_speed_layer = 3

[retraction]
deretraction_speed = 30.0
wipe = true
wipe_distance = 2.0

[machine]
printable_height = 300.0
max_speed_x = 600.0
start_gcode = "G28 ; home"
printer_model = "TestPrinter"
nozzle_diameters = [0.4, 0.6]
jerk_values_x = [9.0, 7.0]
min_layer_height = 0.05

[accel]
outer_wall = 1000.0
bridge = 500.0

[filament]
filament_type = "PETG"
max_volumetric_speed = 12.0
nozzle_temperatures = [230.0, 240.0]
bed_temperatures = [80.0]
first_layer_nozzle_temperatures = [235.0]
first_layer_bed_temperatures = [85.0]
filament_retraction_length = 1.5
"#;
        let config = PrintConfig::from_toml(toml).unwrap();

        // Process misc flat fields
        assert!((config.bridge_flow - 0.95).abs() < 1e-9);
        assert!(config.spiral_mode);
        assert!((config.infill_direction - 90.0).abs() < 1e-9);

        // LineWidthConfig
        assert!((config.line_widths.outer_wall - 0.40).abs() < 1e-9);
        assert!((config.line_widths.inner_wall - 0.50).abs() < 1e-9);
        assert!((config.line_widths.infill - 0.55).abs() < 1e-9);
        // Unspecified fields retain defaults
        assert!((config.line_widths.top_surface - 0.42).abs() < 1e-9);

        // SpeedConfig
        assert!((config.speeds.bridge - 30.0).abs() < 1e-9);
        assert!((config.speeds.inner_wall - 40.0).abs() < 1e-9);
        assert!((config.speeds.overhang_1_4 - 15.0).abs() < 1e-9);
        assert!((config.speeds.travel_z - 5.0).abs() < 1e-9);
        // Unspecified retains default
        assert!((config.speeds.gap_fill - 0.0).abs() < 1e-9);

        // CoolingConfig
        assert!((config.cooling.fan_max_speed - 80.0).abs() < 1e-9);
        assert!((config.cooling.fan_min_speed - 20.0).abs() < 1e-9);
        assert!(!config.cooling.slow_down_for_layer_cooling);
        assert_eq!(config.cooling.full_fan_speed_layer, 3);

        // RetractionConfig
        assert!((config.retraction.deretraction_speed - 30.0).abs() < 1e-9);
        assert!(config.retraction.wipe);
        assert!((config.retraction.wipe_distance - 2.0).abs() < 1e-9);

        // MachineConfig
        assert!((config.machine.printable_height - 300.0).abs() < 1e-9);
        assert!((config.machine.max_speed_x - 600.0).abs() < 1e-9);
        assert_eq!(config.machine.start_gcode, "G28 ; home");
        assert_eq!(config.machine.printer_model, "TestPrinter");
        assert_eq!(config.machine.nozzle_diameters, vec![0.4, 0.6]);
        assert_eq!(config.machine.jerk_values_x, vec![9.0, 7.0]);
        assert!((config.machine.min_layer_height - 0.05).abs() < 1e-9);

        // AccelerationConfig
        assert!((config.accel.outer_wall - 1000.0).abs() < 1e-9);
        assert!((config.accel.bridge - 500.0).abs() < 1e-9);
        // Unspecified retains default
        assert!((config.accel.inner_wall - 0.0).abs() < 1e-9);

        // FilamentPropsConfig
        assert_eq!(config.filament.filament_type, "PETG");
        assert!((config.filament.max_volumetric_speed - 12.0).abs() < 1e-9);
        assert_eq!(config.filament.nozzle_temperatures, vec![230.0, 240.0]);
        assert_eq!(config.filament.bed_temperatures, vec![80.0]);
        assert_eq!(config.filament.first_layer_nozzle_temperatures, vec![235.0]);
        assert_eq!(config.filament.first_layer_bed_temperatures, vec![85.0]);
        assert_eq!(config.filament.filament_retraction_length, Some(1.5));
    }

    #[test]
    fn passthrough_round_trip() {
        let mut config = PrintConfig::default();
        config
            .passthrough
            .insert("custom_key".to_string(), "custom_value".to_string());
        config
            .passthrough
            .insert("ams_drying_temp".to_string(), "55".to_string());

        // Serialize to TOML and back
        let toml_str = toml::to_string(&config).unwrap();
        let restored: PrintConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(restored.passthrough.len(), 2);
        assert_eq!(
            restored.passthrough.get("custom_key").unwrap(),
            "custom_value"
        );
        assert_eq!(restored.passthrough.get("ams_drying_temp").unwrap(), "55");
    }

    #[test]
    fn vec_f64_array_fields() {
        let config = PrintConfig::default();

        // MachineConfig Vec<f64> defaults
        assert_eq!(config.machine.nozzle_diameters, vec![0.4]);
        assert_eq!(config.machine.jerk_values_x, vec![8.0]);
        assert_eq!(config.machine.jerk_values_y, vec![8.0]);
        assert_eq!(config.machine.jerk_values_z, vec![0.4]);
        assert_eq!(config.machine.jerk_values_e, vec![2.5]);

        // MachineConfig convenience accessors
        assert!((config.machine.nozzle_diameter() - 0.4).abs() < 1e-9);
        assert!((config.machine.jerk_x() - 8.0).abs() < 1e-9);
        assert!((config.machine.jerk_y() - 8.0).abs() < 1e-9);
        assert!((config.machine.jerk_z() - 0.4).abs() < 1e-9);
        assert!((config.machine.jerk_e() - 2.5).abs() < 1e-9);

        // FilamentPropsConfig Vec<f64> defaults
        assert_eq!(config.filament.nozzle_temperatures, vec![200.0]);
        assert_eq!(config.filament.bed_temperatures, vec![60.0]);
        assert_eq!(config.filament.first_layer_nozzle_temperatures, vec![210.0]);
        assert_eq!(config.filament.first_layer_bed_temperatures, vec![65.0]);

        // FilamentPropsConfig convenience accessors
        assert!((config.filament.nozzle_temp() - 200.0).abs() < 1e-9);
        assert!((config.filament.bed_temp() - 60.0).abs() < 1e-9);
        assert!((config.filament.first_layer_nozzle_temp() - 210.0).abs() < 1e-9);
        assert!((config.filament.first_layer_bed_temp() - 65.0).abs() < 1e-9);

        // Empty vec accessors return fallback defaults
        let empty_machine = MachineConfig {
            nozzle_diameters: vec![],
            jerk_values_x: vec![],
            jerk_values_y: vec![],
            jerk_values_z: vec![],
            jerk_values_e: vec![],
            ..Default::default()
        };
        assert!((empty_machine.nozzle_diameter() - 0.4).abs() < 1e-9);
        assert!((empty_machine.jerk_x() - 8.0).abs() < 1e-9);
        assert!((empty_machine.jerk_y() - 8.0).abs() < 1e-9);
        assert!((empty_machine.jerk_z() - 0.4).abs() < 1e-9);
        assert!((empty_machine.jerk_e() - 2.5).abs() < 1e-9);

        let empty_filament = FilamentPropsConfig {
            nozzle_temperatures: vec![],
            bed_temperatures: vec![],
            first_layer_nozzle_temperatures: vec![],
            first_layer_bed_temperatures: vec![],
            ..Default::default()
        };
        assert!((empty_filament.nozzle_temp() - 200.0).abs() < 1e-9);
        assert!((empty_filament.bed_temp() - 60.0).abs() < 1e-9);
        assert!((empty_filament.first_layer_nozzle_temp() - 210.0).abs() < 1e-9);
        assert!((empty_filament.first_layer_bed_temp() - 65.0).abs() < 1e-9);
    }

    #[test]
    fn vec_f64_toml_round_trip() {
        let toml = r#"
[machine]
nozzle_diameters = [0.4, 0.6]
jerk_values_x = [8.0, 6.0]
jerk_values_y = [8.0, 6.0]
jerk_values_z = [0.4, 0.3]
jerk_values_e = [2.5, 2.0]

[filament]
nozzle_temperatures = [200.0, 210.0]
bed_temperatures = [60.0, 70.0]
first_layer_nozzle_temperatures = [210.0, 220.0]
first_layer_bed_temperatures = [65.0, 75.0]
"#;
        let config = PrintConfig::from_toml(toml).unwrap();

        // Verify deserialization
        assert_eq!(config.machine.nozzle_diameters, vec![0.4, 0.6]);
        assert_eq!(config.machine.jerk_values_x, vec![8.0, 6.0]);
        assert_eq!(config.machine.jerk_values_y, vec![8.0, 6.0]);
        assert_eq!(config.machine.jerk_values_z, vec![0.4, 0.3]);
        assert_eq!(config.machine.jerk_values_e, vec![2.5, 2.0]);
        assert_eq!(config.filament.nozzle_temperatures, vec![200.0, 210.0]);
        assert_eq!(config.filament.bed_temperatures, vec![60.0, 70.0]);
        assert_eq!(
            config.filament.first_layer_nozzle_temperatures,
            vec![210.0, 220.0]
        );
        assert_eq!(
            config.filament.first_layer_bed_temperatures,
            vec![65.0, 75.0]
        );

        // Convenience accessors return first element
        assert!((config.machine.nozzle_diameter() - 0.4).abs() < 1e-9);
        assert!((config.filament.nozzle_temp() - 200.0).abs() < 1e-9);

        // Round-trip: serialize then deserialize
        let toml_output = toml::to_string(&config).unwrap();
        let restored: PrintConfig = toml::from_str(&toml_output).unwrap();
        assert_eq!(restored.machine.nozzle_diameters, vec![0.4, 0.6]);
        assert_eq!(restored.machine.jerk_values_x, vec![8.0, 6.0]);
        assert_eq!(restored.filament.nozzle_temperatures, vec![200.0, 210.0]);
        assert_eq!(restored.filament.bed_temperatures, vec![60.0, 70.0]);
        assert_eq!(
            restored.filament.first_layer_nozzle_temperatures,
            vec![210.0, 220.0]
        );
        assert_eq!(
            restored.filament.first_layer_bed_temperatures,
            vec![65.0, 75.0]
        );
    }
}
