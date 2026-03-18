//! Multi-material tool change sequences and purge tower generation.
//!
//! Implements MMU (multi-material unit) support:
//! - **Tool change sequences**: retract-park-change-prime flow for switching
//!   between extruders during multi-color or multi-material prints.
//! - **Purge tower**: rectangular waste tower maintained on every layer,
//!   dense on tool-change layers (actual purge) and sparse on non-change
//!   layers (structural integrity).
//! - **Tool assignment**: simple region-to-tool mapping using modifier meshes.

use serde::{Deserialize, Serialize};
use slicecore_gcode_io::GcodeCommand;
use slicecore_geo::ValidPolygon;

use crate::config::{MultiMaterialConfig, PrintConfig};
use crate::modifier::ModifierMesh;
use crate::toolpath::FeatureType;

/// A complete tool change command sequence.
///
/// Contains the ordered list of G-code commands needed to switch from
/// one tool to another: retract, travel to purge tower, tool change
/// command (T-code), prime, and wipe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChangeSequence {
    /// The full ordered sequence of G-code commands for this tool change.
    #[serde(skip)]
    pub commands: Vec<GcodeCommand>,
}

/// A single purge tower layer.
///
/// On tool-change layers the tower is dense (full purge infill).
/// On non-change layers the tower is sparse (perimeters only for
/// structural integrity).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurgeTowerLayer {
    /// G-code commands for this purge tower layer.
    #[serde(skip)]
    pub commands: Vec<GcodeCommand>,
    /// Whether this is a dense (tool-change) or sparse (maintenance) layer.
    pub is_dense: bool,
}

/// Generates a tool change sequence from one tool to another.
///
/// The sequence follows the retract-park-change-prime flow:
/// 1. Retract current tool filament
/// 2. Travel to purge tower position (parking location)
/// 3. Emit T-code to switch to the new tool
/// 4. Prime new tool (unretract + purge extrusion)
/// 5. Wipe move across purge tower to clean the nozzle
///
/// # Parameters
///
/// - `from_tool`: Currently active tool index.
/// - `to_tool`: Target tool index.
/// - `config`: Multi-material configuration.
/// - `print_config`: Base print configuration (for retraction/speed params).
pub fn generate_tool_change(
    from_tool: u8,
    to_tool: u8,
    config: &MultiMaterialConfig,
    print_config: &PrintConfig,
) -> ToolChangeSequence {
    let mut commands = Vec::new();

    // 1. Retract current tool.
    let retract_len = if (from_tool as usize) < config.tools.len() {
        config.tools[from_tool as usize].retract_length
    } else {
        print_config.retraction.length
    };
    let retract_speed = if (from_tool as usize) < config.tools.len() {
        config.tools[from_tool as usize].retract_speed * 60.0 // mm/s -> mm/min
    } else {
        print_config.retraction.speed * 60.0
    };

    commands.push(GcodeCommand::Comment(format!(
        "Tool change: T{} -> T{}",
        from_tool, to_tool
    )));
    commands.push(GcodeCommand::Retract {
        distance: retract_len,
        feedrate: retract_speed,
    });

    // 2. Travel to purge tower position.
    let tower_x = config.purge_tower_position[0];
    let tower_y = config.purge_tower_position[1];
    let travel_speed = print_config.speeds.travel * 60.0;

    commands.push(GcodeCommand::RapidMove {
        x: Some(tower_x),
        y: Some(tower_y),
        z: None,
        f: Some(travel_speed),
    });

    // 3. Emit T-code (tool change command).
    commands.push(GcodeCommand::ToolChange(to_tool));

    // 4. Prime new tool (unretract + purge).
    let prime_len = if (to_tool as usize) < config.tools.len() {
        config.tools[to_tool as usize].retract_length
    } else {
        print_config.retraction.length
    };
    let prime_speed = if (to_tool as usize) < config.tools.len() {
        config.tools[to_tool as usize].retract_speed * 60.0
    } else {
        print_config.retraction.speed * 60.0
    };

    commands.push(GcodeCommand::Unretract {
        distance: prime_len,
        feedrate: prime_speed,
    });

    // 5. Wipe move across the purge tower.
    let wipe_end_x = tower_x + config.wipe_length;
    let wipe_speed = print_config.speeds.perimeter * 60.0;

    commands.push(GcodeCommand::LinearMove {
        x: Some(wipe_end_x),
        y: Some(tower_y),
        z: None,
        e: Some(0.0), // Wipe with no extrusion
        f: Some(wipe_speed),
    });

    ToolChangeSequence { commands }
}

/// Generates a purge tower layer.
///
/// On tool-change layers (`has_tool_change=true`), generates dense perimeters
/// and infill to provide the purge volume. On non-change layers, generates
/// sparse perimeters (2 loops only) to maintain tower structural integrity.
///
/// The purge tower is a simple rectangular region at the configured position.
///
/// # Parameters
///
/// - `layer_z`: Z height of this layer in mm.
/// - `layer_height`: Height of this layer in mm.
/// - `config`: Multi-material configuration.
/// - `has_tool_change`: Whether this layer has a tool change.
/// - `nozzle_diameter`: Nozzle diameter in mm.
pub fn generate_purge_tower_layer(
    layer_z: f64,
    layer_height: f64,
    config: &MultiMaterialConfig,
    has_tool_change: bool,
    nozzle_diameter: f64,
) -> PurgeTowerLayer {
    let mut commands = Vec::new();
    let tower_x = config.purge_tower_position[0];
    let tower_y = config.purge_tower_position[1];
    let tower_w = config.purge_tower_width;
    let extrusion_width = nozzle_diameter * 1.1;

    // Feature type comment for slicer visualization.
    commands.push(GcodeCommand::Comment(format!(
        "TYPE: {}",
        if has_tool_change {
            "PurgeTower (dense)"
        } else {
            "PurgeTower (sparse)"
        }
    )));

    // Move to tower start position.
    commands.push(GcodeCommand::RapidMove {
        x: Some(tower_x),
        y: Some(tower_y),
        z: Some(layer_z),
        f: None,
    });

    if has_tool_change {
        // Dense tower: perimeters + infill filling the purge volume.
        // Compute how many lines we need to purge the configured volume.
        // Volume per line = extrusion_width * layer_height * line_length
        // Total purge volume = config.purge_volume mm^3
        let line_length = tower_w;
        let volume_per_line = extrusion_width * layer_height * line_length;
        let num_lines = if volume_per_line > 0.0 {
            (config.purge_volume / volume_per_line).ceil() as usize
        } else {
            10
        };
        let line_spacing = if num_lines > 1 {
            tower_w / num_lines as f64
        } else {
            extrusion_width
        };

        // Cross-section area for E-value computation.
        let cross_section = extrusion_cross_section(extrusion_width, layer_height);
        let filament_area = std::f64::consts::PI * (1.75 / 2.0) * (1.75 / 2.0);

        // Outer perimeter of tower.
        let corners = [
            (tower_x, tower_y),
            (tower_x + tower_w, tower_y),
            (tower_x + tower_w, tower_y + tower_w),
            (tower_x, tower_y + tower_w),
        ];
        let perim_speed = 1800.0; // 30mm/s for tower perimeters
        for i in 0..4 {
            let (nx, ny) = corners[(i + 1) % 4];
            let (cx, cy) = corners[i];
            let seg_len = ((nx - cx).powi(2) + (ny - cy).powi(2)).sqrt();
            let e = seg_len * cross_section / filament_area;
            commands.push(GcodeCommand::LinearMove {
                x: Some(nx),
                y: Some(ny),
                z: None,
                e: Some(e),
                f: Some(perim_speed),
            });
        }

        // Dense infill lines.
        let infill_speed = 2400.0; // 40mm/s for tower infill
        for i in 0..num_lines {
            let y_offset = (i as f64 + 0.5) * line_spacing;
            let y_pos = tower_y + y_offset.min(tower_w);

            let (sx, ex) = if i % 2 == 0 {
                (
                    tower_x + extrusion_width,
                    tower_x + tower_w - extrusion_width,
                )
            } else {
                (
                    tower_x + tower_w - extrusion_width,
                    tower_x + extrusion_width,
                )
            };

            let seg_len = (ex - sx).abs();
            let e = seg_len * cross_section / filament_area;

            // Travel to line start.
            commands.push(GcodeCommand::RapidMove {
                x: Some(sx),
                y: Some(y_pos),
                z: None,
                f: None,
            });
            // Extrude the infill line.
            commands.push(GcodeCommand::LinearMove {
                x: Some(ex),
                y: Some(y_pos),
                z: None,
                e: Some(e),
                f: Some(infill_speed),
            });
        }
    } else {
        // Sparse tower: 2 perimeter loops only (no infill).
        let cross_section = extrusion_cross_section(extrusion_width, layer_height);
        let filament_area = std::f64::consts::PI * (1.75 / 2.0) * (1.75 / 2.0);
        let perim_speed = 1800.0;

        for loop_idx in 0..2u32 {
            let inset = (loop_idx as f64 + 0.5) * extrusion_width;
            let x0 = tower_x + inset;
            let y0 = tower_y + inset;
            let x1 = tower_x + tower_w - inset;
            let y1 = tower_y + tower_w - inset;

            if x1 <= x0 || y1 <= y0 {
                break;
            }

            let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
            // Travel to first corner.
            commands.push(GcodeCommand::RapidMove {
                x: Some(corners[0].0),
                y: Some(corners[0].1),
                z: None,
                f: None,
            });

            for i in 0..4 {
                let (nx, ny) = corners[(i + 1) % 4];
                let (cx, cy) = corners[i];
                let seg_len = ((nx - cx).powi(2) + (ny - cy).powi(2)).sqrt();
                let e = seg_len * cross_section / filament_area;
                commands.push(GcodeCommand::LinearMove {
                    x: Some(nx),
                    y: Some(ny),
                    z: None,
                    e: Some(e),
                    f: Some(perim_speed),
                });
            }
        }
    }

    PurgeTowerLayer {
        commands,
        is_dense: has_tool_change,
    }
}

/// Assigns tool indices per contour region based on modifier meshes.
///
/// For each contour group, checks if it falls within a modifier mesh
/// assigned to a specific tool. Regions not covered by any modifier
/// default to tool 0.
///
/// # Parameters
///
/// - `contours`: Contour groups per region.
/// - `modifier_tools`: Pairs of (modifier mesh, tool index).
///
/// # Returns
///
/// Tool index per contour region (same length as `contours`).
pub fn assign_tools_per_region(
    contours: &[Vec<ValidPolygon>],
    modifier_tools: &[(ModifierMesh, u8)],
) -> Vec<u8> {
    if modifier_tools.is_empty() {
        return vec![0u8; contours.len()];
    }

    contours
        .iter()
        .map(|region_contours| {
            // Check each modifier to see if this region overlaps.
            for (modifier, tool_idx) in modifier_tools {
                // Slice modifier at a representative Z (use 0 for simple check).
                // In practice, the engine would slice at the actual layer Z.
                if let Some(region) = crate::modifier::slice_modifier(modifier, 1.0) {
                    // Check if any model contour intersects with the modifier contour.
                    let intersection =
                        slicecore_geo::polygon_intersection(region_contours, &region.contours)
                            .unwrap_or_default();
                    if !intersection.is_empty() {
                        return *tool_idx;
                    }
                }
            }
            0u8 // Default tool
        })
        .collect()
}

/// Computes the extrusion cross-section area using the Slic3r model.
///
/// Cross-section = (width - height) * height + PI * (height / 2)^2
fn extrusion_cross_section(width: f64, height: f64) -> f64 {
    (width - height) * height + std::f64::consts::PI * (height / 2.0).powi(2)
}

/// Feature type accessor for purge tower segments (used by engine integration).
pub fn purge_tower_feature() -> FeatureType {
    FeatureType::PurgeTower
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{MultiMaterialConfig, PrintConfig, ToolConfig};

    #[test]
    fn generate_tool_change_produces_tool_change_command() {
        let config = MultiMaterialConfig {
            enabled: true,
            tool_count: 2,
            tools: vec![ToolConfig::default(), ToolConfig::default()],
            ..Default::default()
        };
        let print_config = PrintConfig::default();

        let seq = generate_tool_change(0, 1, &config, &print_config);

        // Should contain a ToolChange command.
        let has_tool_change = seq
            .commands
            .iter()
            .any(|cmd| matches!(cmd, GcodeCommand::ToolChange(1)));
        assert!(
            has_tool_change,
            "Tool change sequence should contain ToolChange(1)"
        );
    }

    #[test]
    fn tool_change_sequence_includes_retract_travel_tcode_prime() {
        let config = MultiMaterialConfig {
            enabled: true,
            tool_count: 2,
            tools: vec![ToolConfig::default(), ToolConfig::default()],
            ..Default::default()
        };
        let print_config = PrintConfig::default();

        let seq = generate_tool_change(0, 1, &config, &print_config);

        let has_retract = seq
            .commands
            .iter()
            .any(|cmd| matches!(cmd, GcodeCommand::Retract { .. }));
        let has_travel = seq
            .commands
            .iter()
            .any(|cmd| matches!(cmd, GcodeCommand::RapidMove { .. }));
        let has_tcode = seq
            .commands
            .iter()
            .any(|cmd| matches!(cmd, GcodeCommand::ToolChange(_)));
        let has_prime = seq
            .commands
            .iter()
            .any(|cmd| matches!(cmd, GcodeCommand::Unretract { .. }));

        assert!(has_retract, "Should have retract command");
        assert!(has_travel, "Should have travel to purge tower");
        assert!(has_tcode, "Should have T-code");
        assert!(has_prime, "Should have prime (unretract)");
    }

    #[test]
    fn tool_change_correct_tool_number() {
        let config = MultiMaterialConfig {
            enabled: true,
            tool_count: 4,
            tools: vec![
                ToolConfig::default(),
                ToolConfig::default(),
                ToolConfig::default(),
                ToolConfig::default(),
            ],
            ..Default::default()
        };
        let print_config = PrintConfig::default();

        let seq = generate_tool_change(1, 3, &config, &print_config);

        let tool_cmd = seq
            .commands
            .iter()
            .find(|cmd| matches!(cmd, GcodeCommand::ToolChange(_)));
        assert_eq!(
            tool_cmd,
            Some(&GcodeCommand::ToolChange(3)),
            "Should switch to tool 3"
        );
    }

    #[test]
    fn purge_tower_layer_with_tool_change_is_dense() {
        let config = MultiMaterialConfig {
            enabled: true,
            tool_count: 2,
            purge_tower_width: 15.0,
            purge_volume: 70.0,
            ..Default::default()
        };

        let layer = generate_purge_tower_layer(0.4, 0.2, &config, true, 0.4);

        assert!(layer.is_dense, "Tool-change layer should be dense");
        // Dense layer should have infill lines (LinearMove commands).
        let has_linear = layer
            .commands
            .iter()
            .any(|cmd| matches!(cmd, GcodeCommand::LinearMove { .. }));
        assert!(has_linear, "Dense tower should have infill extrusion");
    }

    #[test]
    fn purge_tower_layer_without_tool_change_is_sparse() {
        let config = MultiMaterialConfig {
            enabled: true,
            tool_count: 2,
            purge_tower_width: 15.0,
            ..Default::default()
        };

        let layer = generate_purge_tower_layer(0.4, 0.2, &config, false, 0.4);

        assert!(!layer.is_dense, "Non-change layer should be sparse");
        // Sparse layer should have perimeter extrusion but fewer commands.
        let linear_count = layer
            .commands
            .iter()
            .filter(|cmd| matches!(cmd, GcodeCommand::LinearMove { .. }))
            .count();
        // Sparse has 2 loops of 4 sides = 8 extrusion moves.
        assert!(
            linear_count <= 16,
            "Sparse tower should have perimeters only, got {} linear moves",
            linear_count
        );
    }

    #[test]
    fn multi_material_config_default_disabled() {
        let config = MultiMaterialConfig::default();
        assert!(!config.enabled, "Multi-material should default to disabled");
        assert_eq!(config.tool_count, 1);
        assert!(config.tools.is_empty());
    }

    #[test]
    fn multi_material_toml_deserialization() {
        let toml = r#"
[multi_material]
enabled = true
tool_count = 4
purge_tower_width = 20.0
purge_volume = 100.0
wipe_length = 3.0
purge_tower_position = [180.0, 180.0]
"#;
        let config = PrintConfig::from_toml(toml).unwrap();
        assert!(config.multi_material.enabled);
        assert_eq!(config.multi_material.tool_count, 4);
        assert!((config.multi_material.purge_tower_width - 20.0).abs() < 1e-9);
        assert!((config.multi_material.purge_volume - 100.0).abs() < 1e-9);
        assert!((config.multi_material.wipe_length - 3.0).abs() < 1e-9);
        assert!((config.multi_material.purge_tower_position[0] - 180.0).abs() < 1e-9);
    }

    #[test]
    fn tool_config_defaults() {
        let tc = ToolConfig::default();
        assert!((tc.nozzle_temp - 200.0).abs() < 1e-9);
        assert!((tc.retract_length - 0.8).abs() < 1e-9);
        assert!((tc.retract_speed - 45.0).abs() < 1e-9);
    }

    #[test]
    fn purge_tower_feature_type() {
        assert_eq!(purge_tower_feature(), FeatureType::PurgeTower);
    }
}
