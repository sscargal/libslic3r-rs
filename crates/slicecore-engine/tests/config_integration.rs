//! Integration tests for Phase 11: Config-Driven Feature Integration.
//!
//! Verifies all five Phase 11 success criteria:
//! - SC1: `plugin_dir` in config triggers auto-loading
//! - SC2: `sequential.enabled` triggers collision detection
//! - SC3: `multi_material.enabled` triggers purge tower generation
//! - SC4: Config-driven features work without manual API calls
//! - SC5: Warnings for empty/nonexistent plugin_dir

use std::sync::{Arc, Mutex};

use slicecore_engine::{
    CallbackSubscriber, Engine, EventBus, MultiMaterialConfig, PrintConfig, SequentialConfig,
    SliceEvent, ToolConfig,
};
use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Creates a synthetic 20mm calibration cube mesh centered at (x_center, y_center).
fn calibration_cube(x_center: f64, y_center: f64) -> TriangleMesh {
    let half = 10.0;
    let vertices = vec![
        Point3::new(x_center - half, y_center - half, 0.0),
        Point3::new(x_center + half, y_center - half, 0.0),
        Point3::new(x_center + half, y_center + half, 0.0),
        Point3::new(x_center - half, y_center + half, 0.0),
        Point3::new(x_center - half, y_center - half, 20.0),
        Point3::new(x_center + half, y_center - half, 20.0),
        Point3::new(x_center + half, y_center + half, 20.0),
        Point3::new(x_center - half, y_center + half, 20.0),
    ];
    let indices: Vec<[u32; 3]> = vec![
        [0, 1, 2],
        [0, 2, 3], // bottom
        [4, 6, 5],
        [4, 7, 6], // top
        [0, 5, 1],
        [0, 4, 5], // front
        [2, 7, 3],
        [2, 6, 7], // back
        [0, 3, 7],
        [0, 7, 4], // left
        [1, 5, 6],
        [1, 6, 2], // right
    ];
    TriangleMesh::new(vertices, indices).unwrap()
}

/// Creates a mesh with two disjoint cubes (for sequential printing tests).
/// Cube A at (50, 50), Cube B at (150, 150) -- well separated.
fn two_cubes_mesh() -> TriangleMesh {
    // Cube A centered at (50, 50)
    let mut vertices = vec![
        Point3::new(40.0, 40.0, 0.0),
        Point3::new(60.0, 40.0, 0.0),
        Point3::new(60.0, 60.0, 0.0),
        Point3::new(40.0, 60.0, 0.0),
        Point3::new(40.0, 40.0, 20.0),
        Point3::new(60.0, 40.0, 20.0),
        Point3::new(60.0, 60.0, 20.0),
        Point3::new(40.0, 60.0, 20.0),
    ];
    // Cube B centered at (150, 150)
    vertices.extend(vec![
        Point3::new(140.0, 140.0, 0.0),
        Point3::new(160.0, 140.0, 0.0),
        Point3::new(160.0, 160.0, 0.0),
        Point3::new(140.0, 160.0, 0.0),
        Point3::new(140.0, 140.0, 15.0),
        Point3::new(160.0, 140.0, 15.0),
        Point3::new(160.0, 160.0, 15.0),
        Point3::new(140.0, 160.0, 15.0),
    ]);
    let mut indices: Vec<[u32; 3]> = vec![
        [0, 1, 2],
        [0, 2, 3],
        [4, 6, 5],
        [4, 7, 6],
        [0, 5, 1],
        [0, 4, 5],
        [2, 7, 3],
        [2, 6, 7],
        [0, 3, 7],
        [0, 7, 4],
        [1, 5, 6],
        [1, 6, 2],
    ];
    // Cube B indices (offset by 8)
    indices.extend(vec![
        [8, 9, 10],
        [8, 10, 11],
        [12, 14, 13],
        [12, 15, 14],
        [8, 13, 9],
        [8, 12, 13],
        [10, 15, 11],
        [10, 14, 15],
        [8, 11, 15],
        [8, 15, 12],
        [9, 13, 14],
        [9, 14, 10],
    ]);
    TriangleMesh::new(vertices, indices).unwrap()
}

/// Captures SliceEvent::Warning messages from an EventBus.
fn capture_warnings() -> (EventBus, Arc<Mutex<Vec<String>>>) {
    let warnings: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let w_clone = Arc::clone(&warnings);
    let mut bus = EventBus::new();
    bus.subscribe(Box::new(CallbackSubscriber::new(move |e: &SliceEvent| {
        if let SliceEvent::Warning { message, .. } = e {
            w_clone.lock().unwrap().push(message.clone());
        }
    })));
    (bus, warnings)
}

// ---------------------------------------------------------------------------
// SC1: plugin_dir auto-loading
// ---------------------------------------------------------------------------

#[test]
fn sc1_plugin_dir_triggers_auto_loading_with_warning_for_nonexistent_dir() {
    // Set plugin_dir to a nonexistent directory.
    let dir_name = "/tmp/nonexistent-plugin-dir-11-test";
    let config = PrintConfig {
        plugin_dir: Some(dir_name.to_string()),
        ..Default::default()
    };
    let engine = Engine::new(config);

    // Engine should have startup warnings about the directory.
    let warnings = engine.startup_warnings();

    // With plugins feature: warning about empty/nonexistent dir must be present.
    #[cfg(feature = "plugins")]
    {
        assert!(
            !warnings.is_empty(),
            "With plugins feature, startup_warnings should be non-empty for nonexistent plugin_dir"
        );
        let has_dir_warning = warnings.iter().any(|w| w.contains(dir_name));
        assert!(
            has_dir_warning,
            "Warning should mention the plugin_dir path '{}'. Got warnings: {:?}",
            dir_name, warnings
        );
    }

    // Without plugins feature: plugin_dir is silently ignored, no warnings.
    #[cfg(not(feature = "plugins"))]
    {
        assert!(
            warnings.is_empty(),
            "Without plugins feature, plugin_dir should be silently ignored. Got warnings: {:?}",
            warnings
        );
    }
}

// ---------------------------------------------------------------------------
// SC2: sequential.enabled triggers collision detection
// ---------------------------------------------------------------------------

#[test]
fn sc2_sequential_enabled_single_object_emits_warning() {
    let config = PrintConfig {
        sequential: SequentialConfig {
            enabled: true,
            extruder_clearance_radius: 35.0,
            extruder_clearance_height: 40.0,
        },
        ..Default::default()
    };
    let engine = Engine::new(config);
    let mesh = calibration_cube(100.0, 100.0);

    let (bus, warnings) = capture_warnings();
    let _result = engine.slice_with_events(&mesh, &bus, None);

    let captured = warnings.lock().unwrap();
    let has_sequential_warning = captured
        .iter()
        .any(|w| w.contains("Sequential") || w.contains("sequential"));
    assert!(
        has_sequential_warning,
        "Should warn about sequential with single object. Got warnings: {:?}",
        *captured
    );
}

#[test]
fn sc2_sequential_enabled_multi_object_validates_clearance() {
    let config = PrintConfig {
        sequential: SequentialConfig {
            enabled: true,
            extruder_clearance_radius: 35.0,
            extruder_clearance_height: 40.0,
        },
        ..Default::default()
    };
    let engine = Engine::new(config);
    let mesh = two_cubes_mesh();

    let (bus, warnings) = capture_warnings();
    let result = engine.slice_with_events(&mesh, &bus, None);

    // Two cubes are well separated (80mm apart) > 35mm clearance.
    // Should succeed without collision error.
    assert!(
        result.is_ok(),
        "Two well-separated objects should pass sequential validation. Error: {:?}",
        result.err()
    );

    let captured = warnings.lock().unwrap();
    let has_validation_msg = captured.iter().any(|w| {
        w.contains("Sequential") || w.contains("sequential") || w.contains("objects")
    });
    assert!(
        has_validation_msg,
        "Should emit sequential validation info. Got warnings: {:?}",
        *captured
    );
}

#[test]
fn sc2_sequential_collision_returns_config_error() {
    // Two cubes that are too close for the clearance radius.
    let mut vertices = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(20.0, 0.0, 0.0),
        Point3::new(20.0, 20.0, 0.0),
        Point3::new(0.0, 20.0, 0.0),
        Point3::new(0.0, 0.0, 50.0),
        Point3::new(20.0, 0.0, 50.0),
        Point3::new(20.0, 20.0, 50.0),
        Point3::new(0.0, 20.0, 50.0),
    ];
    // Second cube only 10mm away (less than 35mm clearance)
    vertices.extend(vec![
        Point3::new(30.0, 0.0, 0.0),
        Point3::new(50.0, 0.0, 0.0),
        Point3::new(50.0, 20.0, 0.0),
        Point3::new(30.0, 20.0, 0.0),
        Point3::new(30.0, 0.0, 50.0),
        Point3::new(50.0, 0.0, 50.0),
        Point3::new(50.0, 20.0, 50.0),
        Point3::new(30.0, 20.0, 50.0),
    ]);
    let mut indices: Vec<[u32; 3]> = vec![
        [0, 1, 2],
        [0, 2, 3],
        [4, 6, 5],
        [4, 7, 6],
        [0, 5, 1],
        [0, 4, 5],
        [2, 7, 3],
        [2, 6, 7],
        [0, 3, 7],
        [0, 7, 4],
        [1, 5, 6],
        [1, 6, 2],
    ];
    indices.extend(vec![
        [8, 9, 10],
        [8, 10, 11],
        [12, 14, 13],
        [12, 15, 14],
        [8, 13, 9],
        [8, 12, 13],
        [10, 15, 11],
        [10, 14, 15],
        [8, 11, 15],
        [8, 15, 12],
        [9, 13, 14],
        [9, 14, 10],
    ]);
    let mesh = TriangleMesh::new(vertices, indices).unwrap();

    let config = PrintConfig {
        sequential: SequentialConfig {
            enabled: true,
            extruder_clearance_radius: 35.0,
            extruder_clearance_height: 40.0,
        },
        ..Default::default()
    };
    let engine = Engine::new(config);
    let result = engine.slice(&mesh, None);

    assert!(
        result.is_err(),
        "Should return error for colliding objects in sequential mode"
    );
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("Collision") || err.contains("collision") || err.contains("Config error"),
        "Error should mention collision: {}",
        err
    );
}

// ---------------------------------------------------------------------------
// SC3: multi_material.enabled triggers purge tower
// ---------------------------------------------------------------------------

#[test]
fn sc3_multi_material_enabled_generates_purge_tower() {
    let config = PrintConfig {
        multi_material: MultiMaterialConfig {
            enabled: true,
            tool_count: 2,
            tools: vec![ToolConfig::default(), ToolConfig::default()],
            ..Default::default()
        },
        ..Default::default()
    };
    let engine = Engine::new(config);
    let mesh = calibration_cube(100.0, 100.0);

    let result = engine.slice(&mesh, None);
    assert!(
        result.is_ok(),
        "Multi-material with valid config should succeed. Error: {:?}",
        result.err()
    );

    // The G-code should contain PurgeTower comments.
    let result = result.unwrap();
    let gcode = String::from_utf8_lossy(&result.gcode);

    assert!(
        gcode.contains("PurgeTower"),
        "G-code should contain PurgeTower comments when multi_material enabled"
    );
}

#[test]
fn sc3_multi_material_emits_warning_about_no_tool_assignments() {
    let config = PrintConfig {
        multi_material: MultiMaterialConfig {
            enabled: true,
            tool_count: 2,
            tools: vec![ToolConfig::default(), ToolConfig::default()],
            ..Default::default()
        },
        ..Default::default()
    };
    let engine = Engine::new(config);
    let mesh = calibration_cube(100.0, 100.0);

    let (bus, warnings) = capture_warnings();
    let _result = engine.slice_with_events(&mesh, &bus, None);

    let captured = warnings.lock().unwrap();
    let has_tool_warning = captured
        .iter()
        .any(|w| w.contains("tool") || w.contains("modifier") || w.contains("Multi-material"));
    assert!(
        has_tool_warning,
        "Should warn about no tool assignments. Got warnings: {:?}",
        *captured
    );
}

// ---------------------------------------------------------------------------
// SC4: config-driven features work without manual API calls
// ---------------------------------------------------------------------------

#[test]
fn sc4_config_driven_features_work_without_manual_api_calls() {
    // This test verifies that setting config fields is sufficient --
    // no calls to specialized methods like plan_sequential_print() or
    // generate_purge_tower_layer() are needed from user code.
    let config = PrintConfig {
        sequential: SequentialConfig {
            enabled: true,
            ..Default::default()
        },
        multi_material: MultiMaterialConfig {
            enabled: true,
            tool_count: 2,
            tools: vec![ToolConfig::default(), ToolConfig::default()],
            ..Default::default()
        },
        ..Default::default()
    };

    // Only Engine::new() and slice_with_events() -- no other API calls.
    let engine = Engine::new(config);
    let mesh = calibration_cube(100.0, 100.0);
    let (bus, warnings) = capture_warnings();
    let result = engine.slice_with_events(&mesh, &bus, None);

    // Should succeed (single object passes sequential, multi-material generates tower).
    assert!(
        result.is_ok(),
        "Config-only features should work. Error: {:?}",
        result.err()
    );

    // Should have at least one warning from each enabled feature.
    let captured = warnings.lock().unwrap();
    assert!(
        !captured.is_empty(),
        "Should have warnings from config-driven features. Got: {:?}",
        *captured
    );
}

// ---------------------------------------------------------------------------
// SC5: plugin_dir warning for empty dir
// ---------------------------------------------------------------------------

#[test]
fn sc5_plugin_dir_empty_dir_warns_user() {
    // Create a temporary empty directory (exists but contains no plugins).
    let tmp_dir = std::env::temp_dir().join("slicecore-test-empty-plugins-11");
    let _ = std::fs::create_dir_all(&tmp_dir);
    let dir_str = tmp_dir.to_string_lossy().to_string();

    let config = PrintConfig {
        plugin_dir: Some(dir_str.clone()),
        ..Default::default()
    };
    let engine = Engine::new(config);

    let warnings = engine.startup_warnings();

    // With plugins feature: must warn that the directory contains no valid plugins,
    // and the warning must mention the directory path.
    #[cfg(feature = "plugins")]
    {
        assert!(
            !warnings.is_empty(),
            "With plugins feature, empty plugin_dir should produce a warning"
        );
        let has_dir_warning = warnings.iter().any(|w| w.contains(&dir_str));
        assert!(
            has_dir_warning,
            "Warning should mention the plugin_dir path '{}'. Got warnings: {:?}",
            dir_str, warnings
        );
    }

    // Without plugins feature: plugin_dir is silently ignored.
    #[cfg(not(feature = "plugins"))]
    {
        assert!(
            warnings.is_empty(),
            "Without plugins feature, plugin_dir should be silently ignored. Got warnings: {:?}",
            warnings
        );
    }

    // Clean up.
    let _ = std::fs::remove_dir_all(&tmp_dir);
}
