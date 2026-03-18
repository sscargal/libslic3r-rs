//! Structured G-code command types.
//!
//! G-code commands are represented as typed enum variants rather than raw strings,
//! enabling semantic-level testing and type-safe construction.

use std::fmt;

/// A structured G-code command.
///
/// Each variant represents a specific G-code command with typed parameters.
/// The `Display` implementation formats each variant as its G-code text line.
#[derive(Debug, Clone, PartialEq)]
pub enum GcodeCommand {
    /// A comment line: `; {text}`
    Comment(String),

    /// Linear move: `G1 [X{x}] [Y{y}] [Z{z}] [E{e}] [F{f}]`
    LinearMove {
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
        e: Option<f64>,
        f: Option<f64>,
    },

    /// Rapid move (travel): `G0 [X{x}] [Y{y}] [Z{z}] [F{f}]`
    RapidMove {
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
        f: Option<f64>,
    },

    /// Home axes: `G28 [X] [Y] [Z]`
    Home { x: bool, y: bool, z: bool },

    /// Set absolute positioning: `G90`
    SetAbsolutePositioning,

    /// Set relative positioning: `G91`
    SetRelativePositioning,

    /// Set absolute extrusion mode: `M82`
    SetAbsoluteExtrusion,

    /// Set relative extrusion mode: `M83`
    SetRelativeExtrusion,

    /// Set extruder temperature: `M104 S{temp}` (no wait) or `M109 S{temp}` (wait)
    SetExtruderTemp { temp: f64, wait: bool },

    /// Set bed temperature: `M140 S{temp}` (no wait) or `M190 S{temp}` (wait)
    SetBedTemp { temp: f64, wait: bool },

    /// Set fan speed: `M106 S{0-255}`
    SetFanSpeed(u8),

    /// Fan off: `M107`
    FanOff,

    /// Reset extruder position: `G92 E0`
    ResetExtruder,

    /// Dwell (pause): `G4 P{ms}`
    Dwell { ms: u32 },

    /// Retract filament: emits `G1 E-{distance} F{feedrate}`
    Retract { distance: f64, feedrate: f64 },

    /// Unretract filament: emits `G1 E{distance} F{feedrate}`
    Unretract { distance: f64, feedrate: f64 },

    /// Clockwise arc move: `G2 [X{x}] [Y{y}] I{i} J{j} [E{e}] [F{f}]`
    ArcMoveCW {
        x: Option<f64>,
        y: Option<f64>,
        i: f64,
        j: f64,
        e: Option<f64>,
        f: Option<f64>,
    },

    /// Counter-clockwise arc move: `G3 [X{x}] [Y{y}] I{i} J{j} [E{e}] [F{f}]`
    ArcMoveCCW {
        x: Option<f64>,
        y: Option<f64>,
        i: f64,
        j: f64,
        e: Option<f64>,
        f: Option<f64>,
    },

    /// Set acceleration: `M204 P{print_accel} T{travel_accel}` (Marlin default)
    SetAcceleration { print_accel: f64, travel_accel: f64 },

    /// Set jerk: `M205 X{x} Y{y} Z{z}` (Marlin default)
    SetJerk { x: f64, y: f64, z: f64 },

    /// Set pressure advance: `M900 K{value}` (Marlin default)
    SetPressureAdvance { value: f64 },

    /// Tool change: `T{n}`
    ToolChange(u8),

    /// Raw G-code pass-through for arbitrary lines.
    Raw(String),
}

impl fmt::Display for GcodeCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GcodeCommand::Comment(text) => write!(f, "; {text}"),

            GcodeCommand::LinearMove { x, y, z, e, f: fr } => {
                write!(f, "G1")?;
                if let Some(v) = x {
                    write!(f, " X{v:.3}")?;
                }
                if let Some(v) = y {
                    write!(f, " Y{v:.3}")?;
                }
                if let Some(v) = z {
                    write!(f, " Z{v:.3}")?;
                }
                if let Some(v) = e {
                    write!(f, " E{v:.5}")?;
                }
                if let Some(v) = fr {
                    write!(f, " F{v:.1}")?;
                }
                Ok(())
            }

            GcodeCommand::RapidMove { x, y, z, f: fr } => {
                write!(f, "G0")?;
                if let Some(v) = x {
                    write!(f, " X{v:.3}")?;
                }
                if let Some(v) = y {
                    write!(f, " Y{v:.3}")?;
                }
                if let Some(v) = z {
                    write!(f, " Z{v:.3}")?;
                }
                if let Some(v) = fr {
                    write!(f, " F{v:.1}")?;
                }
                Ok(())
            }

            GcodeCommand::Home { x, y, z } => {
                write!(f, "G28")?;
                if *x {
                    write!(f, " X")?;
                }
                if *y {
                    write!(f, " Y")?;
                }
                if *z {
                    write!(f, " Z")?;
                }
                Ok(())
            }

            GcodeCommand::SetAbsolutePositioning => write!(f, "G90"),
            GcodeCommand::SetRelativePositioning => write!(f, "G91"),
            GcodeCommand::SetAbsoluteExtrusion => write!(f, "M82"),
            GcodeCommand::SetRelativeExtrusion => write!(f, "M83"),

            GcodeCommand::SetExtruderTemp { temp, wait } => {
                let cmd = if *wait { "M109" } else { "M104" };
                write!(f, "{cmd} S{temp:.0}")
            }

            GcodeCommand::SetBedTemp { temp, wait } => {
                let cmd = if *wait { "M190" } else { "M140" };
                write!(f, "{cmd} S{temp:.0}")
            }

            GcodeCommand::SetFanSpeed(speed) => write!(f, "M106 S{speed}"),
            GcodeCommand::FanOff => write!(f, "M107"),
            GcodeCommand::ResetExtruder => write!(f, "G92 E0"),
            GcodeCommand::Dwell { ms } => write!(f, "G4 P{ms}"),

            GcodeCommand::Retract { distance, feedrate } => {
                write!(f, "G1 E-{distance:.5} F{feedrate:.1}")
            }

            GcodeCommand::Unretract { distance, feedrate } => {
                write!(f, "G1 E{distance:.5} F{feedrate:.1}")
            }

            GcodeCommand::ArcMoveCW {
                x,
                y,
                i,
                j,
                e,
                f: fr,
            } => {
                write!(f, "G2")?;
                if let Some(v) = x {
                    write!(f, " X{v:.3}")?;
                }
                if let Some(v) = y {
                    write!(f, " Y{v:.3}")?;
                }
                write!(f, " I{i:.3} J{j:.3}")?;
                if let Some(v) = e {
                    write!(f, " E{v:.5}")?;
                }
                if let Some(v) = fr {
                    write!(f, " F{v:.1}")?;
                }
                Ok(())
            }

            GcodeCommand::ArcMoveCCW {
                x,
                y,
                i,
                j,
                e,
                f: fr,
            } => {
                write!(f, "G3")?;
                if let Some(v) = x {
                    write!(f, " X{v:.3}")?;
                }
                if let Some(v) = y {
                    write!(f, " Y{v:.3}")?;
                }
                write!(f, " I{i:.3} J{j:.3}")?;
                if let Some(v) = e {
                    write!(f, " E{v:.5}")?;
                }
                if let Some(v) = fr {
                    write!(f, " F{v:.1}")?;
                }
                Ok(())
            }

            GcodeCommand::SetAcceleration {
                print_accel,
                travel_accel,
            } => write!(f, "M204 P{print_accel:.0} T{travel_accel:.0}"),

            GcodeCommand::SetJerk { x, y, z } => {
                write!(f, "M205 X{x:.1} Y{y:.1} Z{z:.1}")
            }

            GcodeCommand::SetPressureAdvance { value } => {
                write!(f, "M900 K{value:.4}")
            }

            GcodeCommand::ToolChange(n) => write!(f, "T{n}"),

            GcodeCommand::Raw(line) => write!(f, "{line}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_move_all_params() {
        let cmd = GcodeCommand::LinearMove {
            x: Some(1.0),
            y: Some(2.0),
            z: Some(0.3),
            e: Some(0.5),
            f: Some(1800.0),
        };
        assert_eq!(cmd.to_string(), "G1 X1.000 Y2.000 Z0.300 E0.50000 F1800.0");
    }

    #[test]
    fn rapid_move_formatting() {
        let cmd = GcodeCommand::RapidMove {
            x: Some(10.0),
            y: Some(10.0),
            z: None,
            f: Some(6000.0),
        };
        assert_eq!(cmd.to_string(), "G0 X10.000 Y10.000 F6000.0");
    }

    #[test]
    fn set_extruder_temp_wait() {
        let cmd = GcodeCommand::SetExtruderTemp {
            temp: 200.0,
            wait: true,
        };
        assert_eq!(cmd.to_string(), "M109 S200");
    }

    #[test]
    fn set_extruder_temp_no_wait() {
        let cmd = GcodeCommand::SetExtruderTemp {
            temp: 210.0,
            wait: false,
        };
        assert_eq!(cmd.to_string(), "M104 S210");
    }

    #[test]
    fn set_bed_temp_wait() {
        let cmd = GcodeCommand::SetBedTemp {
            temp: 60.0,
            wait: true,
        };
        assert_eq!(cmd.to_string(), "M190 S60");
    }

    #[test]
    fn set_bed_temp_no_wait() {
        let cmd = GcodeCommand::SetBedTemp {
            temp: 60.0,
            wait: false,
        };
        assert_eq!(cmd.to_string(), "M140 S60");
    }

    #[test]
    fn comment_formatting() {
        let cmd = GcodeCommand::Comment("this is a comment".to_string());
        assert_eq!(cmd.to_string(), "; this is a comment");
    }

    #[test]
    fn coordinates_use_three_decimal_places() {
        let cmd = GcodeCommand::LinearMove {
            x: Some(1.12345),
            y: None,
            z: None,
            e: None,
            f: None,
        };
        assert_eq!(cmd.to_string(), "G1 X1.123");
    }

    #[test]
    fn feedrate_uses_one_decimal_place() {
        let cmd = GcodeCommand::LinearMove {
            x: None,
            y: None,
            z: None,
            e: None,
            f: Some(1200.0),
        };
        assert_eq!(cmd.to_string(), "G1 F1200.0");
    }

    #[test]
    fn temperature_uses_zero_decimal_places() {
        let cmd = GcodeCommand::SetExtruderTemp {
            temp: 200.7,
            wait: false,
        };
        // Rounded to 0 decimal places
        assert_eq!(cmd.to_string(), "M104 S201");
    }

    #[test]
    fn optional_params_omitted_when_none() {
        let cmd = GcodeCommand::LinearMove {
            x: Some(5.0),
            y: None,
            z: None,
            e: Some(0.1),
            f: None,
        };
        assert_eq!(cmd.to_string(), "G1 X5.000 E0.10000");
    }

    #[test]
    fn home_all_axes() {
        let cmd = GcodeCommand::Home {
            x: false,
            y: false,
            z: false,
        };
        assert_eq!(cmd.to_string(), "G28");
    }

    #[test]
    fn home_xy_only() {
        let cmd = GcodeCommand::Home {
            x: true,
            y: true,
            z: false,
        };
        assert_eq!(cmd.to_string(), "G28 X Y");
    }

    #[test]
    fn retract_formatting() {
        let cmd = GcodeCommand::Retract {
            distance: 0.8,
            feedrate: 2700.0,
        };
        assert_eq!(cmd.to_string(), "G1 E-0.80000 F2700.0");
    }

    #[test]
    fn unretract_formatting() {
        let cmd = GcodeCommand::Unretract {
            distance: 0.8,
            feedrate: 2700.0,
        };
        assert_eq!(cmd.to_string(), "G1 E0.80000 F2700.0");
    }

    #[test]
    fn fan_speed() {
        assert_eq!(GcodeCommand::SetFanSpeed(255).to_string(), "M106 S255");
        assert_eq!(GcodeCommand::SetFanSpeed(0).to_string(), "M106 S0");
    }

    #[test]
    fn fan_off() {
        assert_eq!(GcodeCommand::FanOff.to_string(), "M107");
    }

    #[test]
    fn reset_extruder() {
        assert_eq!(GcodeCommand::ResetExtruder.to_string(), "G92 E0");
    }

    #[test]
    fn dwell() {
        assert_eq!(GcodeCommand::Dwell { ms: 500 }.to_string(), "G4 P500");
    }

    #[test]
    fn positioning_modes() {
        assert_eq!(GcodeCommand::SetAbsolutePositioning.to_string(), "G90");
        assert_eq!(GcodeCommand::SetRelativePositioning.to_string(), "G91");
        assert_eq!(GcodeCommand::SetAbsoluteExtrusion.to_string(), "M82");
        assert_eq!(GcodeCommand::SetRelativeExtrusion.to_string(), "M83");
    }

    #[test]
    fn raw_passthrough() {
        let cmd = GcodeCommand::Raw("M84".to_string());
        assert_eq!(cmd.to_string(), "M84");
    }

    #[test]
    fn extrusion_uses_five_decimal_places() {
        let cmd = GcodeCommand::LinearMove {
            x: None,
            y: None,
            z: None,
            e: Some(0.12345),
            f: None,
        };
        assert_eq!(cmd.to_string(), "G1 E0.12345");
    }

    #[test]
    fn arc_move_cw_all_params() {
        let cmd = GcodeCommand::ArcMoveCW {
            x: Some(10.0),
            y: Some(20.0),
            i: 5.0,
            j: 0.0,
            e: Some(0.5),
            f: Some(1800.0),
        };
        assert_eq!(
            cmd.to_string(),
            "G2 X10.000 Y20.000 I5.000 J0.000 E0.50000 F1800.0"
        );
    }

    #[test]
    fn arc_move_ccw_minimal_params() {
        let cmd = GcodeCommand::ArcMoveCCW {
            x: None,
            y: None,
            i: 3.0,
            j: -2.5,
            e: None,
            f: None,
        };
        assert_eq!(cmd.to_string(), "G3 I3.000 J-2.500");
    }

    #[test]
    fn set_acceleration_formatting() {
        let cmd = GcodeCommand::SetAcceleration {
            print_accel: 1000.0,
            travel_accel: 1500.0,
        };
        assert_eq!(cmd.to_string(), "M204 P1000 T1500");
    }

    #[test]
    fn set_jerk_formatting() {
        let cmd = GcodeCommand::SetJerk {
            x: 8.0,
            y: 8.0,
            z: 0.4,
        };
        assert_eq!(cmd.to_string(), "M205 X8.0 Y8.0 Z0.4");
    }

    #[test]
    fn set_pressure_advance_formatting() {
        let cmd = GcodeCommand::SetPressureAdvance { value: 0.05 };
        assert_eq!(cmd.to_string(), "M900 K0.0500");
    }

    #[test]
    fn set_pressure_advance_zero() {
        let cmd = GcodeCommand::SetPressureAdvance { value: 0.0 };
        assert_eq!(cmd.to_string(), "M900 K0.0000");
    }

    #[test]
    fn tool_change_formatting() {
        let cmd = GcodeCommand::ToolChange(0);
        assert_eq!(cmd.to_string(), "T0");
        let cmd = GcodeCommand::ToolChange(3);
        assert_eq!(cmd.to_string(), "T3");
    }
}
