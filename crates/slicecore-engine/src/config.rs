//! Print configuration for the slicing pipeline.
//!
//! [`PrintConfig`] contains all parameters that control the slicing and
//! G-code generation pipeline. It is designed for TOML deserialization with
//! `#[serde(default)]`, so any field not specified in the TOML input will
//! use sensible FDM defaults.
//!
//! [`WallOrder`] controls whether perimeters are printed inside-out or
//! outside-in.

use serde::{Deserialize, Serialize};
use slicecore_gcode_io::GcodeDialect;

use crate::custom_gcode::CustomGcodeHooks;
use crate::error::EngineError;
use crate::flow_control::PerFeatureFlow;
use crate::infill::InfillPattern;
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

/// Print configuration controlling the entire slicing pipeline.
///
/// All fields have sensible FDM defaults. Use [`PrintConfig::from_toml`] to
/// parse from a TOML string, or [`PrintConfig::from_toml_file`] to load from
/// a file. Fields not specified in the TOML input use defaults.
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

    // --- Arachne Variable-Width Perimeters ---
    /// Enable Arachne variable-width perimeters for thin walls.
    /// When enabled, thin-wall regions use medial-axis-derived perimeters
    /// with per-segment width instead of fixed-width perimeters.
    pub arachne_enabled: bool,

    // --- Scarf Joint Seam ---
    /// Scarf joint seam configuration.
    pub scarf_joint: ScarfJointConfig,

    // --- Support Structures ---
    /// Support structure generation configuration.
    pub support: SupportConfig,

    // --- Per-Feature Flow ---
    /// Per-feature flow multipliers for fine-tuning extrusion per feature type.
    pub per_feature_flow: PerFeatureFlow,

    // --- Custom G-code Injection ---
    /// Custom G-code hooks for injection at layer transitions and specific Z heights.
    pub custom_gcode: CustomGcodeHooks,

    // --- G-code Dialect ---
    /// G-code firmware dialect (Marlin, Klipper, RepRapFirmware, Bambu).
    pub gcode_dialect: GcodeDialect,

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

            adaptive_layer_height: false,
            adaptive_min_layer_height: 0.05,
            adaptive_max_layer_height: 0.3,
            adaptive_layer_quality: 0.5,

            gap_fill_enabled: true,
            gap_fill_min_width: 0.1,

            arachne_enabled: false,

            scarf_joint: ScarfJointConfig::default(),

            support: SupportConfig::default(),

            per_feature_flow: PerFeatureFlow::default(),

            custom_gcode: CustomGcodeHooks::default(),

            gcode_dialect: GcodeDialect::Marlin,

            print_acceleration: 1000.0,
            travel_acceleration: 1500.0,
            jerk_x: 8.0,
            jerk_y: 8.0,
            jerk_z: 0.4,
            pressure_advance: 0.0,
            acceleration_enabled: false,
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

    /// Returns the extrusion width in mm.
    ///
    /// Currently uses a simple heuristic of `nozzle_diameter * 1.1`.
    /// Phase 3 uses a single extrusion width; more sophisticated
    /// per-feature widths may be added in later phases.
    pub fn extrusion_width(&self) -> f64 {
        self.nozzle_diameter * 1.1
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

        // Overridden values
        assert!((config.layer_height - 0.1).abs() < 1e-9);
        assert!((config.infill_density - 0.5).abs() < 1e-9);

        // Non-overridden values remain at defaults
        assert!((config.nozzle_diameter - 0.4).abs() < 1e-9);
        assert_eq!(config.wall_count, 2);
        assert!((config.perimeter_speed - 45.0).abs() < 1e-9);
    }

    #[test]
    fn wall_order_serde_round_trip() {
        // Serialize
        let outer_first = WallOrder::OuterFirst;
        let json = serde_json::to_string(&outer_first).unwrap();
        assert_eq!(json, "\"outer_first\"");

        let inner_first = WallOrder::InnerFirst;
        let json = serde_json::to_string(&inner_first).unwrap();
        assert_eq!(json, "\"inner_first\"");

        // Deserialize
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
        assert!(!config.scarf_joint.conditional_scarf);
        assert!((config.scarf_joint.scarf_speed - 0.0).abs() < 1e-9);
        assert!((config.scarf_joint.scarf_start_height - 0.5).abs() < 1e-9);
        assert!(!config.scarf_joint.scarf_around_entire_wall);
        assert!((config.scarf_joint.scarf_length - 10.0).abs() < 1e-9);
        assert_eq!(config.scarf_joint.scarf_steps, 10);
        assert!((config.scarf_joint.scarf_flow_ratio - 1.0).abs() < 1e-9);
        assert!(!config.scarf_joint.scarf_inner_walls);
        assert!(!config.scarf_joint.role_based_wipe_speed);
        assert!((config.scarf_joint.wipe_speed - 0.0).abs() < 1e-9);
        assert!(!config.scarf_joint.wipe_on_loop);
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
    fn scarf_joint_empty_toml_uses_defaults() {
        let config = PrintConfig::from_toml("").unwrap();
        assert!(!config.scarf_joint.enabled);
        assert!((config.scarf_joint.scarf_length - 10.0).abs() < 1e-9);
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
    fn gcode_dialect_defaults_to_marlin() {
        let config = PrintConfig::default();
        assert_eq!(config.gcode_dialect, GcodeDialect::Marlin);
    }

    #[test]
    fn acceleration_defaults() {
        let config = PrintConfig::default();
        assert!((config.print_acceleration - 1000.0).abs() < 1e-9);
        assert!((config.travel_acceleration - 1500.0).abs() < 1e-9);
        assert!((config.jerk_x - 8.0).abs() < 1e-9);
        assert!((config.jerk_y - 8.0).abs() < 1e-9);
        assert!((config.jerk_z - 0.4).abs() < 1e-9);
        assert!((config.pressure_advance - 0.0).abs() < 1e-9);
        assert!(!config.acceleration_enabled);
    }

    #[test]
    fn gcode_dialect_klipper_from_toml() {
        let toml = r#"gcode_dialect = "Klipper""#;
        let config = PrintConfig::from_toml(toml).unwrap();
        assert_eq!(config.gcode_dialect, GcodeDialect::Klipper);
    }

    #[test]
    fn acceleration_fields_from_toml() {
        let toml = r#"
acceleration_enabled = true
print_acceleration = 2000.0
travel_acceleration = 3000.0
jerk_x = 10.0
jerk_y = 10.0
jerk_z = 0.5
pressure_advance = 0.05
"#;
        let config = PrintConfig::from_toml(toml).unwrap();
        assert!(config.acceleration_enabled);
        assert!((config.print_acceleration - 2000.0).abs() < 1e-9);
        assert!((config.travel_acceleration - 3000.0).abs() < 1e-9);
        assert!((config.jerk_x - 10.0).abs() < 1e-9);
        assert!((config.jerk_y - 10.0).abs() < 1e-9);
        assert!((config.jerk_z - 0.5).abs() < 1e-9);
        assert!((config.pressure_advance - 0.05).abs() < 1e-9);
    }

    #[test]
    fn all_dialects_parse_from_toml() {
        let dialects = ["Marlin", "Klipper", "RepRapFirmware", "Bambu"];
        for d in &dialects {
            let toml = format!("gcode_dialect = \"{}\"", d);
            let config = PrintConfig::from_toml(&toml).unwrap();
            let expected = match *d {
                "Marlin" => GcodeDialect::Marlin,
                "Klipper" => GcodeDialect::Klipper,
                "RepRapFirmware" => GcodeDialect::RepRapFirmware,
                "Bambu" => GcodeDialect::Bambu,
                _ => unreachable!(),
            };
            assert_eq!(config.gcode_dialect, expected, "Failed to parse dialect: {}", d);
        }
    }
}
