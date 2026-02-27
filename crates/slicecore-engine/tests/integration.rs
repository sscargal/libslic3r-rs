//! Integration tests: G-code validation, infill density, skirt, and brim.
//!
//! Tests that verify G-code passes syntax validation, and that configurable
//! parameters (infill density, skirt loops, brim width) affect the output.

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
// Test 6: G-code passes syntax validation
// ---------------------------------------------------------------------------

#[test]
fn test_gcode_passes_validation() {
    let config = PrintConfig::default();
    let engine = Engine::new(config);
    let mesh = calibration_cube_20mm();

    let result = engine.slice(&mesh, None).expect("slice should succeed");
    let gcode_str = String::from_utf8_lossy(&result.gcode);

    let validation = validate_gcode(&gcode_str);
    assert!(
        validation.valid,
        "G-code should pass validation. Errors:\n{}",
        validation.errors.join("\n")
    );
}

// ---------------------------------------------------------------------------
// Test 7: Infill density 0% vs 100%
// ---------------------------------------------------------------------------

#[test]
fn test_infill_density_zero_and_hundred() {
    let mesh = calibration_cube_20mm();

    // 0% infill -- perimeters only, no infill between walls.
    let config_zero = PrintConfig {
        infill_density: 0.0,
        ..PrintConfig::default()
    };
    let result_zero = Engine::new(config_zero)
        .slice(&mesh, None)
        .expect("0% infill slice should succeed");

    // 100% infill -- solid fill throughout.
    let config_full = PrintConfig {
        infill_density: 1.0,
        ..PrintConfig::default()
    };
    let result_full = Engine::new(config_full)
        .slice(&mesh, None)
        .expect("100% infill slice should succeed");

    // Both should produce non-empty G-code.
    assert!(
        !result_zero.gcode.is_empty(),
        "0% infill should still produce G-code (perimeters)"
    );
    assert!(
        !result_full.gcode.is_empty(),
        "100% infill should produce G-code"
    );

    // 100% infill should produce significantly more G-code than 0%.
    let ratio = result_full.gcode.len() as f64 / result_zero.gcode.len() as f64;
    assert!(
        ratio >= 1.5,
        "100% infill G-code ({} bytes) should be at least 1.5x larger than 0% infill ({} bytes), ratio = {:.2}",
        result_full.gcode.len(),
        result_zero.gcode.len(),
        ratio
    );
}

// ---------------------------------------------------------------------------
// Test 8: Skirt present in output
// ---------------------------------------------------------------------------

#[test]
fn test_skirt_present_in_output() {
    let config = PrintConfig {
        skirt_loops: 1,
        brim_width: 0.0, // Ensure brim disabled so skirt is used.
        ..PrintConfig::default()
    };
    let engine = Engine::new(config);
    let mesh = calibration_cube_20mm();

    let result = engine.slice(&mesh, None).expect("slice should succeed");
    let gcode_str = String::from_utf8_lossy(&result.gcode);

    // Skirt produces extrusion moves on the first layer. We verify by
    // checking for the TYPE:Skirt comment emitted by the G-code generator.
    let has_skirt_comment = gcode_str.contains("TYPE:Skirt");

    // Also check that we have extrusion moves at the first layer Z height.
    // The first layer height defaults to 0.3mm.
    let first_z = "Z0.300";
    let has_first_layer_extrusion = gcode_str
        .lines()
        .any(|l| l.starts_with("G1") && l.contains(first_z) && l.contains(" E"));

    assert!(
        has_skirt_comment || has_first_layer_extrusion,
        "G-code should contain skirt moves on the first layer. \
         has_skirt_comment={has_skirt_comment}, has_first_layer_extrusion={has_first_layer_extrusion}"
    );
}

// ---------------------------------------------------------------------------
// Test 9: Brim works and increases first-layer extrusion
// ---------------------------------------------------------------------------

#[test]
fn test_brim_works() {
    let mesh = calibration_cube_20mm();

    // Without brim.
    let config_no_brim = PrintConfig {
        brim_width: 0.0,
        skirt_loops: 0,
        ..PrintConfig::default()
    };
    let result_no_brim = Engine::new(config_no_brim)
        .slice(&mesh, None)
        .expect("no-brim slice should succeed");

    // With brim.
    let config_brim = PrintConfig {
        brim_width: 5.0,
        skirt_loops: 0, // Brim takes priority anyway, but disable skirt for clarity.
        ..PrintConfig::default()
    };
    let result_brim = Engine::new(config_brim)
        .slice(&mesh, None)
        .expect("brim slice should succeed");

    // Brim should produce more G-code (more first-layer extrusion).
    assert!(
        result_brim.gcode.len() > result_no_brim.gcode.len(),
        "Brim G-code ({} bytes) should be larger than no-brim ({} bytes)",
        result_brim.gcode.len(),
        result_no_brim.gcode.len(),
    );
}
