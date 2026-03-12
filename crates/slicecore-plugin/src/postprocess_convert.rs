//! Bidirectional conversion between [`GcodeCommand`] and [`FfiGcodeCommand`].
//!
//! These conversions are lossless: round-tripping a `GcodeCommand` through
//! `gcode_to_ffi` and `ffi_to_gcode` produces an identical value.

use abi_stable::std_types::{ROption, RString, RVec};
use slicecore_gcode_io::GcodeCommand;
use slicecore_plugin_api::FfiGcodeCommand;

/// Converts an `Option<f64>` to an `ROption<f64>`.
fn option_to_roption(opt: Option<f64>) -> ROption<f64> {
    ROption::from(opt)
}

/// Converts an `ROption<f64>` to an `Option<f64>`.
fn roption_to_option(opt: ROption<f64>) -> Option<f64> {
    opt.into_option()
}

/// Converts a [`GcodeCommand`] to an [`FfiGcodeCommand`].
///
/// The conversion is lossless: all fields are preserved using FFI-safe
/// wrapper types (`ROption`, `RString`).
pub fn gcode_to_ffi(cmd: &GcodeCommand) -> FfiGcodeCommand {
    match cmd {
        GcodeCommand::Comment(text) => FfiGcodeCommand::Comment(RString::from(text.as_str())),
        GcodeCommand::LinearMove { x, y, z, e, f } => FfiGcodeCommand::LinearMove {
            x: option_to_roption(*x),
            y: option_to_roption(*y),
            z: option_to_roption(*z),
            e: option_to_roption(*e),
            f: option_to_roption(*f),
        },
        GcodeCommand::RapidMove { x, y, z, f } => FfiGcodeCommand::RapidMove {
            x: option_to_roption(*x),
            y: option_to_roption(*y),
            z: option_to_roption(*z),
            f: option_to_roption(*f),
        },
        GcodeCommand::Home { x, y, z } => FfiGcodeCommand::Home {
            x: *x,
            y: *y,
            z: *z,
        },
        GcodeCommand::SetAbsolutePositioning => FfiGcodeCommand::SetAbsolutePositioning,
        GcodeCommand::SetRelativePositioning => FfiGcodeCommand::SetRelativePositioning,
        GcodeCommand::SetAbsoluteExtrusion => FfiGcodeCommand::SetAbsoluteExtrusion,
        GcodeCommand::SetRelativeExtrusion => FfiGcodeCommand::SetRelativeExtrusion,
        GcodeCommand::SetExtruderTemp { temp, wait } => FfiGcodeCommand::SetExtruderTemp {
            temp: *temp,
            wait: *wait,
        },
        GcodeCommand::SetBedTemp { temp, wait } => FfiGcodeCommand::SetBedTemp {
            temp: *temp,
            wait: *wait,
        },
        GcodeCommand::SetFanSpeed(speed) => FfiGcodeCommand::SetFanSpeed(*speed),
        GcodeCommand::FanOff => FfiGcodeCommand::FanOff,
        GcodeCommand::ResetExtruder => FfiGcodeCommand::ResetExtruder,
        GcodeCommand::Dwell { ms } => FfiGcodeCommand::Dwell { ms: *ms },
        GcodeCommand::Retract { distance, feedrate } => FfiGcodeCommand::Retract {
            distance: *distance,
            feedrate: *feedrate,
        },
        GcodeCommand::Unretract { distance, feedrate } => FfiGcodeCommand::Unretract {
            distance: *distance,
            feedrate: *feedrate,
        },
        GcodeCommand::ArcMoveCW { x, y, i, j, e, f } => FfiGcodeCommand::ArcMoveCW {
            x: option_to_roption(*x),
            y: option_to_roption(*y),
            i: *i,
            j: *j,
            e: option_to_roption(*e),
            f: option_to_roption(*f),
        },
        GcodeCommand::ArcMoveCCW { x, y, i, j, e, f } => FfiGcodeCommand::ArcMoveCCW {
            x: option_to_roption(*x),
            y: option_to_roption(*y),
            i: *i,
            j: *j,
            e: option_to_roption(*e),
            f: option_to_roption(*f),
        },
        GcodeCommand::SetAcceleration {
            print_accel,
            travel_accel,
        } => FfiGcodeCommand::SetAcceleration {
            print_accel: *print_accel,
            travel_accel: *travel_accel,
        },
        GcodeCommand::SetJerk { x, y, z } => FfiGcodeCommand::SetJerk {
            x: *x,
            y: *y,
            z: *z,
        },
        GcodeCommand::SetPressureAdvance { value } => {
            FfiGcodeCommand::SetPressureAdvance { value: *value }
        }
        GcodeCommand::ToolChange(n) => FfiGcodeCommand::ToolChange(*n),
        GcodeCommand::Raw(line) => FfiGcodeCommand::Raw(RString::from(line.as_str())),
    }
}

/// Converts an [`FfiGcodeCommand`] back to a [`GcodeCommand`].
///
/// The conversion is lossless: all fields are restored from their FFI-safe
/// wrapper types. The `RawGcode` variant maps to `GcodeCommand::Raw`.
pub fn ffi_to_gcode(cmd: &FfiGcodeCommand) -> GcodeCommand {
    match cmd {
        FfiGcodeCommand::Comment(text) => GcodeCommand::Comment(text.to_string()),
        FfiGcodeCommand::LinearMove { x, y, z, e, f } => GcodeCommand::LinearMove {
            x: roption_to_option(*x),
            y: roption_to_option(*y),
            z: roption_to_option(*z),
            e: roption_to_option(*e),
            f: roption_to_option(*f),
        },
        FfiGcodeCommand::RapidMove { x, y, z, f } => GcodeCommand::RapidMove {
            x: roption_to_option(*x),
            y: roption_to_option(*y),
            z: roption_to_option(*z),
            f: roption_to_option(*f),
        },
        FfiGcodeCommand::Home { x, y, z } => GcodeCommand::Home {
            x: *x,
            y: *y,
            z: *z,
        },
        FfiGcodeCommand::SetAbsolutePositioning => GcodeCommand::SetAbsolutePositioning,
        FfiGcodeCommand::SetRelativePositioning => GcodeCommand::SetRelativePositioning,
        FfiGcodeCommand::SetAbsoluteExtrusion => GcodeCommand::SetAbsoluteExtrusion,
        FfiGcodeCommand::SetRelativeExtrusion => GcodeCommand::SetRelativeExtrusion,
        FfiGcodeCommand::SetExtruderTemp { temp, wait } => GcodeCommand::SetExtruderTemp {
            temp: *temp,
            wait: *wait,
        },
        FfiGcodeCommand::SetBedTemp { temp, wait } => GcodeCommand::SetBedTemp {
            temp: *temp,
            wait: *wait,
        },
        FfiGcodeCommand::SetFanSpeed(speed) => GcodeCommand::SetFanSpeed(*speed),
        FfiGcodeCommand::FanOff => GcodeCommand::FanOff,
        FfiGcodeCommand::ResetExtruder => GcodeCommand::ResetExtruder,
        FfiGcodeCommand::Dwell { ms } => GcodeCommand::Dwell { ms: *ms },
        FfiGcodeCommand::Retract { distance, feedrate } => GcodeCommand::Retract {
            distance: *distance,
            feedrate: *feedrate,
        },
        FfiGcodeCommand::Unretract { distance, feedrate } => GcodeCommand::Unretract {
            distance: *distance,
            feedrate: *feedrate,
        },
        FfiGcodeCommand::ArcMoveCW { x, y, i, j, e, f } => GcodeCommand::ArcMoveCW {
            x: roption_to_option(*x),
            y: roption_to_option(*y),
            i: *i,
            j: *j,
            e: roption_to_option(*e),
            f: roption_to_option(*f),
        },
        FfiGcodeCommand::ArcMoveCCW { x, y, i, j, e, f } => GcodeCommand::ArcMoveCCW {
            x: roption_to_option(*x),
            y: roption_to_option(*y),
            i: *i,
            j: *j,
            e: roption_to_option(*e),
            f: roption_to_option(*f),
        },
        FfiGcodeCommand::SetAcceleration {
            print_accel,
            travel_accel,
        } => GcodeCommand::SetAcceleration {
            print_accel: *print_accel,
            travel_accel: *travel_accel,
        },
        FfiGcodeCommand::SetJerk { x, y, z } => GcodeCommand::SetJerk {
            x: *x,
            y: *y,
            z: *z,
        },
        FfiGcodeCommand::SetPressureAdvance { value } => {
            GcodeCommand::SetPressureAdvance { value: *value }
        }
        FfiGcodeCommand::ToolChange(n) => GcodeCommand::ToolChange(*n),
        FfiGcodeCommand::Raw(line) => GcodeCommand::Raw(line.to_string()),
        FfiGcodeCommand::RawGcode(line) => GcodeCommand::Raw(line.to_string()),
    }
}

/// Converts a slice of [`GcodeCommand`] to an `RVec<FfiGcodeCommand>`.
pub fn commands_to_ffi(cmds: &[GcodeCommand]) -> RVec<FfiGcodeCommand> {
    cmds.iter().map(gcode_to_ffi).collect()
}

/// Converts an `RVec<FfiGcodeCommand>` to a `Vec<GcodeCommand>`.
pub fn commands_from_ffi(cmds: &RVec<FfiGcodeCommand>) -> Vec<GcodeCommand> {
    cmds.iter().map(ffi_to_gcode).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to assert a round-trip: gcode -> ffi -> gcode produces the same value.
    fn assert_roundtrip(cmd: GcodeCommand) {
        let ffi = gcode_to_ffi(&cmd);
        let back = ffi_to_gcode(&ffi);
        assert_eq!(cmd, back, "Round-trip failed for {cmd:?}");
    }

    #[test]
    fn roundtrip_comment() {
        assert_roundtrip(GcodeCommand::Comment("test comment".to_string()));
    }

    #[test]
    fn roundtrip_linear_move() {
        assert_roundtrip(GcodeCommand::LinearMove {
            x: Some(1.0),
            y: Some(2.0),
            z: None,
            e: Some(0.5),
            f: Some(1800.0),
        });
    }

    #[test]
    fn roundtrip_rapid_move() {
        assert_roundtrip(GcodeCommand::RapidMove {
            x: Some(10.0),
            y: None,
            z: Some(0.3),
            f: Some(6000.0),
        });
    }

    #[test]
    fn roundtrip_home() {
        assert_roundtrip(GcodeCommand::Home {
            x: true,
            y: false,
            z: true,
        });
    }

    #[test]
    fn roundtrip_positioning_modes() {
        assert_roundtrip(GcodeCommand::SetAbsolutePositioning);
        assert_roundtrip(GcodeCommand::SetRelativePositioning);
        assert_roundtrip(GcodeCommand::SetAbsoluteExtrusion);
        assert_roundtrip(GcodeCommand::SetRelativeExtrusion);
    }

    #[test]
    fn roundtrip_temperatures() {
        assert_roundtrip(GcodeCommand::SetExtruderTemp {
            temp: 200.0,
            wait: true,
        });
        assert_roundtrip(GcodeCommand::SetBedTemp {
            temp: 60.0,
            wait: false,
        });
    }

    #[test]
    fn roundtrip_fan() {
        assert_roundtrip(GcodeCommand::SetFanSpeed(255));
        assert_roundtrip(GcodeCommand::FanOff);
    }

    #[test]
    fn roundtrip_misc() {
        assert_roundtrip(GcodeCommand::ResetExtruder);
        assert_roundtrip(GcodeCommand::Dwell { ms: 500 });
    }

    #[test]
    fn roundtrip_retract_unretract() {
        assert_roundtrip(GcodeCommand::Retract {
            distance: 0.8,
            feedrate: 2700.0,
        });
        assert_roundtrip(GcodeCommand::Unretract {
            distance: 0.8,
            feedrate: 2700.0,
        });
    }

    #[test]
    fn roundtrip_arcs() {
        assert_roundtrip(GcodeCommand::ArcMoveCW {
            x: Some(10.0),
            y: Some(20.0),
            i: 5.0,
            j: 0.0,
            e: Some(0.5),
            f: Some(1800.0),
        });
        assert_roundtrip(GcodeCommand::ArcMoveCCW {
            x: None,
            y: None,
            i: 3.0,
            j: -2.5,
            e: None,
            f: None,
        });
    }

    #[test]
    fn roundtrip_acceleration_jerk_pa() {
        assert_roundtrip(GcodeCommand::SetAcceleration {
            print_accel: 1000.0,
            travel_accel: 1500.0,
        });
        assert_roundtrip(GcodeCommand::SetJerk {
            x: 8.0,
            y: 8.0,
            z: 0.4,
        });
        assert_roundtrip(GcodeCommand::SetPressureAdvance { value: 0.05 });
    }

    #[test]
    fn roundtrip_tool_change() {
        assert_roundtrip(GcodeCommand::ToolChange(3));
    }

    #[test]
    fn roundtrip_raw() {
        assert_roundtrip(GcodeCommand::Raw("M84".to_string()));
    }

    #[test]
    fn batch_conversion_roundtrip() {
        let cmds = vec![
            GcodeCommand::Comment("start".to_string()),
            GcodeCommand::SetAbsolutePositioning,
            GcodeCommand::LinearMove {
                x: Some(1.0),
                y: Some(2.0),
                z: None,
                e: Some(0.5),
                f: None,
            },
            GcodeCommand::SetFanSpeed(128),
        ];
        let ffi = commands_to_ffi(&cmds);
        assert_eq!(ffi.len(), 4);
        let back = commands_from_ffi(&ffi);
        assert_eq!(cmds, back);
    }

    #[test]
    fn raw_gcode_variant_converts_to_raw() {
        let ffi = FfiGcodeCommand::RawGcode(RString::from("G29"));
        let gcode = ffi_to_gcode(&ffi);
        assert_eq!(gcode, GcodeCommand::Raw("G29".to_string()));
    }
}
