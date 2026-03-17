//! Phase 33 P1 integration tests: config field verification.
//!
//! Comprehensive tests verifying all P1 config fields:
//! - Sub-struct defaults and TOML round-trip
//! - BrimType enum serde
//! - JSON import mapping
//! - Template variable resolution
//! - Validation range checks

use slicecore_engine::config::{BrimType, PrintConfig};
use slicecore_engine::config_validate::{resolve_template_variables, validate_config};
use slicecore_engine::profile_import::import_upstream_profile;

// ===========================================================================
// Group 1: Sub-struct defaults and TOML round-trip
// ===========================================================================

#[test]
fn p1_fuzzy_skin_defaults() {
    let config = PrintConfig::default();
    assert!(!config.fuzzy_skin.enabled);
    assert!((config.fuzzy_skin.thickness - 0.3).abs() < 1e-9);
    assert!((config.fuzzy_skin.point_distance - 0.8).abs() < 1e-9);
}

#[test]
fn p1_brim_skirt_defaults() {
    let config = PrintConfig::default();
    assert_eq!(config.brim_skirt.brim_type, BrimType::None);
    assert!(!config.brim_skirt.brim_ears);
    assert!((config.brim_skirt.brim_ears_max_angle - 125.0).abs() < 1e-9);
    assert_eq!(config.brim_skirt.skirt_height, 1);
}

#[test]
fn p1_input_shaping_defaults() {
    let config = PrintConfig::default();
    assert!(!config.input_shaping.accel_to_decel_enable);
    assert!((config.input_shaping.accel_to_decel_factor - 0.5).abs() < 1e-9);
}

#[test]
fn p1_tool_change_retraction_defaults() {
    let config = PrintConfig::default();
    assert!(
        (config
            .multi_material
            .tool_change_retraction
            .retraction_distance_when_cut
            - 18.0)
            .abs()
            < 1e-9
    );
    assert!(!config.multi_material.tool_change_retraction.long_retraction_when_cut);
}

#[test]
fn p1_accel_extensions_defaults() {
    let config = PrintConfig::default();
    assert!((config.accel.internal_solid_infill_acceleration - 0.0).abs() < 1e-9);
    assert!((config.accel.support_acceleration - 0.0).abs() < 1e-9);
    assert!((config.accel.support_interface_acceleration - 0.0).abs() < 1e-9);
}

#[test]
fn p1_cooling_extensions_defaults() {
    let config = PrintConfig::default();
    assert!((config.cooling.additional_cooling_fan_speed - 0.0).abs() < 1e-9);
    assert!(!config.cooling.auxiliary_fan);
}

#[test]
fn p1_speed_extension_defaults() {
    let config = PrintConfig::default();
    assert!(config.speeds.enable_overhang_speed);
}

#[test]
fn p1_filament_colour_default() {
    let config = PrintConfig::default();
    assert!(config.filament.filament_colour.is_empty());
}

#[test]
fn p1_multi_material_extensions_defaults() {
    let config = PrintConfig::default();
    assert_eq!(config.multi_material.wall_filament, None);
    assert_eq!(config.multi_material.solid_infill_filament, None);
    assert_eq!(config.multi_material.support_filament, None);
    assert_eq!(config.multi_material.support_interface_filament, None);
}

#[test]
fn p1_top_level_defaults() {
    let config = PrintConfig::default();
    assert!(!config.precise_outer_wall);
    assert!(!config.draft_shield);
    assert!(!config.ooze_prevention);
    assert_eq!(config.infill_combination, 0);
    assert!((config.infill_anchor_max - 12.0).abs() < 1e-9);
    assert!((config.min_bead_width - 0.315).abs() < 1e-9);
    assert!((config.min_feature_size - 0.25).abs() < 1e-9);
}

#[test]
fn p1_support_bottom_interface_default() {
    let config = PrintConfig::default();
    assert_eq!(config.support.support_bottom_interface_layers, 0);
}

#[test]
fn p1_toml_round_trip() {
    let mut config = PrintConfig::default();
    config.fuzzy_skin.enabled = true;
    config.fuzzy_skin.thickness = 0.5;
    config.brim_skirt.brim_type = BrimType::Outer;
    config.brim_skirt.brim_ears = true;
    config.input_shaping.accel_to_decel_enable = true;
    config.precise_outer_wall = true;
    config.infill_combination = 3;
    config.min_bead_width = 0.4;
    config.multi_material.wall_filament = Some(1);
    config.filament.filament_colour = "#FF0000".to_string();

    let toml_str = toml::to_string(&config).expect("serialize");
    let roundtrip: PrintConfig = toml::from_str(&toml_str).expect("deserialize");

    assert!(roundtrip.fuzzy_skin.enabled);
    assert!((roundtrip.fuzzy_skin.thickness - 0.5).abs() < 1e-9);
    assert_eq!(roundtrip.brim_skirt.brim_type, BrimType::Outer);
    assert!(roundtrip.brim_skirt.brim_ears);
    assert!(roundtrip.input_shaping.accel_to_decel_enable);
    assert!(roundtrip.precise_outer_wall);
    assert_eq!(roundtrip.infill_combination, 3);
    assert!((roundtrip.min_bead_width - 0.4).abs() < 1e-9);
    assert_eq!(roundtrip.multi_material.wall_filament, Some(1));
    assert_eq!(roundtrip.filament.filament_colour, "#FF0000");
}

// ===========================================================================
// Group 2: BrimType enum serde
// ===========================================================================

#[test]
fn p1_brim_type_serde() {
    // Test that BrimType serializes to snake_case strings
    assert_eq!(
        serde_json::to_string(&BrimType::None).unwrap(),
        "\"none\""
    );
    assert_eq!(
        serde_json::to_string(&BrimType::Outer).unwrap(),
        "\"outer\""
    );
    assert_eq!(
        serde_json::to_string(&BrimType::Inner).unwrap(),
        "\"inner\""
    );
    assert_eq!(
        serde_json::to_string(&BrimType::Both).unwrap(),
        "\"both\""
    );

    // Test deserialization
    assert_eq!(
        serde_json::from_str::<BrimType>("\"none\"").unwrap(),
        BrimType::None
    );
    assert_eq!(
        serde_json::from_str::<BrimType>("\"outer\"").unwrap(),
        BrimType::Outer
    );
}

// ===========================================================================
// Group 3: JSON import mapping tests
// ===========================================================================

#[test]
fn p1_import_fuzzy_skin() {
    let json = serde_json::json!({
        "fuzzy_skin": "1",
        "fuzzy_skin_thickness": "0.5",
        "fuzzy_skin_point_dist": "1.2"
    });
    let result = import_upstream_profile(&json).unwrap();
    assert!(result.config.fuzzy_skin.enabled);
    assert!((result.config.fuzzy_skin.thickness - 0.5).abs() < 1e-9);
    assert!((result.config.fuzzy_skin.point_distance - 1.2).abs() < 1e-9);
}

#[test]
fn p1_import_brim_type() {
    let json = serde_json::json!({
        "brim_type": "outer_only"
    });
    let result = import_upstream_profile(&json).unwrap();
    assert_eq!(result.config.brim_skirt.brim_type, BrimType::Outer);
}

#[test]
fn p1_import_wall_filament_nonzero() {
    // OrcaSlicer uses 1-based indexing; "2" should map to Some(1) (0-based)
    let json = serde_json::json!({
        "wall_filament": "2"
    });
    let result = import_upstream_profile(&json).unwrap();
    assert_eq!(result.config.multi_material.wall_filament, Some(1));
}

#[test]
fn p1_import_wall_filament_zero() {
    // "0" means "default" -> None
    let json = serde_json::json!({
        "wall_filament": "0"
    });
    let result = import_upstream_profile(&json).unwrap();
    assert_eq!(result.config.multi_material.wall_filament, None);
}

#[test]
fn p1_import_tool_change_retraction() {
    let json = serde_json::json!({
        "retraction_distances_when_cut": "18"
    });
    let result = import_upstream_profile(&json).unwrap();
    assert!(
        (result
            .config
            .multi_material
            .tool_change_retraction
            .retraction_distance_when_cut
            - 18.0)
            .abs()
            < 1e-9
    );
}

#[test]
fn p1_import_additional_cooling_fan() {
    let json = serde_json::json!({
        "additional_cooling_fan_speed": "70"
    });
    let result = import_upstream_profile(&json).unwrap();
    assert!((result.config.cooling.additional_cooling_fan_speed - 70.0).abs() < 1e-9);
}

#[test]
fn p1_import_enable_overhang_speed() {
    let json = serde_json::json!({
        "enable_overhang_speed": "1"
    });
    let result = import_upstream_profile(&json).unwrap();
    assert!(result.config.speeds.enable_overhang_speed);
}

#[test]
fn p1_import_filament_colour() {
    let json = serde_json::json!({
        "filament_colour": "#00FF00"
    });
    let result = import_upstream_profile(&json).unwrap();
    assert_eq!(result.config.filament.filament_colour, "#00FF00");
}

#[test]
fn p1_import_support_bottom_interface_layers() {
    let json = serde_json::json!({
        "support_bottom_interface_layers": "2"
    });
    let result = import_upstream_profile(&json).unwrap();
    assert_eq!(result.config.support.support_bottom_interface_layers, 2);
}

// ===========================================================================
// Group 4: Template variable resolution tests
// ===========================================================================

#[test]
fn p1_template_variables() {
    let mut config = PrintConfig::default();
    config.fuzzy_skin.enabled = true;
    config.fuzzy_skin.thickness = 0.5;
    config.brim_skirt.brim_type = BrimType::Outer;
    config.multi_material.wall_filament = Some(2);
    config.precise_outer_wall = true;
    config.infill_combination = 3;

    assert_eq!(resolve_template_variables("{fuzzy_skin}", &config), "1");
    assert_eq!(
        resolve_template_variables("{fuzzy_skin_thickness}", &config),
        "0.5"
    );
    assert!(resolve_template_variables("{brim_type}", &config).contains("outer"));
    // 0-based 2 -> 1-based 3
    assert_eq!(
        resolve_template_variables("{wall_filament}", &config),
        "3"
    );
    assert_eq!(
        resolve_template_variables("{precise_outer_wall}", &config),
        "1"
    );
    assert_eq!(
        resolve_template_variables("{infill_combination}", &config),
        "3"
    );
}

#[test]
fn p1_template_support_bottom_interface() {
    let mut config = PrintConfig::default();
    config.support.support_bottom_interface_layers = 4;
    assert_eq!(
        resolve_template_variables("{support_bottom_interface_layers}", &config),
        "4"
    );
}

#[test]
fn p1_template_filament_colour() {
    let mut config = PrintConfig::default();
    config.filament.filament_colour = "#ABCDEF".to_string();
    assert_eq!(
        resolve_template_variables("{filament_colour}", &config),
        "#ABCDEF"
    );
}

// ===========================================================================
// Group 5: Validation tests
// ===========================================================================

#[test]
fn p1_validation_fuzzy_skin_out_of_range() {
    let mut config = PrintConfig::default();
    config.fuzzy_skin.enabled = true;
    config.fuzzy_skin.thickness = 2.0; // > 1.0, should warn

    let issues = validate_config(&config);
    assert!(
        issues.iter().any(|i| i.field.contains("fuzzy_skin.thickness")),
        "Expected validation issue for fuzzy_skin.thickness > 1.0, got: {:?}",
        issues.iter().map(|i| &i.field).collect::<Vec<_>>()
    );
}

#[test]
fn p1_validation_infill_combination_high() {
    let mut config = PrintConfig::default();
    config.infill_combination = 15; // > 10, should warn

    let issues = validate_config(&config);
    assert!(
        issues.iter().any(|i| i.field.contains("infill_combination")),
        "Expected validation issue for infill_combination > 10, got: {:?}",
        issues.iter().map(|i| &i.field).collect::<Vec<_>>()
    );
}

#[test]
fn p1_validation_accel_to_decel_factor_out_of_range() {
    let mut config = PrintConfig::default();
    config.input_shaping.accel_to_decel_enable = true;
    config.input_shaping.accel_to_decel_factor = 1.5; // > 1.0, should warn

    let issues = validate_config(&config);
    assert!(
        issues
            .iter()
            .any(|i| i.field.contains("accel_to_decel_factor")),
        "Expected validation issue for accel_to_decel_factor > 1.0, got: {:?}",
        issues.iter().map(|i| &i.field).collect::<Vec<_>>()
    );
}
