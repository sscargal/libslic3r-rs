//! Integration tests: determinism and layer height variation.
//!
//! These tests directly verify Phase 3 Success Criteria:
//! - SC3: Deterministic output -- same input produces bit-for-bit identical G-code
//! - SC4: Layer height variation -- changing layer_height from 0.2 to 0.1 roughly doubles layer count
//!
//! Phase 3 SC verification summary:
//! - SC1: Calibration cube produces G-code (tested in calibration_cube.rs)
//! - SC2: CLI accepts correct arguments (tested in 03-05 plan build + help text)
//! - SC3: Deterministic output (test_deterministic_output below)
//! - SC4: Layer height variation (test_layer_height_variation below)
//! - SC5: Skirt/brim and infill density (tested in integration.rs)

use slicecore_engine::{Engine, PrintConfig};
use slicecore_gcode_io::validate_gcode;
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

// ---------------------------------------------------------------------------
// Test 1: Deterministic output with default config (SC3)
// ---------------------------------------------------------------------------

#[test]
fn test_deterministic_output() {
    let config = PrintConfig::default();
    let mesh = calibration_cube_20mm();

    let engine1 = Engine::new(config.clone());
    let result1 = engine1
        .slice(&mesh, None)
        .expect("first slice should succeed");

    let engine2 = Engine::new(config);
    let result2 = engine2
        .slice(&mesh, None)
        .expect("second slice should succeed");

    assert_eq!(
        result1.gcode,
        result2.gcode,
        "Determinism: identical input must produce bit-for-bit identical G-code output. \
         First output: {} bytes, second output: {} bytes",
        result1.gcode.len(),
        result2.gcode.len()
    );

    assert_eq!(
        result1.layer_count, result2.layer_count,
        "Determinism: layer counts must be identical"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Deterministic output with custom config
// ---------------------------------------------------------------------------

#[test]
fn test_deterministic_with_custom_config() {
    let config = PrintConfig {
        layer_height: 0.15,
        infill_density: 0.3,
        wall_count: 3,
        ..PrintConfig::default()
    };
    let mesh = calibration_cube_20mm();

    let engine1 = Engine::new(config.clone());
    let result1 = engine1
        .slice(&mesh, None)
        .expect("first slice should succeed");

    let engine2 = Engine::new(config);
    let result2 = engine2
        .slice(&mesh, None)
        .expect("second slice should succeed");

    assert_eq!(
        result1.gcode, result2.gcode,
        "Custom config: identical input must produce bit-for-bit identical G-code output"
    );

    assert_eq!(
        result1.layer_count, result2.layer_count,
        "Custom config: layer counts must be identical"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Layer height variation (SC4)
// ---------------------------------------------------------------------------

#[test]
fn test_layer_height_variation() {
    let mesh = calibration_cube_20mm();

    // 0.2mm layers.
    let config_02 = PrintConfig {
        layer_height: 0.2,
        first_layer_height: 0.2,
        ..PrintConfig::default()
    };
    let result_02 = Engine::new(config_02)
        .slice(&mesh, None)
        .expect("0.2mm slice should succeed");

    // 0.1mm layers.
    let config_01 = PrintConfig {
        layer_height: 0.1,
        first_layer_height: 0.1,
        ..PrintConfig::default()
    };
    let result_01 = Engine::new(config_01)
        .slice(&mesh, None)
        .expect("0.1mm slice should succeed");

    // Both should produce non-empty G-code.
    assert!(
        !result_02.gcode.is_empty(),
        "0.2mm slice should produce non-empty G-code"
    );
    assert!(
        !result_01.gcode.is_empty(),
        "0.1mm slice should produce non-empty G-code"
    );

    // 0.1mm layers should produce approximately 2x the layer count.
    // Allow 10% tolerance for first-layer height effects and rounding.
    let ratio = result_01.layer_count as f64 / result_02.layer_count as f64;
    assert!(
        ratio >= 1.8 && ratio <= 2.2,
        "Layer height variation: 0.1mm layers ({}) should be ~2x 0.2mm layers ({}), ratio = {:.2}",
        result_01.layer_count,
        result_02.layer_count,
        ratio
    );
}

// ---------------------------------------------------------------------------
// Test 4: Both layer heights produce valid G-code
// ---------------------------------------------------------------------------

#[test]
fn test_layer_height_produces_valid_gcode_both() {
    let mesh = calibration_cube_20mm();

    // 0.2mm layers.
    let config_02 = PrintConfig {
        layer_height: 0.2,
        first_layer_height: 0.2,
        ..PrintConfig::default()
    };
    let result_02 = Engine::new(config_02)
        .slice(&mesh, None)
        .expect("0.2mm slice should succeed");
    let gcode_02 = String::from_utf8_lossy(&result_02.gcode);
    let validation_02 = validate_gcode(&gcode_02);
    assert!(
        validation_02.valid,
        "0.2mm layer height G-code should pass validation. Errors:\n{}",
        validation_02.errors.join("\n")
    );

    // 0.1mm layers.
    let config_01 = PrintConfig {
        layer_height: 0.1,
        first_layer_height: 0.1,
        ..PrintConfig::default()
    };
    let result_01 = Engine::new(config_01)
        .slice(&mesh, None)
        .expect("0.1mm slice should succeed");
    let gcode_01 = String::from_utf8_lossy(&result_01.gcode);
    let validation_01 = validate_gcode(&gcode_01);
    assert!(
        validation_01.valid,
        "0.1mm layer height G-code should pass validation. Errors:\n{}",
        validation_01.errors.join("\n")
    );
}

// ---------------------------------------------------------------------------
// Test 5: Different configs produce different output (sanity check)
// ---------------------------------------------------------------------------

#[test]
fn test_different_configs_produce_different_output() {
    let mesh = calibration_cube_20mm();

    let config_a = PrintConfig {
        layer_height: 0.2,
        first_layer_height: 0.2,
        ..PrintConfig::default()
    };
    let result_a = Engine::new(config_a)
        .slice(&mesh, None)
        .expect("config A slice should succeed");

    let config_b = PrintConfig {
        layer_height: 0.1,
        first_layer_height: 0.1,
        ..PrintConfig::default()
    };
    let result_b = Engine::new(config_b)
        .slice(&mesh, None)
        .expect("config B slice should succeed");

    assert_ne!(
        result_a.gcode, result_b.gcode,
        "Different configs (0.2mm vs 0.1mm layer height) must produce different G-code output"
    );

    assert_ne!(
        result_a.layer_count, result_b.layer_count,
        "Different layer heights should produce different layer counts"
    );
}

// ===========================================================================
// Parallel vs Sequential Determinism Tests (Phase 25)
// ===========================================================================

/// Verifies that parallel and sequential slicing produce byte-identical G-code
/// for the same input mesh and config with default settings.
#[cfg(feature = "parallel")]
#[test]
fn test_parallel_sequential_determinism() {
    let mesh = calibration_cube_20mm();

    let config_parallel = PrintConfig {
        parallel_slicing: true,
        ..PrintConfig::default()
    };
    let config_sequential = PrintConfig {
        parallel_slicing: false,
        ..PrintConfig::default()
    };

    let result_parallel = Engine::new(config_parallel)
        .slice(&mesh, None)
        .expect("parallel slice should succeed");

    let result_sequential = Engine::new(config_sequential)
        .slice(&mesh, None)
        .expect("sequential slice should succeed");

    assert_eq!(
        result_parallel.layer_count, result_sequential.layer_count,
        "Parallel and sequential should produce same layer count: parallel={}, sequential={}",
        result_parallel.layer_count, result_sequential.layer_count
    );

    assert_eq!(
        result_parallel.gcode,
        result_sequential.gcode,
        "Parallel and sequential G-code must be byte-identical. \
         Parallel: {} bytes, Sequential: {} bytes",
        result_parallel.gcode.len(),
        result_sequential.gcode.len()
    );
}

/// Verifies parallel/sequential determinism with Aligned seam position
/// (the seam strategy most affected by parallelization due to cross-layer tracking).
#[cfg(feature = "parallel")]
#[test]
fn test_parallel_sequential_determinism_aligned_seam() {
    use slicecore_engine::seam::SeamPosition;

    let mesh = calibration_cube_20mm();

    let config_parallel = PrintConfig {
        parallel_slicing: true,
        seam_position: SeamPosition::Aligned,
        ..PrintConfig::default()
    };
    let config_sequential = PrintConfig {
        parallel_slicing: false,
        seam_position: SeamPosition::Aligned,
        ..PrintConfig::default()
    };

    let result_parallel = Engine::new(config_parallel)
        .slice(&mesh, None)
        .expect("parallel aligned-seam slice should succeed");

    let result_sequential = Engine::new(config_sequential)
        .slice(&mesh, None)
        .expect("sequential aligned-seam slice should succeed");

    assert_eq!(
        result_parallel.gcode,
        result_sequential.gcode,
        "Aligned seam: parallel and sequential G-code must be byte-identical. \
         Parallel: {} bytes, Sequential: {} bytes",
        result_parallel.gcode.len(),
        result_sequential.gcode.len()
    );
}

/// Verifies parallel/sequential determinism with Grid infill pattern.
#[cfg(feature = "parallel")]
#[test]
fn test_parallel_sequential_determinism_grid_infill() {
    use slicecore_engine::infill::InfillPattern;

    let mesh = calibration_cube_20mm();

    let config_parallel = PrintConfig {
        parallel_slicing: true,
        infill_pattern: InfillPattern::Grid,
        infill_density: 0.2,
        ..PrintConfig::default()
    };
    let config_sequential = PrintConfig {
        parallel_slicing: false,
        infill_pattern: InfillPattern::Grid,
        infill_density: 0.2,
        ..PrintConfig::default()
    };

    let result_parallel = Engine::new(config_parallel)
        .slice(&mesh, None)
        .expect("parallel grid slice should succeed");

    let result_sequential = Engine::new(config_sequential)
        .slice(&mesh, None)
        .expect("sequential grid slice should succeed");

    assert_eq!(
        result_parallel.gcode, result_sequential.gcode,
        "Grid infill: parallel and sequential G-code must be byte-identical"
    );
}

/// Verifies parallel/sequential determinism with Rectilinear infill pattern.
#[cfg(feature = "parallel")]
#[test]
fn test_parallel_sequential_determinism_rectilinear_infill() {
    use slicecore_engine::infill::InfillPattern;

    let mesh = calibration_cube_20mm();

    let config_parallel = PrintConfig {
        parallel_slicing: true,
        infill_pattern: InfillPattern::Rectilinear,
        infill_density: 0.15,
        ..PrintConfig::default()
    };
    let config_sequential = PrintConfig {
        parallel_slicing: false,
        infill_pattern: InfillPattern::Rectilinear,
        infill_density: 0.15,
        ..PrintConfig::default()
    };

    let result_parallel = Engine::new(config_parallel)
        .slice(&mesh, None)
        .expect("parallel rectilinear slice should succeed");

    let result_sequential = Engine::new(config_sequential)
        .slice(&mesh, None)
        .expect("sequential rectilinear slice should succeed");

    assert_eq!(
        result_parallel.gcode, result_sequential.gcode,
        "Rectilinear infill: parallel and sequential G-code must be byte-identical"
    );
}

/// Verifies that parallel_slicing=false produces identical output to previous runs.
#[test]
fn test_sequential_explicit_determinism() {
    let mesh = calibration_cube_20mm();

    let config = PrintConfig {
        parallel_slicing: false,
        ..PrintConfig::default()
    };

    let result1 = Engine::new(config.clone())
        .slice(&mesh, None)
        .expect("first sequential slice should succeed");

    let result2 = Engine::new(config)
        .slice(&mesh, None)
        .expect("second sequential slice should succeed");

    assert_eq!(
        result1.gcode, result2.gcode,
        "Explicit parallel_slicing=false must produce deterministic output"
    );
}
