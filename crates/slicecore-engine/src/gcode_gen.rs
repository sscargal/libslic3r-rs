//! G-code generation from toolpaths.
//!
//! Converts the internal toolpath representation ([`LayerToolpath`]) into
//! [`GcodeCommand`] sequences ready for output via `GcodeWriter`.
//!
//! The main entry points are:
//! - [`generate_layer_gcode`]: Converts a single layer's toolpath to G-code commands
//! - [`generate_full_gcode`]: Converts all layers into a complete print body
//!
//! Start/end G-code is NOT generated here -- that is handled by
//! `GcodeWriter` from slicecore-gcode-io.
//! This module produces only the print body commands.

use slicecore_gcode_io::{format_acceleration, format_pressure_advance, GcodeCommand};

use crate::config::PrintConfig;
use crate::custom_gcode::substitute_placeholders;
use crate::planner::{plan_bridge_fan, plan_fan, plan_retraction, plan_temperatures};
use crate::toolpath::{FeatureType, LayerToolpath};

/// Generates G-code commands for a single layer's toolpath.
///
/// Converts each [`ToolpathSegment`](crate::toolpath::ToolpathSegment) in the
/// layer into the appropriate [`GcodeCommand`] sequence, handling:
/// - Feature type comments for readability
/// - Z-moves for layer changes
/// - Travel moves with optional retraction and Z-hop
/// - Extrusion moves with E-values and feedrates
///
/// The `retracted` state is tracked across layers via a mutable reference.
/// The `total_layers` parameter is used for custom G-code placeholder substitution.
pub fn generate_layer_gcode(
    toolpath: &LayerToolpath,
    config: &PrintConfig,
    retracted: &mut bool,
    total_layers: usize,
) -> Vec<GcodeCommand> {
    let mut cmds = Vec::new();

    // 0. Custom G-code: before layer change.
    let before_layer = config.custom_gcode.effective_before_layer();
    if !before_layer.is_empty() {
        let substituted = substitute_placeholders(
            before_layer,
            toolpath.layer_index,
            toolpath.z,
            total_layers,
        );
        for line in substituted.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                cmds.push(GcodeCommand::Raw(trimmed.to_string()));
            }
        }
    }

    // 1. Layer comment.
    cmds.push(GcodeCommand::Comment(format!(
        "Layer {} at Z={:.3}",
        toolpath.layer_index, toolpath.z
    )));

    // 2. Z-move to layer height.
    cmds.push(GcodeCommand::RapidMove {
        x: None,
        y: None,
        z: Some(toolpath.z),
        f: None,
    });

    // 2b. Custom G-code: after layer change.
    let after_layer = &config.custom_gcode.after_layer_change;
    if !after_layer.is_empty() {
        let substituted = substitute_placeholders(
            after_layer,
            toolpath.layer_index,
            toolpath.z,
            total_layers,
        );
        for line in substituted.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                cmds.push(GcodeCommand::Raw(trimmed.to_string()));
            }
        }
    }

    // 2c. Custom G-code: per-Z injection (within 0.001mm tolerance).
    for (z_height, gcode) in &config.custom_gcode.custom_gcode_per_z {
        if (toolpath.z - z_height).abs() < 0.001 {
            let substituted = substitute_placeholders(
                gcode,
                toolpath.layer_index,
                toolpath.z,
                total_layers,
            );
            for line in substituted.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    cmds.push(GcodeCommand::Raw(trimmed.to_string()));
                }
            }
        }
    }

    let retract_feedrate = config.retract_speed * 60.0; // mm/s -> mm/min

    // Track the last feature type to insert comments on transitions.
    let mut last_feature: Option<FeatureType> = None;

    // Track current Z to emit Z changes for scarf joint per-segment Z.
    let mut current_z = toolpath.z;

    // 3. Process each segment.
    for seg in &toolpath.segments {
        // Insert feature type comment when feature changes.
        if last_feature != Some(seg.feature) {
            // Handle bridge fan override: when entering Bridge, set max fan.
            // When leaving Bridge, restore normal fan.
            if seg.feature == FeatureType::Bridge {
                let bridge_fan_cmds = plan_bridge_fan(config.support.bridge.fan_speed);
                cmds.extend(bridge_fan_cmds);
            } else if last_feature == Some(FeatureType::Bridge) {
                // Restore normal fan speed when leaving bridge.
                cmds.push(GcodeCommand::SetFanSpeed(config.fan_speed));
            }

            let label = feature_label(seg.feature);
            cmds.push(GcodeCommand::Comment(format!("TYPE:{label}")));

            // Emit acceleration commands at feature transitions when enabled.
            if config.acceleration_enabled {
                let (print_accel, travel_accel) = match seg.feature {
                    FeatureType::Travel => {
                        (config.travel_acceleration, config.travel_acceleration)
                    }
                    _ => (config.print_acceleration, config.travel_acceleration),
                };
                let accel_str =
                    format_acceleration(config.gcode_dialect, print_accel, travel_accel);
                cmds.push(GcodeCommand::Raw(accel_str));
            }

            last_feature = Some(seg.feature);
        }

        match seg.feature {
            FeatureType::Travel => {
                // Check if retraction is needed.
                let retraction = plan_retraction(seg.length(), config);

                if let Some(ret) = &retraction {
                    // Retract if not already retracted.
                    if !*retracted {
                        cmds.push(GcodeCommand::Retract {
                            distance: ret.retract_length,
                            feedrate: retract_feedrate,
                        });
                        *retracted = true;
                    }

                    // Z-hop if configured.
                    if ret.z_hop > 0.0 {
                        cmds.push(GcodeCommand::RapidMove {
                            x: None,
                            y: None,
                            z: Some(seg.z + ret.z_hop),
                            f: None,
                        });
                    }
                }

                // Emit rapid move to travel destination.
                cmds.push(GcodeCommand::RapidMove {
                    x: Some(seg.end.x),
                    y: Some(seg.end.y),
                    z: None,
                    f: Some(seg.feedrate),
                });

                // If Z-hop was applied, move back down.
                if let Some(ret) = &retraction {
                    if ret.z_hop > 0.0 {
                        cmds.push(GcodeCommand::RapidMove {
                            x: None,
                            y: None,
                            z: Some(seg.z),
                            f: None,
                        });
                    }
                }

                // Unretract after travel if retracted.
                if *retracted {
                    if let Some(ret) = &retraction {
                        cmds.push(GcodeCommand::Unretract {
                            distance: ret.retract_length,
                            feedrate: retract_feedrate,
                        });
                        *retracted = false;
                    }
                }
            }

            // Extrusion features: perimeter, infill, skirt, brim.
            _ => {
                // If retracted from a previous travel, unretract first.
                if *retracted {
                    cmds.push(GcodeCommand::Unretract {
                        distance: config.retract_length,
                        feedrate: retract_feedrate,
                    });
                    *retracted = false;
                }

                // Include Z in the move if the segment's Z differs from the
                // current Z (used by scarf joint for per-segment Z ramps).
                let z_changed = (seg.z - current_z).abs() > 1e-6;
                let z_val = if z_changed {
                    current_z = seg.z;
                    Some(seg.z)
                } else {
                    None
                };

                // Apply per-feature flow multiplier.
                let flow_mult = config.per_feature_flow.get_multiplier(seg.feature);
                let adjusted_e = seg.e_value * flow_mult;

                // Emit linear extrusion move.
                cmds.push(GcodeCommand::LinearMove {
                    x: Some(seg.end.x),
                    y: Some(seg.end.y),
                    z: z_val,
                    e: Some(adjusted_e),
                    f: Some(seg.feedrate),
                });
            }
        }
    }

    cmds
}

/// Generates a complete print body from all layer toolpaths.
///
/// This produces:
/// 1. Relative extrusion mode (M83) and extruder reset (G92 E0)
/// 2. For each layer: temperature commands, fan commands, and toolpath G-code
///
/// Start/end G-code is NOT included -- that is handled by the
/// [`GcodeWriter`](slicecore_gcode_io::GcodeWriter).
pub fn generate_full_gcode(
    layer_toolpaths: &[LayerToolpath],
    config: &PrintConfig,
) -> Vec<GcodeCommand> {
    let mut cmds = Vec::new();

    // Preamble: relative extrusion mode and extruder reset.
    cmds.push(GcodeCommand::SetRelativeExtrusion);
    cmds.push(GcodeCommand::ResetExtruder);

    // Emit pressure advance at print start if configured.
    if config.pressure_advance > 0.0 {
        let pa_str = format_pressure_advance(config.gcode_dialect, config.pressure_advance);
        cmds.push(GcodeCommand::Raw(pa_str));
    }

    let mut retracted = false;
    let total_layers = layer_toolpaths.len();

    for toolpath in layer_toolpaths {
        // Temperature commands for this layer.
        let temp_cmds = plan_temperatures(toolpath.layer_index, config);
        cmds.extend(temp_cmds);

        // Fan commands for this layer.
        let layer_time = toolpath.estimated_time_seconds();
        let fan_cmds = plan_fan(toolpath.layer_index, layer_time, config);
        cmds.extend(fan_cmds);

        // Generate layer G-code.
        let layer_cmds = generate_layer_gcode(toolpath, config, &mut retracted, total_layers);
        cmds.extend(layer_cmds);
    }

    cmds
}

/// Returns a human-readable label for a feature type.
fn feature_label(feature: FeatureType) -> &'static str {
    match feature {
        FeatureType::OuterPerimeter => "Outer perimeter",
        FeatureType::InnerPerimeter => "Inner perimeter",
        FeatureType::SolidInfill => "Solid infill",
        FeatureType::SparseInfill => "Sparse infill",
        FeatureType::Skirt => "Skirt",
        FeatureType::Brim => "Brim",
        FeatureType::Travel => "Travel",
        FeatureType::GapFill => "Gap fill",
        FeatureType::VariableWidthPerimeter => "Variable width perimeter",
        FeatureType::Support => "Support",
        FeatureType::SupportInterface => "Support interface",
        FeatureType::Bridge => "Bridge",
        FeatureType::Ironing => "Ironing",
        FeatureType::PurgeTower => "Purge tower",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolpath::{ToolpathSegment, LayerToolpath};
    use slicecore_math::Point2;

    fn default_config() -> PrintConfig {
        PrintConfig::default()
    }

    /// Helper: creates a simple 2-segment extrusion layer.
    fn simple_extrusion_layer() -> LayerToolpath {
        LayerToolpath {
            layer_index: 1,
            z: 0.4,
            layer_height: 0.2,
            segments: vec![
                ToolpathSegment {
                    start: Point2::new(0.0, 0.0),
                    end: Point2::new(10.0, 0.0),
                    feature: FeatureType::OuterPerimeter,
                    e_value: 0.5,
                    feedrate: 2700.0,
                    z: 0.4,
                extrusion_width: None,
                },
                ToolpathSegment {
                    start: Point2::new(10.0, 0.0),
                    end: Point2::new(10.0, 10.0),
                    feature: FeatureType::OuterPerimeter,
                    e_value: 0.5,
                    feedrate: 2700.0,
                    z: 0.4,
                extrusion_width: None,
                },
            ],
        }
    }

    /// Helper: creates a layer with travel + extrusion.
    fn travel_and_extrusion_layer(travel_length: f64) -> LayerToolpath {
        LayerToolpath {
            layer_index: 1,
            z: 0.4,
            layer_height: 0.2,
            segments: vec![
                ToolpathSegment {
                    start: Point2::new(0.0, 0.0),
                    end: Point2::new(5.0, 0.0),
                    feature: FeatureType::OuterPerimeter,
                    e_value: 0.25,
                    feedrate: 2700.0,
                    z: 0.4,
                extrusion_width: None,
                },
                ToolpathSegment {
                    start: Point2::new(5.0, 0.0),
                    end: Point2::new(5.0 + travel_length, 0.0),
                    feature: FeatureType::Travel,
                    e_value: 0.0,
                    feedrate: 9000.0,
                    z: 0.4,
                extrusion_width: None,
                },
                ToolpathSegment {
                    start: Point2::new(5.0 + travel_length, 0.0),
                    end: Point2::new(5.0 + travel_length + 5.0, 0.0),
                    feature: FeatureType::OuterPerimeter,
                    e_value: 0.25,
                    feedrate: 2700.0,
                    z: 0.4,
                extrusion_width: None,
                },
            ],
        }
    }

    #[test]
    fn simple_extrusion_produces_g1_moves() {
        let layer = simple_extrusion_layer();
        let config = default_config();
        let mut retracted = false;

        let cmds = generate_layer_gcode(&layer, &config, &mut retracted, 10);

        // Should contain G1 moves with E values.
        let g1_moves: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, GcodeCommand::LinearMove { e: Some(_), .. }))
            .collect();

        assert_eq!(
            g1_moves.len(),
            2,
            "Should have 2 G1 extrusion moves, got {}",
            g1_moves.len()
        );
    }

    #[test]
    fn travel_produces_g0_move() {
        let layer = travel_and_extrusion_layer(0.5); // short travel, no retract
        let config = default_config();
        let mut retracted = false;

        let cmds = generate_layer_gcode(&layer, &config, &mut retracted, 10);

        let g0_moves: Vec<_> = cmds
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    GcodeCommand::RapidMove {
                        x: Some(_),
                        y: Some(_),
                        ..
                    }
                )
            })
            .collect();

        assert!(
            !g0_moves.is_empty(),
            "Should have at least one G0 travel move"
        );
    }

    #[test]
    fn long_travel_inserts_retraction() {
        // Travel of 5mm, default min_travel_for_retract is 1.5mm.
        let layer = travel_and_extrusion_layer(5.0);
        let config = default_config();
        let mut retracted = false;

        let cmds = generate_layer_gcode(&layer, &config, &mut retracted, 10);

        let has_retract = cmds.iter().any(|c| matches!(c, GcodeCommand::Retract { .. }));
        let has_unretract = cmds
            .iter()
            .any(|c| matches!(c, GcodeCommand::Unretract { .. }));

        assert!(has_retract, "Long travel should insert Retract command");
        assert!(has_unretract, "Long travel should insert Unretract command");
    }

    #[test]
    fn short_travel_skips_retraction() {
        // Travel of 0.5mm, well under default 1.5mm threshold.
        let layer = travel_and_extrusion_layer(0.5);
        let config = default_config();
        let mut retracted = false;

        let cmds = generate_layer_gcode(&layer, &config, &mut retracted, 10);

        let has_retract = cmds.iter().any(|c| matches!(c, GcodeCommand::Retract { .. }));
        assert!(
            !has_retract,
            "Short travel (0.5mm) should not insert Retract"
        );
    }

    #[test]
    fn z_hop_during_retraction() {
        let layer = travel_and_extrusion_layer(5.0);
        let config = PrintConfig {
            retract_z_hop: 0.4,
            ..Default::default()
        };
        let mut retracted = false;

        let cmds = generate_layer_gcode(&layer, &config, &mut retracted, 10);

        // Should have a Z-hop up (Z > layer Z) and then Z-hop down (back to layer Z).
        let z_hops: Vec<_> = cmds
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    GcodeCommand::RapidMove {
                        x: None,
                        y: None,
                        z: Some(_),
                        ..
                    }
                )
            })
            .collect();

        // Should have: initial Z-move, Z-hop up, Z-hop down = 3 Z-only rapid moves
        assert!(
            z_hops.len() >= 3,
            "Should have at least 3 Z-only rapid moves (layer + hop up + hop down), got {}",
            z_hops.len()
        );
    }

    #[test]
    fn full_gcode_starts_with_m83_and_g92() {
        let layers = vec![simple_extrusion_layer()];
        let config = default_config();

        let cmds = generate_full_gcode(&layers, &config);

        assert!(cmds.len() >= 2);
        assert_eq!(cmds[0], GcodeCommand::SetRelativeExtrusion, "First command should be M83");
        assert_eq!(cmds[1], GcodeCommand::ResetExtruder, "Second command should be G92 E0");
    }

    #[test]
    fn full_gcode_includes_temperature_and_fan() {
        let layer0 = LayerToolpath {
            layer_index: 0,
            z: 0.3,
            layer_height: 0.3,
            segments: vec![ToolpathSegment {
                start: Point2::new(0.0, 0.0),
                end: Point2::new(10.0, 0.0),
                feature: FeatureType::OuterPerimeter,
                e_value: 0.5,
                feedrate: 1200.0,
                z: 0.3,
            extrusion_width: None,
            }],
        };

        let layer1 = LayerToolpath {
            layer_index: 1,
            z: 0.5,
            layer_height: 0.2,
            segments: vec![ToolpathSegment {
                start: Point2::new(0.0, 0.0),
                end: Point2::new(10.0, 0.0),
                feature: FeatureType::OuterPerimeter,
                e_value: 0.5,
                feedrate: 2700.0,
                z: 0.5,
            extrusion_width: None,
            }],
        };

        let config = default_config();
        let cmds = generate_full_gcode(&[layer0, layer1], &config);

        // Should contain temperature commands.
        let has_bed_temp = cmds
            .iter()
            .any(|c| matches!(c, GcodeCommand::SetBedTemp { .. }));
        let has_nozzle_temp = cmds
            .iter()
            .any(|c| matches!(c, GcodeCommand::SetExtruderTemp { .. }));
        let has_fan = cmds.iter().any(|c| {
            matches!(c, GcodeCommand::FanOff | GcodeCommand::SetFanSpeed(_))
        });

        assert!(has_bed_temp, "Full G-code should contain bed temperature commands");
        assert!(has_nozzle_temp, "Full G-code should contain nozzle temperature commands");
        assert!(has_fan, "Full G-code should contain fan commands");
    }

    #[test]
    fn feature_comments_included() {
        let layer = simple_extrusion_layer();
        let config = default_config();
        let mut retracted = false;

        let cmds = generate_layer_gcode(&layer, &config, &mut retracted, 10);

        let has_feature_comment = cmds.iter().any(|c| {
            if let GcodeCommand::Comment(text) = c {
                text.starts_with("TYPE:")
            } else {
                false
            }
        });

        assert!(
            has_feature_comment,
            "Should include feature type comments (TYPE:...)"
        );
    }

    #[test]
    fn layer_comment_included() {
        let layer = simple_extrusion_layer();
        let config = default_config();
        let mut retracted = false;

        let cmds = generate_layer_gcode(&layer, &config, &mut retracted, 10);

        let has_layer_comment = cmds.iter().any(|c| {
            if let GcodeCommand::Comment(text) = c {
                text.starts_with("Layer ")
            } else {
                false
            }
        });

        assert!(has_layer_comment, "Should include layer comment");
    }

    #[test]
    fn retracted_state_persists_across_layers() {
        // If a layer ends in a retracted state, the next layer should unretract.
        // layer1 would end in a retracted state in a real scenario.
        // We simulate this by starting with retracted = true.

        let layer2 = LayerToolpath {
            layer_index: 2,
            z: 0.6,
            layer_height: 0.2,
            segments: vec![ToolpathSegment {
                start: Point2::new(50.0, 0.0),
                end: Point2::new(60.0, 0.0),
                feature: FeatureType::OuterPerimeter,
                e_value: 0.5,
                feedrate: 2700.0,
                z: 0.6,
            extrusion_width: None,
            }],
        };

        let config = default_config();
        let mut retracted = true; // Simulate being in retracted state.

        let cmds = generate_layer_gcode(&layer2, &config, &mut retracted, 10);

        // The extrusion move should trigger an unretract first.
        let has_unretract = cmds
            .iter()
            .any(|c| matches!(c, GcodeCommand::Unretract { .. }));

        assert!(
            has_unretract,
            "Extrusion after retracted state should emit Unretract"
        );
        assert!(!retracted, "Should no longer be retracted after extrusion");
    }

    #[test]
    fn empty_layer_produces_minimal_commands() {
        let layer = LayerToolpath {
            layer_index: 5,
            z: 1.2,
            layer_height: 0.2,
            segments: Vec::new(),
        };
        let config = default_config();
        let mut retracted = false;

        let cmds = generate_layer_gcode(&layer, &config, &mut retracted, 10);

        // Should have at least the layer comment and Z-move.
        assert!(
            cmds.len() >= 2,
            "Empty layer should still have comment and Z-move"
        );
        assert!(matches!(&cmds[0], GcodeCommand::Comment(text) if text.starts_with("Layer")));
    }

    #[test]
    fn scarf_z_changes_produce_z_in_g1_moves() {
        // Create a layer with varying Z values (simulating scarf joint).
        let layer = LayerToolpath {
            layer_index: 1,
            z: 0.4,
            layer_height: 0.2,
            segments: vec![
                ToolpathSegment {
                    start: Point2::new(0.0, 0.0),
                    end: Point2::new(5.0, 0.0),
                    feature: FeatureType::OuterPerimeter,
                    e_value: 0.25,
                    feedrate: 2700.0,
                    z: 0.30, // Below layer Z (scarf ramp).
                    extrusion_width: None,
                },
                ToolpathSegment {
                    start: Point2::new(5.0, 0.0),
                    end: Point2::new(10.0, 0.0),
                    feature: FeatureType::OuterPerimeter,
                    e_value: 0.25,
                    feedrate: 2700.0,
                    z: 0.35, // Rising Z.
                    extrusion_width: None,
                },
                ToolpathSegment {
                    start: Point2::new(10.0, 0.0),
                    end: Point2::new(15.0, 0.0),
                    feature: FeatureType::OuterPerimeter,
                    e_value: 0.25,
                    feedrate: 2700.0,
                    z: 0.40, // At layer Z.
                    extrusion_width: None,
                },
            ],
        };

        let config = default_config();
        let mut retracted = false;

        let cmds = generate_layer_gcode(&layer, &config, &mut retracted, 10);

        // Count G1 moves that include a Z value.
        let g1_with_z: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, GcodeCommand::LinearMove { z: Some(_), .. }))
            .collect();

        // The first segment has Z=0.30 which differs from layer Z=0.4,
        // and the second has Z=0.35 which differs from 0.30.
        // So we should see Z values in G1 commands.
        assert!(
            g1_with_z.len() >= 2,
            "Scarf Z changes should produce G1 moves with Z values, got {}",
            g1_with_z.len()
        );
    }

    #[test]
    fn uniform_z_segments_omit_z_in_g1() {
        // All segments at the same Z as the layer -- no Z in G1 moves.
        let layer = simple_extrusion_layer();
        let config = default_config();
        let mut retracted = false;

        let cmds = generate_layer_gcode(&layer, &config, &mut retracted, 10);

        let g1_with_z = cmds
            .iter()
            .filter(|c| matches!(c, GcodeCommand::LinearMove { z: Some(_), .. }))
            .count();

        assert_eq!(
            g1_with_z, 0,
            "Uniform Z segments should not include Z in G1 moves"
        );
    }

    #[test]
    fn variable_width_perimeter_produces_correct_comment() {
        let layer = LayerToolpath {
            layer_index: 1,
            z: 0.4,
            layer_height: 0.2,
            segments: vec![
                ToolpathSegment {
                    start: Point2::new(0.0, 0.0),
                    end: Point2::new(5.0, 0.0),
                    feature: FeatureType::VariableWidthPerimeter,
                    e_value: 0.15,
                    feedrate: 2700.0,
                    z: 0.4,
                    extrusion_width: Some(0.35),
                },
            ],
        };
        let config = default_config();
        let mut retracted = false;

        let cmds = generate_layer_gcode(&layer, &config, &mut retracted, 10);

        let has_vw_comment = cmds.iter().any(|c| {
            if let GcodeCommand::Comment(text) = c {
                text.contains("Variable width perimeter")
            } else {
                false
            }
        });

        assert!(
            has_vw_comment,
            "Variable-width perimeter feature should produce TYPE comment"
        );
    }

    #[test]
    fn per_feature_flow_reduces_outer_perimeter_e_value() {
        let layer = simple_extrusion_layer(); // OuterPerimeter with e_value=0.5
        let mut config = default_config();
        config.per_feature_flow.outer_perimeter = 0.95;
        let mut retracted = false;

        let cmds = generate_layer_gcode(&layer, &config, &mut retracted, 10);

        // Find the G1 moves and verify E-values are scaled by 0.95.
        let g1_e_values: Vec<f64> = cmds
            .iter()
            .filter_map(|c| {
                if let GcodeCommand::LinearMove { e: Some(e), .. } = c {
                    Some(*e)
                } else {
                    None
                }
            })
            .collect();

        assert!(!g1_e_values.is_empty(), "Should have G1 extrusion moves");
        for e in &g1_e_values {
            // Original e_value is 0.5, scaled by 0.95 = 0.475.
            assert!(
                (*e - 0.475).abs() < 1e-9,
                "Outer perimeter E should be 0.5 * 0.95 = 0.475, got {}",
                e
            );
        }
    }

    #[test]
    fn default_per_feature_flow_does_not_change_e_values() {
        let layer = simple_extrusion_layer();
        let config = default_config(); // All flow multipliers are 1.0.
        let mut retracted = false;

        let cmds = generate_layer_gcode(&layer, &config, &mut retracted, 10);

        let g1_e_values: Vec<f64> = cmds
            .iter()
            .filter_map(|c| {
                if let GcodeCommand::LinearMove { e: Some(e), .. } = c {
                    Some(*e)
                } else {
                    None
                }
            })
            .collect();

        for e in &g1_e_values {
            assert!(
                (*e - 0.5).abs() < 1e-9,
                "Default flow (1.0) should not change E-value 0.5, got {}",
                e
            );
        }
    }

    #[test]
    fn custom_gcode_placeholder_substitution_in_layer() {
        let layer = simple_extrusion_layer(); // layer_index=1, z=0.4
        let mut config = default_config();
        config.custom_gcode.after_layer_change = "M117 L{layer_num} Z{layer_z}".to_string();
        let mut retracted = false;

        let cmds = generate_layer_gcode(&layer, &config, &mut retracted, 100);

        let has_raw = cmds.iter().any(|c| {
            if let GcodeCommand::Raw(text) = c {
                text.contains("M117 L1 Z0.400")
            } else {
                false
            }
        });

        assert!(
            has_raw,
            "Custom G-code after layer change should be injected with substituted placeholders"
        );
    }

    #[test]
    fn custom_gcode_per_z_injection() {
        let layer = LayerToolpath {
            layer_index: 5,
            z: 1.0,
            layer_height: 0.2,
            segments: vec![ToolpathSegment {
                start: Point2::new(0.0, 0.0),
                end: Point2::new(10.0, 0.0),
                feature: FeatureType::OuterPerimeter,
                e_value: 0.5,
                feedrate: 2700.0,
                z: 1.0,
                extrusion_width: None,
            }],
        };
        let mut config = default_config();
        config.custom_gcode.custom_gcode_per_z = vec![
            (1.0, "M600 ; filament change".to_string()),
        ];
        let mut retracted = false;

        let cmds = generate_layer_gcode(&layer, &config, &mut retracted, 20);

        let has_filament_change = cmds.iter().any(|c| {
            if let GcodeCommand::Raw(text) = c {
                text.contains("M600")
            } else {
                false
            }
        });

        assert!(
            has_filament_change,
            "Custom G-code at matching Z height should be injected"
        );
    }

    #[test]
    fn ironing_feature_produces_correct_comment() {
        let layer = LayerToolpath {
            layer_index: 1,
            z: 0.4,
            layer_height: 0.2,
            segments: vec![ToolpathSegment {
                start: Point2::new(0.0, 0.0),
                end: Point2::new(10.0, 0.0),
                feature: FeatureType::Ironing,
                e_value: 0.05,
                feedrate: 900.0,
                z: 0.4,
                extrusion_width: None,
            }],
        };
        let config = default_config();
        let mut retracted = false;

        let cmds = generate_layer_gcode(&layer, &config, &mut retracted, 10);

        let has_ironing_comment = cmds.iter().any(|c| {
            if let GcodeCommand::Comment(text) = c {
                text.contains("Ironing")
            } else {
                false
            }
        });

        assert!(
            has_ironing_comment,
            "Ironing feature should produce TYPE:Ironing comment"
        );
    }

    #[test]
    fn purge_tower_feature_produces_correct_comment() {
        let layer = LayerToolpath {
            layer_index: 1,
            z: 0.4,
            layer_height: 0.2,
            segments: vec![ToolpathSegment {
                start: Point2::new(0.0, 0.0),
                end: Point2::new(10.0, 0.0),
                feature: FeatureType::PurgeTower,
                e_value: 0.5,
                feedrate: 2700.0,
                z: 0.4,
                extrusion_width: None,
            }],
        };
        let config = default_config();
        let mut retracted = false;

        let cmds = generate_layer_gcode(&layer, &config, &mut retracted, 10);

        let has_purge_comment = cmds.iter().any(|c| {
            if let GcodeCommand::Comment(text) = c {
                text.contains("Purge tower")
            } else {
                false
            }
        });

        assert!(
            has_purge_comment,
            "PurgeTower feature should produce TYPE:Purge tower comment"
        );
    }
}
