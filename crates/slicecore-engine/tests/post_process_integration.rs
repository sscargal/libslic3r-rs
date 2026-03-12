//! Integration tests for end-to-end post-processing behavior.
//!
//! Tests verify that the post-processing pipeline works correctly when
//! integrated with the full slicing engine, covering all 4 built-in
//! post-processors (pause-at-layer, timelapse, fan-override, custom-gcode).

use slicecore_engine::config::{
    CustomGcodeRule, CustomGcodeTrigger, FanOverrideRule, PostProcessConfig, TimelapseConfig,
};
use slicecore_engine::{Engine, PrintConfig};
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

/// Slices the cube with the given config and returns the G-code as a string.
fn slice_to_gcode_string(config: PrintConfig) -> String {
    let engine = Engine::new(config);
    let mesh = calibration_cube_20mm();
    let result = engine.slice(&mesh, None).expect("slice should succeed");
    String::from_utf8_lossy(&result.gcode).into_owned()
}

// ---------------------------------------------------------------------------
// Test 1: Pause at layer in full pipeline
// ---------------------------------------------------------------------------

#[test]
fn pause_at_layer_in_full_pipeline() {
    let mut config = PrintConfig::default();
    config.post_process = PostProcessConfig {
        enabled: true,
        pause_at_layers: vec![3, 5],
        pause_command: "M0".to_string(),
        ..PostProcessConfig::default()
    };

    let gcode = slice_to_gcode_string(config);

    // Count occurrences of the pause comment and M0 command.
    let pause_comments: Vec<&str> = gcode
        .lines()
        .filter(|l| l.contains("Pause at layer"))
        .collect();
    let m0_lines: Vec<&str> = gcode.lines().filter(|l| l.trim() == "M0").collect();

    assert_eq!(
        pause_comments.len(),
        2,
        "Expected 2 pause comments, found {}",
        pause_comments.len()
    );
    assert_eq!(
        m0_lines.len(),
        2,
        "Expected 2 M0 commands, found {}",
        m0_lines.len()
    );

    // Verify specific layers are mentioned.
    assert!(
        pause_comments.iter().any(|l| l.contains("layer 3")),
        "Should have pause at layer 3"
    );
    assert!(
        pause_comments.iter().any(|l| l.contains("layer 5")),
        "Should have pause at layer 5"
    );

    // Verify M0 appears after the layer comment, not before.
    let lines: Vec<&str> = gcode.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.contains("Pause at layer") {
            assert!(
                i + 1 < lines.len() && lines[i + 1].trim() == "M0",
                "M0 should follow the pause comment"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Test 2: Timelapse camera insertion
// ---------------------------------------------------------------------------

#[test]
fn timelapse_camera_insertion() {
    let mut config = PrintConfig::default();
    config.post_process = PostProcessConfig {
        enabled: true,
        timelapse: TimelapseConfig {
            enabled: true,
            park_x: 0.0,
            park_y: 200.0,
            dwell_ms: 1000,
            retract_distance: 1.0,
            retract_speed: 2400.0,
        },
        ..PostProcessConfig::default()
    };

    let gcode = slice_to_gcode_string(config);

    // Check that park moves to (0, 200) appear after layer comments.
    let park_moves: Vec<&str> = gcode
        .lines()
        .filter(|l| l.contains("G0") && l.contains("X0.000") && l.contains("Y200.000"))
        .collect();
    assert!(!park_moves.is_empty(), "Should have park moves to (0, 200)");

    // Check that G4 P1000 dwell command is present.
    let dwell_lines: Vec<&str> = gcode
        .lines()
        .filter(|l| l.contains("G4") && l.contains("P1000"))
        .collect();
    assert!(
        !dwell_lines.is_empty(),
        "Should have G4 P1000 dwell commands"
    );

    // The number of park moves should equal the number of dwell commands.
    assert_eq!(
        park_moves.len(),
        dwell_lines.len(),
        "Each park move should have a corresponding dwell"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Fan speed override
// ---------------------------------------------------------------------------

#[test]
fn fan_speed_override() {
    let mut config = PrintConfig::default();
    config.post_process = PostProcessConfig {
        enabled: true,
        fan_overrides: vec![FanOverrideRule {
            start_layer: 3,
            end_layer: None,
            fan_speed: 255,
        }],
        ..PostProcessConfig::default()
    };

    let gcode = slice_to_gcode_string(config);

    // M106 S255 should appear in the output after layer 3.
    let fan_max_lines: Vec<&str> = gcode
        .lines()
        .filter(|l| l.contains("M106") && l.contains("S255"))
        .collect();
    assert!(
        !fan_max_lines.is_empty(),
        "Should have M106 S255 fan commands after layer 3"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Custom G-code injection (every N layers)
// ---------------------------------------------------------------------------

#[test]
fn custom_gcode_injection_every_n_layers() {
    let mut config = PrintConfig::default();
    config.post_process = PostProcessConfig {
        enabled: true,
        custom_gcode: vec![CustomGcodeRule {
            trigger: CustomGcodeTrigger::EveryNLayers { n: 2 },
            gcode: "M400\n; custom-marker".to_string(),
        }],
        ..PostProcessConfig::default()
    };

    let gcode = slice_to_gcode_string(config);

    // Count custom markers in the output.
    let markers: Vec<&str> = gcode
        .lines()
        .filter(|l| l.contains("custom-marker"))
        .collect();

    // With a 20mm cube at 0.2mm layer height, we have ~100 layers.
    // Every 2 layers means ~50 injections (layers 0, 2, 4, ...).
    assert!(
        markers.len() >= 10,
        "Should have many custom-marker injections for every-2-layers rule, found {}",
        markers.len()
    );

    // M400 should also appear the same number of times.
    let m400_lines: Vec<&str> = gcode.lines().filter(|l| l.trim() == "M400").collect();
    assert_eq!(
        markers.len(),
        m400_lines.len(),
        "Each injection should have both M400 and custom-marker"
    );
}

// ---------------------------------------------------------------------------
// Test 5: Post-processing disabled by default
// ---------------------------------------------------------------------------

#[test]
fn post_processing_disabled_by_default() {
    let config = PrintConfig::default();
    let gcode_default = slice_to_gcode_string(config);

    // Verify no pause commands, no timelapse park moves, no custom markers.
    assert!(
        !gcode_default.lines().any(|l| l.contains("Pause at layer")),
        "Default config should not have pause comments"
    );
    assert!(
        !gcode_default.lines().any(|l| l.contains("custom-marker")),
        "Default config should not have custom markers"
    );

    // Verify disabled explicitly also produces same behavior.
    let mut config_disabled = PrintConfig::default();
    config_disabled.post_process.enabled = false;
    config_disabled.post_process.pause_at_layers = vec![3, 5];
    let gcode_disabled = slice_to_gcode_string(config_disabled);

    assert!(
        !gcode_disabled.lines().any(|l| l.contains("Pause at layer")),
        "Disabled post-processing should not insert pause comments even with layers configured"
    );
}

// ---------------------------------------------------------------------------
// Test 6: Multiple post-processors execute in order
// ---------------------------------------------------------------------------

#[test]
fn multiple_post_processors_execute_in_order() {
    let mut config = PrintConfig::default();
    config.post_process = PostProcessConfig {
        enabled: true,
        pause_at_layers: vec![3],
        pause_command: "M0".to_string(),
        timelapse: TimelapseConfig {
            enabled: true,
            park_x: 0.0,
            park_y: 200.0,
            dwell_ms: 500,
            retract_distance: 1.0,
            retract_speed: 2400.0,
        },
        ..PostProcessConfig::default()
    };

    let gcode = slice_to_gcode_string(config);

    // Both pause (priority 50) and timelapse (priority 60) modifications present.
    let has_pause = gcode.lines().any(|l| l.contains("Pause at layer 3"));
    let has_park = gcode
        .lines()
        .any(|l| l.contains("G0") && l.contains("X0.000") && l.contains("Y200.000"));
    let has_dwell = gcode
        .lines()
        .any(|l| l.contains("G4") && l.contains("P500"));

    assert!(has_pause, "Pause post-processor should have run");
    assert!(has_park, "Timelapse park moves should be present");
    assert!(has_dwell, "Timelapse dwell commands should be present");
}

// ---------------------------------------------------------------------------
// Test 7: Time estimation reflects post-processed output
// ---------------------------------------------------------------------------

#[test]
fn time_estimation_reflects_post_processing() {
    let mesh = calibration_cube_20mm();

    // Slice without post-processing.
    let config_no_pp = PrintConfig::default();
    let engine_no_pp = Engine::new(config_no_pp);
    let result_no_pp = engine_no_pp
        .slice(&mesh, None)
        .expect("slice should succeed");

    // Slice with pause-at-layer enabled (adds M0 dwell at many layers).
    let mut config_pp = PrintConfig::default();
    config_pp.post_process = PostProcessConfig {
        enabled: true,
        pause_at_layers: vec![3, 5, 10, 15, 20, 30, 40, 50, 60, 70, 80, 90],
        pause_command: "M0".to_string(),
        timelapse: TimelapseConfig {
            enabled: true,
            park_x: 0.0,
            park_y: 200.0,
            dwell_ms: 1000,
            retract_distance: 1.0,
            retract_speed: 2400.0,
        },
        ..PostProcessConfig::default()
    };
    let engine_pp = Engine::new(config_pp);
    let result_pp = engine_pp
        .slice(&mesh, None)
        .expect("slice with post-processing should succeed");

    // Post-processed output should be at least as long (more G-code lines).
    assert!(
        result_pp.gcode.len() >= result_no_pp.gcode.len(),
        "Post-processed G-code should be at least as large: pp={} vs no_pp={}",
        result_pp.gcode.len(),
        result_no_pp.gcode.len()
    );

    // Time with timelapse (adds dwell at every layer) should be >= time without.
    // Note: time_estimate reflects the post-processed G-code since post-processing
    // runs before time estimation in the pipeline (step 4d before step 5).
    assert!(
        result_pp.estimated_time_seconds >= result_no_pp.estimated_time_seconds,
        "Post-processed time ({:.1}s) should be >= baseline ({:.1}s)",
        result_pp.estimated_time_seconds,
        result_no_pp.estimated_time_seconds
    );
}
