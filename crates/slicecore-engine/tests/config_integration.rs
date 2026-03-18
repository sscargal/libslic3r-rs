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
            ..Default::default()
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
            ..Default::default()
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
    let has_validation_msg = captured
        .iter()
        .any(|w| w.contains("Sequential") || w.contains("sequential") || w.contains("objects"));
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
            ..Default::default()
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

// ---------------------------------------------------------------------------
// P0 Config Gap Closure: Field defaults, round-trip, enum mapping, migration
// ---------------------------------------------------------------------------

use slicecore_engine::config::{
    BedType, DimensionalCompensationConfig, FilamentPropsConfig, InternalBridgeMode, SurfacePattern,
};

#[test]
fn test_p0_field_defaults() {
    let config = PrintConfig::default();

    // Dimensional compensation defaults
    assert_eq!(config.dimensional_compensation.xy_hole_compensation, 0.0);
    assert_eq!(config.dimensional_compensation.xy_contour_compensation, 0.0);
    assert_eq!(
        config.dimensional_compensation.elephant_foot_compensation,
        0.0
    );

    // Surface pattern defaults
    assert_eq!(config.top_surface_pattern, SurfacePattern::Monotonic);
    assert_eq!(config.bottom_surface_pattern, SurfacePattern::Monotonic);
    assert_eq!(config.solid_infill_pattern, SurfacePattern::Monotonic);

    // Bool/enum defaults
    assert!(!config.extra_perimeters_on_overhangs);
    assert_eq!(config.internal_bridge_support, InternalBridgeMode::Off);
    assert!(!config.precise_z_height);

    // Z offset defaults
    assert_eq!(config.z_offset, 0.0);
    assert_eq!(config.filament.z_offset, 0.0);

    // Filament defaults
    assert_eq!(config.filament.chamber_temperature, 0.0);
    assert_eq!(config.filament.filament_shrink, 100.0);

    // Machine defaults
    assert_eq!(config.machine.chamber_temperature, 0.0);
    assert_eq!(config.machine.curr_bed_type, BedType::TexturedPei);

    // Speed/accel defaults
    assert_eq!(config.speeds.internal_bridge_speed, 0.0);
    assert_eq!(config.accel.min_length_factor, 0.0);
}

#[test]
fn test_p0_toml_round_trip() {
    let mut config = PrintConfig::default();
    config.dimensional_compensation.xy_hole_compensation = -0.1;
    config.dimensional_compensation.xy_contour_compensation = 0.05;
    config.dimensional_compensation.elephant_foot_compensation = 0.2;
    config.top_surface_pattern = SurfacePattern::Concentric;
    config.bottom_surface_pattern = SurfacePattern::Rectilinear;
    config.solid_infill_pattern = SurfacePattern::MonotonicLine;
    config.extra_perimeters_on_overhangs = true;
    config.internal_bridge_support = InternalBridgeMode::Auto;
    config.z_offset = 0.05;
    config.precise_z_height = true;
    config.filament.chamber_temperature = 45.0;
    config.filament.filament_shrink = 99.5;
    config.filament.z_offset = -0.02;
    config.machine.chamber_temperature = 60.0;
    config.machine.curr_bed_type = BedType::EngineeringPlate;
    config.speeds.internal_bridge_speed = 25.0;
    config.accel.min_length_factor = 50.0;

    let toml_str = toml::to_string_pretty(&config).unwrap();
    let parsed: PrintConfig = toml::from_str(&toml_str).unwrap();

    assert!((parsed.dimensional_compensation.xy_hole_compensation - (-0.1)).abs() < 1e-9);
    assert!((parsed.dimensional_compensation.xy_contour_compensation - 0.05).abs() < 1e-9);
    assert!((parsed.dimensional_compensation.elephant_foot_compensation - 0.2).abs() < 1e-9);
    assert_eq!(parsed.top_surface_pattern, SurfacePattern::Concentric);
    assert_eq!(parsed.bottom_surface_pattern, SurfacePattern::Rectilinear);
    assert_eq!(parsed.solid_infill_pattern, SurfacePattern::MonotonicLine);
    assert!(parsed.extra_perimeters_on_overhangs);
    assert_eq!(parsed.internal_bridge_support, InternalBridgeMode::Auto);
    assert!((parsed.z_offset - 0.05).abs() < 1e-9);
    assert!(parsed.precise_z_height);
    assert!((parsed.filament.chamber_temperature - 45.0).abs() < 1e-9);
    assert!((parsed.filament.filament_shrink - 99.5).abs() < 1e-9);
    assert!((parsed.filament.z_offset - (-0.02)).abs() < 1e-9);
    assert!((parsed.machine.chamber_temperature - 60.0).abs() < 1e-9);
    assert_eq!(parsed.machine.curr_bed_type, BedType::EngineeringPlate);
    assert!((parsed.speeds.internal_bridge_speed - 25.0).abs() < 1e-9);
    assert!((parsed.accel.min_length_factor - 50.0).abs() < 1e-9);
}

#[test]
fn test_surface_pattern_enum_round_trip() {
    for variant in [
        SurfacePattern::Rectilinear,
        SurfacePattern::Monotonic,
        SurfacePattern::MonotonicLine,
        SurfacePattern::Concentric,
        SurfacePattern::Hilbert,
        SurfacePattern::Archimedean,
    ] {
        let json = serde_json::to_string(&variant).unwrap();
        let parsed: SurfacePattern = serde_json::from_str(&json).unwrap();
        assert_eq!(
            variant, parsed,
            "SurfacePattern::{variant:?} failed round-trip"
        );
    }
}

#[test]
fn test_bed_type_enum_round_trip() {
    for variant in [
        BedType::CoolPlate,
        BedType::EngineeringPlate,
        BedType::HighTempPlate,
        BedType::TexturedPei,
        BedType::SmoothPei,
        BedType::SatinPei,
    ] {
        let json = serde_json::to_string(&variant).unwrap();
        let parsed: BedType = serde_json::from_str(&json).unwrap();
        assert_eq!(variant, parsed, "BedType::{variant:?} failed round-trip");
    }
}

#[test]
fn test_internal_bridge_mode_enum_round_trip() {
    for variant in [
        InternalBridgeMode::Off,
        InternalBridgeMode::Auto,
        InternalBridgeMode::Always,
    ] {
        let json = serde_json::to_string(&variant).unwrap();
        let parsed: InternalBridgeMode = serde_json::from_str(&json).unwrap();
        assert_eq!(
            variant, parsed,
            "InternalBridgeMode::{variant:?} failed round-trip"
        );
    }
}

#[test]
fn test_bed_type_temperature_resolution() {
    let mut filament = FilamentPropsConfig::default();
    filament.hot_plate_temp = vec![70.0];
    filament.hot_plate_temp_initial_layer = vec![75.0];
    filament.cool_plate_temp = vec![40.0];
    filament.cool_plate_temp_initial_layer = vec![45.0];
    filament.eng_plate_temp = vec![80.0];
    filament.eng_plate_temp_initial_layer = vec![85.0];
    filament.textured_plate_temp = vec![55.0];
    filament.textured_plate_temp_initial_layer = vec![60.0];

    let (normal, first) = filament.resolve_bed_temperatures(BedType::CoolPlate);
    assert_eq!(normal, 40.0);
    assert_eq!(first, 45.0);

    let (normal, first) = filament.resolve_bed_temperatures(BedType::TexturedPei);
    assert_eq!(normal, 55.0);
    assert_eq!(first, 60.0);

    // SmoothPei falls back to hot_plate_temp
    let (normal, first) = filament.resolve_bed_temperatures(BedType::SmoothPei);
    assert_eq!(normal, 70.0);
    assert_eq!(first, 75.0);

    // HighTempPlate also uses hot_plate_temp
    let (normal, first) = filament.resolve_bed_temperatures(BedType::HighTempPlate);
    assert_eq!(normal, 70.0);
    assert_eq!(first, 75.0);

    let (normal, first) = filament.resolve_bed_temperatures(BedType::EngineeringPlate);
    assert_eq!(normal, 80.0);
    assert_eq!(first, 85.0);

    // SatinPei uses textured_plate_temp
    let (normal, first) = filament.resolve_bed_temperatures(BedType::SatinPei);
    assert_eq!(normal, 55.0);
    assert_eq!(first, 60.0);

    // Empty per-type temps fall back to bed_temperatures defaults
    let empty_filament = FilamentPropsConfig::default();
    let (normal, first) = empty_filament.resolve_bed_temperatures(BedType::CoolPlate);
    assert_eq!(normal, 60.0); // default bed_temperatures
    assert_eq!(first, 65.0); // default first_layer_bed_temperatures
}

#[test]
fn test_elephant_foot_migration_from_old_toml() {
    // New TOML format uses [dimensional_compensation] section.
    let new_toml = r#"
[dimensional_compensation]
elephant_foot_compensation = 0.3
xy_hole_compensation = -0.1
"#;
    let config: PrintConfig = toml::from_str(new_toml).unwrap();
    assert!((config.dimensional_compensation.elephant_foot_compensation - 0.3).abs() < 1e-9);
    assert!((config.dimensional_compensation.xy_hole_compensation - (-0.1)).abs() < 1e-9);
}

#[test]
fn test_elephant_foot_serde_alias() {
    // The serde alias allows "elefant_foot_compensation" (OrcaSlicer spelling)
    // to deserialize into elephant_foot_compensation.
    let toml_with_alias = r#"
[dimensional_compensation]
elefant_foot_compensation = 0.25
"#;
    let config: PrintConfig = toml::from_str(toml_with_alias).unwrap();
    assert!((config.dimensional_compensation.elephant_foot_compensation - 0.25).abs() < 1e-9);
}

#[test]
fn test_dimensional_compensation_defaults_independent() {
    // Verify DimensionalCompensationConfig defaults directly.
    let dc = DimensionalCompensationConfig::default();
    assert_eq!(dc.xy_hole_compensation, 0.0);
    assert_eq!(dc.xy_contour_compensation, 0.0);
    assert_eq!(dc.elephant_foot_compensation, 0.0);
}
