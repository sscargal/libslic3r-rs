//! G-code generation from toolpaths.
//!
//! Converts the internal toolpath representation ([`LayerToolpath`]) into
//! [`GcodeCommand`] sequences ready for output via `GcodeWriter`.
//!
//! The main entry points are:
//! - [`generate_layer_gcode`]: Converts a single layer's toolpath to G-code commands
//! - [`generate_full_gcode`]: Converts all layers into a complete print body
//! - [`generate_plate_header`]: Generates a G-code header with per-object sections
//! - [`plate_checksum`]: Computes SHA-256 checksum of a plate configuration
//! - [`reproduce_command`]: Generates a CLI command to reproduce the plate
//!
//! Start/end G-code is NOT generated here -- that is handled by
//! `GcodeWriter` from slicecore-gcode-io.
//! This module produces only the print body commands.

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};
use slicecore_gcode_io::{format_acceleration, format_pressure_advance, GcodeCommand};

use crate::config::PrintConfig;
use crate::custom_gcode::substitute_placeholders;
use crate::engine::PlateSliceResult;
use crate::planner::{plan_bridge_fan, plan_fan, plan_retraction, plan_temperatures};
use crate::plate_config::{MeshSource, PlateConfig};
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
        let substituted =
            substitute_placeholders(before_layer, toolpath.layer_index, toolpath.z, total_layers);
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
        let substituted =
            substitute_placeholders(after_layer, toolpath.layer_index, toolpath.z, total_layers);
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
            let substituted =
                substitute_placeholders(gcode, toolpath.layer_index, toolpath.z, total_layers);
            for line in substituted.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    cmds.push(GcodeCommand::Raw(trimmed.to_string()));
                }
            }
        }
    }

    let retract_feedrate = config.retraction.speed * 60.0; // mm/s -> mm/min

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
                cmds.push(GcodeCommand::SetFanSpeed(config.cooling.fan_speed));
            }

            let label = feature_label(seg.feature);
            cmds.push(GcodeCommand::Comment(format!("TYPE:{label}")));

            // Emit acceleration commands at feature transitions when enabled.
            if config.acceleration_enabled {
                let (print_accel, travel_accel) = match seg.feature {
                    FeatureType::Travel => (config.accel.travel, config.accel.travel),
                    _ => (config.accel.print, config.accel.travel),
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
                        distance: config.retraction.length,
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
// Per-object plate header generation
// ---------------------------------------------------------------------------

/// A single field that differs between an object's config and the base config.
#[derive(Debug, Clone)]
pub struct OverrideDiffEntry {
    /// Dotted key path (e.g. `"layer_height"`).
    pub key: String,
    /// Base config value.
    pub base_value: serde_json::Value,
    /// Object's overridden value.
    pub override_value: serde_json::Value,
}

/// Computes the SHA-256 checksum of a [`PlateConfig`]'s TOML serialization.
///
/// Returns a string in the format `sha256:<hex-digest>`.
///
/// # Examples
///
/// ```
/// use slicecore_engine::plate_config::PlateConfig;
/// use slicecore_engine::gcode_gen::plate_checksum;
///
/// let plate = PlateConfig::default();
/// let checksum = plate_checksum(&plate);
/// assert!(checksum.starts_with("sha256:"));
/// assert!(checksum.len() > 10);
/// ```
pub fn plate_checksum(plate: &PlateConfig) -> String {
    let toml_str = toml::to_string(plate).unwrap_or_default();
    let hash = Sha256::digest(toml_str.as_bytes());
    format!("sha256:{hash:x}")
}

/// Generates a CLI command string that can reproduce the plate slice.
///
/// If `plate_file` is provided, uses `--plate` reference. Otherwise builds
/// a full command from the plate config with `--object` flags for overrides.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use slicecore_engine::plate_config::PlateConfig;
/// use slicecore_engine::gcode_gen::reproduce_command;
///
/// let plate = PlateConfig::default();
/// let cmd = reproduce_command(&plate, Some(Path::new("plate.toml")), Path::new("out.gcode"));
/// assert!(cmd.contains("--plate plate.toml"));
/// ```
pub fn reproduce_command(
    plate: &PlateConfig,
    plate_file: Option<&Path>,
    output_file: &Path,
) -> String {
    if let Some(plate_path) = plate_file {
        format!(
            "slicecore slice --plate {} --output {}",
            plate_path.display(),
            output_file.display()
        )
    } else {
        let mut cmd = String::from("slicecore slice");
        for obj in &plate.objects {
            if let MeshSource::File(path) = &obj.mesh_source {
                cmd.push_str(&format!(" {}", path.display()));
            }
        }
        cmd.push_str(&format!(" --output {}", output_file.display()));
        cmd
    }
}

/// Computes override diffs between an object's config and the base config.
///
/// Serializes both configs to JSON, flattens to dotted keys, and returns
/// entries where the values differ.
pub fn compute_override_diffs(
    base_config: &PrintConfig,
    object_config: &PrintConfig,
) -> Vec<OverrideDiffEntry> {
    let base_json = serde_json::to_value(base_config).unwrap_or_default();
    let obj_json = serde_json::to_value(object_config).unwrap_or_default();

    let mut base_flat = BTreeMap::new();
    let mut obj_flat = BTreeMap::new();
    flatten_json("", &base_json, &mut base_flat);
    flatten_json("", &obj_json, &mut obj_flat);

    let mut diffs = Vec::new();
    for (key, obj_val) in &obj_flat {
        if let Some(base_val) = base_flat.get(key) {
            if base_val != obj_val {
                diffs.push(OverrideDiffEntry {
                    key: key.clone(),
                    base_value: base_val.clone(),
                    override_value: obj_val.clone(),
                });
            }
        }
    }
    diffs
}

/// Recursively flattens a JSON value into dotted-key entries.
fn flatten_json(
    prefix: &str,
    value: &serde_json::Value,
    out: &mut BTreeMap<String, serde_json::Value>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let full_key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_json(&full_key, v, out);
            }
        }
        other => {
            if !prefix.is_empty() {
                out.insert(prefix.to_string(), other.clone());
            }
        }
    }
}

/// Formats a time in seconds as a human-readable `Xh Ym` or `Ym Zs` string.
fn format_time_short(seconds: f64) -> String {
    let total_secs = seconds as u64;
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    if hours > 0 {
        format!("{hours}h{mins:02}m")
    } else {
        format!("{mins}m{secs:02}s")
    }
}

/// Generates G-code comment lines for a plate header with per-object sections.
///
/// The header includes:
/// - Plate SHA-256 checksum
/// - Per-object sections with override diffs and statistics
/// - Reproduce command
///
/// Returns a list of [`GcodeCommand::Comment`] entries to prepend to the G-code.
pub fn generate_plate_header(
    plate: &PlateConfig,
    plate_result: &PlateSliceResult,
    base_config: &PrintConfig,
    resolved_configs: &[&PrintConfig],
    plate_file: Option<&Path>,
    output_file: &Path,
) -> Vec<GcodeCommand> {
    let mut cmds = Vec::new();

    let checksum = plate_checksum(plate);
    cmds.push(GcodeCommand::Comment(
        "=== SliceCore Plate Configuration ===".to_string(),
    ));
    cmds.push(GcodeCommand::Comment(format!("Plate checksum: {checksum}")));
    cmds.push(GcodeCommand::Comment(format!(
        "Objects: {}",
        plate_result.objects.len()
    )));
    cmds.push(GcodeCommand::Comment(String::new()));

    for (i, obj_result) in plate_result.objects.iter().enumerate() {
        let obj_config = resolved_configs.get(i).copied().unwrap_or(base_config);
        let diffs = compute_override_diffs(base_config, obj_config);

        cmds.push(GcodeCommand::Comment(format!(
            "--- Object {}: {} ---",
            i + 1,
            obj_result.name
        )));

        if diffs.is_empty() {
            cmds.push(GcodeCommand::Comment(
                "Overrides: none (uses base config)".to_string(),
            ));
        } else {
            // Find the override set name if any.
            if let Some(obj_cfg) = plate.objects.get(i) {
                if let Some(ref set_name) = obj_cfg.override_set {
                    cmds.push(GcodeCommand::Comment(format!("Override set: {set_name}")));
                }
            }
            cmds.push(GcodeCommand::Comment("Overrides from base:".to_string()));
            for diff in &diffs {
                cmds.push(GcodeCommand::Comment(format!(
                    "  {} = {} (base: {})",
                    diff.key, diff.override_value, diff.base_value
                )));
            }
        }

        cmds.push(GcodeCommand::Comment(format!(
            "Copies: {}",
            obj_result.copies
        )));
        cmds.push(GcodeCommand::Comment(format!(
            "Layers: {}",
            obj_result.result.layer_count
        )));
        cmds.push(GcodeCommand::Comment(format!(
            "Filament: {:.1}g ({:.1}m)",
            obj_result.result.filament_usage.weight_g, obj_result.result.filament_usage.length_m,
        )));
        cmds.push(GcodeCommand::Comment(format!(
            "Time: {}",
            format_time_short(obj_result.result.estimated_time_seconds)
        )));
        cmds.push(GcodeCommand::Comment(String::new()));
    }

    let repr_cmd = reproduce_command(plate, plate_file, output_file);
    cmds.push(GcodeCommand::Comment(
        "=== Reproduce Command ===".to_string(),
    ));
    cmds.push(GcodeCommand::Comment(repr_cmd));

    cmds
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolpath::{LayerToolpath, ToolpathSegment};
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

        let has_retract = cmds
            .iter()
            .any(|c| matches!(c, GcodeCommand::Retract { .. }));
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

        let has_retract = cmds
            .iter()
            .any(|c| matches!(c, GcodeCommand::Retract { .. }));
        assert!(
            !has_retract,
            "Short travel (0.5mm) should not insert Retract"
        );
    }

    #[test]
    fn z_hop_during_retraction() {
        let layer = travel_and_extrusion_layer(5.0);
        let mut config = PrintConfig::default();
        config.z_hop.height = 0.4;
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
        assert_eq!(
            cmds[0],
            GcodeCommand::SetRelativeExtrusion,
            "First command should be M83"
        );
        assert_eq!(
            cmds[1],
            GcodeCommand::ResetExtruder,
            "Second command should be G92 E0"
        );
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
        let has_fan = cmds
            .iter()
            .any(|c| matches!(c, GcodeCommand::FanOff | GcodeCommand::SetFanSpeed(_)));

        assert!(
            has_bed_temp,
            "Full G-code should contain bed temperature commands"
        );
        assert!(
            has_nozzle_temp,
            "Full G-code should contain nozzle temperature commands"
        );
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
            segments: vec![ToolpathSegment {
                start: Point2::new(0.0, 0.0),
                end: Point2::new(5.0, 0.0),
                feature: FeatureType::VariableWidthPerimeter,
                e_value: 0.15,
                feedrate: 2700.0,
                z: 0.4,
                extrusion_width: Some(0.35),
            }],
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
        config.custom_gcode.custom_gcode_per_z = vec![(1.0, "M600 ; filament change".to_string())];
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

    // ---------------------------------------------------------------------------
    // Plate header / checksum / reproduce command tests
    // ---------------------------------------------------------------------------

    #[test]
    fn plate_checksum_produces_sha256_prefix() {
        use crate::plate_config::PlateConfig;
        let plate = PlateConfig::default();
        let checksum = plate_checksum(&plate);
        assert!(
            checksum.starts_with("sha256:"),
            "Checksum should start with sha256:"
        );
        assert!(checksum.len() > 10, "Checksum should be non-trivially long");
    }

    #[test]
    fn reproduce_command_with_plate_file() {
        use crate::plate_config::PlateConfig;
        let plate = PlateConfig::default();
        let cmd = reproduce_command(
            &plate,
            Some(std::path::Path::new("plate.toml")),
            std::path::Path::new("output.gcode"),
        );
        assert!(cmd.contains("--plate plate.toml"));
        assert!(cmd.contains("--output output.gcode"));
    }

    #[test]
    fn reproduce_command_without_plate_file() {
        use crate::plate_config::{MeshSource, ObjectConfig, PlateConfig};
        let plate = PlateConfig {
            objects: vec![ObjectConfig {
                mesh_source: MeshSource::File(std::path::PathBuf::from("model.stl")),
                ..ObjectConfig::default()
            }],
            ..PlateConfig::default()
        };
        let cmd = reproduce_command(&plate, None, std::path::Path::new("output.gcode"));
        assert!(cmd.contains("model.stl"));
        assert!(cmd.contains("--output output.gcode"));
        assert!(!cmd.contains("--plate"));
    }

    #[test]
    fn compute_override_diffs_detects_changed_fields() {
        let base = PrintConfig::default();
        let mut modified = base.clone();
        modified.layer_height = 0.1;
        modified.wall_count = 5;

        let diffs = compute_override_diffs(&base, &modified);
        let keys: Vec<&str> = diffs.iter().map(|d| d.key.as_str()).collect();
        assert!(
            keys.contains(&"layer_height"),
            "Should detect layer_height diff"
        );
        assert!(
            keys.contains(&"wall_count"),
            "Should detect wall_count diff"
        );
    }

    #[test]
    fn compute_override_diffs_empty_for_identical_configs() {
        let config = PrintConfig::default();
        let diffs = compute_override_diffs(&config, &config);
        assert!(diffs.is_empty(), "Identical configs should have no diffs");
    }

    #[test]
    fn generate_plate_header_contains_object_sections() {
        use crate::engine::{ObjectSliceResult, PlateSliceResult, SliceResult};
        use crate::estimation::PrintTimeEstimate;
        use crate::filament::FilamentUsage;
        use crate::plate_config::PlateConfig;

        let plate = PlateConfig::default();
        let base_config = PrintConfig::default();
        let result = PlateSliceResult {
            objects: vec![ObjectSliceResult {
                name: "TestObject".to_string(),
                index: 0,
                result: SliceResult {
                    gcode: Vec::new(),
                    layer_count: 100,
                    estimated_time_seconds: 3600.0,
                    time_estimate: PrintTimeEstimate {
                        total_seconds: 3600.0,
                        move_time_seconds: 2800.0,
                        travel_time_seconds: 600.0,
                        retraction_count: 50,
                    },
                    filament_usage: FilamentUsage {
                        length_mm: 5000.0,
                        length_m: 5.0,
                        weight_g: 15.0,
                        cost: 0.38,
                    },
                    preview: None,
                    statistics: None,
                    travel_opt_stats: None,
                },
                copies: 1,
            }],
        };

        let configs = vec![&base_config];
        let cmds = generate_plate_header(
            &plate,
            &result,
            &base_config,
            &configs,
            None,
            std::path::Path::new("output.gcode"),
        );

        let text: String = cmds
            .iter()
            .filter_map(|c| {
                if let GcodeCommand::Comment(t) = c {
                    Some(t.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            text.contains("Plate checksum: sha256:"),
            "Header should contain checksum"
        );
        assert!(
            text.contains("Objects: 1"),
            "Header should contain object count"
        );
        assert!(
            text.contains("Object 1: TestObject"),
            "Header should contain object name"
        );
        assert!(
            text.contains("Reproduce Command"),
            "Header should contain reproduce section"
        );
    }

    #[test]
    fn plate_header_json_output_contains_per_object_data() {
        use crate::engine::{ObjectSliceResult, SliceResult};
        use crate::estimation::PrintTimeEstimate;
        use crate::filament::FilamentUsage;
        use crate::output::build_plate_output_json;

        let base_config = PrintConfig::default();
        let mut obj_config = base_config.clone();
        obj_config.layer_height = 0.1;

        let objects = vec![ObjectSliceResult {
            name: "Test".to_string(),
            index: 0,
            result: SliceResult {
                gcode: Vec::new(),
                layer_count: 200,
                estimated_time_seconds: 2700.0,
                time_estimate: PrintTimeEstimate {
                    total_seconds: 2700.0,
                    move_time_seconds: 2000.0,
                    travel_time_seconds: 500.0,
                    retraction_count: 30,
                },
                filament_usage: FilamentUsage {
                    length_mm: 4200.0,
                    length_m: 4.2,
                    weight_g: 12.5,
                    cost: 0.30,
                },
                preview: None,
                statistics: None,
                travel_opt_stats: None,
            },
            copies: 2,
        }];

        let configs = vec![&obj_config as &PrintConfig];
        let plate_json =
            build_plate_output_json("sha256:test123", &objects, &base_config, &configs);

        assert_eq!(plate_json.objects.len(), 1);
        assert_eq!(plate_json.objects[0].name, "Test");
        assert_eq!(plate_json.objects[0].copies, 2);
        assert!(plate_json.objects[0].overrides.contains_key("layer_height"));
        // Totals should account for copies
        assert!((plate_json.totals.filament_grams - 25.0).abs() < 0.01);
    }
}
