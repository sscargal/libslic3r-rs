//! Arc fitting algorithm for G-code post-processing.
//!
//! Converts sequences of linear G1 moves into G2/G3 arc commands,
//! reducing G-code file size by 20-40% on curved models. The algorithm
//! uses a sliding window approach to find the longest sequence of
//! consecutive linear moves that lie on a circular arc within a
//! configurable tolerance.
//!
//! # Algorithm
//!
//! 1. Scan the command stream for consecutive `LinearMove` (G1) sequences
//! 2. For each candidate window (starting at `min_arc_points` moves),
//!    extract XY positions and test with [`points_fit_arc`]
//! 3. If the points fit an arc, extend the window; otherwise emit the arc
//! 4. Replace the window with a single `ArcMoveCW` or `ArcMoveCCW` command
//! 5. Preserve non-G1 commands (comments, retractions, Z-moves) as-is
//!
//! # Constraints
//!
//! - Minimum arc radius: 0.5mm (tighter arcs are skipped)
//! - Maximum arc radius: 1000mm (nearly straight lines are skipped)
//! - E-value for the arc = sum of replaced segment E-values
//! - Feedrate for the arc = feedrate of the last replaced segment

use crate::commands::GcodeCommand;

/// Computes the circumscribed circle (circumcircle) from 3 points.
///
/// Returns `Some((center, radius))` if the points define a valid circle,
/// or `None` if the points are collinear (determinant < 1e-10).
///
/// # Arguments
///
/// * `p1`, `p2`, `p3` - Three 2D points (x, y).
pub fn circumcircle(p1: (f64, f64), p2: (f64, f64), p3: (f64, f64)) -> Option<((f64, f64), f64)> {
    let ax = p1.0;
    let ay = p1.1;
    let bx = p2.0;
    let by = p2.1;
    let cx = p3.0;
    let cy = p3.1;

    // Determinant of the system (2x the signed area of the triangle).
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));

    if d.abs() < 1e-10 {
        return None; // Points are collinear.
    }

    let ax2_ay2 = ax * ax + ay * ay;
    let bx2_by2 = bx * bx + by * by;
    let cx2_cy2 = cx * cx + cy * cy;

    let ux = (ax2_ay2 * (by - cy) + bx2_by2 * (cy - ay) + cx2_cy2 * (ay - by)) / d;
    let uy = (ax2_ay2 * (cx - bx) + bx2_by2 * (ax - cx) + cx2_cy2 * (bx - ax)) / d;

    let radius = ((ax - ux).powi(2) + (ay - uy).powi(2)).sqrt();

    Some(((ux, uy), radius))
}

/// Tests whether a sequence of points lies on a circular arc within tolerance.
///
/// Returns `Some((center, radius, is_ccw))` if all points lie within
/// `tolerance` of the arc radius, or `None` if any point deviates.
///
/// Arc direction is determined by the cross product of vectors from
/// center to start and center to end.
///
/// # Arguments
///
/// * `points` - A slice of at least 3 (x, y) points.
/// * `tolerance` - Maximum distance any point may deviate from the arc radius.
pub fn points_fit_arc(points: &[(f64, f64)], tolerance: f64) -> Option<((f64, f64), f64, bool)> {
    if points.len() < 3 {
        return None;
    }

    let first = points[0];
    let mid = points[points.len() / 2];
    let last = points[points.len() - 1];

    let (center, radius) = circumcircle(first, mid, last)?;

    // Validate all intermediate points lie within tolerance.
    for &(px, py) in points {
        let dist = ((px - center.0).powi(2) + (py - center.1).powi(2)).sqrt();
        if (dist - radius).abs() > tolerance {
            return None;
        }
    }

    // Determine direction via cross product of successive edge vectors.
    // Use the first three points: cross product of (p1->p2) x (p2->p3).
    let v1x = points[1].0 - points[0].0;
    let v1y = points[1].1 - points[0].1;
    let v2x = points[2].0 - points[1].0;
    let v2y = points[2].1 - points[1].1;
    let cross = v1x * v2y - v1y * v2x;
    let is_ccw = cross > 0.0;

    Some((center, radius, is_ccw))
}

/// Computes the arc length between two points on a circle.
///
/// # Arguments
///
/// * `center` - Center of the circle.
/// * `start` - Start point on the arc.
/// * `end` - End point on the arc.
/// * `is_ccw` - Whether the arc goes counter-clockwise.
///
/// # Returns
///
/// The arc length in the same units as the input coordinates.
pub fn arc_length(center: (f64, f64), start: (f64, f64), end: (f64, f64), is_ccw: bool) -> f64 {
    let radius = ((start.0 - center.0).powi(2) + (start.1 - center.1).powi(2)).sqrt();
    if radius < 1e-12 {
        return 0.0;
    }

    let angle_start = (start.1 - center.1).atan2(start.0 - center.0);
    let angle_end = (end.1 - center.1).atan2(end.0 - center.0);

    let mut sweep = if is_ccw {
        angle_end - angle_start
    } else {
        angle_start - angle_end
    };

    if sweep < 0.0 {
        sweep += 2.0 * std::f64::consts::PI;
    }
    // Full circle case: if sweep is essentially zero, treat as full circle.
    if sweep < 1e-12 {
        sweep = 2.0 * std::f64::consts::PI;
    }

    radius * sweep
}

/// Checks whether a `GcodeCommand` is a linear move with XY coordinates
/// (and optionally E/F) but NOT a Z-only move.
fn is_xy_linear_move(cmd: &GcodeCommand) -> bool {
    matches!(
        cmd,
        GcodeCommand::LinearMove {
            x: Some(_),
            y: Some(_),
            z: None,
            ..
        }
    )
}

/// Extracts the (x, y) position from a linear move command.
/// Panics if the command is not a LinearMove with x and y.
fn extract_xy(cmd: &GcodeCommand) -> (f64, f64) {
    match cmd {
        GcodeCommand::LinearMove {
            x: Some(x),
            y: Some(y),
            ..
        } => (*x, *y),
        _ => unreachable!("extract_xy called on non-LinearMove"),
    }
}

/// Extracts the E-value from a linear move command (0.0 if None).
fn extract_e(cmd: &GcodeCommand) -> f64 {
    match cmd {
        GcodeCommand::LinearMove { e, .. } => e.unwrap_or(0.0),
        _ => 0.0,
    }
}

/// Extracts the feedrate from a linear move command (None if not set).
fn extract_f(cmd: &GcodeCommand) -> Option<f64> {
    match cmd {
        GcodeCommand::LinearMove { f, .. } => *f,
        _ => None,
    }
}

/// Fits arcs to a sequence of G-code commands.
///
/// Scans the command stream for consecutive `LinearMove` (G1) sequences
/// that form circular arcs, and replaces them with `ArcMoveCW` (G2) or
/// `ArcMoveCCW` (G3) commands.
///
/// # Arguments
///
/// * `commands` - The input G-code command stream.
/// * `tolerance` - Maximum deviation (mm) a point may have from the arc.
/// * `min_arc_points` - Minimum number of consecutive G1 moves to consider
///   for arc fitting (at least 3).
///
/// # Returns
///
/// A new command vector with eligible G1 sequences replaced by arc commands.
pub fn fit_arcs(
    commands: &[GcodeCommand],
    tolerance: f64,
    min_arc_points: usize,
) -> Vec<GcodeCommand> {
    let min_arc_points = min_arc_points.max(3);
    let mut result = Vec::with_capacity(commands.len());
    let mut i = 0;

    while i < commands.len() {
        // Look for a run of consecutive XY linear moves.
        if !is_xy_linear_move(&commands[i]) {
            result.push(commands[i].clone());
            i += 1;
            continue;
        }

        // Found start of a potential G1 run. Find the extent.
        let run_start = i;
        let mut run_end = i;
        while run_end < commands.len() && is_xy_linear_move(&commands[run_end]) {
            run_end += 1;
        }
        // run_start..run_end is a range of consecutive XY linear moves.
        let run_len = run_end - run_start;

        if run_len < min_arc_points {
            // Not enough moves for an arc -- emit as-is.
            for cmd in &commands[run_start..run_end] {
                result.push(cmd.clone());
            }
            i = run_end;
            continue;
        }

        // We need one extra point as the "start position" for the arc.
        // The first move's endpoint is position[1], but we need position[0].
        // For the first move in the run, the start XY comes from the move
        // itself -- but G1 moves specify the destination, not the start.
        // We need the previous position. If the run_start > 0 and the
        // previous command was a LinearMove or RapidMove, use its XY.
        // Otherwise, we use the first move's XY as both start and first
        // endpoint, meaning we need run_len+1 points but only have run_len
        // endpoints. To handle this cleanly, we collect all endpoints and
        // note that point[0] is the destination of commands[run_start],
        // point[1] is the destination of commands[run_start+1], etc.
        // An arc from commands[j..j+n] replaces those n commands,
        // where the arc goes from the position before commands[j]
        // (i.e., the endpoint of commands[j-1], or commands[j]'s
        // implicit start) to the endpoint of commands[j+n-1].
        //
        // For simplicity: collect endpoints. points[k] = endpoint of
        // commands[run_start + k]. An arc spanning commands[j..j+n]
        // (indices relative to run_start) uses points j-1..j+n-1
        // (n+1 points including the start).
        //
        // To handle the "start point" for the first arc, we look at the
        // command before run_start. If it provides XY, use that.
        // Otherwise, we skip the first move and start from index 1.

        let mut points: Vec<(f64, f64)> = Vec::with_capacity(run_len + 1);

        // Try to get the position before the run.
        let has_pre_point = if run_start > 0 {
            match &commands[run_start - 1] {
                GcodeCommand::LinearMove {
                    x: Some(x),
                    y: Some(y),
                    ..
                }
                | GcodeCommand::RapidMove {
                    x: Some(x),
                    y: Some(y),
                    ..
                }
                | GcodeCommand::ArcMoveCW {
                    x: Some(x),
                    y: Some(y),
                    ..
                }
                | GcodeCommand::ArcMoveCCW {
                    x: Some(x),
                    y: Some(y),
                    ..
                } => {
                    points.push((*x, *y));
                    true
                }
                _ => false,
            }
        } else {
            false
        };

        // Collect endpoints of the run.
        for cmd in &commands[run_start..run_end] {
            points.push(extract_xy(cmd));
        }

        // Now try to fit arcs using a sliding window.
        // points[0] is either the pre-point or the first move endpoint.
        // commands[run_start + k] corresponds to the move that goes
        // from points[offset + k - 1] to points[offset + k],
        // where offset = 1 if has_pre_point else 0.
        //
        // Actually: if has_pre_point, points has run_len+1 entries:
        //   points[0] = pre-position
        //   points[1] = endpoint of commands[run_start]
        //   points[k+1] = endpoint of commands[run_start + k]
        // So arc spanning commands[run_start+a .. run_start+b]
        // (b exclusive) uses points[a .. b+1] (if has_pre_point)
        // or points[a-1 .. b] adjusted... This is getting complex.
        //
        // Let's simplify: we process the run with a greedy approach.
        // Maintain a "cursor" into the commands of the run.

        let offset = if has_pre_point { 1 } else { 0 };
        // points[offset + k] = endpoint of commands[run_start + k]
        // points[0] = pre-point if has_pre_point

        let mut cursor = 0usize; // index into run (0 = commands[run_start])

        while cursor < run_len {
            // The start point of the arc is:
            // - If cursor == 0 && has_pre_point: points[0]
            // - If cursor > 0: points[offset + cursor - 1]
            //   (the endpoint of the previous command)
            // - If cursor == 0 && !has_pre_point: we don't have a start
            //   point, so we need at least min_arc_points + 1 moves
            //   but we can use the first move's endpoint as the start
            //   of the arc from the second move onward.
            //   For the very first move, just emit it as-is.

            let start_point_idx = if cursor == 0 && has_pre_point {
                Some(0)
            } else if cursor > 0 {
                Some(offset + cursor - 1)
            } else {
                // cursor == 0, no pre-point. Emit first move, advance.
                None
            };

            if start_point_idx.is_none() {
                result.push(commands[run_start + cursor].clone());
                cursor += 1;
                continue;
            }

            let start_pt_idx = start_point_idx.unwrap();

            // Try to fit an arc starting at cursor.
            // We need at least min_arc_points moves (min_arc_points + 1
            // points including the start).
            let remaining = run_len - cursor;
            if remaining < min_arc_points {
                // Not enough moves left -- emit remaining as-is.
                for j in cursor..run_len {
                    result.push(commands[run_start + j].clone());
                }
                cursor = run_len;
                continue;
            }

            // Greedily extend the arc window.
            let mut best_arc_len = 0usize; // number of commands in best arc
            let mut best_center = (0.0, 0.0);
            let mut best_radius = 0.0;
            let mut best_is_ccw = false;

            // Start with min_arc_points moves, try extending.
            let max_window = remaining;
            for window_size in min_arc_points..=max_window {
                // Points for this window: start_pt + endpoints of
                // commands[cursor..cursor+window_size].
                let pt_start = start_pt_idx;
                let pt_end = offset + cursor + window_size; // exclusive
                if pt_end > points.len() {
                    break;
                }

                let window_points = &points[pt_start..pt_end];

                if let Some((center, radius, is_ccw)) = points_fit_arc(window_points, tolerance) {
                    // Check radius constraints.
                    if (0.5..=1000.0).contains(&radius) {
                        best_arc_len = window_size;
                        best_center = center;
                        best_radius = radius;
                        best_is_ccw = is_ccw;
                    } else {
                        break; // Radius out of range, stop extending.
                    }
                } else {
                    break; // Points don't fit arc, stop extending.
                }
            }

            if best_arc_len >= min_arc_points {
                // Emit an arc command replacing commands[cursor..cursor+best_arc_len].
                let _start_xy = points[start_pt_idx];
                let end_xy = points[offset + cursor + best_arc_len - 1];

                // Sum E-values from replaced commands.
                let total_e: f64 = (cursor..cursor + best_arc_len)
                    .map(|j| extract_e(&commands[run_start + j]))
                    .sum();

                // Use feedrate from last replaced command.
                let last_f = extract_f(&commands[run_start + cursor + best_arc_len - 1]);

                // I/J offsets = center - start point.
                let i_offset = best_center.0 - points[start_pt_idx].0;
                let j_offset = best_center.1 - points[start_pt_idx].1;

                let _ = best_radius; // Used for validation above.

                let arc_cmd = if best_is_ccw {
                    GcodeCommand::ArcMoveCCW {
                        x: Some(end_xy.0),
                        y: Some(end_xy.1),
                        i: i_offset,
                        j: j_offset,
                        e: if total_e.abs() > 1e-10 {
                            Some(total_e)
                        } else {
                            None
                        },
                        f: last_f,
                    }
                } else {
                    GcodeCommand::ArcMoveCW {
                        x: Some(end_xy.0),
                        y: Some(end_xy.1),
                        i: i_offset,
                        j: j_offset,
                        e: if total_e.abs() > 1e-10 {
                            Some(total_e)
                        } else {
                            None
                        },
                        f: last_f,
                    }
                };

                result.push(arc_cmd);
                cursor += best_arc_len;
            } else {
                // Couldn't fit an arc, emit current command and advance.
                result.push(commands[run_start + cursor].clone());
                cursor += 1;
            }
        }

        i = run_end;
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn circumcircle_known_circle() {
        // Points on a circle of radius 1 centered at (0, 0).
        let p1 = (1.0, 0.0);
        let p2 = (0.0, 1.0);
        let p3 = (-1.0, 0.0);

        let (center, radius) = circumcircle(p1, p2, p3).expect("should find circumcircle");

        assert!(
            (center.0).abs() < 1e-9,
            "center.x should be 0, got {}",
            center.0
        );
        assert!(
            (center.1).abs() < 1e-9,
            "center.y should be 0, got {}",
            center.1
        );
        assert!(
            (radius - 1.0).abs() < 1e-9,
            "radius should be 1.0, got {}",
            radius
        );
    }

    #[test]
    fn circumcircle_off_origin() {
        // Circle centered at (5, 5), radius 10.
        let r = 10.0;
        let cx = 5.0;
        let cy = 5.0;
        let p1 = (cx + r, cy);
        let p2 = (cx, cy + r);
        let p3 = (cx - r, cy);

        let (center, radius) = circumcircle(p1, p2, p3).expect("should find circumcircle");

        assert!(
            (center.0 - cx).abs() < 1e-6,
            "center.x should be {}, got {}",
            cx,
            center.0
        );
        assert!(
            (center.1 - cy).abs() < 1e-6,
            "center.y should be {}, got {}",
            cy,
            center.1
        );
        assert!(
            (radius - r).abs() < 1e-6,
            "radius should be {}, got {}",
            r,
            radius
        );
    }

    #[test]
    fn circumcircle_collinear_returns_none() {
        let p1 = (0.0, 0.0);
        let p2 = (1.0, 1.0);
        let p3 = (2.0, 2.0);

        assert!(
            circumcircle(p1, p2, p3).is_none(),
            "Collinear points should return None"
        );
    }

    #[test]
    fn points_fit_arc_on_circle() {
        // Generate points on a circle of radius 10 centered at (0, 0).
        let r = 10.0;
        let n = 10;
        let points: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let angle = PI * (i as f64) / ((n - 1) as f64); // 0 to PI
                (r * angle.cos(), r * angle.sin())
            })
            .collect();

        let result = points_fit_arc(&points, 0.05);
        assert!(result.is_some(), "Points on a circle should fit an arc");

        let (center, radius, _is_ccw) = result.unwrap();
        assert!(
            (center.0).abs() < 0.1,
            "center.x should be ~0, got {}",
            center.0
        );
        assert!(
            (center.1).abs() < 0.1,
            "center.y should be ~0, got {}",
            center.1
        );
        assert!(
            (radius - r).abs() < 0.1,
            "radius should be ~{}, got {}",
            r,
            radius
        );
    }

    #[test]
    fn points_fit_arc_with_outlier_returns_none() {
        // Points on a circle with one outlier.
        let r = 10.0;
        let n = 10;
        let mut points: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let angle = PI * (i as f64) / ((n - 1) as f64);
                (r * angle.cos(), r * angle.sin())
            })
            .collect();

        // Move one point way off the arc.
        points[5] = (0.0, 0.0);

        let result = points_fit_arc(&points, 0.05);
        assert!(
            result.is_none(),
            "Points with outlier should not fit an arc"
        );
    }

    #[test]
    fn fit_arcs_semicircle_produces_arc_command() {
        // Generate G1 moves forming a semicircle.
        let r = 10.0;
        let n = 20;
        let mut commands = Vec::new();

        // Add a starting position command (RapidMove) so fit_arcs knows the start point.
        let start_angle: f64 = 0.0;
        commands.push(GcodeCommand::RapidMove {
            x: Some(r * start_angle.cos()),
            y: Some(r * start_angle.sin()),
            z: None,
            f: Some(9000.0),
        });

        // G1 moves along the semicircle.
        for i in 1..=n {
            let angle = PI * (i as f64) / (n as f64);
            let e_per_move = 0.1;
            commands.push(GcodeCommand::LinearMove {
                x: Some(r * angle.cos()),
                y: Some(r * angle.sin()),
                z: None,
                e: Some(e_per_move),
                f: Some(1800.0),
            });
        }

        let result = fit_arcs(&commands, 0.1, 3);

        // Should contain at least one arc command.
        let arc_count = result
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    GcodeCommand::ArcMoveCW { .. } | GcodeCommand::ArcMoveCCW { .. }
                )
            })
            .count();

        assert!(
            arc_count > 0,
            "Semicircular G1 sequence should produce at least one arc command, got 0. Result has {} commands",
            result.len()
        );

        // Should have fewer commands than the original.
        assert!(
            result.len() < commands.len(),
            "Arc fitting should reduce command count: {} -> {}",
            commands.len(),
            result.len()
        );
    }

    #[test]
    fn fit_arcs_preserves_non_g1_commands() {
        let commands = vec![
            GcodeCommand::Comment("start".to_string()),
            GcodeCommand::LinearMove {
                x: Some(10.0),
                y: Some(0.0),
                z: None,
                e: Some(0.1),
                f: Some(1800.0),
            },
            GcodeCommand::Comment("mid".to_string()),
            GcodeCommand::LinearMove {
                x: Some(20.0),
                y: Some(0.0),
                z: None,
                e: Some(0.1),
                f: Some(1800.0),
            },
            GcodeCommand::Retract {
                distance: 0.8,
                feedrate: 2700.0,
            },
        ];

        let result = fit_arcs(&commands, 0.05, 3);

        // Comments and retraction should be preserved.
        let has_start_comment = result
            .iter()
            .any(|c| matches!(c, GcodeCommand::Comment(t) if t == "start"));
        let has_mid_comment = result
            .iter()
            .any(|c| matches!(c, GcodeCommand::Comment(t) if t == "mid"));
        let has_retract = result
            .iter()
            .any(|c| matches!(c, GcodeCommand::Retract { .. }));

        assert!(has_start_comment, "Start comment should be preserved");
        assert!(has_mid_comment, "Mid comment should be preserved");
        assert!(has_retract, "Retraction should be preserved");
    }

    #[test]
    fn fit_arcs_preserves_total_e_value() {
        // Generate G1 moves forming a semicircle with known E-values.
        let r = 10.0;
        let n = 20;
        let e_per_move = 0.1;
        let mut commands = Vec::new();

        // Start position.
        commands.push(GcodeCommand::RapidMove {
            x: Some(r),
            y: Some(0.0),
            z: None,
            f: Some(9000.0),
        });

        let mut total_original_e = 0.0;
        for i in 1..=n {
            let angle = PI * (i as f64) / (n as f64);
            commands.push(GcodeCommand::LinearMove {
                x: Some(r * angle.cos()),
                y: Some(r * angle.sin()),
                z: None,
                e: Some(e_per_move),
                f: Some(1800.0),
            });
            total_original_e += e_per_move;
        }

        let result = fit_arcs(&commands, 0.1, 3);

        // Sum E-values in the result.
        let total_result_e: f64 = result
            .iter()
            .map(|cmd| match cmd {
                GcodeCommand::LinearMove { e: Some(e), .. } => *e,
                GcodeCommand::ArcMoveCW { e: Some(e), .. } => *e,
                GcodeCommand::ArcMoveCCW { e: Some(e), .. } => *e,
                _ => 0.0,
            })
            .sum();

        assert!(
            (total_result_e - total_original_e).abs() < 1e-6,
            "Total E should be preserved: original={}, result={}",
            total_original_e,
            total_result_e
        );
    }

    #[test]
    fn fit_arcs_straight_lines_unchanged() {
        // Purely straight-line moves should not be converted to arcs.
        let commands = vec![
            GcodeCommand::RapidMove {
                x: Some(0.0),
                y: Some(0.0),
                z: None,
                f: Some(9000.0),
            },
            GcodeCommand::LinearMove {
                x: Some(10.0),
                y: Some(0.0),
                z: None,
                e: Some(0.5),
                f: Some(1800.0),
            },
            GcodeCommand::LinearMove {
                x: Some(20.0),
                y: Some(0.0),
                z: None,
                e: Some(0.5),
                f: Some(1800.0),
            },
            GcodeCommand::LinearMove {
                x: Some(30.0),
                y: Some(0.0),
                z: None,
                e: Some(0.5),
                f: Some(1800.0),
            },
            GcodeCommand::LinearMove {
                x: Some(40.0),
                y: Some(0.0),
                z: None,
                e: Some(0.5),
                f: Some(1800.0),
            },
        ];

        let result = fit_arcs(&commands, 0.05, 3);

        let arc_count = result
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    GcodeCommand::ArcMoveCW { .. } | GcodeCommand::ArcMoveCCW { .. }
                )
            })
            .count();

        assert_eq!(
            arc_count, 0,
            "Straight lines should not produce arc commands"
        );
    }

    #[test]
    fn fit_arcs_short_sequence_unchanged() {
        // Only 2 G1 moves -- below min_arc_points (3).
        let commands = vec![
            GcodeCommand::RapidMove {
                x: Some(0.0),
                y: Some(0.0),
                z: None,
                f: Some(9000.0),
            },
            GcodeCommand::LinearMove {
                x: Some(1.0),
                y: Some(0.0),
                z: None,
                e: Some(0.1),
                f: Some(1800.0),
            },
            GcodeCommand::LinearMove {
                x: Some(0.0),
                y: Some(1.0),
                z: None,
                e: Some(0.1),
                f: Some(1800.0),
            },
        ];

        let result = fit_arcs(&commands, 0.05, 3);

        let arc_count = result
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    GcodeCommand::ArcMoveCW { .. } | GcodeCommand::ArcMoveCCW { .. }
                )
            })
            .count();

        assert_eq!(
            arc_count, 0,
            "Sequence shorter than min_arc_points should not produce arcs"
        );
    }

    #[test]
    fn arc_length_quarter_circle() {
        let center = (0.0, 0.0);
        let start = (10.0, 0.0);
        let end = (0.0, 10.0);
        let is_ccw = true;

        let length = arc_length(center, start, end, is_ccw);
        let expected = 10.0 * PI / 2.0; // quarter circle, radius 10

        assert!(
            (length - expected).abs() < 1e-6,
            "Quarter circle arc length should be {}, got {}",
            expected,
            length
        );
    }

    #[test]
    fn arc_length_semicircle() {
        let center = (0.0, 0.0);
        let start = (5.0, 0.0);
        let end = (-5.0, 0.0);
        let is_ccw = true;

        let length = arc_length(center, start, end, is_ccw);
        let expected = 5.0 * PI; // half circle, radius 5

        assert!(
            (length - expected).abs() < 1e-6,
            "Semicircle arc length should be {}, got {}",
            expected,
            length
        );
    }
}
