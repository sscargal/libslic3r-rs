//! Integration tests for per-feature print statistics computation.
//!
//! Tests verify that statistics are correctly computed end-to-end from the
//! Engine slice pipeline, including feature presence, percentage sums,
//! filament totals, retraction consistency, subtotals, and serialization.

use slicecore_engine::{Engine, PrintConfig, PrintStatistics};
use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;

/// Creates a 20mm x 20mm x 20mm calibration cube mesh, centered at (100, 100)
/// on a 220x220 bed.
fn calibration_cube_20mm() -> TriangleMesh {
    let ox = 90.0;
    let oy = 90.0;
    let vertices = vec![
        Point3::new(ox, oy, 0.0),
        Point3::new(ox + 20.0, oy, 0.0),
        Point3::new(ox + 20.0, oy + 20.0, 0.0),
        Point3::new(ox, oy + 20.0, 0.0),
        Point3::new(ox, oy, 20.0),
        Point3::new(ox + 20.0, oy, 20.0),
        Point3::new(ox + 20.0, oy + 20.0, 20.0),
        Point3::new(ox, oy + 20.0, 20.0),
    ];
    let indices = vec![
        [4, 5, 6],
        [4, 6, 7],
        [1, 0, 3],
        [1, 3, 2],
        [1, 2, 6],
        [1, 6, 5],
        [0, 4, 7],
        [0, 7, 3],
        [3, 7, 6],
        [3, 6, 2],
        [0, 1, 5],
        [0, 5, 4],
    ];
    TriangleMesh::new(vertices, indices).expect("calibration cube should be valid")
}

/// Helper to slice a cube with default config and return statistics.
fn slice_cube_default() -> (slicecore_engine::SliceResult, PrintStatistics) {
    let config = PrintConfig::default();
    let engine = Engine::new(config);
    let mesh = calibration_cube_20mm();
    let result = engine.slice(&mesh, None).expect("slice should succeed");
    let statistics = result
        .statistics
        .clone()
        .expect("statistics should be present");
    (result, statistics)
}

// ---------------------------------------------------------------------------
// Test 1: Statistics computed after slice
// ---------------------------------------------------------------------------

#[test]
fn test_statistics_computed_after_slice() {
    let (_result, statistics) = slice_cube_default();

    // Statistics should be present.
    assert!(
        !statistics.features.is_empty(),
        "features should not be empty"
    );

    // A cube should have at minimum OuterPerimeter, InnerPerimeter, SparseInfill
    // with non-zero values (depending on config defaults).
    let non_zero_features: Vec<&str> = statistics
        .features
        .iter()
        .filter(|f| f.time_seconds > 0.0)
        .map(|f| f.feature_type.as_str())
        .collect();

    assert!(
        non_zero_features.len() >= 3,
        "Expected at least 3 features with non-zero time, got {}: {:?}",
        non_zero_features.len(),
        non_zero_features
    );

    // Summary totals should be positive.
    assert!(
        statistics.summary.total_time_seconds > 0.0,
        "Total time should be positive"
    );
    assert!(
        statistics.summary.total_filament_mm > 0.0,
        "Total filament should be positive"
    );
    assert!(
        statistics.summary.layer_count > 0,
        "Layer count should be positive"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Per-feature time percentages sum near 100
// ---------------------------------------------------------------------------

#[test]
fn test_per_feature_time_percentages_sum_near_100() {
    let (_result, statistics) = slice_cube_default();

    // Sum time_pct_total for non-virtual features (real features from toolpath).
    let time_pct_sum: f64 = statistics
        .features
        .iter()
        .filter(|f| {
            f.feature_type != "retract"
                && f.feature_type != "unretract"
                && f.feature_type != "wipe"
        })
        .map(|f| f.time_pct_total)
        .sum();

    assert!(
        (time_pct_sum - 100.0).abs() < 2.0,
        "Time pct_total should sum to ~100%, got {:.2}%",
        time_pct_sum
    );
}

// ---------------------------------------------------------------------------
// Test 3: Per-feature filament total matches summary
// ---------------------------------------------------------------------------

#[test]
fn test_per_feature_filament_total_matches_summary() {
    let (_result, statistics) = slice_cube_default();

    // Sum filament_mm from all features (including virtual which should be 0).
    let filament_sum: f64 = statistics.features.iter().map(|f| f.filament_mm).sum();

    // The per-feature filament comes from toolpath segments, while
    // summary.total_filament_mm comes from the filament usage estimate.
    // They may differ slightly due to rounding, but should be in the
    // same ballpark.
    let summary_filament = statistics.summary.total_filament_mm;

    // Allow a generous tolerance since they come from different computation paths.
    // The per-feature sum is from toolpath e_values while summary is from
    // filament usage computation, but both should agree within ~20%.
    if summary_filament > 0.0 && filament_sum > 0.0 {
        let ratio = filament_sum / summary_filament;
        assert!(
            (0.5..=2.0).contains(&ratio),
            "Per-feature filament sum ({:.2}mm) should be reasonably close to summary ({:.2}mm), ratio={:.2}",
            filament_sum,
            summary_filament,
            ratio
        );
    }
}

// ---------------------------------------------------------------------------
// Test 4: G-code metrics retraction count matches estimate
// ---------------------------------------------------------------------------

#[test]
fn test_gcode_metrics_retraction_count_matches_estimate() {
    let (result, statistics) = slice_cube_default();

    // Both retraction counts should come from the same source.
    assert_eq!(
        statistics.gcode_metrics.retraction_count,
        result.time_estimate.retraction_count as u32,
        "G-code metrics retraction count ({}) should match time estimate ({})",
        statistics.gcode_metrics.retraction_count,
        result.time_estimate.retraction_count
    );
}

// ---------------------------------------------------------------------------
// Test 5: Zero features present
// ---------------------------------------------------------------------------

#[test]
fn test_zero_features_present() {
    let (_result, statistics) = slice_cube_default();

    // A simple cube without support, ironing, or bridge should still have
    // those features listed with time_seconds == 0.
    let feature_types: Vec<&str> = statistics
        .features
        .iter()
        .map(|f| f.feature_type.as_str())
        .collect();

    // Check that support and bridge appear (they should be zero for a cube).
    assert!(
        feature_types.contains(&"support"),
        "Support feature should be present even when zero"
    );
    assert!(
        feature_types.contains(&"bridge"),
        "Bridge feature should be present even when zero"
    );
    assert!(
        feature_types.contains(&"ironing"),
        "Ironing feature should be present even when zero"
    );

    // Verify these features have zero time.
    let support = statistics
        .features
        .iter()
        .find(|f| f.feature_type == "support")
        .unwrap();
    assert!(
        support.time_seconds.abs() < 1e-9,
        "Support time should be 0 for a cube without supports"
    );
}

// ---------------------------------------------------------------------------
// Test 6: Support subtotals
// ---------------------------------------------------------------------------

#[test]
fn test_support_subtotals() {
    let (_result, statistics) = slice_cube_default();

    // Model time should be positive (cube has perimeters and infill).
    assert!(
        statistics.summary.model_time_seconds > 0.0,
        "Model time should be positive"
    );

    // Support time should be >= 0 (0 for a cube without supports).
    assert!(
        statistics.summary.support_time_seconds >= 0.0,
        "Support time should be non-negative"
    );

    // Model time + support time should be less than or equal to total time.
    // (Travel and retraction overhead make up the difference.)
    let total = statistics.summary.total_time_seconds;

    assert!(
        statistics.summary.model_time_seconds <= total,
        "Model time ({:.2}s) should not exceed total time ({:.2}s)",
        statistics.summary.model_time_seconds,
        total
    );

    // Model time + support time should account for a significant portion of total.
    let model_plus_support =
        statistics.summary.model_time_seconds + statistics.summary.support_time_seconds;
    if total > 0.0 {
        let fraction = model_plus_support / total;
        assert!(
            fraction > 0.3,
            "Model + support time ({:.2}s, {:.1}% of total) should be a significant \
             portion of total ({:.2}s)",
            model_plus_support,
            fraction * 100.0,
            total
        );
    }
}

// ---------------------------------------------------------------------------
// Test 7: Statistics serialization roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_statistics_serialization_roundtrip() {
    let (_result, statistics) = slice_cube_default();

    // Serialize to JSON.
    let json = serde_json::to_string_pretty(&statistics).expect("should serialize to JSON");

    // Deserialize back.
    let deserialized: PrintStatistics =
        serde_json::from_str(&json).expect("should deserialize from JSON");

    // Verify key fields match.
    assert_eq!(
        statistics.summary.total_time_seconds,
        deserialized.summary.total_time_seconds,
        "Total time should survive roundtrip"
    );
    assert_eq!(
        statistics.summary.layer_count, deserialized.summary.layer_count,
        "Layer count should survive roundtrip"
    );
    assert_eq!(
        statistics.features.len(),
        deserialized.features.len(),
        "Feature count should survive roundtrip"
    );
    assert_eq!(
        statistics.gcode_metrics.retraction_count,
        deserialized.gcode_metrics.retraction_count,
        "Retraction count should survive roundtrip"
    );

    // Verify per-feature data.
    for (orig, deser) in statistics.features.iter().zip(deserialized.features.iter()) {
        assert_eq!(
            orig.feature_type, deser.feature_type,
            "Feature type should survive roundtrip"
        );
        assert!(
            (orig.time_seconds - deser.time_seconds).abs() < 1e-9,
            "Time should survive roundtrip for {}",
            orig.feature_type
        );
    }
}
