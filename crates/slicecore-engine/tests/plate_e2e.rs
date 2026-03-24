//! End-to-end integration tests for plate-level slicing with overrides.
//!
//! Tests the full cascade resolution pipeline: base config -> default
//! object overrides -> named override sets -> inline overrides.

use std::sync::Arc;

use slicecore_engine::cascade::CascadeResolver;
use slicecore_engine::config::PrintConfig;
use slicecore_engine::plate_config::{MeshSource, ObjectConfig, PlateConfig};
use slicecore_engine::profile_compose::{ComposedConfig, ProfileComposer};

/// Helper: compose a base config from defaults (layers 1-6).
fn base_composed() -> ComposedConfig {
    let composer = ProfileComposer::new();
    composer.compose().expect("default compose should work")
}

// --------------------------------------------------------------------------
// Regression: single-object PlateConfig produces identical config to direct
// --------------------------------------------------------------------------

#[test]
fn single_object_plate_regression() {
    let base = base_composed();
    let default_config = PrintConfig::default();

    // Direct: just the default config
    let direct_layer_height = default_config.layer_height;
    let direct_wall_count = default_config.wall_count;
    let direct_infill = default_config.infill_density;

    // Via PlateConfig (single-object, no overrides)
    let plate = PlateConfig::single_object(PrintConfig::default());
    assert!(plate.is_simple(), "single-object plate should be simple");

    let results = CascadeResolver::resolve_all(&plate, &base).unwrap();
    assert_eq!(results.len(), 1);

    let resolved = &results[0];
    assert!(
        (resolved.config.layer_height - direct_layer_height).abs() < f64::EPSILON,
        "layer_height mismatch: plate={} direct={}",
        resolved.config.layer_height,
        direct_layer_height,
    );
    assert_eq!(
        resolved.config.wall_count, direct_wall_count,
        "wall_count mismatch"
    );
    assert!(
        (resolved.config.infill_density - direct_infill).abs() < f64::EPSILON,
        "infill_density mismatch: plate={} direct={}",
        resolved.config.infill_density,
        direct_infill,
    );
}

// --------------------------------------------------------------------------
// Multi-object override resolution
// --------------------------------------------------------------------------

/// Build the multi-object plate config programmatically (mirrors multi-object.toml).
fn multi_object_plate() -> PlateConfig {
    let mut plate = PlateConfig::default();

    // Default object overrides (layer 7)
    let mut defaults = toml::map::Map::new();
    defaults.insert("infill_density".to_string(), toml::Value::Float(0.3));
    plate.default_object_overrides = Some(defaults);

    // Named override sets
    let mut high_detail = toml::map::Map::new();
    high_detail.insert("layer_height".to_string(), toml::Value::Float(0.1));
    high_detail.insert("wall_count".to_string(), toml::Value::Integer(4));
    high_detail.insert("infill_density".to_string(), toml::Value::Float(0.5));
    plate
        .override_sets
        .insert("high_detail".to_string(), high_detail);

    let mut fast_draft = toml::map::Map::new();
    fast_draft.insert("layer_height".to_string(), toml::Value::Float(0.3));
    fast_draft.insert("wall_count".to_string(), toml::Value::Integer(2));
    fast_draft.insert("infill_density".to_string(), toml::Value::Float(0.15));
    plate
        .override_sets
        .insert("fast_draft".to_string(), fast_draft);

    // Object 1: "Detailed Part" - high_detail set + inline infill_density=0.8
    let mut inline_detailed = toml::map::Map::new();
    inline_detailed.insert("infill_density".to_string(), toml::Value::Float(0.8));
    plate.objects.push(ObjectConfig {
        mesh_source: MeshSource::InMemory,
        name: Some("Detailed Part".to_string()),
        override_set: Some("high_detail".to_string()),
        inline_overrides: Some(inline_detailed),
        copies: 1,
        ..ObjectConfig::default()
    });

    // Object 2: "Draft Part" - fast_draft set, no inline
    plate.objects.push(ObjectConfig {
        mesh_source: MeshSource::InMemory,
        name: Some("Draft Part".to_string()),
        override_set: Some("fast_draft".to_string()),
        copies: 2,
        ..ObjectConfig::default()
    });

    // Object 3: "Default Part" - no override set, no inline
    plate.objects.push(ObjectConfig {
        mesh_source: MeshSource::InMemory,
        name: Some("Default Part".to_string()),
        copies: 1,
        ..ObjectConfig::default()
    });

    plate
}

#[test]
fn multi_object_plate_overrides() {
    let base = base_composed();
    let plate = multi_object_plate();

    let results = CascadeResolver::resolve_all(&plate, &base).unwrap();
    assert_eq!(results.len(), 3, "should resolve 3 objects");

    // Object 0: "Detailed Part"
    // high_detail set: layer_height=0.1, wall_count=4, infill_density=0.5
    // inline override: infill_density=0.8 (wins over set's 0.5)
    let detailed = &results[0];
    assert_eq!(detailed.name, "Detailed Part");
    assert!(
        (detailed.config.layer_height - 0.1).abs() < f64::EPSILON,
        "Detailed Part layer_height should be 0.1, got {}",
        detailed.config.layer_height,
    );
    assert_eq!(
        detailed.config.wall_count, 4,
        "Detailed Part wall_count should be 4"
    );
    assert!(
        (detailed.config.infill_density - 0.8).abs() < f64::EPSILON,
        "Detailed Part infill_density should be 0.8 (inline wins over set), got {}",
        detailed.config.infill_density,
    );

    // Object 1: "Draft Part"
    // fast_draft set: layer_height=0.3, wall_count=2, infill_density=0.15
    let draft = &results[1];
    assert_eq!(draft.name, "Draft Part");
    assert!(
        (draft.config.layer_height - 0.3).abs() < f64::EPSILON,
        "Draft Part layer_height should be 0.3, got {}",
        draft.config.layer_height,
    );
    assert_eq!(
        draft.config.wall_count, 2,
        "Draft Part wall_count should be 2"
    );
    assert!(
        (draft.config.infill_density - 0.15).abs() < f64::EPSILON,
        "Draft Part infill_density should be 0.15, got {}",
        draft.config.infill_density,
    );
    assert_eq!(draft.copies, 2, "Draft Part copies should be 2");

    // Object 2: "Default Part"
    // No override set, no inline -> only default_object_overrides apply
    // default_object_overrides: infill_density=0.3
    let default_part = &results[2];
    assert_eq!(default_part.name, "Default Part");
    assert!(
        (default_part.config.infill_density - 0.3).abs() < f64::EPSILON,
        "Default Part infill_density should be 0.3 (from default overrides), got {}",
        default_part.config.infill_density,
    );
}

// --------------------------------------------------------------------------
// TOML round-trip: serialize and re-parse PlateConfig
// --------------------------------------------------------------------------

#[test]
fn plate_config_toml_round_trip() {
    let plate = multi_object_plate();

    // Serialize to TOML
    let toml_str = toml::to_string_pretty(&plate).expect("should serialize to TOML");

    // Parse back
    let parsed: PlateConfig = toml::from_str(&toml_str).expect("should parse back from TOML");

    // Verify structural equality
    assert_eq!(parsed.objects.len(), plate.objects.len());
    assert_eq!(parsed.override_sets.len(), plate.override_sets.len());
    assert!(parsed.default_object_overrides.is_some());

    // Verify override set contents survived round-trip
    let hd = parsed.override_sets.get("high_detail").unwrap();
    assert_eq!(
        hd.get("layer_height").and_then(toml::Value::as_float),
        Some(0.1),
    );
    assert_eq!(
        hd.get("wall_count").and_then(toml::Value::as_integer),
        Some(4),
    );

    // Verify objects survived round-trip
    assert_eq!(parsed.objects[0].name.as_deref(), Some("Detailed Part"),);
    assert_eq!(
        parsed.objects[0].override_set.as_deref(),
        Some("high_detail"),
    );
    assert_eq!(parsed.objects[1].copies, 2);
}

// --------------------------------------------------------------------------
// Error: invalid override set name with "did you mean?" suggestion
// --------------------------------------------------------------------------

#[test]
fn plate_config_invalid_set_name() {
    let base = base_composed();
    let mut plate = PlateConfig::default();

    let mut set_table = toml::map::Map::new();
    set_table.insert("wall_count".to_string(), toml::Value::Integer(6));
    plate
        .override_sets
        .insert("thick_walls".to_string(), set_table);

    plate.objects.push(ObjectConfig {
        mesh_source: MeshSource::InMemory,
        name: Some("Bad Object".to_string()),
        override_set: Some("thik_walls".to_string()), // typo
        ..ObjectConfig::default()
    });

    let err = CascadeResolver::resolve_all(&plate, &base).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown override set"),
        "error should mention unknown override set: {msg}"
    );
    assert!(
        msg.contains("did you mean"),
        "error should suggest correction: {msg}"
    );
}

// --------------------------------------------------------------------------
// Z-schedule: different layer heights produce correct layer-range behavior
// --------------------------------------------------------------------------

#[test]
fn z_schedule_with_different_layer_heights() {
    use slicecore_engine::plate_config::LayerRangeOverride;

    let base = base_composed();
    let mut plate = PlateConfig::default();

    // Object 1: layer_height=0.1 with a layer-range override at z=0.5-1.0
    let mut inline_fine = toml::map::Map::new();
    inline_fine.insert("layer_height".to_string(), toml::Value::Float(0.1));
    plate.objects.push(ObjectConfig {
        mesh_source: MeshSource::InMemory,
        name: Some("Fine Object".to_string()),
        inline_overrides: Some(inline_fine),
        layer_overrides: vec![LayerRangeOverride {
            z_range: Some((0.5, 1.0)),
            layer_range: None,
            overrides: {
                let mut m = toml::map::Map::new();
                m.insert("wall_count".to_string(), toml::Value::Integer(6));
                m
            },
        }],
        ..ObjectConfig::default()
    });

    // Object 2: layer_height=0.3
    let mut inline_coarse = toml::map::Map::new();
    inline_coarse.insert("layer_height".to_string(), toml::Value::Float(0.3));
    plate.objects.push(ObjectConfig {
        mesh_source: MeshSource::InMemory,
        name: Some("Coarse Object".to_string()),
        inline_overrides: Some(inline_coarse),
        ..ObjectConfig::default()
    });

    let results = CascadeResolver::resolve_all(&plate, &base).unwrap();
    assert_eq!(results.len(), 2);

    // Verify fine object has layer_height=0.1
    assert!(
        (results[0].config.layer_height - 0.1).abs() < f64::EPSILON,
        "Fine Object layer_height should be 0.1"
    );

    // Verify coarse object has layer_height=0.3
    assert!(
        (results[1].config.layer_height - 0.3).abs() < f64::EPSILON,
        "Coarse Object layer_height should be 0.3"
    );

    // Test resolve_for_z on the fine object: z=0.7 is within [0.5, 1.0]
    let fine_obj_config = &plate.objects[0];
    let resolved_at_z =
        CascadeResolver::resolve_for_z(&results[0], fine_obj_config, 0.7, 7).unwrap();
    assert_eq!(
        resolved_at_z.wall_count, 6,
        "wall_count should be 6 within z-range override"
    );

    // Outside the z-range: z=2.0 should not apply the override
    let resolved_outside =
        CascadeResolver::resolve_for_z(&results[0], fine_obj_config, 2.0, 20).unwrap();
    assert!(
        Arc::ptr_eq(&resolved_outside, &results[0].config),
        "outside z-range should return the same Arc (no override applied)"
    );
}

// --------------------------------------------------------------------------
// All workspace tests pass: this test exists as a canary
// --------------------------------------------------------------------------

#[test]
fn plate_config_default_is_simple() {
    let plate = PlateConfig::from(PrintConfig::default());
    assert!(plate.is_simple());
}
