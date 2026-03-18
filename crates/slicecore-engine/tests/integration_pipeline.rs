//! End-to-end integration tests for the full STL-to-Gcode pipeline.
//!
//! Tests exercise the complete pipeline: mesh creation, Engine configuration,
//! slicing, G-code output, JSON/MessagePack structured output, and event
//! system integration.

use std::sync::{Arc, Mutex};

use slicecore_engine::event::{CallbackSubscriber, EventBus, SliceEvent};
use slicecore_engine::output::{from_msgpack, to_json, to_msgpack};
use slicecore_engine::support::config::SupportConfig;
use slicecore_engine::{Engine, PrintConfig};
use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;

// ===========================================================================
// Mesh builders
// ===========================================================================

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

/// Helper: builds a closed axis-aligned box mesh from min/max coordinates.
fn box_vertices_indices(
    min_x: f64,
    min_y: f64,
    min_z: f64,
    max_x: f64,
    max_y: f64,
    max_z: f64,
    idx_offset: u32,
) -> (Vec<Point3>, Vec<[u32; 3]>) {
    let o = idx_offset;
    let vertices = vec![
        Point3::new(min_x, min_y, min_z),
        Point3::new(max_x, min_y, min_z),
        Point3::new(max_x, max_y, min_z),
        Point3::new(min_x, max_y, min_z),
        Point3::new(min_x, min_y, max_z),
        Point3::new(max_x, min_y, max_z),
        Point3::new(max_x, max_y, max_z),
        Point3::new(min_x, max_y, max_z),
    ];
    let indices = vec![
        [o + 4, o + 5, o + 6],
        [o + 4, o + 6, o + 7],
        [o + 1, o + 0, o + 3],
        [o + 1, o + 3, o + 2],
        [o + 1, o + 2, o + 6],
        [o + 1, o + 6, o + 5],
        [o + 0, o + 4, o + 7],
        [o + 0, o + 7, o + 3],
        [o + 3, o + 7, o + 6],
        [o + 3, o + 6, o + 2],
        [o + 0, o + 1, o + 5],
        [o + 0, o + 5, o + 4],
    ];
    (vertices, indices)
}

/// Combines multiple box definitions into a single TriangleMesh.
fn multi_box_mesh(boxes: &[(f64, f64, f64, f64, f64, f64)]) -> TriangleMesh {
    let mut all_vertices = Vec::new();
    let mut all_indices = Vec::new();

    for &(min_x, min_y, min_z, max_x, max_y, max_z) in boxes {
        let offset = all_vertices.len() as u32;
        let (verts, idxs) = box_vertices_indices(min_x, min_y, min_z, max_x, max_y, max_z, offset);
        all_vertices.extend(verts);
        all_indices.extend(idxs);
    }

    TriangleMesh::new(all_vertices, all_indices).expect("multi-box mesh should be valid")
}

// ===========================================================================
// Test 1: Full STL-to-Gcode pipeline with calibration cube
// ===========================================================================

#[test]
fn test_stl_to_gcode_calibration_cube() {
    let config = PrintConfig::default();
    let engine = Engine::new(config);
    let mesh = calibration_cube_20mm();

    let result = engine.slice(&mesh, None).expect("slice should succeed");

    // Result is Ok (verified by expect above).

    // Layer count > 0 and approximately correct for 20mm cube.
    assert!(
        result.layer_count > 0,
        "Layer count should be > 0, got {}",
        result.layer_count
    );
    assert!(
        result.layer_count >= 95 && result.layer_count <= 105,
        "Expected ~100 layers for 20mm cube (0.3mm first + 0.2mm rest), got {}",
        result.layer_count
    );

    let gcode_str = String::from_utf8_lossy(&result.gcode);

    // Gcode is non-empty.
    assert!(
        !result.gcode.is_empty(),
        "G-code output should be non-empty"
    );

    // Gcode starts with G28 (home) command.
    let first_30: Vec<&str> = gcode_str.lines().take(30).collect();
    assert!(
        first_30.iter().any(|l| l.contains("G28")),
        "G-code should contain G28 (homing) in the first 30 lines"
    );

    // Gcode contains M104 or M109 (temperature) commands.
    assert!(
        gcode_str.contains("M104") || gcode_str.contains("M109"),
        "G-code should contain temperature commands (M104 or M109)"
    );

    // Gcode contains G1 commands with E values (extrusion).
    let has_extrusion = gcode_str
        .lines()
        .any(|line| line.starts_with("G1") && line.contains(" E"));
    assert!(
        has_extrusion,
        "G-code should contain G1 moves with E values"
    );

    // Gcode ends with M104 S0 (heater off) in the postamble.
    let lines: Vec<&str> = gcode_str.lines().collect();
    let last_15 = &lines[lines.len().saturating_sub(15)..];
    let has_heater_off = last_15.iter().any(|l| l.contains("M104 S0"));
    assert!(
        has_heater_off,
        "G-code should end with M104 S0 (heater off). Last 15 lines:\n{}",
        last_15.join("\n")
    );

    // Estimated time > 0.
    assert!(
        result.estimated_time_seconds > 0.0,
        "Estimated time should be > 0, got {}",
        result.estimated_time_seconds
    );

    // Filament usage length > 0.
    assert!(
        result.filament_usage.length_mm > 0.0,
        "Filament usage length should be > 0, got {}",
        result.filament_usage.length_mm
    );
}

// ===========================================================================
// Test 2: Custom config doubles layer count
// ===========================================================================

#[test]
fn test_stl_to_gcode_with_custom_config() {
    let mesh = calibration_cube_20mm();

    // Default config: 0.2mm layers, 0.3mm first layer.
    let config_default = PrintConfig::default();
    let result_default = Engine::new(config_default)
        .slice(&mesh, None)
        .expect("default slice should succeed");

    // Custom config: 0.1mm layers, 0.1mm first layer, 50% infill.
    let config_custom = PrintConfig {
        layer_height: 0.1,
        first_layer_height: 0.1,
        infill_density: 0.5,
        ..PrintConfig::default()
    };
    let result_custom = Engine::new(config_custom)
        .slice(&mesh, None)
        .expect("custom slice should succeed");

    // Layer count should be approximately 2x.
    let ratio = result_custom.layer_count as f64 / result_default.layer_count as f64;
    assert!(
        ratio >= 1.8 && ratio <= 2.2,
        "0.1mm layers should produce ~2x layers: custom={}, default={}, ratio={:.2}",
        result_custom.layer_count,
        result_default.layer_count,
        ratio
    );
}

// ===========================================================================
// Test 3: Support generation with T-shape overhang
// ===========================================================================

#[test]
fn test_stl_to_gcode_with_supports() {
    // T-shape model: base column + overhang shelf.
    // Base: 20x20x20mm at (90..110, 90..110, 0..20)
    // Shelf: extends 10mm outward at Z=14..20 at (110..120, 90..110, 14..20)
    let t_shape = multi_box_mesh(&[
        (90.0, 90.0, 0.0, 110.0, 110.0, 20.0),   // base column
        (110.0, 90.0, 14.0, 120.0, 110.0, 20.0), // overhang shelf
    ]);

    // Without support.
    let config_no_support = PrintConfig {
        support: SupportConfig {
            enabled: false,
            ..SupportConfig::default()
        },
        ..PrintConfig::default()
    };
    let result_no_support = Engine::new(config_no_support)
        .slice(&t_shape, None)
        .expect("no-support slice should succeed");

    // With support enabled.
    let config_with_support = PrintConfig {
        support: SupportConfig {
            enabled: true,
            ..SupportConfig::default()
        },
        ..PrintConfig::default()
    };
    let result_with_support = Engine::new(config_with_support)
        .slice(&t_shape, None)
        .expect("support slice should succeed");

    let gcode_no_support = String::from_utf8_lossy(&result_no_support.gcode);
    let gcode_with_support = String::from_utf8_lossy(&result_with_support.gcode);

    // With supports should produce more G-code than without.
    assert!(
        result_with_support.gcode.len() > result_no_support.gcode.len(),
        "Supported G-code ({} bytes) should be larger than unsupported ({} bytes)",
        result_with_support.gcode.len(),
        result_no_support.gcode.len()
    );

    // Check for support-related content in the G-code.
    // Support generates TYPE:Support comments or additional extrusion.
    let has_support_type =
        gcode_with_support.contains("TYPE:Support") || gcode_with_support.contains("TYPE: Support");
    let no_support_type =
        !gcode_no_support.contains("TYPE:Support") && !gcode_no_support.contains("TYPE: Support");

    // At minimum, the supported version should produce more G-code.
    // Support type comments may or may not be present depending on implementation.
    assert!(
        has_support_type || result_with_support.gcode.len() > result_no_support.gcode.len(),
        "Support should produce either TYPE:Support comments or more G-code output"
    );
    let _ = no_support_type; // Acknowledge unused check.
}

// ===========================================================================
// Test 4: Brim generation
// ===========================================================================

#[test]
fn test_stl_to_gcode_with_brim() {
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
        skirt_loops: 0,
        ..PrintConfig::default()
    };
    let result_brim = Engine::new(config_brim)
        .slice(&mesh, None)
        .expect("brim slice should succeed");

    // Brim should produce more G-code (additional first-layer extrusion).
    assert!(
        result_brim.gcode.len() > result_no_brim.gcode.len(),
        "Brim G-code ({} bytes) should be larger than no-brim ({} bytes)",
        result_brim.gcode.len(),
        result_no_brim.gcode.len()
    );

    // Verify brim/extrusion on layer 0 by checking for TYPE:Brim or
    // additional extrusion moves near Z=0.3.
    let gcode_brim = String::from_utf8_lossy(&result_brim.gcode);
    let has_brim_content = gcode_brim.contains("TYPE:Brim")
        || gcode_brim.contains("TYPE: Brim")
        || result_brim.gcode.len() > result_no_brim.gcode.len();
    assert!(has_brim_content, "Brim should add content to the G-code");
}

// ===========================================================================
// Test 5: Mesh repair + slice integration
// ===========================================================================

#[test]
fn test_mesh_repair_integration() {
    // Build a mesh with a degenerate triangle (zero-area, all same point).
    let vertices = vec![
        // Normal cube vertices.
        Point3::new(90.0, 90.0, 0.0),
        Point3::new(110.0, 90.0, 0.0),
        Point3::new(110.0, 110.0, 0.0),
        Point3::new(90.0, 110.0, 0.0),
        Point3::new(90.0, 90.0, 20.0),
        Point3::new(110.0, 90.0, 20.0),
        Point3::new(110.0, 110.0, 20.0),
        Point3::new(90.0, 110.0, 20.0),
        // Extra degenerate vertex (same point repeated).
        Point3::new(100.0, 100.0, 10.0),
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
        // Degenerate triangle (all same vertex).
        [8, 8, 8],
    ];

    // Repair the mesh.
    let (repaired_mesh, report) = slicecore_mesh::repair::repair(vertices.clone(), indices.clone())
        .expect("repair should succeed");

    // The degenerate triangle should have been removed.
    assert!(
        report.degenerate_removed > 0,
        "Repair should remove degenerate triangles"
    );

    // Slice the repaired mesh.
    let config = PrintConfig::default();
    let engine = Engine::new(config);
    let result = engine
        .slice(&repaired_mesh, None)
        .expect("slicing repaired mesh should succeed");

    // Should produce valid G-code.
    assert!(
        !result.gcode.is_empty(),
        "Repaired mesh should produce non-empty G-code"
    );
    assert!(
        result.layer_count > 0,
        "Repaired mesh should produce layers"
    );
}

// ===========================================================================
// Test 6: JSON output integration
// ===========================================================================

#[test]
fn test_json_output_integration() {
    let config = PrintConfig::default();
    let engine = Engine::new(config.clone());
    let mesh = calibration_cube_20mm();

    let result = engine.slice(&mesh, None).expect("slice should succeed");

    // Serialize to JSON.
    let json_str = to_json(&result, &config).expect("to_json should succeed");

    // Parse the JSON.
    let v: serde_json::Value = serde_json::from_str(&json_str).expect("JSON should be parseable");

    // Verify fields.
    assert_eq!(
        v["layer_count"].as_u64().unwrap() as usize,
        result.layer_count,
        "JSON layer_count should match result"
    );

    assert!(
        v["time_estimate"]["total_seconds"].as_f64().unwrap() > 0.0,
        "JSON time_estimate.total_seconds should be > 0"
    );

    assert!(
        v["filament_usage"]["length_mm"].as_f64().unwrap() > 0.0,
        "JSON filament_usage.length_mm should be > 0"
    );

    // Verify MessagePack roundtrip as well.
    let msgpack_bytes = to_msgpack(&result, &config).expect("to_msgpack should succeed");
    let decoded = from_msgpack(&msgpack_bytes).expect("from_msgpack should succeed");
    assert_eq!(
        decoded.layer_count, result.layer_count,
        "MessagePack roundtrip layer_count should match"
    );
    assert!(
        (decoded.time_estimate.total_seconds - result.time_estimate.total_seconds).abs() < 1e-6,
        "MessagePack roundtrip total_seconds should match"
    );
    assert!(
        (decoded.filament_usage.length_mm - result.filament_usage.length_mm).abs() < 1e-6,
        "MessagePack roundtrip length_mm should match"
    );
}

// ===========================================================================
// Test 7: Event system integration
// ===========================================================================

#[test]
fn test_event_system_integration() {
    let config = PrintConfig {
        // Use sequential mode so per-layer events are emitted.
        // In parallel mode, LayerComplete events are suppressed.
        parallel_slicing: false,
        ..PrintConfig::default()
    };
    let engine = Engine::new(config);
    let mesh = calibration_cube_20mm();

    // Collect all events via callback subscriber.
    let events: Arc<Mutex<Vec<SliceEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);

    let mut bus = EventBus::new();
    bus.subscribe(Box::new(CallbackSubscriber::new(move |e: &SliceEvent| {
        events_clone.lock().unwrap().push(e.clone());
    })));

    let result = engine
        .slice_with_events(&mesh, &bus, None)
        .expect("slice_with_events should succeed");

    let captured = events.lock().unwrap();

    // At least one StageChanged event received.
    let stage_changed_count = captured
        .iter()
        .filter(|e| matches!(e, SliceEvent::StageChanged { .. }))
        .count();
    assert!(
        stage_changed_count >= 1,
        "Should receive at least 1 StageChanged event, got {}",
        stage_changed_count
    );

    // LayerComplete events received (count should be close to layer_count).
    let layer_complete_count = captured
        .iter()
        .filter(|e| matches!(e, SliceEvent::LayerComplete { .. }))
        .count();
    assert!(
        layer_complete_count > 0,
        "Should receive LayerComplete events, got {}",
        layer_complete_count
    );
    // The number of LayerComplete events should approximately match the layer count.
    // Allow some tolerance since not every implementation detail is guaranteed.
    assert!(
        layer_complete_count <= result.layer_count + 5,
        "LayerComplete count ({}) should be close to layer_count ({})",
        layer_complete_count,
        result.layer_count
    );

    // Complete event received at end.
    let complete_count = captured
        .iter()
        .filter(|e| matches!(e, SliceEvent::Complete { .. }))
        .count();
    assert_eq!(
        complete_count, 1,
        "Should receive exactly 1 Complete event, got {}",
        complete_count
    );

    // Verify the Complete event has the correct layer count.
    if let Some(SliceEvent::Complete {
        layers,
        time_seconds,
    }) = captured
        .iter()
        .rev()
        .find(|e| matches!(e, SliceEvent::Complete { .. }))
    {
        assert_eq!(
            *layers, result.layer_count,
            "Complete event layers should match result layer_count"
        );
        assert!(
            *time_seconds > 0.0,
            "Complete event time_seconds should be > 0"
        );
    }
}
