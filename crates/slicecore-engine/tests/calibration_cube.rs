//! Integration tests: calibration cube G-code structure validation.
//!
//! Verifies that slicing a 20mm calibration cube produces G-code with correct
//! structure including start/end sequences, temperature commands, retraction,
//! and fan control.

use slicecore_engine::{Engine, PrintConfig};
use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;

/// Creates a 20mm x 20mm x 20mm calibration cube mesh, centered at (100, 100)
/// on a 220x220 bed.
///
/// 8 vertices, 12 triangles (2 per face). Uses the same winding convention
/// as the engine unit test `unit_cube`, scaled 20x and translated to bed center.
fn calibration_cube_20mm() -> TriangleMesh {
    let ox = 90.0; // center at X=100 (90..110)
    let oy = 90.0; // center at Y=100 (90..110)
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
        // Top face (z=20)
        [4, 5, 6],
        [4, 6, 7],
        // Bottom face (z=0)
        [1, 0, 3],
        [1, 3, 2],
        // Right face (x=ox+20)
        [1, 2, 6],
        [1, 6, 5],
        // Left face (x=ox)
        [0, 4, 7],
        [0, 7, 3],
        // Back face (y=oy+20)
        [3, 7, 6],
        [3, 6, 2],
        // Front face (y=oy)
        [0, 1, 5],
        [0, 5, 4],
    ];
    TriangleMesh::new(vertices, indices).expect("calibration cube should be valid")
}

// ---------------------------------------------------------------------------
// Test 1: Calibration cube produces non-empty G-code with expected layer count
// ---------------------------------------------------------------------------

#[test]
fn test_calibration_cube_produces_gcode() {
    let config = PrintConfig::default();
    let engine = Engine::new(config);
    let mesh = calibration_cube_20mm();

    let result = engine.slice(&mesh).expect("slice should succeed");

    // G-code should be non-empty.
    assert!(
        !result.gcode.is_empty(),
        "Calibration cube G-code should be non-empty"
    );

    // 20mm cube with 0.2mm layer_height and 0.3mm first_layer_height:
    // first layer at 0.3mm, then (20.0 - 0.3) / 0.2 = 98.5 -> ~99 layers
    // Total ~100 layers. Allow tolerance of +/-5.
    assert!(
        result.layer_count >= 95 && result.layer_count <= 105,
        "Expected ~100 layers for 20mm cube (0.3mm first + 0.2mm rest), got {}",
        result.layer_count
    );

    let gcode_str = String::from_utf8_lossy(&result.gcode);

    // Verify G-code contains expected commands.
    let expected_commands = ["G28", "M83", "M104", "M109", "M140", "M190", "G1", "M107"];
    for cmd in &expected_commands {
        assert!(
            gcode_str.contains(cmd),
            "G-code should contain {cmd}, but it was not found"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 2: G-code has start and end sequences in correct positions
// ---------------------------------------------------------------------------

#[test]
fn test_gcode_has_start_and_end_sequences() {
    let config = PrintConfig::default();
    let engine = Engine::new(config);
    let mesh = calibration_cube_20mm();

    let result = engine.slice(&mesh).expect("slice should succeed");
    let gcode_str = String::from_utf8_lossy(&result.gcode);
    let lines: Vec<&str> = gcode_str.lines().collect();

    // Verify G28 (homing) appears within the first 20 lines.
    let first_20 = &lines[..lines.len().min(20)];
    assert!(
        first_20.iter().any(|l| l.contains("G28")),
        "G28 (homing) should appear within the first 20 lines. First 20 lines:\n{}",
        first_20.join("\n")
    );

    // Verify M107 (fan off) and M84 (steppers disabled) appear in the last 10 lines.
    let last_10_start = lines.len().saturating_sub(10);
    let last_10 = &lines[last_10_start..];

    assert!(
        last_10.iter().any(|l| l.contains("M107")),
        "M107 (fan off) should appear in the last 10 lines. Last 10:\n{}",
        last_10.join("\n")
    );

    assert!(
        last_10.iter().any(|l| l.contains("M84")),
        "M84 (steppers disabled) should appear in the last 10 lines. Last 10:\n{}",
        last_10.join("\n")
    );
}

// ---------------------------------------------------------------------------
// Test 3: G-code has temperature commands matching config
// ---------------------------------------------------------------------------

#[test]
fn test_gcode_has_temperature_commands() {
    let config = PrintConfig::default();
    let engine = Engine::new(config.clone());
    let mesh = calibration_cube_20mm();

    let result = engine.slice(&mesh).expect("slice should succeed");
    let gcode_str = String::from_utf8_lossy(&result.gcode);

    // M109 (wait for nozzle temp) with first_layer_nozzle_temp.
    let expected_nozzle = format!("M109 S{:.0}", config.filament.first_layer_nozzle_temp());
    assert!(
        gcode_str.contains(&expected_nozzle),
        "G-code should contain '{}' for first-layer nozzle temp. G-code start:\n{}",
        expected_nozzle,
        gcode_str.lines().take(30).collect::<Vec<_>>().join("\n")
    );

    // M190 (wait for bed temp) with first_layer_bed_temp.
    let expected_bed = format!("M190 S{:.0}", config.filament.first_layer_bed_temp());
    assert!(
        gcode_str.contains(&expected_bed),
        "G-code should contain '{}' for first-layer bed temp",
        expected_bed,
    );
}

// ---------------------------------------------------------------------------
// Test 4: G-code has retraction patterns
// ---------------------------------------------------------------------------

#[test]
fn test_gcode_has_retraction() {
    let config = PrintConfig::default();
    let engine = Engine::new(config.clone());
    let mesh = calibration_cube_20mm();

    let result = engine.slice(&mesh).expect("slice should succeed");
    let gcode_str = String::from_utf8_lossy(&result.gcode);

    // Retraction: a negative E value in a G1 command (e.g., "G1 E-0.800").
    let has_retract = gcode_str.lines().any(|line| {
        if !line.starts_with("G1") {
            return false;
        }
        // Check for a negative E parameter.
        line.split_whitespace()
            .any(|param| param.starts_with("E-"))
    });

    assert!(
        has_retract,
        "G-code should contain retraction moves (G1 E-<distance>)"
    );
}

// ---------------------------------------------------------------------------
// Test 5: G-code has fan commands in correct positions
// ---------------------------------------------------------------------------

#[test]
fn test_gcode_has_fan_commands() {
    let mut config = PrintConfig::default();
    config.cooling.disable_fan_first_layers = 1;
    let engine = Engine::new(config);
    let mesh = calibration_cube_20mm();

    let result = engine.slice(&mesh).expect("slice should succeed");
    let gcode_str = String::from_utf8_lossy(&result.gcode);

    // G-code should contain M106 (fan on) somewhere.
    assert!(
        gcode_str.contains("M106"),
        "G-code should contain M106 (fan on)"
    );

    // M106 should NOT appear before the first layer's extrusion moves.
    // Find the first G1 move with Z parameter (start of printing), then check
    // that M106 does not appear before that region.
    let lines: Vec<&str> = gcode_str.lines().collect();

    // Find the index of the first "layer 1" comment or the second Z-move
    // as a heuristic for "after first layer".
    let first_m106_idx = lines.iter().position(|l| l.contains("M106"));
    let first_layer_z_move = lines
        .iter()
        .position(|l| l.starts_with("G1") && l.contains(" Z"));

    if let (Some(m106_idx), Some(z_idx)) = (first_m106_idx, first_layer_z_move) {
        // M106 should appear after the first extrusion at Z (i.e., after
        // the start sequence which includes G1 Z moves for homing/leveling).
        // More importantly, the fan should NOT turn on during the start sequence.
        // We just verify M106 comes after at least a few G1 moves.
        assert!(
            m106_idx > z_idx,
            "M106 (fan on) should appear after the first extrusion moves (M106 at line {}, first Z move at line {})",
            m106_idx,
            z_idx
        );
    }
}
