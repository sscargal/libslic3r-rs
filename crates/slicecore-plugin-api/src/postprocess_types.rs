//! FFI-safe types for G-code post-processing plugins.
//!
//! These types cross the plugin boundary and use [`abi_stable`] FFI-safe
//! wrappers (`RVec`, `RString`, `ROption`) instead of standard library types.
//! All types derive [`StableAbi`] for load-time layout verification.

use abi_stable::std_types::{ROption, RString, RVec};
use abi_stable::StableAbi;

/// FFI-safe representation of a G-code command.
///
/// Mirrors [`slicecore_gcode_io::GcodeCommand`] but uses FFI-safe types
/// (`ROption`, `RString`) for safe transport across the plugin boundary.
///
/// # Variants
///
/// All 23 variants from `GcodeCommand` are represented, plus `RawGcode`
/// for arbitrary printer-specific codes.
#[repr(u8)]
#[derive(StableAbi, Clone, Debug)]
pub enum FfiGcodeCommand {
    /// A comment line.
    Comment(RString),

    /// Linear move (G1).
    LinearMove {
        /// X position.
        x: ROption<f64>,
        /// Y position.
        y: ROption<f64>,
        /// Z position.
        z: ROption<f64>,
        /// Extrusion amount.
        e: ROption<f64>,
        /// Feedrate.
        f: ROption<f64>,
    },

    /// Rapid move / travel (G0).
    RapidMove {
        /// X position.
        x: ROption<f64>,
        /// Y position.
        y: ROption<f64>,
        /// Z position.
        z: ROption<f64>,
        /// Feedrate.
        f: ROption<f64>,
    },

    /// Home axes (G28).
    Home {
        /// Home X axis.
        x: bool,
        /// Home Y axis.
        y: bool,
        /// Home Z axis.
        z: bool,
    },

    /// Set absolute positioning (G90).
    SetAbsolutePositioning,

    /// Set relative positioning (G91).
    SetRelativePositioning,

    /// Set absolute extrusion (M82).
    SetAbsoluteExtrusion,

    /// Set relative extrusion (M83).
    SetRelativeExtrusion,

    /// Set extruder temperature (M104/M109).
    SetExtruderTemp {
        /// Target temperature in degrees Celsius.
        temp: f64,
        /// Whether to wait for the temperature to be reached.
        wait: bool,
    },

    /// Set bed temperature (M140/M190).
    SetBedTemp {
        /// Target temperature in degrees Celsius.
        temp: f64,
        /// Whether to wait for the temperature to be reached.
        wait: bool,
    },

    /// Set fan speed (M106).
    SetFanSpeed(u8),

    /// Fan off (M107).
    FanOff,

    /// Reset extruder position (G92 E0).
    ResetExtruder,

    /// Dwell / pause (G4).
    Dwell {
        /// Duration in milliseconds.
        ms: u32,
    },

    /// Retract filament.
    Retract {
        /// Retraction distance in mm.
        distance: f64,
        /// Retraction feedrate in mm/min.
        feedrate: f64,
    },

    /// Unretract filament.
    Unretract {
        /// Unretraction distance in mm.
        distance: f64,
        /// Unretraction feedrate in mm/min.
        feedrate: f64,
    },

    /// Clockwise arc move (G2).
    ArcMoveCW {
        /// X endpoint.
        x: ROption<f64>,
        /// Y endpoint.
        y: ROption<f64>,
        /// I center offset.
        i: f64,
        /// J center offset.
        j: f64,
        /// Extrusion amount.
        e: ROption<f64>,
        /// Feedrate.
        f: ROption<f64>,
    },

    /// Counter-clockwise arc move (G3).
    ArcMoveCCW {
        /// X endpoint.
        x: ROption<f64>,
        /// Y endpoint.
        y: ROption<f64>,
        /// I center offset.
        i: f64,
        /// J center offset.
        j: f64,
        /// Extrusion amount.
        e: ROption<f64>,
        /// Feedrate.
        f: ROption<f64>,
    },

    /// Set acceleration (M204).
    SetAcceleration {
        /// Print acceleration in mm/s^2.
        print_accel: f64,
        /// Travel acceleration in mm/s^2.
        travel_accel: f64,
    },

    /// Set jerk (M205).
    SetJerk {
        /// X jerk in mm/s.
        x: f64,
        /// Y jerk in mm/s.
        y: f64,
        /// Z jerk in mm/s.
        z: f64,
    },

    /// Set pressure advance (M900).
    SetPressureAdvance {
        /// Pressure advance K-factor.
        value: f64,
    },

    /// Tool change (Tn).
    ToolChange(u8),

    /// Raw G-code pass-through from `GcodeCommand::Raw`.
    Raw(RString),

    /// Arbitrary printer-specific raw G-code.
    ///
    /// Unlike `Raw` (which maps to `GcodeCommand::Raw`), this variant
    /// is for plugin-generated arbitrary G-code that may not have a
    /// structured representation.
    RawGcode(RString),
}

/// FFI-safe snapshot of print configuration parameters.
///
/// Provides the most commonly needed print settings to post-processor
/// plugins so they can make informed decisions about G-code modifications.
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct FfiPrintConfigSnapshot {
    /// Nozzle diameter in mm.
    pub nozzle_diameter: f64,
    /// Layer height in mm.
    pub layer_height: f64,
    /// First layer height in mm.
    pub first_layer_height: f64,
    /// Bed X dimension in mm.
    pub bed_x: f64,
    /// Bed Y dimension in mm.
    pub bed_y: f64,
    /// Print speed in mm/s.
    pub print_speed: f64,
    /// Travel speed in mm/s.
    pub travel_speed: f64,
    /// Retract length in mm.
    pub retract_length: f64,
    /// Retract speed in mm/s.
    pub retract_speed: f64,
    /// Nozzle temperature in degrees Celsius.
    pub nozzle_temp: f64,
    /// Bed temperature in degrees Celsius.
    pub bed_temp: f64,
    /// Fan speed (0-255).
    pub fan_speed: u8,
    /// Total number of layers in the print.
    pub total_layers: u64,
}

/// Processing mode for post-processor plugins.
///
/// Determines whether a plugin processes the entire G-code at once,
/// layer-by-layer, or both.
#[repr(u8)]
#[derive(StableAbi, Clone, Debug, PartialEq, Eq)]
pub enum ProcessingMode {
    /// Process all G-code commands at once.
    All,
    /// Process commands layer-by-layer.
    PerLayer,
    /// Process both ways (all then per-layer, or vice versa).
    Both,
}

/// FFI-safe configuration parameter for plugins.
///
/// Plugins can receive typed configuration values from the host.
#[repr(u8)]
#[derive(StableAbi, Clone, Debug)]
pub enum FfiConfigParam {
    /// A string value.
    StringVal(RString),
    /// An integer value.
    IntVal(i64),
    /// A floating-point value.
    FloatVal(f64),
    /// A boolean value.
    BoolVal(bool),
    /// A list of string values.
    StringListVal(RVec<RString>),
}

/// FFI-safe post-processing request for the entire G-code.
///
/// Contains all G-code commands, print configuration, and plugin parameters.
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct PostProcessRequest {
    /// All G-code commands to process.
    pub commands: RVec<FfiGcodeCommand>,
    /// Print configuration snapshot.
    pub config: FfiPrintConfigSnapshot,
    /// Plugin-specific configuration parameters.
    pub params: RVec<FfiConfigParam>,
}

/// FFI-safe post-processing request for a single layer.
///
/// Contains the G-code commands for one layer along with the layer index.
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct LayerPostProcessRequest {
    /// G-code commands for this layer.
    pub commands: RVec<FfiGcodeCommand>,
    /// Zero-based layer index.
    pub layer_index: u64,
    /// Print configuration snapshot.
    pub config: FfiPrintConfigSnapshot,
    /// Plugin-specific configuration parameters.
    pub params: RVec<FfiConfigParam>,
}

/// FFI-safe result of post-processing.
///
/// Contains the modified G-code commands after processing.
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct PostProcessResult {
    /// Modified G-code commands.
    pub commands: RVec<FfiGcodeCommand>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use abi_stable::std_types::{RNone, RSome};

    #[test]
    fn ffi_gcode_command_all_variants_constructible() {
        // Verify every variant can be constructed (StableAbi layout check)
        let _comment = FfiGcodeCommand::Comment(RString::from("test"));
        let _linear = FfiGcodeCommand::LinearMove {
            x: RSome(1.0),
            y: RSome(2.0),
            z: RNone,
            e: RSome(0.5),
            f: RNone,
        };
        let _rapid = FfiGcodeCommand::RapidMove {
            x: RSome(10.0),
            y: RNone,
            z: RNone,
            f: RSome(6000.0),
        };
        let _home = FfiGcodeCommand::Home {
            x: true,
            y: true,
            z: false,
        };
        let _abs_pos = FfiGcodeCommand::SetAbsolutePositioning;
        let _rel_pos = FfiGcodeCommand::SetRelativePositioning;
        let _abs_ext = FfiGcodeCommand::SetAbsoluteExtrusion;
        let _rel_ext = FfiGcodeCommand::SetRelativeExtrusion;
        let _ext_temp = FfiGcodeCommand::SetExtruderTemp {
            temp: 200.0,
            wait: true,
        };
        let _bed_temp = FfiGcodeCommand::SetBedTemp {
            temp: 60.0,
            wait: false,
        };
        let _fan = FfiGcodeCommand::SetFanSpeed(255);
        let _fan_off = FfiGcodeCommand::FanOff;
        let _reset = FfiGcodeCommand::ResetExtruder;
        let _dwell = FfiGcodeCommand::Dwell { ms: 500 };
        let _retract = FfiGcodeCommand::Retract {
            distance: 0.8,
            feedrate: 2700.0,
        };
        let _unretract = FfiGcodeCommand::Unretract {
            distance: 0.8,
            feedrate: 2700.0,
        };
        let _arc_cw = FfiGcodeCommand::ArcMoveCW {
            x: RSome(10.0),
            y: RSome(20.0),
            i: 5.0,
            j: 0.0,
            e: RSome(0.5),
            f: RSome(1800.0),
        };
        let _arc_ccw = FfiGcodeCommand::ArcMoveCCW {
            x: RNone,
            y: RNone,
            i: 3.0,
            j: -2.5,
            e: RNone,
            f: RNone,
        };
        let _accel = FfiGcodeCommand::SetAcceleration {
            print_accel: 1000.0,
            travel_accel: 1500.0,
        };
        let _jerk = FfiGcodeCommand::SetJerk {
            x: 8.0,
            y: 8.0,
            z: 0.4,
        };
        let _pa = FfiGcodeCommand::SetPressureAdvance { value: 0.05 };
        let _tool = FfiGcodeCommand::ToolChange(1);
        let _raw = FfiGcodeCommand::Raw(RString::from("M84"));
        let _raw_gcode = FfiGcodeCommand::RawGcode(RString::from("G29"));
    }

    #[test]
    fn processing_mode_variants() {
        assert_eq!(ProcessingMode::All, ProcessingMode::All);
        assert_eq!(ProcessingMode::PerLayer, ProcessingMode::PerLayer);
        assert_eq!(ProcessingMode::Both, ProcessingMode::Both);
        assert_ne!(ProcessingMode::All, ProcessingMode::PerLayer);
    }

    #[test]
    fn ffi_config_param_variants() {
        let _str = FfiConfigParam::StringVal(RString::from("hello"));
        let _int = FfiConfigParam::IntVal(42);
        let _float = FfiConfigParam::FloatVal(3.14);
        let _bool = FfiConfigParam::BoolVal(true);
        let _list = FfiConfigParam::StringListVal(RVec::from(vec![
            RString::from("a"),
            RString::from("b"),
        ]));
    }

    #[test]
    fn post_process_request_construction() {
        let config = FfiPrintConfigSnapshot {
            nozzle_diameter: 0.4,
            layer_height: 0.2,
            first_layer_height: 0.3,
            bed_x: 220.0,
            bed_y: 220.0,
            print_speed: 60.0,
            travel_speed: 120.0,
            retract_length: 0.8,
            retract_speed: 45.0,
            nozzle_temp: 200.0,
            bed_temp: 60.0,
            fan_speed: 255,
            total_layers: 100,
        };
        let request = PostProcessRequest {
            commands: RVec::from(vec![FfiGcodeCommand::Comment(RString::from("start"))]),
            config,
            params: RVec::new(),
        };
        assert_eq!(request.commands.len(), 1);
    }

    #[test]
    fn layer_post_process_request_construction() {
        let config = FfiPrintConfigSnapshot {
            nozzle_diameter: 0.4,
            layer_height: 0.2,
            first_layer_height: 0.3,
            bed_x: 220.0,
            bed_y: 220.0,
            print_speed: 60.0,
            travel_speed: 120.0,
            retract_length: 0.8,
            retract_speed: 45.0,
            nozzle_temp: 200.0,
            bed_temp: 60.0,
            fan_speed: 255,
            total_layers: 100,
        };
        let request = LayerPostProcessRequest {
            commands: RVec::from(vec![FfiGcodeCommand::Comment(RString::from("layer 5"))]),
            layer_index: 5,
            config,
            params: RVec::new(),
        };
        assert_eq!(request.layer_index, 5);
    }

    #[test]
    fn post_process_result_construction() {
        let result = PostProcessResult {
            commands: RVec::from(vec![
                FfiGcodeCommand::Comment(RString::from("modified")),
                FfiGcodeCommand::SetFanSpeed(128),
            ]),
        };
        assert_eq!(result.commands.len(), 2);
    }
}
