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

use crate::error::EngineError;
use crate::infill::InfillPattern;

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
}

impl Default for PrintConfig {
    fn default() -> Self {
        Self {
            layer_height: 0.2,
            first_layer_height: 0.3,
            nozzle_diameter: 0.4,

            wall_count: 2,
            wall_order: WallOrder::default(),

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
}
