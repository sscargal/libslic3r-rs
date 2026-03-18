//! N-file comparison with delta computation for G-code analyses.
//!
//! Compares one or more [`GcodeAnalysis`] results against a baseline,
//! computing absolute and percentage deltas for time, filament, layer count,
//! retractions, moves, and per-feature breakdowns.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::metrics::GcodeAnalysis;

/// Result of comparing multiple G-code analyses against a baseline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    /// The baseline analysis (first file).
    pub baseline: GcodeAnalysis,
    /// The other analyses being compared against the baseline.
    pub others: Vec<GcodeAnalysis>,
    /// Deltas for each non-baseline file against the baseline.
    pub deltas: Vec<ComparisonDelta>,
}

/// Delta between one G-code analysis and the baseline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonDelta {
    /// Filename of the compared file.
    pub filename: String,
    // Time deltas
    /// Absolute delta in total computed time (seconds).
    pub total_time_delta_s: f64,
    /// Percentage delta in total computed time.
    pub total_time_delta_pct: f64,
    /// Absolute delta in slicer header estimated time (seconds), if both have it.
    pub header_time_delta_s: Option<f64>,
    /// Percentage delta in slicer header estimated time, if both have it.
    pub header_time_delta_pct: Option<f64>,
    // Filament deltas
    /// Absolute delta in total filament length (mm).
    pub filament_delta_mm: f64,
    /// Percentage delta in total filament length.
    pub filament_delta_pct: f64,
    /// Absolute delta in total filament weight (grams).
    pub filament_weight_delta_g: f64,
    // Structure deltas
    /// Delta in layer count.
    pub layer_count_delta: i64,
    /// Delta in retraction count.
    pub retraction_count_delta: i32,
    /// Delta in total move count.
    pub total_moves_delta: i64,
    /// Delta in total travel distance (mm).
    pub total_travel_delta_mm: f64,
    /// Delta in total extrusion distance (mm).
    pub total_extrusion_delta_mm: f64,
    // Per-feature deltas
    /// Deltas for each feature type (union of baseline and other features).
    pub feature_deltas: HashMap<String, FeatureDelta>,
}

/// Delta for a single feature type between two analyses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureDelta {
    /// Absolute delta in time (seconds).
    pub time_delta_s: f64,
    /// Percentage delta in time.
    pub time_delta_pct: f64,
    /// Absolute delta in filament length (mm).
    pub filament_delta_mm: f64,
    /// Percentage delta in filament length.
    pub filament_delta_pct: f64,
}

/// Compute percentage delta: `((other - baseline) / baseline) * 100`.
///
/// Returns 0.0 if baseline is 0 (to avoid division by zero).
fn pct_delta(baseline: f64, other: f64) -> f64 {
    if baseline.abs() < 1e-12 {
        0.0
    } else {
        ((other - baseline) / baseline) * 100.0
    }
}

/// Compare multiple G-code analyses against a baseline.
///
/// The baseline is the first file; each entry in `others` is compared
/// against it, producing per-file deltas.
pub fn compare_gcode_analyses(
    baseline: GcodeAnalysis,
    others: Vec<GcodeAnalysis>,
) -> ComparisonResult {
    let deltas: Vec<ComparisonDelta> = others
        .iter()
        .map(|other| compute_delta(&baseline, other))
        .collect();

    ComparisonResult {
        baseline,
        others,
        deltas,
    }
}

/// Compute the delta between a single other analysis and the baseline.
fn compute_delta(baseline: &GcodeAnalysis, other: &GcodeAnalysis) -> ComparisonDelta {
    // Time deltas
    let total_time_delta_s = other.total_time_estimate_s - baseline.total_time_estimate_s;
    let total_time_delta_pct =
        pct_delta(baseline.total_time_estimate_s, other.total_time_estimate_s);

    // Header time deltas (only if both have header time)
    let (header_time_delta_s, header_time_delta_pct) = match (
        baseline.header.estimated_time_s,
        other.header.estimated_time_s,
    ) {
        (Some(bt), Some(ot)) => (Some(ot - bt), Some(pct_delta(bt, ot))),
        _ => (None, None),
    };

    // Filament deltas
    let filament_delta_mm = other.total_filament_mm - baseline.total_filament_mm;
    let filament_delta_pct = pct_delta(baseline.total_filament_mm, other.total_filament_mm);
    let filament_weight_delta_g = other.total_filament_weight_g - baseline.total_filament_weight_g;

    // Structure deltas
    let layer_count_delta = other.layers.len() as i64 - baseline.layers.len() as i64;
    let retraction_count_delta = other.retraction_count as i32 - baseline.retraction_count as i32;
    let total_moves_delta = other.total_moves as i64 - baseline.total_moves as i64;
    let total_travel_delta_mm = other.total_travel_mm - baseline.total_travel_mm;
    let total_extrusion_delta_mm = other.total_extrusion_mm - baseline.total_extrusion_mm;

    // Per-feature deltas: iterate over union of feature keys
    let mut feature_deltas = HashMap::new();
    let mut all_keys: Vec<String> = baseline.features.keys().cloned().collect();
    for key in other.features.keys() {
        if !all_keys.contains(key) {
            all_keys.push(key.clone());
        }
    }

    for key in &all_keys {
        let b_time = baseline
            .features
            .get(key)
            .map(|f| f.time_estimate_s)
            .unwrap_or(0.0);
        let o_time = other
            .features
            .get(key)
            .map(|f| f.time_estimate_s)
            .unwrap_or(0.0);
        let b_filament = baseline
            .features
            .get(key)
            .map(|f| f.extrusion_e_mm)
            .unwrap_or(0.0);
        let o_filament = other
            .features
            .get(key)
            .map(|f| f.extrusion_e_mm)
            .unwrap_or(0.0);

        feature_deltas.insert(
            key.clone(),
            FeatureDelta {
                time_delta_s: o_time - b_time,
                time_delta_pct: pct_delta(b_time, o_time),
                filament_delta_mm: o_filament - b_filament,
                filament_delta_pct: pct_delta(b_filament, o_filament),
            },
        );
    }

    ComparisonDelta {
        filename: other.filename.clone(),
        total_time_delta_s,
        total_time_delta_pct,
        header_time_delta_s,
        header_time_delta_pct,
        filament_delta_mm,
        filament_delta_pct,
        filament_weight_delta_g,
        layer_count_delta,
        retraction_count_delta,
        total_moves_delta,
        total_travel_delta_mm,
        total_extrusion_delta_mm,
        feature_deltas,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcode_analysis::metrics::{
        FeatureMetrics, HeaderMetadata, LayerMetrics, SpeedStats,
    };
    use crate::gcode_analysis::slicer_detect::SlicerType;

    fn make_analysis(
        filename: &str,
        total_time: f64,
        filament_mm: f64,
        filament_weight_g: f64,
        header_time: Option<f64>,
        layers: usize,
        retractions: u32,
        total_moves: u64,
        travel_mm: f64,
        extrusion_mm: f64,
    ) -> GcodeAnalysis {
        let mut features = HashMap::new();
        features.insert(
            "Outer wall".to_string(),
            FeatureMetrics {
                move_count: 100,
                travel_distance_mm: 10.0,
                extrusion_distance_mm: 50.0,
                extrusion_e_mm: filament_mm * 0.5,
                time_estimate_s: total_time * 0.4,
                speed_stats: SpeedStats::default(),
            },
        );
        features.insert(
            "Infill".to_string(),
            FeatureMetrics {
                move_count: 80,
                travel_distance_mm: 5.0,
                extrusion_distance_mm: 40.0,
                extrusion_e_mm: filament_mm * 0.3,
                time_estimate_s: total_time * 0.3,
                speed_stats: SpeedStats::default(),
            },
        );

        GcodeAnalysis {
            filename: filename.to_string(),
            header: HeaderMetadata {
                estimated_time_s: header_time,
                ..Default::default()
            },
            slicer: SlicerType::Unknown,
            layers: (0..layers)
                .map(|i| LayerMetrics {
                    z_height: (i + 1) as f64 * 0.2,
                    layer_height: 0.2,
                    ..Default::default()
                })
                .collect(),
            features,
            total_time_estimate_s: total_time,
            total_filament_mm: filament_mm,
            total_filament_volume_mm3: filament_mm * 2.405, // approx for 1.75mm
            total_filament_weight_g: filament_weight_g,
            total_travel_mm: travel_mm,
            total_extrusion_mm: extrusion_mm,
            total_moves: total_moves,
            retraction_count: retractions,
            retraction_distance_mm: retractions as f64 * 0.8,
            zhop_count: retractions / 2,
            zhop_distance_mm: retractions as f64 * 0.2,
            unknown_command_count: 0,
            line_count: 10000,
        }
    }

    #[test]
    fn delta_computation_basic() {
        let baseline = make_analysis(
            "baseline.gcode",
            600.0,
            3000.0,
            9.0,
            Some(580.0),
            100,
            200,
            5000,
            1000.0,
            4000.0,
        );
        let other = make_analysis(
            "other.gcode",
            660.0,
            3300.0,
            9.9,
            Some(640.0),
            110,
            220,
            5500,
            1100.0,
            4400.0,
        );

        let result = compare_gcode_analyses(baseline, vec![other]);
        assert_eq!(result.deltas.len(), 1);

        let delta = &result.deltas[0];
        assert_eq!(delta.filename, "other.gcode");

        // Time: 660 - 600 = 60, pct = 10%
        assert!((delta.total_time_delta_s - 60.0).abs() < 1e-6);
        assert!((delta.total_time_delta_pct - 10.0).abs() < 1e-6);

        // Header time: 640 - 580 = 60, pct = ~10.34%
        assert!(delta.header_time_delta_s.is_some());
        assert!((delta.header_time_delta_s.unwrap() - 60.0).abs() < 1e-6);

        // Filament: 3300 - 3000 = 300, pct = 10%
        assert!((delta.filament_delta_mm - 300.0).abs() < 1e-6);
        assert!((delta.filament_delta_pct - 10.0).abs() < 1e-6);

        // Weight: 9.9 - 9.0 = 0.9
        assert!((delta.filament_weight_delta_g - 0.9).abs() < 1e-6);

        // Structure
        assert_eq!(delta.layer_count_delta, 10);
        assert_eq!(delta.retraction_count_delta, 20);
        assert_eq!(delta.total_moves_delta, 500);
        assert!((delta.total_travel_delta_mm - 100.0).abs() < 1e-6);
        assert!((delta.total_extrusion_delta_mm - 400.0).abs() < 1e-6);
    }

    #[test]
    fn delta_with_zero_baseline() {
        let baseline = make_analysis("baseline.gcode", 0.0, 0.0, 0.0, None, 0, 0, 0, 0.0, 0.0);
        let other = make_analysis(
            "other.gcode",
            100.0,
            500.0,
            1.5,
            None,
            10,
            5,
            100,
            50.0,
            200.0,
        );

        let result = compare_gcode_analyses(baseline, vec![other]);
        let delta = &result.deltas[0];

        // Percentage should be 0 when baseline is 0
        assert!((delta.total_time_delta_pct).abs() < 1e-6);
        assert!((delta.filament_delta_pct).abs() < 1e-6);

        // Absolute deltas still correct
        assert!((delta.total_time_delta_s - 100.0).abs() < 1e-6);
        assert!((delta.filament_delta_mm - 500.0).abs() < 1e-6);
    }

    #[test]
    fn delta_header_time_missing_on_one_side() {
        let baseline = make_analysis(
            "baseline.gcode",
            600.0,
            3000.0,
            9.0,
            Some(580.0),
            100,
            200,
            5000,
            1000.0,
            4000.0,
        );
        let other = make_analysis(
            "other.gcode",
            660.0,
            3300.0,
            9.9,
            None,
            110,
            220,
            5500,
            1100.0,
            4400.0,
        );

        let result = compare_gcode_analyses(baseline, vec![other]);
        let delta = &result.deltas[0];

        // No header time delta when one side is missing
        assert!(delta.header_time_delta_s.is_none());
        assert!(delta.header_time_delta_pct.is_none());
    }

    #[test]
    fn per_feature_deltas_union_of_keys() {
        let mut baseline = make_analysis(
            "baseline.gcode",
            600.0,
            3000.0,
            9.0,
            None,
            100,
            200,
            5000,
            1000.0,
            4000.0,
        );
        // Add a feature only in baseline
        baseline.features.insert(
            "Bridge".to_string(),
            FeatureMetrics {
                time_estimate_s: 30.0,
                extrusion_e_mm: 50.0,
                ..Default::default()
            },
        );

        let mut other = make_analysis(
            "other.gcode",
            660.0,
            3300.0,
            9.9,
            None,
            110,
            220,
            5500,
            1100.0,
            4400.0,
        );
        // Add a feature only in other
        other.features.insert(
            "Support".to_string(),
            FeatureMetrics {
                time_estimate_s: 45.0,
                extrusion_e_mm: 80.0,
                ..Default::default()
            },
        );

        let result = compare_gcode_analyses(baseline, vec![other]);
        let delta = &result.deltas[0];

        // Union should contain all features: Outer wall, Infill, Bridge, Support
        assert!(delta.feature_deltas.contains_key("Outer wall"));
        assert!(delta.feature_deltas.contains_key("Infill"));
        assert!(delta.feature_deltas.contains_key("Bridge"));
        assert!(delta.feature_deltas.contains_key("Support"));

        // Bridge exists only in baseline: delta = 0 - 30 = -30
        let bridge = &delta.feature_deltas["Bridge"];
        assert!((bridge.time_delta_s - (-30.0)).abs() < 1e-6);

        // Support exists only in other: delta = 45 - 0 = 45
        let support = &delta.feature_deltas["Support"];
        assert!((support.time_delta_s - 45.0).abs() < 1e-6);
    }

    #[test]
    fn multiple_others() {
        let baseline = make_analysis(
            "baseline.gcode",
            600.0,
            3000.0,
            9.0,
            None,
            100,
            200,
            5000,
            1000.0,
            4000.0,
        );
        let other1 = make_analysis(
            "v1.gcode", 660.0, 3300.0, 9.9, None, 110, 220, 5500, 1100.0, 4400.0,
        );
        let other2 = make_analysis(
            "v2.gcode", 540.0, 2700.0, 8.1, None, 90, 180, 4500, 900.0, 3600.0,
        );

        let result = compare_gcode_analyses(baseline, vec![other1, other2]);
        assert_eq!(result.deltas.len(), 2);
        assert_eq!(result.deltas[0].filename, "v1.gcode");
        assert_eq!(result.deltas[1].filename, "v2.gcode");

        // v1 is 10% more time
        assert!((result.deltas[0].total_time_delta_pct - 10.0).abs() < 1e-6);
        // v2 is 10% less time
        assert!((result.deltas[1].total_time_delta_pct - (-10.0)).abs() < 1e-6);
    }

    #[test]
    fn pct_delta_helper() {
        assert!((pct_delta(100.0, 110.0) - 10.0).abs() < 1e-6);
        assert!((pct_delta(100.0, 90.0) - (-10.0)).abs() < 1e-6);
        assert!((pct_delta(0.0, 100.0)).abs() < 1e-6); // zero baseline returns 0
        assert!((pct_delta(200.0, 200.0)).abs() < 1e-6); // identical
    }
}
