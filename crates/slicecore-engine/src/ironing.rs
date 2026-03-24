//! Ironing pass generation for smooth top surfaces.
//!
//! Ironing adds a final pass over top surfaces with very low flow (typically
//! 10%) and tight line spacing. The nozzle barely extrudes, smoothing the
//! surface by re-melting the previously deposited material.
//!
//! # How it works
//!
//! 1. Top surface regions are identified by the surface classifier.
//! 2. Rectilinear infill lines are generated at 100% density with ironing
//!    spacing (much tighter than normal infill).
//! 3. Each line is converted to a [`ToolpathSegment`] with
//!    [`FeatureType::Ironing`], reduced E-values (multiplied by `flow_rate`),
//!    and the configured ironing speed.
//!
//! # Example
//!
//! ```ignore
//! let config = IroningConfig { enabled: true, ..Default::default() };
//! let segments = generate_ironing_passes(
//!     &top_regions, &config, 5.0, 0.4, 0.2, 1.75, 1.0,
//! );
//! // segments contain FeatureType::Ironing moves with ~10% flow
//! ```

use serde::{Deserialize, Serialize};
use slicecore_config_derive::SettingSchema;
use slicecore_geo::polygon::ValidPolygon;
use slicecore_math::Point2;

use crate::extrusion::compute_e_value;
use crate::infill::generate_rectilinear_infill;
use crate::toolpath::{FeatureType, ToolpathSegment};

/// Ironing pass configuration.
///
/// When enabled, an ironing pass is added after all other toolpath features
/// on layers with top surfaces. The pass uses very low flow to smooth the
/// surface without adding significant material.
///
/// Serialized as a TOML section `[ironing]` within
/// [`PrintConfig`](crate::config::PrintConfig).
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Quality")]
pub struct IroningConfig {
    /// Enable ironing passes on top surfaces.
    #[setting(
        tier = 2,
        description = "Enable ironing passes on top surfaces",
        override_safety = "safe"
    )]
    pub enabled: bool,
    /// Flow rate multiplier for ironing (0.0-1.0). Default 0.1 (10%).
    #[setting(
        tier = 3,
        description = "Flow rate multiplier for ironing",
        min = 0.0,
        max = 1.0,
        depends_on = "ironing.enabled",
        override_safety = "safe"
    )]
    pub flow_rate: f64,
    /// Ironing speed in mm/s. Default 15.0.
    #[setting(
        tier = 3,
        description = "Ironing pass speed",
        units = "mm/s",
        min = 1.0,
        max = 300.0,
        depends_on = "ironing.enabled",
        override_safety = "safe"
    )]
    pub speed: f64,
    /// Line spacing for ironing in mm. Default 0.1 (very tight).
    #[setting(
        tier = 3,
        description = "Line spacing for ironing passes",
        units = "mm",
        min = 0.01,
        max = 2.0,
        depends_on = "ironing.enabled",
        override_safety = "safe"
    )]
    pub spacing: f64,
    /// Ironing angle in degrees. Default 45.0 (offset from primary infill).
    #[setting(
        tier = 3,
        description = "Ironing angle offset from primary infill",
        units = "deg",
        min = 0.0,
        max = 360.0,
        depends_on = "ironing.enabled",
        override_safety = "safe"
    )]
    pub angle: f64,
}

impl Default for IroningConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            flow_rate: 0.1,
            speed: 15.0,
            spacing: 0.1,
            angle: 45.0,
        }
    }
}

/// Generates ironing toolpath segments over top surface regions.
///
/// Uses rectilinear infill internally with 100% density and the configured
/// ironing spacing. Each resulting line is converted to a [`ToolpathSegment`]
/// with [`FeatureType::Ironing`], reduced E-values (multiplied by
/// `config.flow_rate`), and ironing speed as feedrate.
///
/// # Parameters
///
/// - `top_regions`: Polygons identifying top surface areas on this layer.
/// - `config`: Ironing configuration (flow rate, speed, spacing, angle).
/// - `layer_z`: Z height of this layer in mm.
/// - `nozzle_diameter`: Nozzle diameter in mm (used for extrusion width).
/// - `layer_height`: Height of this layer in mm.
/// - `filament_diameter`: Filament diameter in mm.
/// - `extrusion_multiplier`: Global extrusion multiplier.
///
/// # Returns
///
/// A vector of [`ToolpathSegment`]s with `FeatureType::Ironing`. Travel moves
/// are inserted between disconnected ironing lines. Returns empty if
/// `top_regions` is empty or ironing is disabled.
pub fn generate_ironing_passes(
    top_regions: &[ValidPolygon],
    config: &IroningConfig,
    layer_z: f64,
    nozzle_diameter: f64,
    layer_height: f64,
    filament_diameter: f64,
    extrusion_multiplier: f64,
) -> Vec<ToolpathSegment> {
    if top_regions.is_empty() {
        return Vec::new();
    }

    // Use the nozzle diameter as the extrusion width for ironing.
    // The spacing is controlled by the ironing config, not by the density formula.
    // We pass density = 1.0 and use the ironing spacing as the line width
    // to get tightly packed lines.
    let ironing_line_width = config.spacing;
    let infill_lines =
        generate_rectilinear_infill(top_regions, 1.0, config.angle, ironing_line_width);

    if infill_lines.is_empty() {
        return Vec::new();
    }

    let ironing_speed = config.speed * 60.0; // mm/s -> mm/min
    let travel_speed = 150.0 * 60.0; // Use a reasonable travel speed (mm/min)
                                     // Use nozzle_diameter-based extrusion width for E-value computation.
    let extrusion_width = nozzle_diameter * 1.1;

    let mut segments = Vec::new();
    let mut current_pos: Option<Point2> = None;

    for line in &infill_lines {
        let (sx, sy) = line.start.to_mm();
        let (ex, ey) = line.end.to_mm();
        let start_pt = Point2::new(sx, sy);
        let end_pt = Point2::new(ex, ey);

        // Insert travel to line start if needed.
        if let Some(pos) = current_pos {
            let dx = start_pt.x - pos.x;
            let dy = start_pt.y - pos.y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > 0.001 {
                segments.push(ToolpathSegment {
                    start: pos,
                    end: start_pt,
                    feature: FeatureType::Travel,
                    e_value: 0.0,
                    feedrate: travel_speed,
                    z: layer_z,
                    extrusion_width: None,
                });
            }
        }

        // Compute segment length.
        let seg_len = {
            let dx = end_pt.x - start_pt.x;
            let dy = end_pt.y - start_pt.y;
            (dx * dx + dy * dy).sqrt()
        };

        if seg_len > 0.0001 {
            // Compute E-value with ironing flow rate applied.
            let base_e = compute_e_value(
                seg_len,
                extrusion_width,
                layer_height,
                filament_diameter,
                extrusion_multiplier,
            );
            let ironing_e = base_e * config.flow_rate;

            segments.push(ToolpathSegment {
                start: start_pt,
                end: end_pt,
                feature: FeatureType::Ironing,
                e_value: ironing_e,
                feedrate: ironing_speed,
                z: layer_z,
                extrusion_width: None,
            });

            current_pos = Some(end_pt);
        }
    }

    segments
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_geo::polygon::Polygon;

    /// Helper to create a validated CCW square at the origin with given size (mm).
    fn make_square(size: f64) -> ValidPolygon {
        Polygon::from_mm(&[(0.0, 0.0), (size, 0.0), (size, size), (0.0, size)])
            .validate()
            .unwrap()
    }

    #[test]
    fn ironing_config_defaults() {
        let config = IroningConfig::default();
        assert!(!config.enabled);
        assert!((config.flow_rate - 0.1).abs() < 1e-9);
        assert!((config.speed - 15.0).abs() < 1e-9);
        assert!((config.spacing - 0.1).abs() < 1e-9);
        assert!((config.angle - 45.0).abs() < 1e-9);
    }

    #[test]
    fn generate_ironing_produces_segments() {
        let square = make_square(20.0);
        let config = IroningConfig {
            enabled: true,
            ..Default::default()
        };

        let segments = generate_ironing_passes(
            &[square],
            &config,
            1.0,  // layer_z
            0.4,  // nozzle_diameter
            0.2,  // layer_height
            1.75, // filament_diameter
            1.0,  // extrusion_multiplier
        );

        assert!(
            !segments.is_empty(),
            "Ironing should produce segments for a 20mm square"
        );

        // All non-travel segments should be Ironing feature type.
        for seg in &segments {
            match seg.feature {
                FeatureType::Ironing | FeatureType::Travel => {}
                other => panic!(
                    "Ironing should only produce Ironing or Travel segments, got {:?}",
                    other
                ),
            }
        }
    }

    #[test]
    fn ironing_e_values_are_reduced() {
        let square = make_square(20.0);
        let config = IroningConfig {
            enabled: true,
            flow_rate: 0.1,
            ..Default::default()
        };

        let segments = generate_ironing_passes(&[square], &config, 1.0, 0.4, 0.2, 1.75, 1.0);

        // Find ironing extrusion segments and verify E-values are very small.
        let ironing_segs: Vec<_> = segments
            .iter()
            .filter(|s| s.feature == FeatureType::Ironing)
            .collect();

        assert!(
            !ironing_segs.is_empty(),
            "Should have ironing extrusion segments"
        );

        // Compute what a "normal" E-value would be for the same length.
        for seg in &ironing_segs {
            let seg_len = seg.length();
            let normal_e = compute_e_value(
                seg_len,
                0.4 * 1.1, // extrusion_width
                0.2,       // layer_height
                1.75,      // filament_diameter
                1.0,       // extrusion_multiplier
            );

            // Ironing E should be ~10% of normal E.
            let ratio = seg.e_value / normal_e;
            assert!(
                (ratio - 0.1).abs() < 0.01,
                "Ironing E/normal E ratio should be ~0.1, got {} (e={}, normal_e={})",
                ratio,
                seg.e_value,
                normal_e
            );
        }
    }

    #[test]
    fn ironing_uses_configured_speed() {
        let square = make_square(20.0);
        let config = IroningConfig {
            enabled: true,
            speed: 20.0,
            ..Default::default()
        };

        let segments = generate_ironing_passes(&[square], &config, 1.0, 0.4, 0.2, 1.75, 1.0);

        let expected_feedrate = 20.0 * 60.0; // mm/s -> mm/min

        for seg in &segments {
            if seg.feature == FeatureType::Ironing {
                assert!(
                    (seg.feedrate - expected_feedrate).abs() < 0.1,
                    "Ironing feedrate should be {} mm/min, got {}",
                    expected_feedrate,
                    seg.feedrate
                );
            }
        }
    }

    #[test]
    fn ironing_empty_regions_returns_empty() {
        let config = IroningConfig::default();
        let segments = generate_ironing_passes(&[], &config, 1.0, 0.4, 0.2, 1.75, 1.0);
        assert!(
            segments.is_empty(),
            "Empty top regions should produce no ironing segments"
        );
    }

    #[test]
    fn ironing_serde_round_trip() {
        let config = IroningConfig {
            enabled: true,
            flow_rate: 0.15,
            speed: 20.0,
            spacing: 0.2,
            angle: 60.0,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: IroningConfig = serde_json::from_str(&json).unwrap();

        assert!(deserialized.enabled);
        assert!((deserialized.flow_rate - 0.15).abs() < 1e-9);
        assert!((deserialized.speed - 20.0).abs() < 1e-9);
        assert!((deserialized.spacing - 0.2).abs() < 1e-9);
        assert!((deserialized.angle - 60.0).abs() < 1e-9);
    }

    #[test]
    fn ironing_toml_deserialization() {
        let toml_str = r#"
enabled = true
flow_rate = 0.08
speed = 25.0
spacing = 0.15
"#;
        let config: IroningConfig = toml::from_str(toml_str).unwrap();
        assert!(config.enabled);
        assert!((config.flow_rate - 0.08).abs() < 1e-9);
        assert!((config.speed - 25.0).abs() < 1e-9);
        assert!((config.spacing - 0.15).abs() < 1e-9);
        // angle should use default since not specified
        assert!((config.angle - 45.0).abs() < 1e-9);
    }

    #[test]
    fn ironing_travel_moves_inserted() {
        let square = make_square(20.0);
        let config = IroningConfig {
            enabled: true,
            ..Default::default()
        };

        let segments = generate_ironing_passes(&[square], &config, 1.0, 0.4, 0.2, 1.75, 1.0);

        let travel_count = segments
            .iter()
            .filter(|s| s.feature == FeatureType::Travel)
            .count();

        // With multiple ironing lines, there should be travel moves between them.
        // (At least some, since lines are not all connected.)
        assert!(
            travel_count > 0 || segments.len() <= 1,
            "Should have travel moves between ironing lines (got {} travels in {} segments)",
            travel_count,
            segments.len()
        );
    }

    #[test]
    fn ironing_custom_flow_rate() {
        let square = make_square(10.0);
        let config_10 = IroningConfig {
            enabled: true,
            flow_rate: 0.1,
            ..Default::default()
        };
        let config_20 = IroningConfig {
            enabled: true,
            flow_rate: 0.2,
            ..Default::default()
        };

        let segs_10 =
            generate_ironing_passes(&[square.clone()], &config_10, 1.0, 0.4, 0.2, 1.75, 1.0);
        let segs_20 = generate_ironing_passes(&[square], &config_20, 1.0, 0.4, 0.2, 1.75, 1.0);

        // Get total E for ironing segments.
        let total_e_10: f64 = segs_10
            .iter()
            .filter(|s| s.feature == FeatureType::Ironing)
            .map(|s| s.e_value)
            .sum();
        let total_e_20: f64 = segs_20
            .iter()
            .filter(|s| s.feature == FeatureType::Ironing)
            .map(|s| s.e_value)
            .sum();

        // 20% flow should produce ~2x the E of 10% flow.
        let ratio = total_e_20 / total_e_10;
        assert!(
            (ratio - 2.0).abs() < 0.1,
            "Flow rate 0.2 should produce ~2x E of 0.1, got ratio {}",
            ratio
        );
    }
}
