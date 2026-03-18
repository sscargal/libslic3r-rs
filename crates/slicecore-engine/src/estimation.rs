//! Print time estimation using a trapezoid motion model.
//!
//! The trapezoid model accounts for acceleration and deceleration ramps,
//! producing more accurate time estimates than naive distance/feedrate
//! calculations. A naive approach under-reports by 30-50% because it
//! assumes instantaneous speed changes.
//!
//! # Model
//!
//! Each move segment follows a trapezoidal velocity profile:
//! 1. **Acceleration phase**: speed ramps from entry speed to cruise speed
//! 2. **Cruise phase**: constant speed at the commanded feedrate
//! 3. **Deceleration phase**: speed ramps down to exit speed
//!
//! When the segment is too short to reach cruise speed, a triangular
//! profile is used instead (acceleration directly into deceleration).

use serde::{Deserialize, Serialize};
use slicecore_gcode_io::GcodeCommand;

/// Result of print time estimation with per-category breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintTimeEstimate {
    /// Total estimated print time in seconds.
    pub total_seconds: f64,
    /// Time spent on extrusion moves in seconds.
    pub move_time_seconds: f64,
    /// Time spent on travel (non-extrusion) moves in seconds.
    pub travel_time_seconds: f64,
    /// Number of retractions performed.
    pub retraction_count: u32,
}

/// Computes the time for a single move segment using a trapezoidal velocity profile.
///
/// The trapezoid model computes acceleration and deceleration ramps between
/// `entry_speed` and `exit_speed` through `cruise_speed`, subject to the
/// given `acceleration` rate.
///
/// # Parameters
///
/// - `distance`: Total distance of the segment in mm.
/// - `entry_speed`: Speed at start of segment in mm/s.
/// - `cruise_speed`: Target (maximum) speed for the segment in mm/s.
/// - `exit_speed`: Speed at end of segment in mm/s.
/// - `acceleration`: Acceleration rate in mm/s^2.
///
/// # Returns
///
/// Time in seconds to traverse the segment.
///
/// # Edge Cases
///
/// - Returns 0.0 if distance <= 0 or acceleration <= 0.
pub fn trapezoid_time(
    distance: f64,
    entry_speed: f64,
    cruise_speed: f64,
    exit_speed: f64,
    acceleration: f64,
) -> f64 {
    if distance <= 0.0 || acceleration <= 0.0 {
        return 0.0;
    }

    // Clamp speeds to be non-negative.
    let entry = entry_speed.max(0.0);
    let cruise = cruise_speed.max(entry).max(exit_speed.max(0.0));
    let exit = exit_speed.max(0.0);

    // Distance needed to accelerate from entry to cruise speed.
    let accel_dist = (cruise * cruise - entry * entry) / (2.0 * acceleration);
    // Distance needed to decelerate from cruise to exit speed.
    let decel_dist = (cruise * cruise - exit * exit) / (2.0 * acceleration);

    if accel_dist + decel_dist > distance {
        // Triangular profile: cannot reach cruise speed.
        // Find the peak speed achievable in this distance.
        // v_peak^2 = (2*a*d + entry^2 + exit^2) / 2
        let v_peak_sq = (2.0 * acceleration * distance + entry * entry + exit * exit) / 2.0;
        if v_peak_sq <= 0.0 {
            // Edge case: very short distance or speeds don't work out.
            if cruise > 0.0 {
                return distance / cruise;
            }
            return 0.0;
        }
        let v_peak = v_peak_sq.sqrt();

        // Time for acceleration phase: (v_peak - entry) / accel
        let t_accel = if acceleration > 0.0 {
            (v_peak - entry) / acceleration
        } else {
            0.0
        };

        // Time for deceleration phase: (v_peak - exit) / accel
        let t_decel = if acceleration > 0.0 {
            (v_peak - exit) / acceleration
        } else {
            0.0
        };

        t_accel.max(0.0) + t_decel.max(0.0)
    } else {
        // Full trapezoidal profile with cruise phase.
        let cruise_dist = distance - accel_dist - decel_dist;

        // Time for each phase.
        let t_accel = if acceleration > 0.0 {
            (cruise - entry) / acceleration
        } else {
            0.0
        };
        let t_cruise = if cruise > 0.0 {
            cruise_dist / cruise
        } else {
            0.0
        };
        let t_decel = if acceleration > 0.0 {
            (cruise - exit) / acceleration
        } else {
            0.0
        };

        t_accel.max(0.0) + t_cruise.max(0.0) + t_decel.max(0.0)
    }
}

/// Estimates print time from a stream of G-code commands using the trapezoid motion model.
///
/// Iterates through the command stream, tracking position and feedrate.
/// For each move, applies the trapezoid model with appropriate acceleration
/// (print acceleration for extrusion moves, travel acceleration for travel moves).
///
/// # Simplifications
///
/// - Entry speed uses `min(current_feedrate, previous_feedrate)` as a simple
///   lookahead approximation (no full junction speed computation).
/// - Fixed time overhead per retraction (0.5s) and per layer change (0.2s).
///
/// # Parameters
///
/// - `commands`: The G-code command stream to estimate.
/// - `print_acceleration`: Acceleration for extrusion moves in mm/s^2.
/// - `travel_acceleration`: Acceleration for travel moves in mm/s^2.
///
/// # Returns
///
/// A [`PrintTimeEstimate`] with total time and per-category breakdown.
pub fn estimate_print_time(
    commands: &[GcodeCommand],
    print_acceleration: f64,
    travel_acceleration: f64,
) -> PrintTimeEstimate {
    let mut total_seconds = 0.0;
    let mut move_time_seconds = 0.0;
    let mut travel_time_seconds = 0.0;
    let mut retraction_count: u32 = 0;

    // Track current position.
    let mut cur_x: f64 = 0.0;
    let mut cur_y: f64 = 0.0;
    let mut cur_z: f64 = 0.0;
    let mut cur_feedrate: f64 = 0.0; // mm/s
    let mut prev_feedrate: f64 = 0.0; // for simple lookahead

    // Fixed overhead constants.
    const RETRACTION_OVERHEAD_S: f64 = 0.5;
    const LAYER_CHANGE_OVERHEAD_S: f64 = 0.2;

    for cmd in commands {
        match cmd {
            GcodeCommand::LinearMove { x, y, z, e, f } => {
                let new_x = x.unwrap_or(cur_x);
                let new_y = y.unwrap_or(cur_y);
                let new_z = z.unwrap_or(cur_z);

                if let Some(fr) = f {
                    prev_feedrate = cur_feedrate;
                    cur_feedrate = *fr / 60.0; // mm/min -> mm/s
                }

                let dx = new_x - cur_x;
                let dy = new_y - cur_y;
                let dz = new_z - cur_z;
                let distance = (dx * dx + dy * dy + dz * dz).sqrt();

                if distance > 1e-6 {
                    let has_extrusion = e.is_some_and(|ev| ev > 0.0);
                    let accel = if has_extrusion {
                        print_acceleration
                    } else {
                        travel_acceleration
                    };

                    // Simple lookahead: entry speed = min of current and previous feedrate.
                    let entry_speed = if prev_feedrate > 0.0 {
                        cur_feedrate.min(prev_feedrate)
                    } else {
                        0.0
                    };

                    let time = trapezoid_time(distance, entry_speed, cur_feedrate, 0.0, accel);

                    if has_extrusion {
                        move_time_seconds += time;
                    } else {
                        travel_time_seconds += time;
                    }
                    total_seconds += time;
                }

                cur_x = new_x;
                cur_y = new_y;
                cur_z = new_z;
            }

            GcodeCommand::RapidMove { x, y, z, f } => {
                let new_x = x.unwrap_or(cur_x);
                let new_y = y.unwrap_or(cur_y);
                let new_z = z.unwrap_or(cur_z);

                if let Some(fr) = f {
                    prev_feedrate = cur_feedrate;
                    cur_feedrate = *fr / 60.0;
                }

                let dx = new_x - cur_x;
                let dy = new_y - cur_y;
                let dz = new_z - cur_z;
                let distance = (dx * dx + dy * dy + dz * dz).sqrt();

                if distance > 1e-6 {
                    let entry_speed = if prev_feedrate > 0.0 {
                        cur_feedrate.min(prev_feedrate)
                    } else {
                        0.0
                    };

                    let time = trapezoid_time(
                        distance,
                        entry_speed,
                        cur_feedrate,
                        0.0,
                        travel_acceleration,
                    );
                    travel_time_seconds += time;
                    total_seconds += time;

                    // Detect layer change: Z-only moves.
                    if x.is_none() && y.is_none() && z.is_some() {
                        total_seconds += LAYER_CHANGE_OVERHEAD_S;
                    }
                }

                cur_x = new_x;
                cur_y = new_y;
                cur_z = new_z;
            }

            GcodeCommand::Retract { .. } => {
                retraction_count += 1;
                total_seconds += RETRACTION_OVERHEAD_S;
            }

            GcodeCommand::ArcMoveCW {
                x, y, i, j, e, f, ..
            }
            | GcodeCommand::ArcMoveCCW {
                x, y, i, j, e, f, ..
            } => {
                let new_x = x.unwrap_or(cur_x);
                let new_y = y.unwrap_or(cur_y);

                if let Some(fr) = f {
                    prev_feedrate = cur_feedrate;
                    cur_feedrate = *fr / 60.0;
                }

                // Compute arc length from center offset (I, J) and endpoint.
                let cx = cur_x + i;
                let cy = cur_y + j;
                let radius = (i * i + j * j).sqrt();

                if radius > 1e-6 {
                    // Angle from center to start point.
                    let start_angle = (cur_y - cy).atan2(cur_x - cx);
                    // Angle from center to end point.
                    let end_angle = (new_y - cy).atan2(new_x - cx);
                    let mut sweep = end_angle - start_angle;

                    // Adjust sweep based on arc direction.
                    let is_cw = matches!(cmd, GcodeCommand::ArcMoveCW { .. });
                    if is_cw {
                        if sweep > 0.0 {
                            sweep -= 2.0 * std::f64::consts::PI;
                        }
                    } else if sweep < 0.0 {
                        sweep += 2.0 * std::f64::consts::PI;
                    }

                    let arc_length = radius * sweep.abs();
                    if arc_length > 1e-6 {
                        let has_extrusion = e.is_some_and(|ev| ev > 0.0);
                        let accel = if has_extrusion {
                            print_acceleration
                        } else {
                            travel_acceleration
                        };

                        let entry_speed = if prev_feedrate > 0.0 {
                            cur_feedrate.min(prev_feedrate)
                        } else {
                            0.0
                        };

                        let time =
                            trapezoid_time(arc_length, entry_speed, cur_feedrate, 0.0, accel);

                        if has_extrusion {
                            move_time_seconds += time;
                        } else {
                            travel_time_seconds += time;
                        }
                        total_seconds += time;
                    }
                }

                cur_x = new_x;
                cur_y = new_y;
            }

            // Ignore non-movement commands.
            _ => {}
        }
    }

    PrintTimeEstimate {
        total_seconds,
        move_time_seconds,
        travel_time_seconds,
        retraction_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trapezoid_time_zero_distance_returns_zero() {
        let t = trapezoid_time(0.0, 0.0, 100.0, 0.0, 1000.0);
        assert!(
            (t - 0.0).abs() < 1e-9,
            "Zero distance should return 0, got {}",
            t
        );
    }

    #[test]
    fn trapezoid_time_zero_acceleration_returns_zero() {
        let t = trapezoid_time(10.0, 0.0, 100.0, 0.0, 0.0);
        assert!(
            (t - 0.0).abs() < 1e-9,
            "Zero acceleration should return 0, got {}",
            t
        );
    }

    #[test]
    fn trapezoid_time_full_profile_reaches_cruise() {
        // Long distance: should have full trapezoid (accel, cruise, decel).
        // Distance = 100mm, cruise = 100mm/s, accel = 1000mm/s^2
        // accel_dist = (100^2 - 0^2) / (2*1000) = 5mm
        // decel_dist = (100^2 - 0^2) / (2*1000) = 5mm
        // cruise_dist = 100 - 5 - 5 = 90mm
        // t_accel = 100/1000 = 0.1s
        // t_cruise = 90/100 = 0.9s
        // t_decel = 100/1000 = 0.1s
        // total = 1.1s
        let t = trapezoid_time(100.0, 0.0, 100.0, 0.0, 1000.0);
        assert!(
            (t - 1.1).abs() < 1e-6,
            "Full trapezoid should be ~1.1s, got {}",
            t
        );

        // Naive distance/feedrate = 100/100 = 1.0s (less than trapezoid).
        let naive = 100.0 / 100.0;
        assert!(
            t > naive,
            "Trapezoid ({}) should be greater than naive ({})",
            t,
            naive
        );
    }

    #[test]
    fn trapezoid_time_triangular_profile_short_distance() {
        // Short distance: cannot reach cruise speed, triangular profile.
        // Distance = 2mm, cruise = 100mm/s, accel = 1000mm/s^2
        // accel_dist = 5mm > distance, so triangular.
        let t = trapezoid_time(2.0, 0.0, 100.0, 0.0, 1000.0);
        // v_peak^2 = (2*1000*2 + 0 + 0) / 2 = 2000, v_peak = ~44.7mm/s
        // t_accel = 44.7/1000 = 0.0447s
        // t_decel = 44.7/1000 = 0.0447s
        // total = ~0.0894s
        let expected = 2.0 * (2.0 * 1000.0f64).sqrt() / 1000.0;
        assert!(
            (t - expected).abs() < 0.001,
            "Triangular profile should be ~{:.4}s, got {:.4}",
            expected,
            t
        );

        // Should be shorter than full profile time.
        let full = trapezoid_time(100.0, 0.0, 100.0, 0.0, 1000.0);
        assert!(
            t < full,
            "Triangular ({}) should be less than full ({})",
            t,
            full
        );
    }

    #[test]
    fn estimate_print_time_simple_moves() {
        // Create a simple 3-move sequence.
        let commands = vec![
            GcodeCommand::LinearMove {
                x: Some(10.0),
                y: Some(0.0),
                z: Some(0.2),
                e: Some(0.5),
                f: Some(3000.0), // 50mm/s
            },
            GcodeCommand::LinearMove {
                x: Some(20.0),
                y: Some(0.0),
                z: None,
                e: Some(0.5),
                f: None, // same feedrate
            },
            GcodeCommand::LinearMove {
                x: Some(20.0),
                y: Some(10.0),
                z: None,
                e: Some(0.5),
                f: None,
            },
        ];

        let estimate = estimate_print_time(&commands, 1000.0, 1500.0);
        assert!(
            estimate.total_seconds > 0.0,
            "Total time should be positive, got {}",
            estimate.total_seconds
        );
        assert!(
            estimate.move_time_seconds > 0.0,
            "Move time should be positive, got {}",
            estimate.move_time_seconds
        );
        assert_eq!(
            estimate.retraction_count, 0,
            "No retractions in this sequence"
        );
    }

    #[test]
    fn estimate_print_time_with_retractions() {
        let commands = vec![
            GcodeCommand::LinearMove {
                x: Some(10.0),
                y: Some(0.0),
                z: None,
                e: Some(0.5),
                f: Some(3000.0),
            },
            GcodeCommand::Retract {
                distance: 0.8,
                feedrate: 2700.0,
            },
            GcodeCommand::RapidMove {
                x: Some(50.0),
                y: Some(50.0),
                z: None,
                f: Some(9000.0),
            },
            GcodeCommand::Unretract {
                distance: 0.8,
                feedrate: 2700.0,
            },
            GcodeCommand::LinearMove {
                x: Some(60.0),
                y: Some(50.0),
                z: None,
                e: Some(0.5),
                f: Some(3000.0),
            },
        ];

        let estimate = estimate_print_time(&commands, 1000.0, 1500.0);
        assert_eq!(estimate.retraction_count, 1, "Should count 1 retraction");
        // Retraction adds 0.5s overhead.
        assert!(
            estimate.total_seconds > 0.5,
            "Total should include retraction overhead, got {}",
            estimate.total_seconds
        );
        assert!(
            estimate.travel_time_seconds > 0.0,
            "Should have travel time from rapid move"
        );
    }

    #[test]
    fn trapezoid_time_greater_than_naive() {
        // Verify that trapezoid estimate is greater than naive for typical moves.
        let distance = 50.0;
        let feedrate = 80.0; // mm/s
        let acceleration = 1000.0;

        let trapezoid = trapezoid_time(distance, 0.0, feedrate, 0.0, acceleration);
        let naive = distance / feedrate;

        assert!(
            trapezoid > naive,
            "Trapezoid ({:.4}s) should exceed naive ({:.4}s) for same segment",
            trapezoid,
            naive
        );
    }

    #[test]
    fn estimate_print_time_greater_than_naive_for_commands() {
        // Build a multi-move sequence and verify trapezoid > naive.
        let commands = vec![
            GcodeCommand::LinearMove {
                x: Some(50.0),
                y: Some(0.0),
                z: Some(0.2),
                e: Some(2.0),
                f: Some(4800.0), // 80mm/s
            },
            GcodeCommand::LinearMove {
                x: Some(50.0),
                y: Some(50.0),
                z: None,
                e: Some(2.0),
                f: None,
            },
            GcodeCommand::LinearMove {
                x: Some(0.0),
                y: Some(50.0),
                z: None,
                e: Some(2.0),
                f: None,
            },
        ];

        let estimate = estimate_print_time(&commands, 1000.0, 1500.0);

        // Naive: total distance / feedrate.
        // Move 1: sqrt(50^2 + 0.2^2) ~= 50mm @ 80mm/s = 0.625s
        // Move 2: 50mm @ 80mm/s = 0.625s
        // Move 3: 50mm @ 80mm/s = 0.625s
        // Total naive ~= 1.875s
        let naive_total =
            (50.0f64.powi(2) + 0.2f64.powi(2)).sqrt() / 80.0 + 50.0 / 80.0 + 50.0 / 80.0;

        assert!(
            estimate.total_seconds > naive_total,
            "Trapezoid estimate ({:.4}s) should exceed naive ({:.4}s)",
            estimate.total_seconds,
            naive_total
        );
    }
}
