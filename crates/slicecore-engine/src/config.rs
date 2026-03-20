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
use slicecore_config_derive::SettingSchema;
use slicecore_gcode_io::GcodeDialect;

use crate::custom_gcode::CustomGcodeHooks;
use crate::error::EngineError;
use crate::flow_control::PerFeatureFlow;
use crate::infill::InfillPattern;
use crate::ironing::IroningConfig;
use crate::seam::SeamPosition;
use crate::support::config::SupportConfig;

/// Controls the order in which perimeter walls are printed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, SettingSchema)]
#[serde(rename_all = "snake_case")]
pub enum WallOrder {
    /// Print inner walls first, then outer wall.
    #[setting(
        display = "Inner First",
        description = "Print inner walls before the outer wall for better overhangs"
    )]
    InnerFirst,
    /// Print outer wall first, then inner walls.
    #[default]
    #[setting(
        display = "Outer First",
        description = "Print outer wall first for better surface quality"
    )]
    OuterFirst,
}

/// Fill pattern for solid surfaces (top, bottom, internal solid).
///
/// Separate from [`InfillPattern`] because solid surfaces use a restricted
/// subset of patterns suitable for dense fills. Patterns like Lightning or
/// Gyroid are unsuitable for solid surfaces and are excluded.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, SettingSchema)]
#[serde(rename_all = "snake_case")]
pub enum SurfacePattern {
    /// Parallel lines alternating direction per layer.
    #[setting(
        display = "Rectilinear",
        description = "Parallel lines alternating direction each layer"
    )]
    Rectilinear,
    /// Unidirectional monotonic lines for smooth top/bottom surfaces.
    #[default]
    #[setting(
        display = "Monotonic",
        description = "Unidirectional lines for smooth surfaces"
    )]
    Monotonic,
    /// Monotonic single-line variant (thinner line overlap).
    #[setting(
        display = "Monotonic Line",
        description = "Monotonic variant with thinner line overlap"
    )]
    MonotonicLine,
    /// Concentric inward-spiraling pattern.
    #[setting(
        display = "Concentric",
        description = "Inward-spiraling concentric rings"
    )]
    Concentric,
    /// Hilbert space-filling curve pattern.
    #[setting(
        display = "Hilbert",
        description = "Hilbert space-filling curve for uniform coverage"
    )]
    Hilbert,
    /// Archimedean spiral pattern.
    #[setting(display = "Archimedean", description = "Archimedean spiral pattern")]
    Archimedean,
}

/// Build plate surface type.
///
/// Used to select per-bed-type temperature profiles from filament settings.
/// Import mappers translate upstream string values (e.g., "Cool Plate") to
/// our snake_case variants.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, SettingSchema)]
#[serde(rename_all = "snake_case")]
pub enum BedType {
    /// Cool/smooth PEI plate (low adhesion).
    #[setting(
        display = "Cool Plate",
        description = "Cool/smooth PEI plate with low adhesion"
    )]
    CoolPlate,
    /// Engineering plate (textured, high adhesion).
    #[setting(
        display = "Engineering Plate",
        description = "Engineering plate with high adhesion for technical materials"
    )]
    EngineeringPlate,
    /// High-temperature plate.
    #[setting(
        display = "High Temp Plate",
        description = "High-temperature resistant build plate"
    )]
    HighTempPlate,
    /// Textured PEI plate (standard).
    #[default]
    #[setting(
        display = "Textured PEI",
        description = "Textured PEI plate for general-purpose printing"
    )]
    TexturedPei,
    /// Smooth PEI plate.
    #[setting(
        display = "Smooth PEI",
        description = "Smooth PEI plate for glossy first layers"
    )]
    SmoothPei,
    /// Satin PEI plate.
    #[setting(display = "Satin PEI", description = "Satin-finish PEI plate")]
    SatinPei,
}

/// Internal bridge support mode.
///
/// Controls whether internal bridges (bridges over infill) receive
/// special speed/flow treatment.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, SettingSchema)]
#[serde(rename_all = "snake_case")]
pub enum InternalBridgeMode {
    /// No internal bridge detection.
    #[default]
    #[setting(display = "Off", description = "No internal bridge detection")]
    Off,
    /// Automatic detection of internal bridges.
    #[setting(
        display = "Auto",
        description = "Automatically detect internal bridges over infill"
    )]
    Auto,
    /// Always treat internal solid layers as bridges.
    #[setting(
        display = "Always",
        description = "Always treat internal solid layers as bridges"
    )]
    Always,
}

/// Brim adhesion type.
///
/// Controls where brim lines are placed relative to the object outline.
/// Import mappers translate OrcaSlicer strings to these variants.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, SettingSchema)]
#[serde(rename_all = "snake_case")]
pub enum BrimType {
    /// No brim (disabled).
    #[default]
    #[setting(display = "None", description = "No brim adhesion aid")]
    None,
    /// Brim on the outer side of the object outline only.
    #[setting(
        display = "Outer Only",
        description = "Brim on the outer side of the object outline"
    )]
    Outer,
    /// Brim on the inner side of the object outline only.
    #[setting(
        display = "Inner Only",
        description = "Brim on the inner side of the object outline"
    )]
    Inner,
    /// Brim on both inner and outer sides.
    #[setting(
        display = "Both Sides",
        description = "Brim on both inner and outer sides of the outline"
    )]
    Both,
}

/// Fuzzy skin configuration for textured surface finish.
///
/// When enabled, random displacements are applied to outer wall points
/// to create a rough, organic texture on the print surface.
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Advanced")]
pub struct FuzzySkinConfig {
    /// Enable fuzzy skin effect on outer walls.
    /// OrcaSlicer: `fuzzy_skin`. PrusaSlicer: `fuzzy_skin`.
    /// Default: false.
    #[setting(tier = 3, description = "Enable fuzzy skin textured surface effect")]
    pub enabled: bool,
    /// Maximum random displacement amplitude in mm.
    /// OrcaSlicer: `fuzzy_skin_thickness`. PrusaSlicer: `fuzzy_skin_thickness`.
    /// Range: 0.0-1.0. Default: 0.3.
    #[setting(
        tier = 3,
        description = "Maximum random displacement amplitude",
        units = "mm",
        min = 0.0,
        max = 1.0,
        depends_on = "fuzzy_skin.enabled"
    )]
    pub thickness: f64,
    /// Distance between displacement points along the wall in mm.
    /// OrcaSlicer: `fuzzy_skin_point_dist`. PrusaSlicer: `fuzzy_skin_point_distance`.
    /// Range: 0.1-5.0. Default: 0.8.
    #[setting(
        tier = 3,
        description = "Distance between displacement points",
        units = "mm",
        min = 0.1,
        max = 5.0,
        depends_on = "fuzzy_skin.enabled"
    )]
    pub point_distance: f64,
}

impl Default for FuzzySkinConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            thickness: 0.3,
            point_distance: 0.8,
        }
    }
}

/// Additional brim and skirt configuration fields.
///
/// These fields supplement the existing top-level `skirt_loops`,
/// `skirt_distance`, and `brim_width` fields in `PrintConfig`.
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Adhesion")]
pub struct BrimSkirtConfig {
    /// Brim adhesion type (none/outer/inner/both).
    /// OrcaSlicer: `brim_type`. PrusaSlicer: `brim_type`.
    /// Default: None.
    #[setting(tier = 2, description = "Brim placement type")]
    pub brim_type: BrimType,
    /// Enable brim ears (brim only at sharp corners).
    /// OrcaSlicer: `brim_ears`. Default: false.
    #[setting(tier = 3, description = "Enable brim only at sharp corners")]
    pub brim_ears: bool,
    /// Maximum overhang angle for brim ears in degrees.
    /// OrcaSlicer: `brim_ears_max_angle`. Range: 0-180. Default: 125.0.
    #[setting(
        tier = 3,
        description = "Maximum overhang angle for brim ears",
        units = "deg",
        min = 0.0,
        max = 180.0,
        depends_on = "brim_skirt.brim_ears"
    )]
    pub brim_ears_max_angle: f64,
    /// Skirt height in layers.
    /// OrcaSlicer: `skirt_height`. PrusaSlicer: `skirt_height`.
    /// Range: 1-100. Default: 1.
    #[setting(
        tier = 3,
        description = "Skirt height in layers",
        min = 1.0,
        max = 100.0
    )]
    pub skirt_height: u32,
}

impl Default for BrimSkirtConfig {
    fn default() -> Self {
        Self {
            brim_type: BrimType::None,
            brim_ears: false,
            brim_ears_max_angle: 125.0,
            skirt_height: 1,
        }
    }
}

/// Input shaping motion configuration.
///
/// Controls accel-to-decel factor used by firmware input shaping
/// to reduce ringing artifacts.
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Advanced")]
pub struct InputShapingConfig {
    /// Enable accel-to-decel factor for input shaping.
    /// OrcaSlicer: `accel_to_decel_enable`. Default: false.
    #[setting(
        tier = 3,
        description = "Enable accel-to-decel factor for input shaping"
    )]
    pub accel_to_decel_enable: bool,
    /// Accel-to-decel factor ratio.
    /// OrcaSlicer: `accel_to_decel_factor`. Range: 0.0-1.0. Default: 0.5.
    #[setting(
        tier = 3,
        description = "Accel-to-decel factor ratio",
        min = 0.0,
        max = 1.0,
        depends_on = "input_shaping.accel_to_decel_enable"
    )]
    pub accel_to_decel_factor: f64,
}

impl Default for InputShapingConfig {
    fn default() -> Self {
        Self {
            accel_to_decel_enable: false,
            accel_to_decel_factor: 0.5,
        }
    }
}

/// Tool change retraction configuration for multi-material printing.
///
/// Controls retraction behavior during filament cutting and tool changes.
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Retraction")]
pub struct ToolChangeRetractionConfig {
    /// Retraction distance when filament is cut during tool change in mm.
    /// OrcaSlicer: `retraction_distances_when_cut`. Default: 18.0.
    #[setting(
        tier = 4,
        description = "Retraction distance when filament is cut during tool change",
        units = "mm"
    )]
    pub retraction_distance_when_cut: f64,
    /// Enable long retraction when filament is cut.
    /// OrcaSlicer: `long_retractions_when_cut`. Default: false.
    #[setting(tier = 4, description = "Enable long retraction when filament is cut")]
    pub long_retraction_when_cut: bool,
}

impl Default for ToolChangeRetractionConfig {
    fn default() -> Self {
        Self {
            retraction_distance_when_cut: 18.0,
            long_retraction_when_cut: false,
        }
    }
}

/// Dimensional compensation configuration.
///
/// Groups XY hole/contour compensation and elephant foot compensation.
/// These fields offset geometry to improve dimensional accuracy of
/// printed parts. All values in mm.
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Advanced")]
pub struct DimensionalCompensationConfig {
    /// XY hole compensation in mm. Negative values shrink holes for tighter fits.
    /// OrcaSlicer: `xy_hole_compensation`. PrusaSlicer: N/A (uses xy_size_compensation).
    /// Range: -2.0 to 2.0 mm. Default: 0.0 (no compensation).
    #[setting(tier = 3, description = "XY hole compensation offset", units = "mm", min = -2.0, max = 2.0)]
    pub xy_hole_compensation: f64,
    /// XY contour compensation in mm. Positive values expand outer contours.
    /// OrcaSlicer: `xy_contour_compensation`. PrusaSlicer: `xy_size_compensation`.
    /// Range: -2.0 to 2.0 mm. Default: 0.0 (no compensation).
    #[setting(tier = 3, description = "XY contour compensation offset", units = "mm", min = -2.0, max = 2.0)]
    pub xy_contour_compensation: f64,
    /// Elephant foot compensation in mm (first layer inward offset).
    /// Migrated from `PrintConfig.elefant_foot_compensation`.
    /// OrcaSlicer: `elefant_foot_compensation`. PrusaSlicer: `elefant_foot_compensation`.
    /// Range: 0.0 to 2.0 mm. Default: 0.0.
    #[serde(alias = "elefant_foot_compensation")]
    #[setting(
        tier = 2,
        description = "First layer inward offset to compensate for elephant foot",
        units = "mm",
        min = 0.0,
        max = 2.0
    )]
    pub elephant_foot_compensation: f64,
}

impl Default for DimensionalCompensationConfig {
    fn default() -> Self {
        Self {
            xy_hole_compensation: 0.0,
            xy_contour_compensation: 0.0,
            elephant_foot_compensation: 0.0,
        }
    }
}

// ============================================================================
// Sub-config structs for organized field grouping
// ============================================================================

/// Per-feature line width configuration.
///
/// Controls the extrusion width for different feature types. A value of 0.0
/// typically means "auto from nozzle diameter". Defaults are BambuStudio
/// reference values for a 0.4mm nozzle.
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "LineWidth")]
pub struct LineWidthConfig {
    /// Outer wall line width in mm.
    #[setting(
        tier = 2,
        description = "Outer wall line width",
        units = "mm",
        min = 0.0,
        max = 2.0
    )]
    pub outer_wall: f64,
    /// Inner wall line width in mm.
    #[setting(
        tier = 2,
        description = "Inner wall line width",
        units = "mm",
        min = 0.0,
        max = 2.0
    )]
    pub inner_wall: f64,
    /// Sparse infill line width in mm.
    #[setting(
        tier = 2,
        description = "Infill line width",
        units = "mm",
        min = 0.0,
        max = 2.0
    )]
    pub infill: f64,
    /// Top surface line width in mm.
    #[setting(
        tier = 2,
        description = "Top surface line width",
        units = "mm",
        min = 0.0,
        max = 2.0
    )]
    pub top_surface: f64,
    /// Initial (first) layer line width in mm.
    #[setting(
        tier = 2,
        description = "Initial layer line width",
        units = "mm",
        min = 0.0,
        max = 2.0
    )]
    pub initial_layer: f64,
    /// Internal solid infill line width in mm.
    #[setting(
        tier = 3,
        description = "Internal solid infill line width",
        units = "mm",
        min = 0.0,
        max = 2.0
    )]
    pub internal_solid_infill: f64,
    /// Support structure line width in mm.
    #[setting(
        tier = 3,
        description = "Support structure line width",
        units = "mm",
        min = 0.0,
        max = 2.0
    )]
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
///
/// The four primary speed fields (`perimeter`, `infill`, `travel`,
/// `first_layer`) were migrated from `PrintConfig` flat fields in Plan 04.
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Speed")]
pub struct SpeedConfig {
    /// Perimeter print speed (mm/s).
    #[setting(tier = 1, description = "Perimeter print speed", units = "mm/s", min = 1.0, max = 1000.0, affects = ["print_time", "quality"])]
    pub perimeter: f64,
    /// Infill print speed (mm/s).
    #[setting(tier = 1, description = "Infill print speed", units = "mm/s", min = 1.0, max = 1000.0, affects = ["print_time"])]
    pub infill: f64,
    /// Travel (non-extrusion) speed (mm/s).
    #[setting(tier = 2, description = "Travel movement speed", units = "mm/s", min = 1.0, max = 1000.0, affects = ["print_time"])]
    pub travel: f64,
    /// First layer print speed (mm/s).
    #[setting(tier = 2, description = "First layer print speed", units = "mm/s", min = 1.0, max = 1000.0, affects = ["adhesion", "print_time"])]
    pub first_layer: f64,
    /// Bridge print speed (mm/s).
    #[setting(tier = 2, description = "Bridge print speed", units = "mm/s", min = 1.0, max = 1000.0, affects = ["quality", "bridging"])]
    pub bridge: f64,
    /// Inner wall speed (mm/s, 0 = inherit from perimeter_speed).
    #[setting(
        tier = 2,
        description = "Inner wall speed",
        units = "mm/s",
        min = 0.0,
        max = 1000.0
    )]
    pub inner_wall: f64,
    /// Gap fill speed (mm/s, 0 = inherit from perimeter_speed).
    #[setting(
        tier = 3,
        description = "Gap fill speed",
        units = "mm/s",
        min = 0.0,
        max = 1000.0
    )]
    pub gap_fill: f64,
    /// Top surface speed (mm/s, 0 = inherit from perimeter_speed).
    #[setting(tier = 2, description = "Top surface speed", units = "mm/s", min = 0.0, max = 1000.0, affects = ["quality"])]
    pub top_surface: f64,
    /// Internal solid infill speed (mm/s, 0 = inherit).
    #[setting(
        tier = 3,
        description = "Internal solid infill speed",
        units = "mm/s",
        min = 0.0,
        max = 1000.0
    )]
    pub internal_solid_infill: f64,
    /// Initial layer infill speed (mm/s, 0 = inherit).
    #[setting(
        tier = 3,
        description = "Initial layer infill speed",
        units = "mm/s",
        min = 0.0,
        max = 1000.0
    )]
    pub initial_layer_infill: f64,
    /// Support structure speed (mm/s, 0 = inherit).
    #[setting(
        tier = 2,
        description = "Support structure print speed",
        units = "mm/s",
        min = 0.0,
        max = 1000.0
    )]
    pub support: f64,
    /// Support interface speed (mm/s, 0 = inherit).
    #[setting(
        tier = 3,
        description = "Support interface layer speed",
        units = "mm/s",
        min = 0.0,
        max = 1000.0
    )]
    pub support_interface: f64,
    /// Small perimeter speed (mm/s, 0 = inherit from perimeter_speed).
    #[setting(
        tier = 3,
        description = "Speed for small perimeter features",
        units = "mm/s",
        min = 0.0,
        max = 1000.0
    )]
    pub small_perimeter: f64,
    /// Solid infill speed (mm/s, 0 = inherit).
    #[setting(
        tier = 3,
        description = "Solid infill speed",
        units = "mm/s",
        min = 0.0,
        max = 1000.0
    )]
    pub solid_infill: f64,
    /// Overhang speed for 0-25% overhang (mm/s, 0 = inherit).
    #[setting(
        tier = 3,
        description = "Overhang speed for 0-25% overhang",
        units = "mm/s",
        min = 0.0,
        max = 1000.0,
        depends_on = "speeds.enable_overhang_speed"
    )]
    pub overhang_1_4: f64,
    /// Overhang speed for 25-50% overhang (mm/s, 0 = inherit).
    #[setting(
        tier = 3,
        description = "Overhang speed for 25-50% overhang",
        units = "mm/s",
        min = 0.0,
        max = 1000.0,
        depends_on = "speeds.enable_overhang_speed"
    )]
    pub overhang_2_4: f64,
    /// Overhang speed for 50-75% overhang (mm/s, 0 = inherit).
    #[setting(
        tier = 3,
        description = "Overhang speed for 50-75% overhang",
        units = "mm/s",
        min = 0.0,
        max = 1000.0,
        depends_on = "speeds.enable_overhang_speed"
    )]
    pub overhang_3_4: f64,
    /// Overhang speed for 75-100% overhang (mm/s, 0 = inherit).
    #[setting(
        tier = 3,
        description = "Overhang speed for 75-100% overhang",
        units = "mm/s",
        min = 0.0,
        max = 1000.0,
        depends_on = "speeds.enable_overhang_speed"
    )]
    pub overhang_4_4: f64,
    /// Z-axis travel speed (mm/s, 0 = use travel_speed).
    #[setting(
        tier = 3,
        description = "Z-axis travel speed",
        units = "mm/s",
        min = 0.0,
        max = 1000.0
    )]
    pub travel_z: f64,
    /// Internal bridge speed (mm/s, 0 = inherit from bridge speed).
    /// OrcaSlicer: `internal_bridge_speed`. PrusaSlicer: N/A.
    /// Range: 0-300. Default: 0.0.
    #[setting(
        tier = 3,
        description = "Internal bridge speed",
        units = "mm/s",
        min = 0.0,
        max = 300.0
    )]
    pub internal_bridge_speed: f64,
    /// Master switch for overhang speed adjustments.
    /// OrcaSlicer: `enable_overhang_speed`. Default: true.
    #[setting(tier = 3, description = "Enable per-overhang speed adjustments")]
    pub enable_overhang_speed: bool,
}

impl Default for SpeedConfig {
    fn default() -> Self {
        Self {
            perimeter: 45.0,
            infill: 80.0,
            travel: 150.0,
            first_layer: 20.0,
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
            internal_bridge_speed: 0.0,
            enable_overhang_speed: true,
        }
    }
}

/// Cooling and fan configuration.
///
/// Controls fan speeds, layer-time-based slowdown, and overhang cooling.
/// The flat `fan_speed`, `fan_below_layer_time`, and `disable_fan_first_layers`
/// fields were migrated from `PrintConfig` in Plan 04.
/// Percentage values are 0-100 (not 0-1 fraction).
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Cooling")]
pub struct CoolingConfig {
    /// Fan speed (0-255).
    #[setting(tier = 2, description = "Part cooling fan speed", min = 0.0, max = 255.0, affects = ["quality", "bridging", "overhangs"])]
    pub fan_speed: u8,
    /// Enable fan when layer time falls below this value (seconds).
    #[setting(tier = 2, description = "Enable fan when layer time falls below this threshold", units = "s", min = 0.0, max = 300.0, affects = ["quality", "bridging"])]
    pub fan_below_layer_time: f64,
    /// Number of initial layers with fan disabled.
    #[setting(tier = 2, description = "Number of initial layers with fan disabled", affects = ["adhesion"])]
    pub disable_fan_first_layers: u32,
    /// Maximum fan speed (percentage, 0-100).
    #[setting(
        tier = 2,
        description = "Maximum fan speed",
        units = "%",
        min = 0.0,
        max = 100.0
    )]
    pub fan_max_speed: f64,
    /// Minimum fan speed (percentage, 0-100).
    #[setting(
        tier = 2,
        description = "Minimum fan speed",
        units = "%",
        min = 0.0,
        max = 100.0
    )]
    pub fan_min_speed: f64,
    /// Slow down if layer time falls below this value (seconds).
    #[setting(tier = 3, description = "Slow down if layer time falls below this threshold", units = "s", min = 0.0, max = 300.0, affects = ["quality"])]
    pub slow_down_layer_time: f64,
    /// Minimum speed when slowing down for layer cooling (mm/s).
    #[setting(
        tier = 3,
        description = "Minimum speed during layer cooling slowdown",
        units = "mm/s",
        min = 1.0,
        max = 100.0
    )]
    pub slow_down_min_speed: f64,
    /// Fan speed for overhang regions (percentage, 0-100).
    #[setting(tier = 3, description = "Fan speed for overhang regions", units = "%", min = 0.0, max = 100.0, affects = ["overhangs"])]
    pub overhang_fan_speed: f64,
    /// Overhang angle threshold for fan override (degrees).
    #[setting(
        tier = 3,
        description = "Overhang angle threshold for fan override",
        units = "deg",
        min = 0.0,
        max = 90.0
    )]
    pub overhang_fan_threshold: f64,
    /// Layer number at which fan reaches full speed (0 = immediate).
    #[setting(tier = 3, description = "Layer at which fan reaches full speed")]
    pub full_fan_speed_layer: u32,
    /// Enable automatic slowdown for layer cooling.
    #[setting(tier = 3, description = "Enable automatic slowdown for layer cooling")]
    pub slow_down_for_layer_cooling: bool,
    /// Auxiliary/additional cooling fan speed as percentage (0-100).
    /// OrcaSlicer: `additional_cooling_fan_speed`. Range: 0-100. Default: 0.0.
    #[setting(
        tier = 3,
        description = "Additional cooling fan speed",
        units = "%",
        min = 0.0,
        max = 100.0
    )]
    pub additional_cooling_fan_speed: f64,
    /// Enable auxiliary cooling fan.
    /// OrcaSlicer: `auxiliary_fan`. Default: false.
    #[setting(tier = 3, description = "Enable auxiliary cooling fan")]
    pub auxiliary_fan: bool,
}

impl Default for CoolingConfig {
    fn default() -> Self {
        Self {
            fan_speed: 255,
            fan_below_layer_time: 60.0,
            disable_fan_first_layers: 1,
            fan_max_speed: 100.0,
            fan_min_speed: 35.0,
            slow_down_layer_time: 5.0,
            slow_down_min_speed: 10.0,
            overhang_fan_speed: 100.0,
            overhang_fan_threshold: 25.0,
            full_fan_speed_layer: 0,
            slow_down_for_layer_cooling: true,
            additional_cooling_fan_speed: 0.0,
            auxiliary_fan: false,
        }
    }
}

/// Retraction configuration.
///
/// The flat `retract_length`, `retract_speed`, `retract_z_hop`, and
/// `min_travel_for_retract` fields were migrated from `PrintConfig` in
/// Plan 04 (renamed to `length`, `speed`, `z_hop`, `min_travel`).
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Retraction")]
pub struct RetractionConfig {
    /// Retraction distance in mm.
    #[setting(tier = 2, description = "Retraction distance", units = "mm", min = 0.0, max = 20.0, affects = ["stringing", "oozing"])]
    pub length: f64,
    /// Retraction speed in mm/s.
    #[setting(tier = 2, description = "Retraction speed", units = "mm/s", min = 1.0, max = 200.0, affects = ["stringing"])]
    pub speed: f64,
    /// Z-hop height during retraction in mm.
    #[setting(
        tier = 2,
        description = "Z-hop height during retraction",
        units = "mm",
        min = 0.0,
        max = 5.0
    )]
    pub z_hop: f64,
    /// Minimum travel distance to trigger retraction in mm.
    #[setting(
        tier = 2,
        description = "Minimum travel distance to trigger retraction",
        units = "mm",
        min = 0.0,
        max = 20.0
    )]
    pub min_travel: f64,
    /// Deretraction (unretract) speed in mm/s (0 = same as retraction speed).
    #[setting(
        tier = 3,
        description = "Deretraction speed",
        units = "mm/s",
        min = 0.0,
        max = 200.0
    )]
    pub deretraction_speed: f64,
    /// Percentage of retraction to perform before wipe (0-100).
    #[setting(
        tier = 3,
        description = "Percentage of retraction before wipe",
        units = "%",
        min = 0.0,
        max = 100.0
    )]
    pub retract_before_wipe: f64,
    /// Whether to retract when changing layers.
    #[setting(tier = 3, description = "Retract when changing layers")]
    pub retract_when_changing_layer: bool,
    /// Enable wipe move during retraction.
    #[setting(tier = 3, description = "Enable wipe move during retraction")]
    pub wipe: bool,
    /// Wipe distance in mm.
    #[setting(
        tier = 3,
        description = "Wipe distance",
        units = "mm",
        min = 0.0,
        max = 10.0,
        depends_on = "retraction.wipe"
    )]
    pub wipe_distance: f64,
}

impl Default for RetractionConfig {
    fn default() -> Self {
        Self {
            length: 0.8,
            speed: 45.0,
            z_hop: 0.0,
            min_travel: 1.5,
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
/// multi-extruder array fields. The flat `bed_x` and `bed_y` fields were
/// migrated from `PrintConfig` in Plan 04. Vec fields use single-element
/// defaults for single-extruder printers.
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Machine")]
pub struct MachineConfig {
    /// Bed X dimension in mm.
    #[setting(
        tier = 2,
        description = "Bed X dimension",
        units = "mm",
        min = 1.0,
        max = 2000.0
    )]
    pub bed_x: f64,
    /// Bed Y dimension in mm.
    #[setting(
        tier = 2,
        description = "Bed Y dimension",
        units = "mm",
        min = 1.0,
        max = 2000.0
    )]
    pub bed_y: f64,
    /// Maximum printable height in mm.
    #[setting(
        tier = 2,
        description = "Maximum printable height",
        units = "mm",
        min = 1.0,
        max = 2000.0
    )]
    pub printable_height: f64,
    /// Maximum X acceleration (mm/s^2).
    #[setting(tier = 4, description = "Maximum X acceleration", units = "mm/s^2")]
    pub max_acceleration_x: f64,
    /// Maximum Y acceleration (mm/s^2).
    #[setting(tier = 4, description = "Maximum Y acceleration", units = "mm/s^2")]
    pub max_acceleration_y: f64,
    /// Maximum Z acceleration (mm/s^2).
    #[setting(tier = 4, description = "Maximum Z acceleration", units = "mm/s^2")]
    pub max_acceleration_z: f64,
    /// Maximum E (extruder) acceleration (mm/s^2).
    #[setting(
        tier = 4,
        description = "Maximum extruder acceleration",
        units = "mm/s^2"
    )]
    pub max_acceleration_e: f64,
    /// Maximum acceleration while extruding (mm/s^2).
    #[setting(
        tier = 4,
        description = "Maximum acceleration while extruding",
        units = "mm/s^2"
    )]
    pub max_acceleration_extruding: f64,
    /// Maximum acceleration while retracting (mm/s^2).
    #[setting(
        tier = 4,
        description = "Maximum acceleration while retracting",
        units = "mm/s^2"
    )]
    pub max_acceleration_retracting: f64,
    /// Maximum acceleration during travel moves (mm/s^2).
    #[setting(
        tier = 4,
        description = "Maximum acceleration during travel moves",
        units = "mm/s^2"
    )]
    pub max_acceleration_travel: f64,
    /// Maximum X speed (mm/s).
    #[setting(tier = 4, description = "Maximum X axis speed", units = "mm/s")]
    pub max_speed_x: f64,
    /// Maximum Y speed (mm/s).
    #[setting(tier = 4, description = "Maximum Y axis speed", units = "mm/s")]
    pub max_speed_y: f64,
    /// Maximum Z speed (mm/s).
    #[setting(tier = 4, description = "Maximum Z axis speed", units = "mm/s")]
    pub max_speed_z: f64,
    /// Maximum E (extruder) speed (mm/s).
    #[setting(tier = 4, description = "Maximum extruder speed", units = "mm/s")]
    pub max_speed_e: f64,
    /// Nozzle diameters per extruder (mm). Multi-extruder array.
    #[setting(tier = 2, description = "Nozzle diameter per extruder", units = "mm")]
    pub nozzle_diameters: Vec<f64>,
    /// Jerk values for X axis per extruder (mm/s). Multi-extruder array.
    #[setting(tier = 4, description = "Jerk X per extruder", units = "mm/s")]
    pub jerk_values_x: Vec<f64>,
    /// Jerk values for Y axis per extruder (mm/s). Multi-extruder array.
    #[setting(tier = 4, description = "Jerk Y per extruder", units = "mm/s")]
    pub jerk_values_y: Vec<f64>,
    /// Jerk values for Z axis per extruder (mm/s). Multi-extruder array.
    #[setting(tier = 4, description = "Jerk Z per extruder", units = "mm/s")]
    pub jerk_values_z: Vec<f64>,
    /// Jerk values for E axis per extruder (mm/s). Multi-extruder array.
    #[setting(tier = 4, description = "Jerk E per extruder", units = "mm/s")]
    pub jerk_values_e: Vec<f64>,
    /// Machine start G-code template.
    #[setting(tier = 3, description = "Machine start G-code template")]
    pub start_gcode: String,
    /// Verbatim start G-code from upstream profile before variable translation.
    #[serde(default)]
    #[setting(tier = 4, description = "Upstream verbatim start G-code")]
    pub start_gcode_original: String,
    /// Machine end G-code template.
    #[setting(tier = 3, description = "Machine end G-code template")]
    pub end_gcode: String,
    /// Verbatim end G-code from upstream profile before variable translation.
    #[serde(default)]
    #[setting(tier = 4, description = "Upstream verbatim end G-code")]
    pub end_gcode_original: String,
    /// G-code inserted at every layer change.
    #[setting(tier = 3, description = "Layer change G-code template")]
    pub layer_change_gcode: String,
    /// Verbatim layer change G-code from upstream profile before variable translation.
    #[serde(default)]
    #[setting(tier = 4, description = "Upstream verbatim layer change G-code")]
    pub layer_change_gcode_original: String,
    /// Nozzle type descriptor (e.g., "hardened_steel", "brass").
    #[setting(tier = 3, description = "Nozzle material type")]
    pub nozzle_type: String,
    /// Printer model identifier.
    #[setting(tier = 3, description = "Printer model identifier")]
    pub printer_model: String,
    /// Bed shape descriptor (serialized polygon or rectangle).
    #[setting(tier = 4, description = "Serialized bed geometry")]
    pub bed_shape: String,
    /// Minimum layer height the printer can handle (mm).
    #[setting(tier = 4, description = "Minimum supported layer height", units = "mm")]
    pub min_layer_height: f64,
    /// Maximum layer height (mm, 0 = auto from nozzle diameter).
    #[setting(tier = 4, description = "Maximum supported layer height", units = "mm")]
    pub max_layer_height: f64,
    /// Number of extruders/toolheads.
    ///
    /// Used to auto-detect multi-head printers for material grouping
    /// decisions in the arrangement algorithm. Use
    /// [`effective_extruder_count`](Self::effective_extruder_count) to get
    /// the count that accounts for both this field and
    /// [`nozzle_diameters`](Self::nozzle_diameters) length.
    #[setting(tier = 3, description = "Number of extruders/toolheads")]
    pub extruder_count: u32,
    /// Printer power consumption in watts (for cost estimation, 0 = not set).
    #[setting(
        tier = 4,
        description = "Printer power consumption for cost estimation",
        units = "W"
    )]
    pub watts: f64,
    /// Maximum chamber temperature the printer can reach (degrees C, 0 = no chamber heater).
    /// Used to validate filament chamber_temperature requests.
    /// OrcaSlicer: `chamber_temperature` (in machine profile). Range: 0-120. Default: 0.0.
    #[setting(
        tier = 4,
        description = "Maximum chamber temperature",
        units = "deg_c",
        min = 0.0,
        max = 120.0
    )]
    pub chamber_temperature: f64,
    /// Currently selected bed/build plate type.
    /// OrcaSlicer: `curr_bed_type`. Default: TexturedPEI.
    #[setting(tier = 2, description = "Current bed/build plate type")]
    pub curr_bed_type: BedType,
    /// Enable silent/stealth mode for quieter operation.
    /// OrcaSlicer/PrusaSlicer: `silent_mode`. Default: false.
    #[serde(default)]
    #[setting(tier = 3, description = "Enable silent/stealth mode")]
    pub silent_mode: bool,
    /// Nozzle hardness rating in HRC (Rockwell C scale).
    /// OrcaSlicer: `nozzle_hrc`. Default: 0.
    #[serde(default)]
    #[setting(tier = 4, description = "Nozzle hardness rating (HRC)")]
    pub nozzle_hrc: u32,
    /// Write machine limits (M201/M203/M204) to G-code output.
    /// OrcaSlicer: `emit_machine_limits_to_gcode`. Default: false.
    #[serde(default)]
    #[setting(tier = 4, description = "Emit machine limits to G-code output")]
    pub emit_machine_limits_to_gcode: bool,
    /// Custom bed texture image path for UI preview.
    /// OrcaSlicer/PrusaSlicer: `bed_custom_texture`. Default: empty.
    #[serde(default)]
    #[setting(tier = 4, description = "Custom bed texture image path")]
    pub bed_custom_texture: String,
    /// Custom bed 3D model path for UI preview.
    /// OrcaSlicer/PrusaSlicer: `bed_custom_model`. Default: empty.
    #[serde(default)]
    #[setting(tier = 4, description = "Custom bed 3D model path")]
    pub bed_custom_model: String,
    /// XY offset per extruder in mm [[x, y], ...].
    /// OrcaSlicer/PrusaSlicer: `extruder_offset`. Default: empty.
    #[serde(default)]
    #[setting(tier = 4, description = "XY offset per extruder")]
    pub extruder_offset: Vec<[f64; 2]>,
    /// Bambu AMS cooling tube length in mm.
    /// OrcaSlicer/PrusaSlicer: `cooling_tube_length`. Default: 0.0.
    #[serde(default)]
    #[setting(tier = 4, description = "AMS cooling tube length", units = "mm")]
    pub cooling_tube_length: f64,
    /// Bambu AMS cooling tube retraction distance in mm.
    /// OrcaSlicer/PrusaSlicer: `cooling_tube_retraction`. Default: 0.0.
    #[serde(default)]
    #[setting(
        tier = 4,
        description = "AMS cooling tube retraction distance",
        units = "mm"
    )]
    pub cooling_tube_retraction: f64,
    /// Bambu AMS parking position retraction distance in mm.
    /// OrcaSlicer/PrusaSlicer: `parking_pos_retraction`. Default: 0.0.
    #[serde(default)]
    #[setting(
        tier = 4,
        description = "AMS parking position retraction distance",
        units = "mm"
    )]
    pub parking_pos_retraction: f64,
    /// Bambu AMS extra loading move distance in mm.
    /// OrcaSlicer/PrusaSlicer: `extra_loading_move`. Default: 0.0.
    #[serde(default)]
    #[setting(
        tier = 4,
        description = "AMS extra loading move distance",
        units = "mm"
    )]
    pub extra_loading_move: f64,
    /// Retraction length for tool change in mm.
    /// PrusaSlicer: `retract_length_toolchange`. Default: 0.0.
    #[serde(default)]
    #[setting(tier = 4, description = "Tool change retraction length", units = "mm")]
    pub retract_length_toolchange: f64,
    /// Extra length to prime after retraction in mm.
    /// PrusaSlicer: `retract_restart_extra`. Default: 0.0.
    #[serde(default)]
    #[setting(tier = 4, description = "Extra prime after retraction", units = "mm")]
    pub retract_restart_extra: f64,
    /// Extra length to prime after tool change retraction in mm.
    /// PrusaSlicer: `retract_restart_extra_toolchange`. Default: 0.0.
    #[serde(default)]
    #[setting(
        tier = 4,
        description = "Extra prime after tool change retraction",
        units = "mm"
    )]
    pub retract_restart_extra_toolchange: f64,
    /// Minimum extruding rate in mm/s.
    /// OrcaSlicer: `machine_min_extruding_rate`. Default: 0.0.
    #[serde(default)]
    #[setting(tier = 4, description = "Minimum extrusion speed", units = "mm/s")]
    pub min_extruding_rate: f64,
    /// Minimum travel rate in mm/s.
    /// OrcaSlicer: `machine_min_travel_rate`. Default: 0.0.
    #[serde(default)]
    #[setting(tier = 4, description = "Minimum travel speed", units = "mm/s")]
    pub min_travel_rate: f64,
}

impl Default for MachineConfig {
    fn default() -> Self {
        Self {
            bed_x: 220.0,
            bed_y: 220.0,
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
            start_gcode_original: String::new(),
            end_gcode: String::new(),
            end_gcode_original: String::new(),
            layer_change_gcode: String::new(),
            layer_change_gcode_original: String::new(),
            nozzle_type: String::new(),
            printer_model: String::new(),
            bed_shape: String::new(),
            min_layer_height: 0.07,
            max_layer_height: 0.0,
            extruder_count: 1,
            watts: 0.0,
            chamber_temperature: 0.0,
            curr_bed_type: BedType::default(),
            silent_mode: false,
            nozzle_hrc: 0,
            emit_machine_limits_to_gcode: false,
            bed_custom_texture: String::new(),
            bed_custom_model: String::new(),
            extruder_offset: Vec::new(),
            cooling_tube_length: 0.0,
            cooling_tube_retraction: 0.0,
            parking_pos_retraction: 0.0,
            extra_loading_move: 0.0,
            retract_length_toolchange: 0.0,
            retract_restart_extra: 0.0,
            retract_restart_extra_toolchange: 0.0,
            min_extruding_rate: 0.0,
            min_travel_rate: 0.0,
        }
    }
}

impl MachineConfig {
    /// Returns the primary nozzle diameter (first extruder), or 0.4 if empty.
    pub fn nozzle_diameter(&self) -> f64 {
        self.nozzle_diameters.first().copied().unwrap_or(0.4)
    }

    /// Returns the effective extruder count, accounting for both the explicit
    /// [`extruder_count`](Self::extruder_count) field and the length of
    /// [`nozzle_diameters`](Self::nozzle_diameters).
    ///
    /// This handles both explicit configuration (e.g., from a profile) and
    /// inferred count (from the number of nozzle diameters). The returned
    /// value is always at least 1.
    #[must_use]
    pub fn effective_extruder_count(&self) -> u32 {
        let from_nozzles = u32::try_from(self.nozzle_diameters.len()).unwrap_or(u32::MAX);
        self.extruder_count.max(from_nozzles).max(1)
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
/// A value of 0.0 means "use the base `print` acceleration". The flat
/// `print_acceleration` and `travel_acceleration` fields were migrated
/// from `PrintConfig` in Plan 04 (renamed to `print` and `travel`).
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Acceleration")]
pub struct AccelerationConfig {
    /// Print acceleration in mm/s^2.
    #[setting(tier = 2, description = "Print acceleration", units = "mm/s^2", min = 0.0, max = 50000.0, affects = ["print_time", "quality"])]
    pub print: f64,
    /// Travel acceleration in mm/s^2.
    #[setting(tier = 2, description = "Travel acceleration", units = "mm/s^2", min = 0.0, max = 50000.0, affects = ["print_time"])]
    pub travel: f64,
    /// Outer wall acceleration (mm/s^2, 0 = use print_acceleration).
    #[setting(tier = 3, description = "Outer wall acceleration", units = "mm/s^2", min = 0.0, max = 50000.0, affects = ["quality"])]
    pub outer_wall: f64,
    /// Inner wall acceleration (mm/s^2, 0 = use print_acceleration).
    #[setting(
        tier = 3,
        description = "Inner wall acceleration",
        units = "mm/s^2",
        min = 0.0,
        max = 50000.0
    )]
    pub inner_wall: f64,
    /// Initial layer acceleration (mm/s^2, 0 = use print_acceleration).
    #[setting(tier = 3, description = "Initial layer acceleration", units = "mm/s^2", min = 0.0, max = 50000.0, affects = ["adhesion"])]
    pub initial_layer: f64,
    /// Initial layer travel acceleration (mm/s^2, 0 = use travel_acceleration).
    #[setting(
        tier = 3,
        description = "Initial layer travel acceleration",
        units = "mm/s^2",
        min = 0.0,
        max = 50000.0
    )]
    pub initial_layer_travel: f64,
    /// Top surface acceleration (mm/s^2, 0 = use print_acceleration).
    #[setting(tier = 3, description = "Top surface acceleration", units = "mm/s^2", min = 0.0, max = 50000.0, affects = ["quality"])]
    pub top_surface: f64,
    /// Sparse infill acceleration (mm/s^2, 0 = use print_acceleration).
    #[setting(
        tier = 3,
        description = "Sparse infill acceleration",
        units = "mm/s^2",
        min = 0.0,
        max = 50000.0
    )]
    pub sparse_infill: f64,
    /// Bridge acceleration (mm/s^2, 0 = use print_acceleration).
    #[setting(
        tier = 3,
        description = "Bridge acceleration",
        units = "mm/s^2",
        min = 0.0,
        max = 50000.0
    )]
    pub bridge: f64,
    /// Minimum segment length factor (percentage, 0-100).
    /// Prevents acceleration changes on segments shorter than this factor
    /// of the nominal acceleration distance. 0 = disabled.
    /// OrcaSlicer: `min_length_factor`. PrusaSlicer: N/A. Default: 0.0.
    #[setting(
        tier = 4,
        description = "Minimum segment length factor for acceleration changes",
        units = "%",
        min = 0.0,
        max = 100.0
    )]
    pub min_length_factor: f64,
    /// Internal solid infill acceleration in mm/s^2.
    /// OrcaSlicer: `internal_solid_infill_acceleration`. Range: 0-50000. Default: 0.0 (use default).
    #[setting(
        tier = 3,
        description = "Internal solid infill acceleration",
        units = "mm/s^2",
        min = 0.0,
        max = 50000.0
    )]
    pub internal_solid_infill_acceleration: f64,
    /// Support acceleration in mm/s^2.
    /// OrcaSlicer: `support_acceleration`. Range: 0-50000. Default: 0.0 (use default).
    #[setting(
        tier = 3,
        description = "Support structure acceleration",
        units = "mm/s^2",
        min = 0.0,
        max = 50000.0
    )]
    pub support_acceleration: f64,
    /// Support interface acceleration in mm/s^2.
    /// OrcaSlicer: `support_interface_acceleration`. Range: 0-50000. Default: 0.0 (use default).
    #[setting(
        tier = 3,
        description = "Support interface acceleration",
        units = "mm/s^2",
        min = 0.0,
        max = 50000.0
    )]
    pub support_interface_acceleration: f64,
    /// Default jerk value in mm/s (0 = firmware default).
    /// OrcaSlicer: `default_jerk`. Default: 0.0.
    #[serde(default)]
    #[setting(tier = 3, description = "Default jerk value", units = "mm/s")]
    pub default_jerk: f64,
    /// Outer wall jerk value in mm/s (0 = use default_jerk).
    /// OrcaSlicer: `outer_wall_jerk`. Default: 0.0.
    #[serde(default)]
    #[setting(tier = 3, description = "Outer wall jerk", units = "mm/s")]
    pub outer_wall_jerk: f64,
    /// Inner wall jerk value in mm/s (0 = use default_jerk).
    /// OrcaSlicer: `inner_wall_jerk`. Default: 0.0.
    #[serde(default)]
    #[setting(tier = 3, description = "Inner wall jerk", units = "mm/s")]
    pub inner_wall_jerk: f64,
    /// Top surface jerk value in mm/s (0 = use default_jerk).
    /// OrcaSlicer: `top_surface_jerk`. Default: 0.0.
    #[serde(default)]
    #[setting(tier = 3, description = "Top surface jerk", units = "mm/s")]
    pub top_surface_jerk: f64,
    /// Infill jerk value in mm/s (0 = use default_jerk).
    /// OrcaSlicer: `infill_jerk`. Default: 0.0.
    #[serde(default)]
    #[setting(tier = 3, description = "Infill jerk", units = "mm/s")]
    pub infill_jerk: f64,
    /// Travel jerk value in mm/s (0 = use default_jerk).
    /// OrcaSlicer: `travel_jerk`. Default: 0.0.
    #[serde(default)]
    #[setting(tier = 3, description = "Travel jerk", units = "mm/s")]
    pub travel_jerk: f64,
    /// Initial layer jerk value in mm/s (0 = use default_jerk).
    /// OrcaSlicer: `initial_layer_jerk`. Default: 0.0.
    #[serde(default)]
    #[setting(tier = 3, description = "Initial layer jerk", units = "mm/s")]
    pub initial_layer_jerk: f64,
}

impl Default for AccelerationConfig {
    fn default() -> Self {
        Self {
            print: 1000.0,
            travel: 1500.0,
            outer_wall: 0.0,
            inner_wall: 0.0,
            initial_layer: 0.0,
            initial_layer_travel: 0.0,
            top_surface: 0.0,
            sparse_infill: 0.0,
            bridge: 0.0,
            min_length_factor: 0.0,
            internal_solid_infill_acceleration: 0.0,
            support_acceleration: 0.0,
            support_interface_acceleration: 0.0,
            default_jerk: 0.0,
            outer_wall_jerk: 0.0,
            inner_wall_jerk: 0.0,
            top_surface_jerk: 0.0,
            infill_jerk: 0.0,
            travel_jerk: 0.0,
            initial_layer_jerk: 0.0,
        }
    }
}

/// Filament properties configuration.
///
/// Contains filament metadata, temperature ranges, per-extruder temperature
/// arrays, and filament-specific retraction overrides. The flat
/// `filament_diameter`, `filament_density`, and `filament_cost_per_kg` fields
/// were migrated from `PrintConfig` in Plan 04 (renamed to `diameter`,
/// `density`, `cost_per_kg`).
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Filament")]
pub struct FilamentPropsConfig {
    /// Filament diameter in mm.
    #[setting(
        tier = 2,
        description = "Filament diameter",
        units = "mm",
        min = 1.0,
        max = 3.0
    )]
    pub diameter: f64,
    /// Filament density in g/cm^3 (PLA ~1.24, ABS ~1.04, PETG ~1.27).
    #[setting(
        tier = 3,
        description = "Filament density",
        units = "g/cm^3",
        min = 0.5,
        max = 3.0
    )]
    pub density: f64,
    /// Filament cost per kilogram in currency units (e.g., USD/kg).
    #[setting(tier = 3, description = "Filament cost per kilogram")]
    pub cost_per_kg: f64,
    /// Filament material type (e.g., "PLA", "ABS", "PETG").
    #[setting(tier = 1, description = "Filament material type", affects = ["temperature", "cooling", "retraction"])]
    pub filament_type: String,
    /// Filament vendor/manufacturer name.
    #[setting(tier = 3, description = "Filament vendor/manufacturer name")]
    pub filament_vendor: String,
    /// Maximum volumetric speed (mm^3/s, 0 = unlimited).
    #[setting(
        tier = 3,
        description = "Maximum volumetric extrusion speed",
        units = "mm^3/s",
        min = 0.0,
        max = 100.0
    )]
    pub max_volumetric_speed: f64,
    /// Low end of recommended nozzle temperature range (degrees C).
    #[setting(
        tier = 3,
        description = "Nozzle temperature range low end",
        units = "deg_c"
    )]
    pub nozzle_temperature_range_low: f64,
    /// High end of recommended nozzle temperature range (degrees C).
    #[setting(
        tier = 3,
        description = "Nozzle temperature range high end",
        units = "deg_c"
    )]
    pub nozzle_temperature_range_high: f64,
    /// Per-extruder nozzle temperatures (degrees C). Multi-extruder array.
    #[setting(tier = 1, description = "Nozzle temperature per extruder", units = "deg_c", affects = ["adhesion", "quality", "stringing"])]
    pub nozzle_temperatures: Vec<f64>,
    /// Per-extruder bed temperatures (degrees C). Multi-extruder array.
    #[setting(tier = 1, description = "Bed temperature per extruder", units = "deg_c", affects = ["adhesion"])]
    pub bed_temperatures: Vec<f64>,
    /// Per-extruder first layer nozzle temperatures (degrees C). Multi-extruder array.
    #[setting(tier = 2, description = "First layer nozzle temperature per extruder", units = "deg_c", affects = ["adhesion"])]
    pub first_layer_nozzle_temperatures: Vec<f64>,
    /// Per-extruder first layer bed temperatures (degrees C). Multi-extruder array.
    #[setting(tier = 2, description = "First layer bed temperature per extruder", units = "deg_c", affects = ["adhesion"])]
    pub first_layer_bed_temperatures: Vec<f64>,
    /// Filament-specific retraction length override (mm, None = use global).
    #[setting(
        tier = 3,
        description = "Filament-specific retraction length override",
        units = "mm"
    )]
    pub filament_retraction_length: Option<f64>,
    /// Filament-specific retraction speed override (mm/s, None = use global).
    #[setting(
        tier = 3,
        description = "Filament-specific retraction speed override",
        units = "mm/s"
    )]
    pub filament_retraction_speed: Option<f64>,
    /// Filament start G-code (run once when filament loaded).
    #[setting(tier = 4, description = "Filament-specific startup G-code")]
    pub filament_start_gcode: String,
    /// Filament end G-code (run once when filament unloaded).
    #[setting(tier = 4, description = "Filament-specific shutdown G-code")]
    pub filament_end_gcode: String,
    /// Desired chamber temperature for this filament (degrees C, 0 = not required).
    /// Validated against `MachineConfig.chamber_temperature` (max) during profile merge.
    /// OrcaSlicer: `chamber_temperature` (in filament profile). Range: 0-80. Default: 0.0.
    #[setting(
        tier = 3,
        description = "Desired chamber temperature for this filament",
        units = "deg_c",
        min = 0.0,
        max = 80.0
    )]
    pub chamber_temperature: f64,
    /// Filament shrinkage compensation percentage (100 = no shrink, >100 = expand).
    /// OrcaSlicer: `filament_shrinkage_compensation`. PrusaSlicer: N/A.
    /// Range: 90.0-110.0. Default: 100.0 (no compensation).
    #[setting(
        tier = 4,
        description = "Filament shrinkage compensation",
        units = "%",
        min = 90.0,
        max = 110.0
    )]
    pub filament_shrink: f64,
    /// Per-filament Z offset additive adjustment (mm). Added to global z_offset.
    /// OrcaSlicer: `z_offset` (in filament profile). PrusaSlicer: N/A.
    /// Range: -2.0 to 2.0. Default: 0.0.
    #[setting(tier = 3, description = "Per-filament Z offset adjustment", units = "mm", min = -2.0, max = 2.0)]
    pub z_offset: f64,
    /// Filament color as hex string (e.g., "#FF0000") for preview rendering.
    /// OrcaSlicer: `filament_colour`. Default: "" (no color set).
    #[setting(tier = 2, description = "Filament color for preview")]
    pub filament_colour: String,
    // --- Per-bed-type temperatures (Vec<f64> for multi-extruder) ---
    /// Smooth/hot plate temperatures per extruder (degrees C).
    /// OrcaSlicer: `hot_plate_temp`. Default: empty (use bed_temperatures).
    #[setting(
        tier = 3,
        description = "Hot plate temperature per extruder",
        units = "deg_c"
    )]
    pub hot_plate_temp: Vec<f64>,
    /// Cool plate temperatures per extruder (degrees C).
    /// OrcaSlicer: `cool_plate_temp`. Default: empty.
    #[setting(
        tier = 3,
        description = "Cool plate temperature per extruder",
        units = "deg_c"
    )]
    pub cool_plate_temp: Vec<f64>,
    /// Engineering plate temperatures per extruder (degrees C).
    /// OrcaSlicer: `eng_plate_temp`. Default: empty.
    #[setting(
        tier = 3,
        description = "Engineering plate temperature per extruder",
        units = "deg_c"
    )]
    pub eng_plate_temp: Vec<f64>,
    /// Textured plate temperatures per extruder (degrees C).
    /// OrcaSlicer: `textured_plate_temp`. Default: empty.
    #[setting(
        tier = 3,
        description = "Textured plate temperature per extruder",
        units = "deg_c"
    )]
    pub textured_plate_temp: Vec<f64>,
    /// Smooth/hot plate first layer temperatures per extruder (degrees C).
    /// OrcaSlicer: `hot_plate_temp_initial_layer`. Default: empty.
    #[setting(
        tier = 3,
        description = "Hot plate first layer temperature per extruder",
        units = "deg_c"
    )]
    pub hot_plate_temp_initial_layer: Vec<f64>,
    /// Cool plate first layer temperatures per extruder (degrees C).
    /// OrcaSlicer: `cool_plate_temp_initial_layer`. Default: empty.
    #[setting(
        tier = 3,
        description = "Cool plate first layer temperature per extruder",
        units = "deg_c"
    )]
    pub cool_plate_temp_initial_layer: Vec<f64>,
    /// Engineering plate first layer temperatures per extruder (degrees C).
    /// OrcaSlicer: `eng_plate_temp_initial_layer`. Default: empty.
    #[setting(
        tier = 3,
        description = "Engineering plate first layer temperature per extruder",
        units = "deg_c"
    )]
    pub eng_plate_temp_initial_layer: Vec<f64>,
    /// Textured plate first layer temperatures per extruder (degrees C).
    /// OrcaSlicer: `textured_plate_temp_initial_layer`. Default: empty.
    #[setting(
        tier = 3,
        description = "Textured plate first layer temperature per extruder",
        units = "deg_c"
    )]
    pub textured_plate_temp_initial_layer: Vec<f64>,
}

impl Default for FilamentPropsConfig {
    fn default() -> Self {
        Self {
            diameter: 1.75,
            density: 1.24,
            cost_per_kg: 25.0,
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
            chamber_temperature: 0.0,
            filament_shrink: 100.0,
            z_offset: 0.0,
            filament_colour: String::new(),
            hot_plate_temp: Vec::new(),
            cool_plate_temp: Vec::new(),
            eng_plate_temp: Vec::new(),
            textured_plate_temp: Vec::new(),
            hot_plate_temp_initial_layer: Vec::new(),
            cool_plate_temp_initial_layer: Vec::new(),
            eng_plate_temp_initial_layer: Vec::new(),
            textured_plate_temp_initial_layer: Vec::new(),
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

    /// Resolves bed temperatures based on the selected bed type.
    ///
    /// Returns `(normal_temp, first_layer_temp)` for the first extruder.
    /// Falls back to `bed_temperatures`/`first_layer_bed_temperatures` if
    /// the per-type temperature array is empty.
    pub fn resolve_bed_temperatures(&self, bed_type: BedType) -> (f64, f64) {
        let (temps, fl_temps) = match bed_type {
            BedType::CoolPlate => (&self.cool_plate_temp, &self.cool_plate_temp_initial_layer),
            BedType::EngineeringPlate => (&self.eng_plate_temp, &self.eng_plate_temp_initial_layer),
            BedType::HighTempPlate | BedType::SmoothPei => {
                (&self.hot_plate_temp, &self.hot_plate_temp_initial_layer)
            }
            BedType::TexturedPei | BedType::SatinPei => (
                &self.textured_plate_temp,
                &self.textured_plate_temp_initial_layer,
            ),
        };
        let normal = temps.first().copied().unwrap_or_else(|| self.bed_temp());
        let first_layer = fl_temps
            .first()
            .copied()
            .unwrap_or_else(|| self.first_layer_bed_temp());
        (normal, first_layer)
    }
}

/// Print configuration controlling the entire slicing pipeline.
///
/// Print ordering strategy for multi-object plates.
///
/// Controls whether features are grouped by layer across all objects,
/// or by object within each layer.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, SettingSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrintOrder {
    /// Process all objects' features per feature group per layer (default).
    #[default]
    #[setting(
        display = "By Layer",
        description = "Process features by layer across all objects"
    )]
    ByLayer,
    /// Complete each object's features per layer before moving to next object.
    #[setting(
        display = "By Object",
        description = "Complete each object per layer before next"
    )]
    ByObject,
}

/// TSP algorithm selection for travel move optimization.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, SettingSchema)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum TravelOptAlgorithm {
    /// Try both NN and greedy, pick shorter, apply 2-opt (recommended).
    #[default]
    #[setting(
        display = "Auto",
        description = "Try both algorithms, pick best result with 2-opt"
    )]
    Auto,
    /// Nearest-neighbor construction with 2-opt refinement.
    #[setting(
        display = "Nearest Neighbor",
        description = "NN construction + 2-opt improvement"
    )]
    NearestNeighbor,
    /// Greedy edge insertion with 2-opt refinement.
    #[setting(
        display = "Greedy Edge Insertion",
        description = "Greedy construction + 2-opt improvement"
    )]
    GreedyEdgeInsertion,
    /// Nearest-neighbor only (no 2-opt).
    #[setting(
        display = "NN Only",
        description = "Nearest-neighbor without 2-opt improvement"
    )]
    NearestNeighborOnly,
    /// Greedy edge insertion only (no 2-opt).
    #[setting(
        display = "Greedy Only",
        description = "Greedy edge insertion without 2-opt"
    )]
    GreedyOnly,
}

/// Travel move optimization configuration.
///
/// Controls TSP-based reordering of printable elements within each layer
/// to minimize non-extrusion travel distance.
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Travel")]
pub struct TravelOptConfig {
    /// Enable travel move optimization.
    #[setting(tier = 3, description = "Enable TSP-based travel move optimization")]
    pub enabled: bool,
    /// TSP algorithm selection.
    #[setting(tier = 4, description = "TSP algorithm for travel optimization")]
    pub algorithm: TravelOptAlgorithm,
    /// Maximum 2-opt improvement iterations (0 = no limit until convergence).
    #[setting(
        tier = 4,
        description = "Maximum 2-opt improvement passes",
        min = 0.0,
        max = 10000.0
    )]
    pub max_iterations: u32,
    /// Optimize travel between objects on the same layer.
    #[setting(
        tier = 3,
        description = "Optimize cross-object travel on multi-object plates"
    )]
    pub optimize_cross_object: bool,
    /// Print ordering strategy for multi-object plates.
    #[setting(tier = 2, description = "Print ordering strategy")]
    pub print_order: PrintOrder,
}

impl Default for TravelOptConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            algorithm: TravelOptAlgorithm::Auto,
            max_iterations: 100,
            optimize_cross_object: true,
            print_order: PrintOrder::ByLayer,
        }
    }
}

/// All fields have sensible FDM defaults. Use [`PrintConfig::from_toml`] to
/// parse from a TOML string, [`PrintConfig::from_json`] to parse from a JSON
/// string (native or OrcaSlicer/BambuStudio format), or [`PrintConfig::from_file`]
/// to auto-detect the format and load from a file. Fields not specified in the
/// input use defaults.
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Quality")]
pub struct PrintConfig {
    // --- Layer geometry ---
    /// Standard layer height in mm.
    #[setting(tier = 1, description = "Layer height", units = "mm", min = 0.01, max = 1.0, affects = ["quality", "print_time", "strength"])]
    pub layer_height: f64,
    /// First layer height in mm (typically thicker for bed adhesion).
    #[setting(tier = 1, description = "First layer height", units = "mm", min = 0.01, max = 1.0, affects = ["adhesion", "quality"])]
    pub first_layer_height: f64,

    // --- Walls ---
    /// Number of perimeter walls.
    #[setting(tier = 1, description = "Number of perimeter walls", affects = ["strength", "quality", "print_time"])]
    pub wall_count: u32,
    /// Order in which walls are printed.
    #[setting(tier = 2, description = "Wall printing order", affects = ["quality"])]
    pub wall_order: WallOrder,
    /// Seam placement strategy for perimeter loops.
    #[setting(tier = 2, description = "Seam placement strategy", affects = ["quality"])]
    pub seam_position: SeamPosition,

    // --- Infill ---
    /// Infill pattern to use for sparse infill regions.
    #[setting(tier = 1, description = "Infill pattern", affects = ["strength", "print_time"])]
    pub infill_pattern: InfillPattern,
    /// Infill density as a fraction (0.0 = hollow, 1.0 = solid).
    #[setting(tier = 1, description = "Infill density", units = "%", min = 0.0, max = 100.0, affects = ["strength", "weight", "print_time"])]
    pub infill_density: f64,
    /// Number of solid top layers.
    #[setting(tier = 1, description = "Number of top solid layers", affects = ["quality", "strength"])]
    pub top_solid_layers: u32,
    /// Number of solid bottom layers.
    #[setting(tier = 1, description = "Number of bottom solid layers", affects = ["quality", "strength"])]
    pub bottom_solid_layers: u32,

    // --- Skirt/Brim ---
    /// Number of skirt loops.
    #[setting(tier = 2, description = "Number of skirt loops", category = "Adhesion")]
    pub skirt_loops: u32,
    /// Distance of skirt from object in mm.
    #[setting(
        tier = 2,
        description = "Skirt distance from object",
        units = "mm",
        category = "Adhesion"
    )]
    pub skirt_distance: f64,
    /// Brim width in mm (0.0 = disabled).
    #[setting(tier = 1, description = "Brim width", units = "mm", min = 0.0, max = 50.0, affects = ["adhesion"], category = "Adhesion")]
    pub brim_width: f64,

    // --- Extrusion ---
    /// Extrusion multiplier (flow rate factor).
    #[setting(tier = 2, description = "Extrusion multiplier (flow rate factor)", min = 0.5, max = 2.0, affects = ["quality"])]
    pub extrusion_multiplier: f64,

    // --- Adaptive Layer Heights ---
    /// Enable adaptive layer heights based on surface curvature.
    #[setting(tier = 2, description = "Enable adaptive layer heights", affects = ["quality", "print_time"])]
    pub adaptive_layer_height: bool,
    /// Minimum layer height for adaptive layers (mm).
    #[setting(
        tier = 3,
        description = "Adaptive minimum layer height",
        units = "mm",
        min = 0.01,
        max = 1.0,
        depends_on = "adaptive_layer_height"
    )]
    pub adaptive_min_layer_height: f64,
    /// Maximum layer height for adaptive layers (mm).
    #[setting(
        tier = 3,
        description = "Adaptive maximum layer height",
        units = "mm",
        min = 0.01,
        max = 1.0,
        depends_on = "adaptive_layer_height"
    )]
    pub adaptive_max_layer_height: f64,
    /// Adaptive layer quality (0.0 = speed, 1.0 = quality).
    #[setting(
        tier = 3,
        description = "Adaptive layer quality factor",
        min = 0.0,
        max = 1.0,
        depends_on = "adaptive_layer_height"
    )]
    pub adaptive_layer_quality: f64,

    // --- Gap Fill ---
    /// Enable gap fill between perimeters.
    #[setting(tier = 3, description = "Enable gap fill between perimeters")]
    pub gap_fill_enabled: bool,
    /// Minimum gap width to fill (mm).
    #[setting(
        tier = 3,
        description = "Minimum gap width to fill",
        units = "mm",
        depends_on = "gap_fill_enabled"
    )]
    pub gap_fill_min_width: f64,

    // --- Polyhole Conversion ---
    /// Enable polyhole conversion for circular holes (dimensional accuracy).
    #[setting(
        tier = 3,
        description = "Enable polyhole conversion for circular holes"
    )]
    pub polyhole_enabled: bool,
    /// Minimum hole diameter (mm) for polyhole conversion (skip very small holes).
    #[setting(
        tier = 3,
        description = "Minimum hole diameter for polyhole conversion",
        units = "mm",
        depends_on = "polyhole_enabled"
    )]
    pub polyhole_min_diameter: f64,

    // --- Arachne Variable-Width Perimeters ---
    /// Enable Arachne variable-width perimeters for thin walls.
    #[setting(tier = 3, description = "Enable Arachne variable-width perimeters")]
    pub arachne_enabled: bool,

    // --- Scarf Joint Seam ---
    /// Scarf joint seam configuration.
    #[setting(flatten)]
    pub scarf_joint: ScarfJointConfig,

    // --- Support Structures ---
    /// Support structure generation configuration.
    #[setting(flatten)]
    pub support: SupportConfig,

    // --- Ironing ---
    /// Ironing pass configuration for smooth top surfaces.
    #[setting(flatten)]
    pub ironing: IroningConfig,

    // --- Per-Feature Flow ---
    /// Per-feature flow multipliers for fine-tuning extrusion per feature type.
    #[setting(flatten)]
    pub per_feature_flow: PerFeatureFlow,

    // --- Custom G-code Injection ---
    /// Custom G-code hooks for injection at layer transitions and specific Z heights.
    #[setting(flatten)]
    pub custom_gcode: CustomGcodeHooks,

    // --- G-code Dialect ---
    /// G-code firmware dialect (Marlin, Klipper, RepRapFirmware, Bambu).
    #[setting(tier = 2, description = "G-code firmware dialect")]
    pub gcode_dialect: GcodeDialect,

    // --- Arc Fitting ---
    /// Enable arc fitting post-processing (G1 -> G2/G3 conversion).
    #[setting(tier = 3, description = "Enable arc fitting G2/G3 conversion")]
    pub arc_fitting_enabled: bool,
    /// Maximum deviation (mm) for arc fitting tolerance.
    #[setting(
        tier = 3,
        description = "Arc fitting deviation tolerance",
        units = "mm",
        depends_on = "arc_fitting_enabled"
    )]
    pub arc_fitting_tolerance: f64,
    /// Minimum number of consecutive G1 moves to consider for arc fitting.
    #[setting(
        tier = 4,
        description = "Minimum points for arc detection",
        depends_on = "arc_fitting_enabled"
    )]
    pub arc_fitting_min_points: usize,

    // --- Cross-cutting flags (stay flat) ---
    /// Pressure advance value (0.0 = disabled).
    #[setting(
        tier = 2,
        description = "Pressure advance value",
        category = "Calibration"
    )]
    pub pressure_advance: f64,
    /// Enable acceleration command emission at feature transitions.
    #[setting(
        tier = 3,
        description = "Enable acceleration commands at feature transitions",
        category = "Acceleration"
    )]
    pub acceleration_enabled: bool,

    // --- Multi-Material ---
    /// Multi-material printing configuration (MMU tool changes and purge tower).
    #[setting(flatten)]
    pub multi_material: MultiMaterialConfig,

    // --- Sequential Printing ---
    /// Sequential (object-by-object) printing configuration.
    #[setting(flatten)]
    pub sequential: SequentialConfig,

    // --- Travel Optimization ---
    /// Travel move optimization configuration (TSP-based toolpath ordering).
    #[setting(flatten)]
    pub travel_opt: TravelOptConfig,

    // --- Plugins ---
    /// Directory to scan for plugins (optional).
    ///
    /// When set, the engine can discover and load infill plugins from this
    /// directory. Each plugin should be in its own subdirectory containing
    /// a `plugin.toml` manifest.
    #[serde(default)]
    #[setting(tier = 4, description = "Plugin scan directory")]
    pub plugin_dir: Option<String>,

    // --- Sub-config structs (Phase 20) ---
    /// Per-feature line width configuration.
    #[setting(flatten, prefix = "line_widths")]
    pub line_widths: LineWidthConfig,
    /// Per-feature speed configuration (includes perimeter, infill, travel, first_layer).
    #[setting(flatten, prefix = "speeds")]
    pub speeds: SpeedConfig,
    /// Cooling and fan configuration (includes fan_speed, fan_below_layer_time, disable_fan_first_layers).
    #[setting(flatten)]
    pub cooling: CoolingConfig,
    /// Retraction configuration (includes length, speed, z_hop, min_travel).
    #[setting(flatten)]
    pub retraction: RetractionConfig,
    /// Machine/printer hardware configuration (includes bed_x, bed_y, nozzle_diameters, jerk).
    #[setting(flatten)]
    pub machine: MachineConfig,
    /// Per-feature acceleration configuration (includes print and travel acceleration).
    #[setting(flatten, prefix = "accel")]
    pub accel: AccelerationConfig,
    /// Filament properties configuration (includes diameter, density, cost_per_kg, temperatures).
    #[setting(flatten, prefix = "filament")]
    pub filament: FilamentPropsConfig,

    /// Passthrough fields from upstream profiles that have no engine equivalent.
    /// Preserved for round-trip fidelity and G-code template variable access.
    /// Uses `BTreeMap` for deterministic serialization order.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[setting(skip)]
    pub passthrough: BTreeMap<String, String>,

    // --- Dimensional Compensation (Phase 32) ---
    /// Dimensional compensation settings (XY hole, XY contour, elephant foot).
    #[setting(flatten)]
    pub dimensional_compensation: DimensionalCompensationConfig,

    // --- Surface Patterns (Phase 32) ---
    /// Top surface fill pattern. Default: Monotonic.
    #[setting(tier = 2, description = "Top surface fill pattern", affects = ["quality"])]
    pub top_surface_pattern: SurfacePattern,
    /// Bottom surface fill pattern. Default: Monotonic.
    #[setting(tier = 3, description = "Bottom surface fill pattern")]
    pub bottom_surface_pattern: SurfacePattern,
    /// Internal solid layer fill pattern. Default: Monotonic.
    #[setting(tier = 3, description = "Internal solid layer fill pattern")]
    pub solid_infill_pattern: SurfacePattern,

    // --- Overhangs (Phase 32) ---
    /// Generate extra perimeters on overhangs for better quality.
    /// OrcaSlicer: `extra_perimeters_on_overhangs`. Default: false.
    #[setting(tier = 3, description = "Generate extra perimeters on overhangs")]
    pub extra_perimeters_on_overhangs: bool,

    // --- Bridges (Phase 32) ---
    /// Internal bridge support mode (off/auto/always).
    /// OrcaSlicer: `internal_bridge_support_enabled`. Default: Off.
    #[setting(tier = 3, description = "Internal bridge support mode")]
    pub internal_bridge_support: InternalBridgeMode,

    // --- Z Offset (Phase 32) ---
    /// Global Z offset in mm. Per-filament z_offset is additive.
    /// OrcaSlicer/PrusaSlicer: `z_offset`. Range: -5.0 to 5.0. Default: 0.0.
    #[setting(tier = 2, description = "Global Z offset", units = "mm", min = -5.0, max = 5.0)]
    pub z_offset: f64,

    // --- Precise Z (Phase 32) ---
    /// Enable precise Z height positioning.
    /// OrcaSlicer: `precise_z_height`. Default: false.
    #[setting(tier = 4, description = "Enable precise Z height positioning")]
    pub precise_z_height: bool,

    // --- Process misc fields (Phase 20) ---
    /// Bridge flow ratio (1.0 = normal flow).
    #[setting(tier = 3, description = "Bridge flow ratio", min = 0.1, max = 2.0, affects = ["bridging"])]
    pub bridge_flow: f64,
    /// Infill line direction in degrees.
    #[setting(
        tier = 3,
        description = "Infill line direction angle",
        units = "deg",
        category = "Infill"
    )]
    pub infill_direction: f64,
    /// Infill-wall overlap as a fraction (0-1).
    #[setting(
        tier = 3,
        description = "Infill-wall overlap fraction",
        min = 0.0,
        max = 1.0,
        category = "Infill"
    )]
    pub infill_wall_overlap: f64,
    /// Enable spiral (vase) mode.
    #[setting(tier = 2, description = "Enable spiral vase mode")]
    pub spiral_mode: bool,
    /// Use only one wall on top surfaces.
    #[setting(tier = 3, description = "Use only one wall on top surfaces")]
    pub only_one_wall_top: bool,
    /// G-code resolution in mm.
    #[setting(tier = 3, description = "G-code point resolution", units = "mm")]
    pub resolution: f64,
    /// Number of raft layers (0 = disabled).
    #[setting(tier = 2, description = "Number of raft layers", category = "Adhesion")]
    pub raft_layers: u32,
    /// Enable thin wall detection.
    #[setting(tier = 3, description = "Enable thin wall detection")]
    pub detect_thin_wall: bool,

    // --- Parallelism ---
    /// Whether to use parallel (rayon) processing for per-layer operations.
    /// When false, layers are processed sequentially (useful for debugging
    /// or determinism verification). Default: true.
    #[setting(tier = 4, description = "Enable parallel slicing")]
    pub parallel_slicing: bool,

    /// Number of threads for parallel processing. None = auto-detect
    /// (rayon default: number of logical CPUs). Only effective when
    /// parallel_slicing is true and the `parallel` feature is enabled.
    #[setting(tier = 4, description = "Number of processing threads")]
    pub thread_count: Option<usize>,

    // --- Thumbnail ---
    /// Thumbnail resolution as [width, height] in pixels.
    /// Used when generating thumbnail images for 3MF or G-code embedding.
    #[serde(default = "default_thumbnail_resolution")]
    #[setting(tier = 4, description = "Thumbnail resolution")]
    pub thumbnail_resolution: [u32; 2],

    // --- Post-Processing ---
    /// Post-processing pipeline configuration.
    /// Controls built-in post-processors (pause at layer, timelapse, fan override,
    /// custom G-code injection) that run after G-code generation.
    #[serde(default)]
    #[setting(flatten)]
    pub post_process: PostProcessConfig,

    // --- Fuzzy Skin (Phase 33) ---
    /// Fuzzy skin configuration for textured surface finish.
    #[setting(flatten)]
    pub fuzzy_skin: FuzzySkinConfig,

    // --- Brim/Skirt Additions (Phase 33) ---
    /// Additional brim and skirt configuration fields.
    /// Note: existing skirt_loops, skirt_distance, brim_width remain at top-level.
    #[setting(flatten)]
    pub brim_skirt: BrimSkirtConfig,

    // --- Input Shaping (Phase 33) ---
    /// Input shaping motion configuration.
    #[setting(flatten)]
    pub input_shaping: InputShapingConfig,

    // --- Precise Outer Wall (Phase 33) ---
    /// Enable precise outer wall positioning for dimensional accuracy.
    /// OrcaSlicer: `precise_outer_wall`. Default: false.
    #[setting(tier = 3, description = "Enable precise outer wall positioning")]
    pub precise_outer_wall: bool,

    // --- Draft Shield (Phase 33) ---
    /// Enable draft shield (wall around print to block air currents).
    /// OrcaSlicer: `draft_shield`. PrusaSlicer: `draft_shield`. Default: false.
    #[setting(tier = 3, description = "Enable draft shield to block air currents")]
    pub draft_shield: bool,

    // --- Ooze Prevention (Phase 33) ---
    /// Enable ooze prevention (standby temp reduction for inactive tools).
    /// OrcaSlicer: `ooze_prevention`. PrusaSlicer: `ooze_prevention`. Default: false.
    #[setting(tier = 3, description = "Enable ooze prevention for multi-tool")]
    pub ooze_prevention: bool,

    // --- Infill Combination (Phase 33) ---
    /// Combine infill every N layers (0 or 1 = disabled).
    /// OrcaSlicer: `infill_combination`. Range: 0-10. Default: 0.
    #[setting(
        tier = 3,
        description = "Combine infill every N layers",
        min = 0.0,
        max = 10.0,
        category = "Infill"
    )]
    pub infill_combination: u32,

    // --- Infill Anchor (Phase 33) ---
    /// Maximum length of infill anchor in mm.
    /// OrcaSlicer: `infill_anchor_max`. PrusaSlicer: `infill_anchor_max`. Range: 0-50. Default: 12.0.
    #[setting(
        tier = 3,
        description = "Maximum infill anchor length",
        units = "mm",
        min = 0.0,
        max = 50.0,
        category = "Infill"
    )]
    pub infill_anchor_max: f64,

    // --- Arachne Parameters (Phase 33) ---
    /// Minimum bead width for Arachne wall generation in mm.
    /// OrcaSlicer: `min_bead_width`. Range: 0.0-1.0. Default: 0.315.
    #[setting(
        tier = 3,
        description = "Minimum Arachne bead width",
        units = "mm",
        min = 0.0,
        max = 1.0,
        depends_on = "arachne_enabled"
    )]
    pub min_bead_width: f64,
    /// Minimum feature size for Arachne detection in mm.
    /// OrcaSlicer: `min_feature_size`. Range: 0.0-1.0. Default: 0.25.
    #[setting(
        tier = 3,
        description = "Minimum Arachne feature size",
        units = "mm",
        min = 0.0,
        max = 1.0,
        depends_on = "arachne_enabled"
    )]
    pub min_feature_size: f64,

    // --- P2 Niche Fields (Phase 34) ---
    /// Slicing tolerance mode for layer boundary positioning.
    /// OrcaSlicer/PrusaSlicer: `slicing_tolerance`. Default: Middle.
    #[serde(default)]
    #[setting(tier = 4, description = "Slicing tolerance mode")]
    pub slicing_tolerance: SlicingTolerance,
    /// Thumbnail size specs (e.g., ["96x96", "400x300"]).
    /// OrcaSlicer/PrusaSlicer: `thumbnails`. Default: empty.
    #[serde(default)]
    #[setting(tier = 4, description = "Thumbnail size specifications")]
    pub thumbnails: Vec<String>,
    /// Cumulative compatible printers condition expressions.
    /// OrcaSlicer: `compatible_printers_condition_cummulative`. Default: empty.
    #[serde(default)]
    #[setting(tier = 4, description = "Compatible printers condition expressions")]
    pub compatible_printers_condition: Vec<String>,
    /// Profile inheritance grouping identifier.
    /// OrcaSlicer: `inherits_group`. Default: empty.
    #[serde(default)]
    #[setting(tier = 4, description = "Profile inheritance group identifier")]
    pub inherits_group: String,
    /// Maximum travel detour length for optimization in mm.
    /// OrcaSlicer: `max_travel_detour_distance`. Default: 0.0.
    #[serde(default)]
    #[setting(tier = 4, description = "Maximum travel detour length", units = "mm")]
    pub max_travel_detour_length: f64,
    /// Enable exclude/cancel object support.
    /// OrcaSlicer: `exclude_object`. Default: false.
    #[serde(default)]
    #[setting(tier = 3, description = "Enable exclude/cancel object support")]
    pub exclude_object: bool,
    /// Reduce retraction on infill-to-infill travel.
    /// OrcaSlicer: `reduce_infill_retraction`. Default: false.
    #[serde(default)]
    #[setting(
        tier = 3,
        description = "Reduce retraction on infill-to-infill travel",
        category = "Infill"
    )]
    pub reduce_infill_retraction: bool,
    /// Reduce wall crossing during travel moves.
    /// OrcaSlicer: `reduce_crossing_wall`. Default: false.
    #[serde(default)]
    #[setting(
        tier = 3,
        description = "Reduce wall crossing during travel moves",
        category = "Retraction"
    )]
    pub reduce_crossing_wall: bool,
}

fn default_thumbnail_resolution() -> [u32; 2] {
    [300, 300]
}

fn default_true() -> bool {
    true
}

// ============================================================================
// Post-Processing Configuration
// ============================================================================

/// Post-processing pipeline configuration.
///
/// Controls the built-in post-processors that run after G-code generation
/// and arc fitting, but before time estimation. All features are disabled
/// by default for backward compatibility.
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "PostProcess")]
pub struct PostProcessConfig {
    /// Master switch for the post-processing pipeline.
    #[setting(tier = 3, description = "Enable post-processing pipeline")]
    pub enabled: bool,
    /// Layer indices (zero-based) at which to insert a pause command.
    #[setting(
        tier = 3,
        description = "Layer indices at which to pause",
        depends_on = "post_process.enabled"
    )]
    pub pause_at_layers: Vec<usize>,
    /// G-code command to insert for pause (typically "M0" or "M600").
    #[setting(
        tier = 4,
        description = "G-code command for pause insertion",
        depends_on = "post_process.enabled"
    )]
    pub pause_command: String,
    /// Timelapse camera configuration.
    #[setting(flatten)]
    pub timelapse: TimelapseConfig,
    /// Fan speed override rules applied to specific layer ranges.
    #[setting(skip)]
    pub fan_overrides: Vec<FanOverrideRule>,
    /// Custom G-code injection rules with various trigger types.
    #[setting(skip)]
    pub custom_gcode: Vec<CustomGcodeRule>,
    /// Explicit plugin execution order by name.
    /// When empty, plugins are sorted by priority.
    #[setting(tier = 4, description = "Plugin execution order")]
    pub plugin_order: Vec<String>,
    /// Post-process script paths (semicolon/newline separated in upstream).
    /// OrcaSlicer/PrusaSlicer: `post_process`. Default: empty.
    #[serde(default)]
    #[setting(tier = 4, description = "Post-process script paths")]
    pub scripts: Vec<String>,
    /// Label objects in G-code output (Exclude Object support).
    /// OrcaSlicer: `gcode_label_objects`. Default: false.
    #[serde(default)]
    #[setting(
        tier = 3,
        description = "Label objects in G-code for exclude object support"
    )]
    pub gcode_label_objects: bool,
    /// Include comments in G-code output.
    /// OrcaSlicer: `gcode_comments`. Default: false.
    #[serde(default)]
    #[setting(tier = 4, description = "Include comments in G-code output")]
    pub gcode_comments: bool,
    /// Add line numbers to G-code output.
    /// OrcaSlicer: `gcode_add_line_number`. Default: false.
    #[serde(default)]
    #[setting(tier = 4, description = "Add line numbers to G-code output")]
    pub gcode_add_line_number: bool,
    /// Output filename format template.
    /// OrcaSlicer: `filename_format`. Default: empty.
    #[serde(default)]
    #[setting(tier = 4, description = "Output filename format template")]
    pub filename_format: String,
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            pause_at_layers: Vec::new(),
            pause_command: "M0".to_string(),
            timelapse: TimelapseConfig::default(),
            fan_overrides: Vec::new(),
            custom_gcode: Vec::new(),
            plugin_order: Vec::new(),
            scripts: Vec::new(),
            gcode_label_objects: false,
            gcode_comments: false,
            gcode_add_line_number: false,
            filename_format: String::new(),
        }
    }
}

/// Timelapse camera configuration for layer-change snapshots.
///
/// When enabled, the post-processor inserts a retract-park-dwell-unretract
/// sequence at every layer change to allow a camera to capture a frame.
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Timelapse")]
pub struct TimelapseConfig {
    /// Enable timelapse camera support.
    #[setting(tier = 3, description = "Enable timelapse camera support")]
    pub enabled: bool,
    /// X position to park at for the snapshot (mm).
    #[setting(
        tier = 3,
        description = "Timelapse park X position",
        units = "mm",
        depends_on = "post_process.timelapse.enabled"
    )]
    pub park_x: f64,
    /// Y position to park at for the snapshot (mm).
    #[setting(
        tier = 3,
        description = "Timelapse park Y position",
        units = "mm",
        depends_on = "post_process.timelapse.enabled"
    )]
    pub park_y: f64,
    /// Dwell time at park position for camera capture (ms).
    #[setting(
        tier = 3,
        description = "Timelapse dwell time at park position",
        units = "ms",
        depends_on = "post_process.timelapse.enabled"
    )]
    pub dwell_ms: u32,
    /// Retraction distance before moving to park position (mm).
    #[setting(
        tier = 4,
        description = "Retraction distance before timelapse park",
        units = "mm",
        depends_on = "post_process.timelapse.enabled"
    )]
    pub retract_distance: f64,
    /// Retraction speed (mm/min).
    #[setting(
        tier = 4,
        description = "Retraction speed before timelapse park",
        units = "mm/min",
        depends_on = "post_process.timelapse.enabled"
    )]
    pub retract_speed: f64,
}

impl Default for TimelapseConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            park_x: 0.0,
            park_y: 0.0,
            dwell_ms: 500,
            retract_distance: 1.0,
            retract_speed: 2400.0,
        }
    }
}

/// A fan speed override rule applied to a range of layers.
///
/// When the post-processor encounters a `SetFanSpeed` command within
/// the specified layer range, it replaces the fan speed with the
/// configured value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanOverrideRule {
    /// First layer (zero-based) where the override applies.
    pub start_layer: usize,
    /// Last layer (zero-based, inclusive) where the override applies.
    /// `None` means the override applies until the end of the print.
    pub end_layer: Option<usize>,
    /// Fan speed to use (0-255).
    pub fan_speed: u8,
}

/// Trigger condition for custom G-code injection.
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CustomGcodeTrigger {
    /// Inject after every N layers.
    #[setting(
        display = "Every N Layers",
        description = "Inject G-code after every N layers"
    )]
    EveryNLayers {
        /// Injection interval in layers.
        n: usize,
    },
    /// Inject at specific layer indices.
    #[setting(
        display = "At Layers",
        description = "Inject G-code at specific layer indices"
    )]
    AtLayers {
        /// Layer indices (zero-based) at which to inject.
        layers: Vec<usize>,
    },
    /// Inject immediately before each retraction.
    #[setting(
        display = "Before Retraction",
        description = "Inject G-code before each retraction"
    )]
    BeforeRetraction,
    /// Inject immediately after each unretraction.
    #[setting(
        display = "After Retraction",
        description = "Inject G-code after each unretraction"
    )]
    AfterRetraction,
}

/// A custom G-code injection rule.
///
/// Injects arbitrary G-code at the specified trigger points.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomGcodeRule {
    /// When to inject the G-code.
    pub trigger: CustomGcodeTrigger,
    /// Raw G-code string to inject (may contain multiple lines).
    pub gcode: String,
}

/// Scarf joint seam configuration.
///
/// The scarf joint gradually ramps Z height and flow rate at the perimeter
/// seam point, creating a smooth overlap instead of an abrupt start/end.
/// This makes seams nearly invisible on smooth surfaces.
///
/// All 12 parameters match OrcaSlicer's scarf joint specification.
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Quality")]
pub struct ScarfJointConfig {
    /// Enable scarf joint seam.
    #[setting(tier = 3, description = "Enable scarf joint seam")]
    pub enabled: bool,
    /// Apply to contours and/or holes.
    #[setting(
        tier = 3,
        description = "Scarf joint contour/hole selection",
        depends_on = "scarf_joint.enabled"
    )]
    pub scarf_joint_type: ScarfJointType,
    /// Only apply on smooth perimeters (no sharp corners near seam).
    #[setting(
        tier = 3,
        description = "Only apply scarf on smooth perimeters",
        depends_on = "scarf_joint.enabled"
    )]
    pub conditional_scarf: bool,
    /// Speed during scarf region (mm/s, 0 = use wall speed).
    #[setting(
        tier = 3,
        description = "Speed during scarf region",
        units = "mm/s",
        min = 0.0,
        max = 1000.0,
        depends_on = "scarf_joint.enabled"
    )]
    pub scarf_speed: f64,
    /// Z offset at ramp start as fraction of layer height (0.0-1.0).
    #[setting(
        tier = 3,
        description = "Z offset at ramp start as fraction of layer height",
        min = 0.0,
        max = 1.0,
        depends_on = "scarf_joint.enabled"
    )]
    pub scarf_start_height: f64,
    /// Apply scarf around entire wall (not just seam region).
    #[setting(
        tier = 3,
        description = "Apply scarf around entire wall",
        depends_on = "scarf_joint.enabled"
    )]
    pub scarf_around_entire_wall: bool,
    /// Horizontal length of the scarf ramp in mm.
    #[setting(
        tier = 3,
        description = "Horizontal scarf ramp length",
        units = "mm",
        min = 0.0,
        max = 100.0,
        depends_on = "scarf_joint.enabled"
    )]
    pub scarf_length: f64,
    /// Number of discrete steps in the ramp.
    #[setting(
        tier = 3,
        description = "Number of discrete ramp steps",
        depends_on = "scarf_joint.enabled"
    )]
    pub scarf_steps: u32,
    /// Extrusion flow ratio during scarf (1.0 = normal).
    #[setting(
        tier = 3,
        description = "Scarf extrusion flow ratio",
        min = 0.1,
        max = 2.0,
        depends_on = "scarf_joint.enabled"
    )]
    pub scarf_flow_ratio: f64,
    /// Apply scarf to inner walls (not just outer).
    #[setting(
        tier = 3,
        description = "Apply scarf to inner walls",
        depends_on = "scarf_joint.enabled"
    )]
    pub scarf_inner_walls: bool,
    /// Use role-based wipe speed at seam.
    #[setting(
        tier = 4,
        description = "Use role-based speed for wipe at seam",
        depends_on = "scarf_joint.enabled"
    )]
    pub role_based_wipe_speed: bool,
    /// Wipe speed at seam end (mm/s).
    #[setting(
        tier = 3,
        description = "Wipe speed at seam end",
        units = "mm/s",
        min = 0.0,
        max = 1000.0,
        depends_on = "scarf_joint.enabled"
    )]
    pub wipe_speed: f64,
    /// Enable inward wipe at seam close.
    #[setting(
        tier = 3,
        description = "Enable inward wipe at seam close",
        depends_on = "scarf_joint.enabled"
    )]
    pub wipe_on_loop: bool,
    /// Gap between scarf ramp end and next layer start in mm.
    /// OrcaSlicer: `seam_slope_gap`.
    #[serde(default)]
    #[setting(
        tier = 4,
        description = "Gap between scarf end and next layer",
        units = "mm",
        depends_on = "scarf_joint.enabled"
    )]
    pub seam_gap: f64,
    /// Minimum angle (degrees) at which scarf joint activates.
    /// OrcaSlicer: `scarf_angle_threshold`.
    #[serde(default)]
    #[setting(
        tier = 4,
        description = "Minimum angle for scarf activation",
        units = "deg",
        depends_on = "scarf_joint.enabled"
    )]
    pub scarf_angle_threshold: f64,
    /// Overhang percentage threshold at which scarf joint is disabled.
    /// OrcaSlicer: `scarf_overhang_threshold`.
    #[serde(default)]
    #[setting(
        tier = 4,
        description = "Overhang threshold to disable scarf",
        depends_on = "scarf_joint.enabled"
    )]
    pub scarf_overhang_threshold: f64,
    /// Override filament-level scarf seam settings with process-level.
    /// OrcaSlicer: `override_filament_scarf_seam_setting`.
    #[serde(default)]
    #[setting(tier = 4, description = "Override filament-level scarf settings")]
    pub override_filament_setting: bool,
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
            seam_gap: 0.0,
            scarf_angle_threshold: 0.0,
            scarf_overhang_threshold: 0.0,
            override_filament_setting: false,
        }
    }
}

/// Slicing tolerance mode for layer boundary positioning.
///
/// Controls how the slicer determines the Z height within a layer for
/// contour intersection. Affects dimensional accuracy of angled surfaces.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, SettingSchema)]
#[serde(rename_all = "snake_case")]
pub enum SlicingTolerance {
    /// Slice at the midpoint of each layer (default, balanced accuracy).
    #[default]
    #[setting(
        display = "Middle",
        description = "Slice at layer midpoint for balanced accuracy"
    )]
    Middle,
    /// Slice at the nearest point to the original mesh surface.
    #[setting(
        display = "Nearest",
        description = "Slice at nearest point to mesh surface"
    )]
    Nearest,
    /// Use Gaussian averaging for smoother contour transitions.
    #[setting(
        display = "Gauss",
        description = "Gaussian averaging for smoother contour transitions"
    )]
    Gauss,
}

/// Controls which perimeter types receive scarf joint treatment.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, SettingSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScarfJointType {
    /// Apply scarf to contours (outer boundaries) only.
    #[default]
    #[setting(
        display = "Contour",
        description = "Apply scarf joint to outer contours only"
    )]
    Contour,
    /// Apply scarf to both contours and holes.
    #[setting(
        display = "Contour And Hole",
        description = "Apply scarf joint to both contours and holes"
    )]
    ContourAndHole,
}

impl Default for PrintConfig {
    fn default() -> Self {
        Self {
            layer_height: 0.2,
            first_layer_height: 0.3,

            wall_count: 2,
            wall_order: WallOrder::default(),
            seam_position: SeamPosition::default(),

            infill_pattern: InfillPattern::default(),
            infill_density: 0.2,
            top_solid_layers: 3,
            bottom_solid_layers: 3,

            skirt_loops: 1,
            skirt_distance: 6.0,
            brim_width: 0.0,

            extrusion_multiplier: 1.0,

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

            pressure_advance: 0.0,
            acceleration_enabled: false,

            multi_material: MultiMaterialConfig::default(),
            sequential: SequentialConfig::default(),
            travel_opt: TravelOptConfig::default(),
            plugin_dir: None,

            line_widths: LineWidthConfig::default(),
            speeds: SpeedConfig::default(),
            cooling: CoolingConfig::default(),
            retraction: RetractionConfig::default(),
            machine: MachineConfig::default(),
            accel: AccelerationConfig::default(),
            filament: FilamentPropsConfig::default(),
            passthrough: BTreeMap::new(),

            dimensional_compensation: DimensionalCompensationConfig::default(),
            top_surface_pattern: SurfacePattern::default(),
            bottom_surface_pattern: SurfacePattern::default(),
            solid_infill_pattern: SurfacePattern::default(),
            extra_perimeters_on_overhangs: false,
            internal_bridge_support: InternalBridgeMode::default(),
            z_offset: 0.0,
            precise_z_height: false,

            bridge_flow: 1.0,
            infill_direction: 45.0,
            infill_wall_overlap: 0.15,
            spiral_mode: false,
            only_one_wall_top: false,
            resolution: 0.012,
            raft_layers: 0,
            detect_thin_wall: true,

            parallel_slicing: true,
            thread_count: None,

            thumbnail_resolution: default_thumbnail_resolution(),

            post_process: PostProcessConfig::default(),

            fuzzy_skin: FuzzySkinConfig::default(),
            brim_skirt: BrimSkirtConfig::default(),
            input_shaping: InputShapingConfig::default(),
            precise_outer_wall: false,
            draft_shield: false,
            ooze_prevention: false,
            infill_combination: 0,
            infill_anchor_max: 12.0,
            min_bead_width: 0.315,
            min_feature_size: 0.25,

            slicing_tolerance: SlicingTolerance::default(),
            thumbnails: Vec::new(),
            compatible_printers_condition: Vec::new(),
            inherits_group: String::new(),
            max_travel_detour_length: 0.0,
            exclude_object: false,
            reduce_infill_retraction: false,
            reduce_crossing_wall: false,
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
    /// Uses `self.machine.nozzle_diameter()` (primary extruder diameter).
    pub fn extrusion_width(&self) -> f64 {
        self.machine.nozzle_diameter() * 1.1
    }
}

/// Per-tool configuration for multi-material printing.
///
/// Each tool (extruder) can have independent temperature and retraction
/// settings for optimal tool-change sequences.
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Machine")]
pub struct ToolConfig {
    /// Nozzle temperature for this tool in degrees Celsius.
    #[setting(tier = 4, description = "Per-tool nozzle temperature", units = "deg_c")]
    pub nozzle_temp: f64,
    /// Retraction length for this tool in mm.
    #[setting(tier = 4, description = "Per-tool retraction length", units = "mm")]
    pub retract_length: f64,
    /// Retraction speed for this tool in mm/s.
    #[setting(tier = 4, description = "Per-tool retraction speed", units = "mm/s")]
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
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "MultiMaterial")]
pub struct MultiMaterialConfig {
    /// Enable multi-material printing.
    #[setting(tier = 3, description = "Enable multi-material printing")]
    pub enabled: bool,
    /// Number of tools (extruders) available.
    #[setting(
        tier = 3,
        description = "Number of tools/extruders available",
        depends_on = "multi_material.enabled"
    )]
    pub tool_count: u8,
    /// Per-tool configuration.
    #[setting(skip)]
    pub tools: Vec<ToolConfig>,
    /// Purge tower position [x, y] in mm.
    #[setting(
        tier = 3,
        description = "Purge tower XY position",
        units = "mm",
        depends_on = "multi_material.enabled"
    )]
    pub purge_tower_position: [f64; 2],
    /// Purge tower width in mm.
    #[setting(
        tier = 3,
        description = "Purge tower width",
        units = "mm",
        depends_on = "multi_material.enabled"
    )]
    pub purge_tower_width: f64,
    /// Purge volume per tool change in mm^3.
    #[setting(
        tier = 3,
        description = "Purge volume per tool change",
        units = "mm^3",
        depends_on = "multi_material.enabled"
    )]
    pub purge_volume: f64,
    /// Wipe length across the purge tower in mm.
    #[setting(
        tier = 3,
        description = "Wipe length across purge tower",
        units = "mm",
        depends_on = "multi_material.enabled"
    )]
    pub wipe_length: f64,
    /// Filament/tool index for wall perimeters (0-based, None = use default).
    /// OrcaSlicer: `wall_filament` (1-based). Import mapper translates to 0-based.
    #[setting(
        tier = 3,
        description = "Filament index for wall perimeters",
        depends_on = "multi_material.enabled"
    )]
    pub wall_filament: Option<usize>,
    /// Filament/tool index for solid infill (0-based, None = use default).
    /// OrcaSlicer: `solid_infill_filament` (1-based).
    #[setting(
        tier = 3,
        description = "Filament index for solid infill",
        depends_on = "multi_material.enabled"
    )]
    pub solid_infill_filament: Option<usize>,
    /// Filament/tool index for support structures (0-based, None = use default).
    /// OrcaSlicer: `support_filament` (1-based).
    #[setting(
        tier = 3,
        description = "Filament index for support structures",
        depends_on = "multi_material.enabled"
    )]
    pub support_filament: Option<usize>,
    /// Filament/tool index for support interface layers (0-based, None = use default).
    /// OrcaSlicer: `support_interface_filament` (1-based).
    #[setting(
        tier = 3,
        description = "Filament index for support interface layers",
        depends_on = "multi_material.enabled"
    )]
    pub support_interface_filament: Option<usize>,
    /// Tool change retraction configuration.
    #[setting(flatten)]
    pub tool_change_retraction: ToolChangeRetractionConfig,
    /// Rotation angle for the purge tower in degrees.
    /// OrcaSlicer/PrusaSlicer: `wipe_tower_rotation_angle`.
    #[serde(default)]
    #[setting(
        tier = 4,
        description = "Purge tower rotation angle",
        units = "deg",
        depends_on = "multi_material.enabled"
    )]
    pub wipe_tower_rotation_angle: f64,
    /// Purge tower bridging flow rate.
    /// PrusaSlicer: `wipe_tower_bridging`.
    #[serde(default)]
    #[setting(
        tier = 4,
        description = "Purge tower bridging flow rate",
        depends_on = "multi_material.enabled"
    )]
    pub wipe_tower_bridging: f64,
    /// Cone angle for tapered purge tower (degrees).
    /// PrusaSlicer: `wipe_tower_cone_angle`.
    #[serde(default)]
    #[setting(
        tier = 4,
        description = "Tapered purge tower cone angle",
        units = "deg",
        depends_on = "multi_material.enabled"
    )]
    pub wipe_tower_cone_angle: f64,
    /// Skip sparse (empty) purge tower layers.
    /// PrusaSlicer: `wipe_tower_no_sparse_layers`.
    #[serde(default)]
    #[setting(
        tier = 4,
        description = "Skip empty purge tower layers",
        depends_on = "multi_material.enabled"
    )]
    pub wipe_tower_no_sparse_layers: bool,
    /// Single extruder multi-material mode (e.g., MMU2).
    /// Shared: `single_extruder_multi_material`.
    #[serde(default)]
    #[setting(
        tier = 4,
        description = "Single extruder multi-material mode",
        depends_on = "multi_material.enabled"
    )]
    pub single_extruder_mmu: bool,
    /// Flush/purge excess filament into infill regions.
    /// OrcaSlicer: `flush_into_infill`.
    #[serde(default)]
    #[setting(
        tier = 3,
        description = "Flush excess filament into infill regions",
        depends_on = "multi_material.enabled"
    )]
    pub flush_into_infill: bool,
    /// Flush/purge excess filament into printed objects.
    /// OrcaSlicer: `flush_into_objects`.
    #[serde(default)]
    #[setting(
        tier = 3,
        description = "Flush excess filament into printed objects",
        depends_on = "multi_material.enabled"
    )]
    pub flush_into_objects: bool,
    /// Flush/purge excess filament into support structures.
    /// OrcaSlicer: `flush_into_support`.
    #[serde(default)]
    #[setting(
        tier = 3,
        description = "Flush excess filament into support structures",
        depends_on = "multi_material.enabled"
    )]
    pub flush_into_support: bool,
    /// Use the prime tower for purging.
    /// OrcaSlicer: `purge_in_prime_tower`.
    #[serde(default = "default_true")]
    #[setting(
        tier = 3,
        description = "Use prime tower for purging",
        depends_on = "multi_material.enabled"
    )]
    pub purge_in_prime_tower: bool,
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
            wall_filament: None,
            solid_infill_filament: None,
            support_filament: None,
            support_interface_filament: None,
            tool_change_retraction: ToolChangeRetractionConfig::default(),
            wipe_tower_rotation_angle: 0.0,
            wipe_tower_bridging: 10.0,
            wipe_tower_cone_angle: 0.0,
            wipe_tower_no_sparse_layers: false,
            single_extruder_mmu: false,
            flush_into_infill: false,
            flush_into_objects: false,
            flush_into_support: false,
            purge_in_prime_tower: true,
        }
    }
}

/// Sequential (object-by-object) printing configuration.
///
/// In sequential mode, each object is printed completely before moving to
/// the next. This requires collision detection to ensure the extruder
/// clearance envelope does not hit previously printed objects.
///
/// # Gantry clearance models
///
/// The clearance zone around the nozzle is determined by one of three models,
/// checked in priority order:
///
/// 1. **Custom polygon** -- If [`extruder_clearance_polygon`](Self::extruder_clearance_polygon)
///    is non-empty, it defines an arbitrary polygon (points in mm relative to
///    the nozzle center) used as the clearance zone.
/// 2. **Rectangle** -- If [`gantry_width`](Self::gantry_width) > 0, a rectangular
///    clearance zone of `gantry_width x gantry_depth` is used.
/// 3. **Cylinder** -- Otherwise, a circular clearance zone with
///    [`extruder_clearance_radius`](Self::extruder_clearance_radius) is used.
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Advanced")]
pub struct SequentialConfig {
    /// Enable sequential (object-by-object) printing.
    #[setting(tier = 3, description = "Enable sequential object-by-object printing")]
    pub enabled: bool,
    /// Extruder clearance radius in mm (XY distance from nozzle to widest
    /// part of the print head assembly). Used for the cylinder gantry model.
    #[setting(
        tier = 3,
        description = "Extruder clearance radius for collision avoidance",
        units = "mm",
        depends_on = "sequential.enabled"
    )]
    pub extruder_clearance_radius: f64,
    /// Extruder clearance height in mm (height above nozzle tip to the
    /// bottom of the X carriage / gantry).
    #[setting(
        tier = 3,
        description = "Extruder clearance height for collision avoidance",
        units = "mm",
        depends_on = "sequential.enabled"
    )]
    pub extruder_clearance_height: f64,
    /// Width of gantry/carriage in mm (X direction).
    ///
    /// A value of 0.0 means the rectangular gantry model is not used and
    /// the cylinder model with [`extruder_clearance_radius`](Self::extruder_clearance_radius)
    /// is used instead.
    #[setting(
        tier = 4,
        description = "Gantry width for rectangular clearance model",
        units = "mm",
        depends_on = "sequential.enabled"
    )]
    pub gantry_width: f64,
    /// Depth of gantry/carriage in mm (Y direction).
    ///
    /// Only used when [`gantry_width`](Self::gantry_width) > 0 (rectangular model).
    #[setting(
        tier = 4,
        description = "Gantry depth for rectangular clearance model",
        units = "mm",
        depends_on = "sequential.enabled"
    )]
    pub gantry_depth: f64,
    /// Custom polygon for the extruder clearance zone.
    ///
    /// Points are in mm relative to the nozzle center. When non-empty, this
    /// takes priority over both the rectangular and cylinder models.
    #[setting(skip)]
    pub extruder_clearance_polygon: Vec<(f64, f64)>,
}

impl Default for SequentialConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            extruder_clearance_radius: 35.0,
            extruder_clearance_height: 40.0,
            gantry_width: 0.0,
            gantry_depth: 0.0,
            extruder_clearance_polygon: Vec::new(),
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
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Calibration")]
pub struct PaCalibrationConfig {
    /// Starting PA value.
    #[setting(tier = 4, description = "Pressure advance start value")]
    pub pa_start: f64,
    /// Ending PA value.
    #[setting(tier = 4, description = "Pressure advance end value")]
    pub pa_end: f64,
    /// PA increment per line.
    #[setting(tier = 4, description = "Pressure advance step increment")]
    pub pa_step: f64,
    /// Slow extrusion speed in mm/s (reveals PA artifacts at transitions).
    #[setting(
        tier = 4,
        description = "Calibration slow extrusion speed",
        units = "mm/s"
    )]
    pub slow_speed: f64,
    /// Fast extrusion speed in mm/s (reveals PA artifacts at transitions).
    #[setting(
        tier = 4,
        description = "Calibration fast extrusion speed",
        units = "mm/s"
    )]
    pub fast_speed: f64,
    /// Extrusion line width in mm.
    #[setting(tier = 4, description = "Calibration line width", units = "mm")]
    pub line_width: f64,
    /// Layer height in mm.
    #[setting(tier = 4, description = "Calibration layer height", units = "mm")]
    pub layer_height: f64,
    /// Bed center X coordinate in mm.
    #[setting(tier = 4, description = "Calibration bed center X", units = "mm")]
    pub bed_center_x: f64,
    /// Bed center Y coordinate in mm.
    #[setting(tier = 4, description = "Calibration bed center Y", units = "mm")]
    pub bed_center_y: f64,
    /// Total pattern width in mm.
    #[setting(tier = 4, description = "Calibration pattern width", units = "mm")]
    pub pattern_width: f64,
    /// Nozzle temperature in degrees Celsius.
    #[setting(
        tier = 4,
        description = "Calibration nozzle temperature",
        units = "deg_c"
    )]
    pub nozzle_temp: f64,
    /// Bed temperature in degrees Celsius.
    #[setting(tier = 4, description = "Calibration bed temperature", units = "deg_c")]
    pub bed_temp: f64,
    /// Filament diameter in mm.
    #[setting(tier = 4, description = "Calibration filament diameter", units = "mm")]
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
            config.speeds.perimeter = v;
        }
        if let Some(v) = self.infill_speed {
            config.speeds.infill = v;
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
        assert!((config.machine.nozzle_diameter() - 0.4).abs() < 1e-9);
        assert_eq!(config.wall_count, 2);
        assert_eq!(config.wall_order, WallOrder::OuterFirst);
        assert!((config.infill_density - 0.2).abs() < 1e-9);
        assert_eq!(config.top_solid_layers, 3);
        assert_eq!(config.bottom_solid_layers, 3);
        assert!((config.speeds.perimeter - 45.0).abs() < 1e-9);
        assert!((config.speeds.infill - 80.0).abs() < 1e-9);
        assert!((config.speeds.travel - 150.0).abs() < 1e-9);
        assert!((config.speeds.first_layer - 20.0).abs() < 1e-9);
        assert!((config.retraction.length - 0.8).abs() < 1e-9);
        assert!((config.retraction.speed - 45.0).abs() < 1e-9);
        assert!((config.retraction.z_hop - 0.0).abs() < 1e-9);
        assert!((config.retraction.min_travel - 1.5).abs() < 1e-9);
        assert!((config.filament.nozzle_temp() - 200.0).abs() < 1e-9);
        assert!((config.filament.bed_temp() - 60.0).abs() < 1e-9);
        assert!((config.filament.first_layer_nozzle_temp() - 210.0).abs() < 1e-9);
        assert!((config.filament.first_layer_bed_temp() - 65.0).abs() < 1e-9);
        assert_eq!(config.cooling.fan_speed, 255);
        assert!((config.cooling.fan_below_layer_time - 60.0).abs() < 1e-9);
        assert_eq!(config.cooling.disable_fan_first_layers, 1);
        assert_eq!(config.skirt_loops, 1);
        assert!((config.skirt_distance - 6.0).abs() < 1e-9);
        assert!((config.brim_width - 0.0).abs() < 1e-9);
        assert!((config.machine.bed_x - 220.0).abs() < 1e-9);
        assert!((config.machine.bed_y - 220.0).abs() < 1e-9);
        assert!((config.extrusion_multiplier - 1.0).abs() < 1e-9);
        assert!((config.filament.diameter - 1.75).abs() < 1e-9);
    }

    #[test]
    fn from_toml_empty_produces_defaults() {
        let config = PrintConfig::from_toml("").unwrap();
        assert!((config.layer_height - 0.2).abs() < 1e-9);
        assert!((config.machine.nozzle_diameter() - 0.4).abs() < 1e-9);
        assert_eq!(config.wall_order, WallOrder::OuterFirst);
    }

    #[test]
    fn from_toml_partial_overrides() {
        let toml = "layer_height = 0.1\ninfill_density = 0.5";
        let config = PrintConfig::from_toml(toml).unwrap();
        assert!((config.layer_height - 0.1).abs() < 1e-9);
        assert!((config.infill_density - 0.5).abs() < 1e-9);
        assert!((config.machine.nozzle_diameter() - 0.4).abs() < 1e-9);
        assert_eq!(config.wall_count, 2);
        assert!((config.speeds.perimeter - 45.0).abs() < 1e-9);
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
        config.machine.nozzle_diameters = vec![0.6];
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
        assert_eq!(
            config.custom_gcode.after_layer_change,
            "M117 Layer {layer_num}"
        );
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
        assert!((merged.speeds.perimeter - 30.0).abs() < 1e-9);
        // Non-overridden fields preserved.
        assert!((merged.speeds.infill - base.speeds.infill).abs() < 1e-9);
        assert_eq!(merged.top_solid_layers, base.top_solid_layers);
        assert_eq!(merged.bottom_solid_layers, base.bottom_solid_layers);
        assert!((merged.layer_height - base.layer_height).abs() < 1e-9);
    }

    #[test]
    fn setting_overrides_merge_preserves_non_overridden() {
        let mut base = PrintConfig::default();
        base.infill_density = 0.3;
        base.wall_count = 3;
        base.speeds.perimeter = 50.0;
        let overrides = SettingOverrides::default(); // all None
        let merged = overrides.merge_into(&base);
        assert!((merged.infill_density - 0.3).abs() < 1e-9);
        assert_eq!(merged.wall_count, 3);
        assert!((merged.speeds.perimeter - 50.0).abs() < 1e-9);
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
            (config.filament.density - 1.24).abs() < 1e-9,
            "filament.density should default to 1.24 (PLA)"
        );
        assert!(
            (config.filament.cost_per_kg - 25.0).abs() < 1e-9,
            "filament.cost_per_kg should default to 25.0"
        );
    }

    #[test]
    fn filament_density_and_cost_from_toml() {
        let toml = r#"
[filament]
density = 1.04
cost_per_kg = 30.0
"#;
        let config = PrintConfig::from_toml(toml).unwrap();
        assert!(
            (config.filament.density - 1.04).abs() < 1e-9,
            "filament.density should parse from TOML"
        );
        assert!(
            (config.filament.cost_per_kg - 30.0).abs() < 1e-9,
            "filament.cost_per_kg should parse from TOML"
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

        // SpeedConfig (including migrated fields)
        assert!((config.speeds.perimeter - 45.0).abs() < 1e-9);
        assert!((config.speeds.infill - 80.0).abs() < 1e-9);
        assert!((config.speeds.travel - 150.0).abs() < 1e-9);
        assert!((config.speeds.first_layer - 20.0).abs() < 1e-9);
        assert!((config.speeds.bridge - 25.0).abs() < 1e-9);
        assert!((config.speeds.inner_wall - 0.0).abs() < 1e-9);
        assert!((config.speeds.gap_fill - 0.0).abs() < 1e-9);
        assert!((config.speeds.top_surface - 0.0).abs() < 1e-9);
        assert!((config.speeds.overhang_1_4 - 0.0).abs() < 1e-9);
        assert!((config.speeds.travel_z - 0.0).abs() < 1e-9);

        // CoolingConfig (including migrated fields)
        assert_eq!(config.cooling.fan_speed, 255);
        assert!((config.cooling.fan_below_layer_time - 60.0).abs() < 1e-9);
        assert_eq!(config.cooling.disable_fan_first_layers, 1);
        assert!((config.cooling.fan_max_speed - 100.0).abs() < 1e-9);
        assert!((config.cooling.fan_min_speed - 35.0).abs() < 1e-9);
        assert!((config.cooling.slow_down_layer_time - 5.0).abs() < 1e-9);
        assert!((config.cooling.slow_down_min_speed - 10.0).abs() < 1e-9);
        assert!((config.cooling.overhang_fan_speed - 100.0).abs() < 1e-9);
        assert!((config.cooling.overhang_fan_threshold - 25.0).abs() < 1e-9);
        assert_eq!(config.cooling.full_fan_speed_layer, 0);
        assert!(config.cooling.slow_down_for_layer_cooling);

        // RetractionConfig (including migrated fields)
        assert!((config.retraction.length - 0.8).abs() < 1e-9);
        assert!((config.retraction.speed - 45.0).abs() < 1e-9);
        assert!((config.retraction.z_hop - 0.0).abs() < 1e-9);
        assert!((config.retraction.min_travel - 1.5).abs() < 1e-9);
        assert!((config.retraction.deretraction_speed - 0.0).abs() < 1e-9);
        assert!((config.retraction.retract_before_wipe - 0.0).abs() < 1e-9);
        assert!(!config.retraction.retract_when_changing_layer);
        assert!(!config.retraction.wipe);
        assert!((config.retraction.wipe_distance - 0.0).abs() < 1e-9);

        // MachineConfig (including migrated fields)
        assert!((config.machine.bed_x - 220.0).abs() < 1e-9);
        assert!((config.machine.bed_y - 220.0).abs() < 1e-9);
        assert!((config.machine.printable_height - 250.0).abs() < 1e-9);
        assert!((config.machine.max_acceleration_x - 5000.0).abs() < 1e-9);
        assert!((config.machine.max_speed_z - 12.0).abs() < 1e-9);
        assert!((config.machine.min_layer_height - 0.07).abs() < 1e-9);
        assert!((config.machine.max_layer_height - 0.0).abs() < 1e-9);
        assert!(config.machine.start_gcode.is_empty());
        assert!(config.machine.printer_model.is_empty());

        // AccelerationConfig (including migrated fields)
        assert!((config.accel.print - 1000.0).abs() < 1e-9);
        assert!((config.accel.travel - 1500.0).abs() < 1e-9);
        assert!((config.accel.outer_wall - 0.0).abs() < 1e-9);
        assert!((config.accel.inner_wall - 0.0).abs() < 1e-9);
        assert!((config.accel.bridge - 0.0).abs() < 1e-9);

        // FilamentPropsConfig (including migrated fields)
        assert!((config.filament.diameter - 1.75).abs() < 1e-9);
        assert!((config.filament.density - 1.24).abs() < 1e-9);
        assert!((config.filament.cost_per_kg - 25.0).abs() < 1e-9);
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
        assert!((config.dimensional_compensation.elephant_foot_compensation - 0.0).abs() < 1e-9);
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

    #[test]
    fn thumbnail_resolution_defaults_to_300x300() {
        let config = PrintConfig::default();
        assert_eq!(config.thumbnail_resolution, [300, 300]);
    }

    #[test]
    fn thumbnail_resolution_toml_roundtrip() {
        let toml_str = "thumbnail_resolution = [220, 124]";
        let config: PrintConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.thumbnail_resolution, [220, 124]);

        let serialized = toml::to_string(&config).unwrap();
        let restored: PrintConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(restored.thumbnail_resolution, [220, 124]);
    }

    #[test]
    fn travel_opt_config_defaults() {
        let config = TravelOptConfig::default();
        assert!(config.enabled);
        assert_eq!(config.algorithm, TravelOptAlgorithm::Auto);
        assert_eq!(config.max_iterations, 100);
        assert!(config.optimize_cross_object);
        assert_eq!(config.print_order, PrintOrder::ByLayer);
    }

    #[test]
    fn travel_opt_algorithm_is_non_exhaustive() {
        // Ensure all 5 variants exist and are distinct
        let variants = [
            TravelOptAlgorithm::Auto,
            TravelOptAlgorithm::NearestNeighbor,
            TravelOptAlgorithm::GreedyEdgeInsertion,
            TravelOptAlgorithm::NearestNeighborOnly,
            TravelOptAlgorithm::GreedyOnly,
        ];
        for (i, a) in variants.iter().enumerate() {
            for (j, b) in variants.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn travel_opt_algorithm_serde_snake_case() {
        let json = serde_json::to_string(&TravelOptAlgorithm::NearestNeighbor).unwrap();
        assert_eq!(json, "\"nearest_neighbor\"");
        let json = serde_json::to_string(&TravelOptAlgorithm::GreedyEdgeInsertion).unwrap();
        assert_eq!(json, "\"greedy_edge_insertion\"");
        let json = serde_json::to_string(&TravelOptAlgorithm::Auto).unwrap();
        assert_eq!(json, "\"auto\"");
    }

    #[test]
    fn travel_opt_config_toml_roundtrip() {
        let config = TravelOptConfig::default();
        let serialized = toml::to_string(&config).unwrap();
        let restored: TravelOptConfig = toml::from_str(&serialized).unwrap();
        assert!(restored.enabled);
        assert_eq!(restored.algorithm, TravelOptAlgorithm::Auto);
        assert_eq!(restored.max_iterations, 100);
        assert!(restored.optimize_cross_object);
        assert_eq!(restored.print_order, PrintOrder::ByLayer);
    }

    #[test]
    fn print_config_has_travel_opt_field() {
        let config = PrintConfig::default();
        assert!(config.travel_opt.enabled);
        assert_eq!(config.travel_opt.algorithm, TravelOptAlgorithm::Auto);
    }

    #[test]
    fn print_order_default_is_by_layer() {
        assert_eq!(PrintOrder::default(), PrintOrder::ByLayer);
    }
}
