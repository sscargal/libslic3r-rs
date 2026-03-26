//! Print planning: skirt/brim generation, retraction, temperature, and fan control.
//!
//! The planner handles all "support" concerns around the actual print geometry:
//! - **Skirt**: Offset loops around the first-layer footprint for priming
//! - **Brim**: Outward offsets attached to the model for bed adhesion
//! - **Retraction**: Filament retract/unretract for travel moves exceeding a threshold
//! - **Temperature**: Nozzle and bed temperature commands per layer
//! - **Fan control**: Fan speed commands respecting disable-first-layers and layer-time

use slicecore_gcode_io::GcodeCommand;
use slicecore_geo::polygon::ValidPolygon;
use slicecore_geo::{convex_hull, offset_polygon, offset_polygons, JoinType};
use slicecore_math::{mm_to_coord, IPoint2};

use crate::config::{PrintConfig, SurfaceEnforce, ZHopConfig, ZHopHeightMode, ZHopType};
use crate::toolpath::FeatureType;

// ---------------------------------------------------------------------------
// Retraction
// ---------------------------------------------------------------------------

/// A planned retraction move with distance and speed.
#[derive(Debug, Clone, PartialEq)]
pub struct RetractionMove {
    /// Retraction distance in mm.
    pub retract_length: f64,
    /// Retraction speed in mm/s.
    pub retract_speed: f64,
}

/// Decides whether to retract for a given travel distance.
///
/// Returns `Some(RetractionMove)` if `travel_distance >= config.retraction.min_travel`,
/// otherwise `None` (short travel, no retraction needed).
pub fn plan_retraction(travel_distance: f64, config: &PrintConfig) -> Option<RetractionMove> {
    if travel_distance >= config.retraction.min_travel {
        Some(RetractionMove {
            retract_length: config.retraction.length,
            retract_speed: config.retraction.speed,
        })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Z-hop planning
// ---------------------------------------------------------------------------

/// A planned z-hop move with computed height, resolved type, and optional speed.
#[derive(Debug, Clone, PartialEq)]
pub struct ZHopDecision {
    /// Computed z-hop height in mm (after proportional calc + clamping).
    pub height: f64,
    /// Resolved hop type (Auto resolved to Normal or Spiral).
    pub hop_type: ZHopType,
    /// Z-hop speed in mm/s (None = use travel speed).
    pub speed: Option<f64>,
    /// Travel angle in degrees for Slope/Spiral.
    pub travel_angle: f64,
}

/// Decides whether z-hop should activate for this travel move.
///
/// Checks (in order):
/// 1. Z-hop enabled (height > 0 for fixed, multiplier > 0 for proportional)
/// 2. Surface enforcement (departure feature must match)
/// 3. Distance gate (travel_distance >= min_travel)
/// 4. Z-range filters (current_z >= above, current_z <= below or below == 0)
/// 5. Compute height (fixed or proportional with min/max clamping)
/// 6. Resolve Auto type based on departure feature
pub fn plan_z_hop(
    departure_feature: FeatureType,
    travel_distance: f64,
    current_z: f64,
    layer_height: f64,
    config: &ZHopConfig,
) -> Option<ZHopDecision> {
    // 1. Check if z-hop is enabled
    if config.height <= 0.0 && config.height_mode == ZHopHeightMode::Fixed {
        return None;
    }
    // For proportional mode, check that multiplier would produce > 0
    if config.height_mode == ZHopHeightMode::Proportional && config.proportional_multiplier <= 0.0 {
        return None;
    }

    // 2. Surface enforcement
    match config.surface_enforce {
        SurfaceEnforce::TopSolidAndIroning => {
            if departure_feature != FeatureType::TopSolidInfill
                && departure_feature != FeatureType::Ironing
            {
                return None;
            }
        }
        SurfaceEnforce::AllSurfaces => {} // all surfaces pass
    }

    // 3. Distance gate
    if travel_distance < config.min_travel {
        return None;
    }

    // 4. Z-range filters
    if config.above > 0.0 && current_z < config.above {
        return None;
    }
    if config.below > 0.0 && current_z > config.below {
        return None;
    }

    // 5. Compute height
    let raw_height = match config.height_mode {
        ZHopHeightMode::Fixed => config.height,
        ZHopHeightMode::Proportional => config.proportional_multiplier * layer_height,
    };

    // Safety: zero or negative height means no hop
    if raw_height <= 0.0 {
        return None;
    }

    let height = raw_height.clamp(config.min_height, config.max_height);

    // 6. Resolve Auto type
    let hop_type = match config.hop_type {
        ZHopType::Auto => {
            if departure_feature == FeatureType::TopSolidInfill
                || departure_feature == FeatureType::Ironing
            {
                ZHopType::Spiral
            } else {
                ZHopType::Normal
            }
        }
        other => other,
    };

    // 7. Speed
    let speed = if config.speed > 0.0 {
        Some(config.speed)
    } else {
        None
    };

    Some(ZHopDecision {
        height,
        hop_type,
        speed,
        travel_angle: config.travel_angle,
    })
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
    let nozzle_offset = mm_to_coord(config.machine.nozzle_diameter());
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

    let brim_loops = (config.brim_width / config.machine.nozzle_diameter()).ceil() as u32;
    let nozzle_coord = mm_to_coord(config.machine.nozzle_diameter());

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
            let mut cmds = vec![
                GcodeCommand::SetBedTemp {
                    temp: config.filament.first_layer_bed_temp(),
                    wait: true,
                },
                GcodeCommand::SetExtruderTemp {
                    temp: config.filament.first_layer_nozzle_temp(),
                    wait: true,
                },
            ];

            // Emit M141 for chamber/enclosure temperature if configured.
            if config.filament.chamber_temperature > 0.0 {
                cmds.push(GcodeCommand::Raw(format!(
                    "M141 S{:.0}",
                    config.filament.chamber_temperature
                )));
            }

            cmds
        }
        1 => {
            let mut cmds = Vec::new();

            // Transition to normal bed temp if different.
            if (config.filament.bed_temp() - config.filament.first_layer_bed_temp()).abs() > 0.1 {
                cmds.push(GcodeCommand::SetBedTemp {
                    temp: config.filament.bed_temp(),
                    wait: false,
                });
            }

            // Transition to normal nozzle temp if different.
            if (config.filament.nozzle_temp() - config.filament.first_layer_nozzle_temp()).abs()
                > 0.1
            {
                cmds.push(GcodeCommand::SetExtruderTemp {
                    temp: config.filament.nozzle_temp(),
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
/// - If `layer_index < config.cooling.disable_fan_first_layers`: emits FanOff.
/// - Otherwise: emits SetFanSpeed at the configured fan speed.
///   (Phase 3 simplification: full fan_speed whenever fan is enabled.)
pub fn plan_fan(
    layer_index: usize,
    _layer_time_seconds: f64,
    config: &PrintConfig,
) -> Vec<GcodeCommand> {
    if (layer_index as u32) < config.cooling.disable_fan_first_layers {
        vec![GcodeCommand::FanOff]
    } else {
        vec![GcodeCommand::SetFanSpeed(config.cooling.fan_speed)]
    }
}

// ---------------------------------------------------------------------------
// Bridge fan control
// ---------------------------------------------------------------------------

/// Generates G-code command to set bridge fan speed.
///
/// Used when entering a bridge feature type to ensure maximum cooling
/// during unsupported bridging.
///
/// # Parameters
///
/// - `bridge_fan_speed`: Fan speed for bridge sections (0-255, typically 255).
///
/// # Returns
///
/// A single `SetFanSpeed` command for the bridge fan speed.
pub fn plan_bridge_fan(bridge_fan_speed: u8) -> Vec<GcodeCommand> {
    vec![GcodeCommand::SetFanSpeed(bridge_fan_speed)]
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
        Polygon::from_mm(&[(0.0, 0.0), (size, 0.0), (size, size), (0.0, size)])
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
        let mut config = PrintConfig::default();
        config.brim_width = 2.0;
        config.machine.nozzle_diameters = vec![0.4];

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
        let mut config = PrintConfig::default();
        config.retraction.min_travel = 1.5;
        assert!(plan_retraction(1.0, &config).is_none());
    }

    #[test]
    fn retraction_long_travel_returns_some() {
        let mut config = PrintConfig::default();
        config.retraction.min_travel = 1.5;
        config.retraction.length = 0.8;
        config.retraction.speed = 45.0;

        let retract = plan_retraction(2.0, &config).unwrap();
        assert!((retract.retract_length - 0.8).abs() < 1e-9);
        assert!((retract.retract_speed - 45.0).abs() < 1e-9);
    }

    #[test]
    fn retraction_exact_threshold_triggers() {
        let mut config = PrintConfig::default();
        config.retraction.min_travel = 1.5;
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
                temp: config.filament.first_layer_bed_temp(),
                wait: true
            }
        );
        // Nozzle temp with wait
        assert_eq!(
            cmds[1],
            GcodeCommand::SetExtruderTemp {
                temp: config.filament.first_layer_nozzle_temp(),
                wait: true
            }
        );
    }

    #[test]
    fn temperature_layer_1_emits_change_when_different() {
        let mut config = PrintConfig::default();
        config.filament.nozzle_temperatures = vec![200.0];
        config.filament.first_layer_nozzle_temperatures = vec![210.0];
        config.filament.bed_temperatures = vec![60.0];
        config.filament.first_layer_bed_temperatures = vec![65.0];

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
        let mut config = PrintConfig::default();
        config.filament.nozzle_temperatures = vec![200.0];
        config.filament.first_layer_nozzle_temperatures = vec![200.0];
        config.filament.bed_temperatures = vec![60.0];
        config.filament.first_layer_bed_temperatures = vec![60.0];

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
        assert!(
            cmds.is_empty(),
            "Layer 5 should emit no temperature commands"
        );
    }

    // --- Fan tests ---

    #[test]
    fn fan_layer_0_disabled_emits_fan_off() {
        let mut config = PrintConfig::default();
        config.cooling.disable_fan_first_layers = 1;

        let cmds = plan_fan(0, 10.0, &config);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0], GcodeCommand::FanOff);
    }

    #[test]
    fn fan_layer_1_emits_set_fan_speed() {
        let mut config = PrintConfig::default();
        config.cooling.disable_fan_first_layers = 1;
        config.cooling.fan_speed = 255;

        let cmds = plan_fan(1, 10.0, &config);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0], GcodeCommand::SetFanSpeed(255));
    }

    #[test]
    fn fan_multiple_disabled_layers() {
        let mut config = PrintConfig::default();
        config.cooling.disable_fan_first_layers = 3;
        config.cooling.fan_speed = 200;

        // Layers 0, 1, 2 should have fan off.
        for i in 0..3 {
            let cmds = plan_fan(i, 10.0, &config);
            assert_eq!(
                cmds[0],
                GcodeCommand::FanOff,
                "Layer {} should have fan off",
                i
            );
        }

        // Layer 3 should enable fan.
        let cmds = plan_fan(3, 10.0, &config);
        assert_eq!(cmds[0], GcodeCommand::SetFanSpeed(200));
    }

    // --- Z-hop planning tests ---

    use crate::config::{SurfaceEnforce, ZHopConfig, ZHopHeightMode, ZHopType};
    use crate::toolpath::FeatureType;

    fn test_zhop_config() -> ZHopConfig {
        ZHopConfig {
            height: 0.4,
            hop_type: ZHopType::Normal,
            height_mode: ZHopHeightMode::Fixed,
            proportional_multiplier: 1.5,
            min_height: 0.1,
            max_height: 2.0,
            surface_enforce: SurfaceEnforce::TopSolidAndIroning,
            travel_angle: 45.0,
            speed: 0.0,
            min_travel: 2.0,
            above: 0.0,
            below: 0.0,
        }
    }

    #[test]
    fn z_hop_disabled_when_height_zero() {
        let mut cfg = test_zhop_config();
        cfg.height = 0.0;
        assert!(plan_z_hop(FeatureType::TopSolidInfill, 5.0, 1.0, 0.2, &cfg).is_none());
    }

    #[test]
    fn z_hop_surface_gate_rejects_wrong_surface() {
        let cfg = test_zhop_config();
        // OuterPerimeter should be rejected with TopSolidAndIroning enforce
        assert!(plan_z_hop(FeatureType::OuterPerimeter, 5.0, 1.0, 0.2, &cfg).is_none());
    }

    #[test]
    fn z_hop_surface_gate_accepts_top_solid_infill() {
        let cfg = test_zhop_config();
        assert!(plan_z_hop(FeatureType::TopSolidInfill, 5.0, 1.0, 0.2, &cfg).is_some());
    }

    #[test]
    fn z_hop_surface_gate_accepts_ironing() {
        let cfg = test_zhop_config();
        assert!(plan_z_hop(FeatureType::Ironing, 5.0, 1.0, 0.2, &cfg).is_some());
    }

    #[test]
    fn z_hop_all_surfaces_accepts_outer_perimeter() {
        let mut cfg = test_zhop_config();
        cfg.surface_enforce = SurfaceEnforce::AllSurfaces;
        assert!(plan_z_hop(FeatureType::OuterPerimeter, 5.0, 1.0, 0.2, &cfg).is_some());
    }

    #[test]
    fn z_hop_distance_gate_rejects_short_travel() {
        let cfg = test_zhop_config();
        // min_travel=2.0, travel_distance=1.0
        assert!(plan_z_hop(FeatureType::TopSolidInfill, 1.0, 1.0, 0.2, &cfg).is_none());
    }

    #[test]
    fn z_hop_distance_gate_accepts_long_travel() {
        let cfg = test_zhop_config();
        // min_travel=2.0, travel_distance=3.0
        assert!(plan_z_hop(FeatureType::TopSolidInfill, 3.0, 1.0, 0.2, &cfg).is_some());
    }

    #[test]
    fn z_hop_above_threshold_rejects_below() {
        let mut cfg = test_zhop_config();
        cfg.above = 1.0;
        // current_z=0.5 < above=1.0
        assert!(plan_z_hop(FeatureType::TopSolidInfill, 5.0, 0.5, 0.2, &cfg).is_none());
    }

    #[test]
    fn z_hop_above_threshold_accepts_above() {
        let mut cfg = test_zhop_config();
        cfg.above = 1.0;
        // current_z=1.5 > above=1.0
        assert!(plan_z_hop(FeatureType::TopSolidInfill, 5.0, 1.5, 0.2, &cfg).is_some());
    }

    #[test]
    fn z_hop_below_threshold_rejects_above_ceiling() {
        let mut cfg = test_zhop_config();
        cfg.below = 2.0;
        // current_z=2.5 > below=2.0
        assert!(plan_z_hop(FeatureType::TopSolidInfill, 5.0, 2.5, 0.2, &cfg).is_none());
    }

    #[test]
    fn z_hop_below_threshold_accepts_below_ceiling() {
        let mut cfg = test_zhop_config();
        cfg.below = 2.0;
        // current_z=1.5 < below=2.0
        assert!(plan_z_hop(FeatureType::TopSolidInfill, 5.0, 1.5, 0.2, &cfg).is_some());
    }

    #[test]
    fn z_hop_below_zero_no_ceiling() {
        let mut cfg = test_zhop_config();
        cfg.below = 0.0;
        // below=0.0 means no ceiling filter
        assert!(plan_z_hop(FeatureType::TopSolidInfill, 5.0, 100.0, 0.2, &cfg).is_some());
    }

    #[test]
    fn z_hop_proportional_height() {
        let mut cfg = test_zhop_config();
        cfg.height_mode = ZHopHeightMode::Proportional;
        cfg.proportional_multiplier = 2.0;
        let decision = plan_z_hop(FeatureType::TopSolidInfill, 5.0, 1.0, 0.2, &cfg).unwrap();
        assert!((decision.height - 0.4).abs() < 1e-9, "2.0 * 0.2 = 0.4");
    }

    #[test]
    fn z_hop_proportional_clamped_max() {
        let mut cfg = test_zhop_config();
        cfg.height_mode = ZHopHeightMode::Proportional;
        cfg.proportional_multiplier = 3.0;
        cfg.max_height = 2.0;
        // 3.0 * 1.0 = 3.0, clamped to max 2.0
        let decision = plan_z_hop(FeatureType::TopSolidInfill, 5.0, 1.0, 1.0, &cfg).unwrap();
        assert!((decision.height - 2.0).abs() < 1e-9, "clamped to max_height");
    }

    #[test]
    fn z_hop_proportional_clamped_min() {
        let mut cfg = test_zhop_config();
        cfg.height_mode = ZHopHeightMode::Proportional;
        cfg.proportional_multiplier = 1.0;
        cfg.min_height = 0.1;
        // 1.0 * 0.05 = 0.05, clamped to min 0.1
        let decision = plan_z_hop(FeatureType::TopSolidInfill, 5.0, 1.0, 0.05, &cfg).unwrap();
        assert!((decision.height - 0.1).abs() < 1e-9, "clamped to min_height");
    }

    #[test]
    fn z_hop_fixed_ignores_layer_height() {
        let cfg = test_zhop_config();
        // Fixed mode with height=0.4, layer_height varies
        let d1 = plan_z_hop(FeatureType::TopSolidInfill, 5.0, 1.0, 0.1, &cfg).unwrap();
        let d2 = plan_z_hop(FeatureType::TopSolidInfill, 5.0, 1.0, 0.3, &cfg).unwrap();
        assert!((d1.height - 0.4).abs() < 1e-9);
        assert!((d2.height - 0.4).abs() < 1e-9);
    }

    #[test]
    fn z_hop_auto_resolves_spiral_on_top_solid() {
        let mut cfg = test_zhop_config();
        cfg.hop_type = ZHopType::Auto;
        let decision = plan_z_hop(FeatureType::TopSolidInfill, 5.0, 1.0, 0.2, &cfg).unwrap();
        assert_eq!(decision.hop_type, ZHopType::Spiral);
    }

    #[test]
    fn z_hop_auto_resolves_spiral_on_ironing() {
        let mut cfg = test_zhop_config();
        cfg.hop_type = ZHopType::Auto;
        let decision = plan_z_hop(FeatureType::Ironing, 5.0, 1.0, 0.2, &cfg).unwrap();
        assert_eq!(decision.hop_type, ZHopType::Spiral);
    }

    #[test]
    fn z_hop_auto_resolves_normal_on_other_surface() {
        let mut cfg = test_zhop_config();
        cfg.hop_type = ZHopType::Auto;
        cfg.surface_enforce = SurfaceEnforce::AllSurfaces;
        let decision = plan_z_hop(FeatureType::OuterPerimeter, 5.0, 1.0, 0.2, &cfg).unwrap();
        assert_eq!(decision.hop_type, ZHopType::Normal);
    }

    #[test]
    fn z_hop_speed_zero_returns_none() {
        let cfg = test_zhop_config();
        let decision = plan_z_hop(FeatureType::TopSolidInfill, 5.0, 1.0, 0.2, &cfg).unwrap();
        assert_eq!(decision.speed, None, "speed=0.0 should yield None");
    }

    #[test]
    fn z_hop_speed_nonzero_returns_some() {
        let mut cfg = test_zhop_config();
        cfg.speed = 15.0;
        let decision = plan_z_hop(FeatureType::TopSolidInfill, 5.0, 1.0, 0.2, &cfg).unwrap();
        assert_eq!(decision.speed, Some(15.0));
    }
}
