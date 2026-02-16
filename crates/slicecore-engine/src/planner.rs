//! Print planning: skirt/brim generation, retraction, temperature, and fan control.
//!
//! The planner handles all "support" concerns around the actual print geometry:
//! - **Skirt**: Offset loops around the first-layer footprint for priming
//! - **Brim**: Outward offsets attached to the model for bed adhesion
//! - **Retraction**: Filament retract/unretract for travel moves exceeding a threshold
//! - **Temperature**: Nozzle and bed temperature commands per layer
//! - **Fan control**: Fan speed commands respecting disable-first-layers and layer-time

use slicecore_geo::polygon::ValidPolygon;
use slicecore_geo::{convex_hull, offset_polygon, offset_polygons, JoinType};
use slicecore_gcode_io::GcodeCommand;
use slicecore_math::{mm_to_coord, IPoint2};

use crate::config::PrintConfig;

// ---------------------------------------------------------------------------
// Retraction
// ---------------------------------------------------------------------------

/// A planned retraction move with distance, speed, and optional Z-hop.
#[derive(Debug, Clone, PartialEq)]
pub struct RetractionMove {
    /// Retraction distance in mm.
    pub retract_length: f64,
    /// Retraction speed in mm/s.
    pub retract_speed: f64,
    /// Z-hop height in mm (0.0 = no hop).
    pub z_hop: f64,
}

/// Decides whether to retract for a given travel distance.
///
/// Returns `Some(RetractionMove)` if `travel_distance >= config.min_travel_for_retract`,
/// otherwise `None` (short travel, no retraction needed).
pub fn plan_retraction(travel_distance: f64, config: &PrintConfig) -> Option<RetractionMove> {
    if travel_distance >= config.min_travel_for_retract {
        Some(RetractionMove {
            retract_length: config.retract_length,
            retract_speed: config.retract_speed,
            z_hop: config.retract_z_hop,
        })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Skirt
// ---------------------------------------------------------------------------

/// Generates skirt loops around the first-layer contours.
///
/// The skirt is a set of offset polygons around the convex hull of all
/// first-layer outer contours. It primes the nozzle before printing begins.
///
/// Returns one polygon per skirt loop, ordered from inner to outer.
pub fn generate_skirt(
    first_layer_contours: &[ValidPolygon],
    config: &PrintConfig,
) -> Vec<ValidPolygon> {
    if first_layer_contours.is_empty() || config.skirt_loops == 0 {
        return Vec::new();
    }

    // 1. Collect all points from all CCW contours and compute convex hull.
    let all_points: Vec<IPoint2> = first_layer_contours
        .iter()
        .flat_map(|p| p.points().iter().copied())
        .collect();

    let hull_points = convex_hull(&all_points);
    if hull_points.len() < 3 {
        return Vec::new();
    }

    // 2. Create a ValidPolygon from the hull.
    let hull_polygon = match slicecore_geo::polygon::Polygon::new(hull_points).validate() {
        Ok(vp) => vp,
        Err(_) => return Vec::new(),
    };

    // 3. Offset hull outward by skirt_distance for the first loop.
    let skirt_offset = mm_to_coord(config.skirt_distance);
    let first_loop = match offset_polygon(&hull_polygon, skirt_offset, JoinType::Round) {
        Ok(polys) => polys,
        Err(_) => return Vec::new(),
    };

    if first_loop.is_empty() {
        return Vec::new();
    }

    let mut result = first_loop;

    // 4. Additional loops: offset outward by one nozzle_diameter per loop.
    let nozzle_offset = mm_to_coord(config.nozzle_diameter);
    for _ in 1..config.skirt_loops {
        // Offset from the last set of polygons.
        let prev = result.last().unwrap();
        match offset_polygon(prev, nozzle_offset, JoinType::Round) {
            Ok(new_polys) if !new_polys.is_empty() => {
                result.extend(new_polys);
            }
            _ => break,
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Brim
// ---------------------------------------------------------------------------

/// Generates brim loops attached to the model for bed adhesion.
///
/// The brim consists of outward offsets from the first-layer outer contours.
/// Each loop is offset by one nozzle diameter more than the previous.
///
/// Returns all brim loop polygons ordered from inner (closest to model) to outer.
pub fn generate_brim(
    first_layer_contours: &[ValidPolygon],
    config: &PrintConfig,
) -> Vec<ValidPolygon> {
    if config.brim_width <= 0.0 || first_layer_contours.is_empty() {
        return Vec::new();
    }

    let brim_loops = (config.brim_width / config.nozzle_diameter).ceil() as u32;
    let nozzle_coord = mm_to_coord(config.nozzle_diameter);

    let mut result = Vec::new();

    for i in 0..brim_loops {
        let delta = nozzle_coord * (i as i64 + 1);
        match offset_polygons(first_layer_contours, delta, JoinType::Round) {
            Ok(polys) => result.extend(polys),
            Err(_) => break,
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Temperature planning
// ---------------------------------------------------------------------------

/// Generates temperature G-code commands for the given layer.
///
/// - Layer 0: Sets first-layer nozzle and bed temps (with wait).
/// - Layer 1: If first-layer temps differ from normal temps, emits change commands
///   (no wait -- temperature changes while printing).
/// - Other layers: Returns empty Vec.
pub fn plan_temperatures(layer_index: usize, config: &PrintConfig) -> Vec<GcodeCommand> {
    match layer_index {
        0 => {
            vec![
                GcodeCommand::SetBedTemp {
                    temp: config.first_layer_bed_temp,
                    wait: true,
                },
                GcodeCommand::SetExtruderTemp {
                    temp: config.first_layer_nozzle_temp,
                    wait: true,
                },
            ]
        }
        1 => {
            let mut cmds = Vec::new();

            // Transition to normal bed temp if different.
            if (config.bed_temp - config.first_layer_bed_temp).abs() > 0.1 {
                cmds.push(GcodeCommand::SetBedTemp {
                    temp: config.bed_temp,
                    wait: false,
                });
            }

            // Transition to normal nozzle temp if different.
            if (config.nozzle_temp - config.first_layer_nozzle_temp).abs() > 0.1 {
                cmds.push(GcodeCommand::SetExtruderTemp {
                    temp: config.nozzle_temp,
                    wait: false,
                });
            }

            cmds
        }
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Fan control
// ---------------------------------------------------------------------------

/// Generates fan control G-code commands for the given layer.
///
/// - If `layer_index < config.disable_fan_first_layers`: emits FanOff.
/// - Otherwise: emits SetFanSpeed at the configured fan speed.
///   (Phase 3 simplification: full fan_speed whenever fan is enabled.)
pub fn plan_fan(
    layer_index: usize,
    _layer_time_seconds: f64,
    config: &PrintConfig,
) -> Vec<GcodeCommand> {
    if (layer_index as u32) < config.disable_fan_first_layers {
        vec![GcodeCommand::FanOff]
    } else {
        vec![GcodeCommand::SetFanSpeed(config.fan_speed)]
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_geo::polygon::Polygon;

    /// Helper to create a validated CCW square.
    fn make_square(size: f64) -> ValidPolygon {
        Polygon::from_mm(&[
            (0.0, 0.0),
            (size, 0.0),
            (size, size),
            (0.0, size),
        ])
        .validate()
        .unwrap()
    }

    fn default_config() -> PrintConfig {
        PrintConfig::default()
    }

    // --- Skirt tests ---

    #[test]
    fn skirt_single_loop_larger_than_input() {
        let square = make_square(20.0);
        let config = PrintConfig {
            skirt_loops: 1,
            skirt_distance: 6.0,
            ..Default::default()
        };

        let skirts = generate_skirt(&[square.clone()], &config);
        assert_eq!(skirts.len(), 1, "Should produce 1 skirt loop");

        // Skirt should be larger than the input square.
        let skirt_area = skirts[0].area_mm2();
        let input_area = square.area_mm2();
        assert!(
            skirt_area > input_area,
            "Skirt area ({}) should be larger than input area ({})",
            skirt_area,
            input_area
        );
    }

    #[test]
    fn skirt_three_loops() {
        let square = make_square(20.0);
        let config = PrintConfig {
            skirt_loops: 3,
            skirt_distance: 6.0,
            ..Default::default()
        };

        let skirts = generate_skirt(&[square], &config);
        assert_eq!(
            skirts.len(),
            3,
            "Should produce 3 skirt loops, got {}",
            skirts.len()
        );
    }

    #[test]
    fn skirt_empty_contours() {
        let config = default_config();
        let skirts = generate_skirt(&[], &config);
        assert!(skirts.is_empty());
    }

    #[test]
    fn skirt_zero_loops() {
        let square = make_square(20.0);
        let config = PrintConfig {
            skirt_loops: 0,
            ..Default::default()
        };
        let skirts = generate_skirt(&[square], &config);
        assert!(skirts.is_empty());
    }

    // --- Brim tests ---

    #[test]
    fn brim_zero_width_returns_empty() {
        let square = make_square(20.0);
        let config = PrintConfig {
            brim_width: 0.0,
            ..Default::default()
        };
        let brims = generate_brim(&[square], &config);
        assert!(brims.is_empty());
    }

    #[test]
    fn brim_2mm_width_produces_5_loops() {
        let square = make_square(20.0);
        let config = PrintConfig {
            brim_width: 2.0,
            nozzle_diameter: 0.4,
            ..Default::default()
        };

        let brims = generate_brim(&[square], &config);
        // 2.0 / 0.4 = 5.0 -> ceil = 5 loops
        // Each loop offsets the contours outward, producing at least 1 polygon.
        assert!(
            brims.len() >= 5,
            "Should produce at least 5 brim loops (one polygon per loop), got {}",
            brims.len()
        );
    }

    #[test]
    fn brim_empty_contours() {
        let config = PrintConfig {
            brim_width: 3.0,
            ..Default::default()
        };
        let brims = generate_brim(&[], &config);
        assert!(brims.is_empty());
    }

    // --- Retraction tests ---

    #[test]
    fn retraction_short_travel_returns_none() {
        let config = PrintConfig {
            min_travel_for_retract: 1.5,
            ..Default::default()
        };
        assert!(plan_retraction(1.0, &config).is_none());
    }

    #[test]
    fn retraction_long_travel_returns_some() {
        let config = PrintConfig {
            min_travel_for_retract: 1.5,
            retract_length: 0.8,
            retract_speed: 45.0,
            retract_z_hop: 0.2,
            ..Default::default()
        };

        let retract = plan_retraction(2.0, &config).unwrap();
        assert!((retract.retract_length - 0.8).abs() < 1e-9);
        assert!((retract.retract_speed - 45.0).abs() < 1e-9);
        assert!((retract.z_hop - 0.2).abs() < 1e-9);
    }

    #[test]
    fn retraction_exact_threshold_triggers() {
        let config = PrintConfig {
            min_travel_for_retract: 1.5,
            ..Default::default()
        };
        assert!(plan_retraction(1.5, &config).is_some());
    }

    // --- Temperature tests ---

    #[test]
    fn temperature_layer_0_emits_wait_commands() {
        let config = default_config();
        let cmds = plan_temperatures(0, &config);

        assert_eq!(cmds.len(), 2, "Layer 0 should emit 2 temperature commands");

        // Bed temp with wait
        assert_eq!(
            cmds[0],
            GcodeCommand::SetBedTemp {
                temp: config.first_layer_bed_temp,
                wait: true
            }
        );
        // Nozzle temp with wait
        assert_eq!(
            cmds[1],
            GcodeCommand::SetExtruderTemp {
                temp: config.first_layer_nozzle_temp,
                wait: true
            }
        );
    }

    #[test]
    fn temperature_layer_1_emits_change_when_different() {
        let config = PrintConfig {
            nozzle_temp: 200.0,
            first_layer_nozzle_temp: 210.0,
            bed_temp: 60.0,
            first_layer_bed_temp: 65.0,
            ..Default::default()
        };

        let cmds = plan_temperatures(1, &config);
        assert_eq!(
            cmds.len(),
            2,
            "Layer 1 should emit 2 temp change commands (bed + nozzle differ)"
        );

        // Bed temp no wait
        assert_eq!(
            cmds[0],
            GcodeCommand::SetBedTemp {
                temp: 60.0,
                wait: false
            }
        );
        // Nozzle temp no wait
        assert_eq!(
            cmds[1],
            GcodeCommand::SetExtruderTemp {
                temp: 200.0,
                wait: false
            }
        );
    }

    #[test]
    fn temperature_layer_1_no_change_when_same() {
        let config = PrintConfig {
            nozzle_temp: 200.0,
            first_layer_nozzle_temp: 200.0,
            bed_temp: 60.0,
            first_layer_bed_temp: 60.0,
            ..Default::default()
        };

        let cmds = plan_temperatures(1, &config);
        assert!(
            cmds.is_empty(),
            "Layer 1 should emit 0 commands when temps are the same"
        );
    }

    #[test]
    fn temperature_layer_5_returns_empty() {
        let config = default_config();
        let cmds = plan_temperatures(5, &config);
        assert!(cmds.is_empty(), "Layer 5 should emit no temperature commands");
    }

    // --- Fan tests ---

    #[test]
    fn fan_layer_0_disabled_emits_fan_off() {
        let config = PrintConfig {
            disable_fan_first_layers: 1,
            ..Default::default()
        };

        let cmds = plan_fan(0, 10.0, &config);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0], GcodeCommand::FanOff);
    }

    #[test]
    fn fan_layer_1_emits_set_fan_speed() {
        let config = PrintConfig {
            disable_fan_first_layers: 1,
            fan_speed: 255,
            ..Default::default()
        };

        let cmds = plan_fan(1, 10.0, &config);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0], GcodeCommand::SetFanSpeed(255));
    }

    #[test]
    fn fan_multiple_disabled_layers() {
        let config = PrintConfig {
            disable_fan_first_layers: 3,
            fan_speed: 200,
            ..Default::default()
        };

        // Layers 0, 1, 2 should have fan off.
        for i in 0..3 {
            let cmds = plan_fan(i, 10.0, &config);
            assert_eq!(cmds[0], GcodeCommand::FanOff, "Layer {} should have fan off", i);
        }

        // Layer 3 should enable fan.
        let cmds = plan_fan(3, 10.0, &config);
        assert_eq!(cmds[0], GcodeCommand::SetFanSpeed(200));
    }
}
