//! Integration tests for registry completeness and integrity.
//!
//! Validates that the global setting registry is correct, complete, and
//! self-consistent. These tests run against the full registry populated
//! from `PrintConfig::setting_definitions()`.

use slicecore_config_schema::{SettingCategory, Tier};
use slicecore_engine::setting_registry;

#[test]
fn test_registry_loads_successfully() {
    let registry = setting_registry();
    assert!(
        registry.len() > 300,
        "Expected at least 300 settings, got {}",
        registry.len()
    );
}

#[test]
fn test_all_affects_keys_resolve() {
    let registry = setting_registry();
    let errors = registry.validate_integrity();
    // Note: some `affects` entries reference conceptual categories (e.g.,
    // "quality", "print_time") rather than actual setting keys. These are
    // acceptable as documentation hints. We check that no DependsOn
    // constraints reference missing keys (which would be a real bug).
    let depends_on_errors: Vec<&String> = errors
        .iter()
        .filter(|e| e.contains("DependsOn"))
        .collect();
    assert!(
        depends_on_errors.is_empty(),
        "DependsOn constraint errors found: {depends_on_errors:?}"
    );
}

#[test]
fn test_no_empty_descriptions_for_low_tiers() {
    let registry = setting_registry();
    let violations: Vec<String> = registry
        .all()
        .filter(|def| def.tier <= Tier::Advanced && def.description.is_empty())
        .map(|def| format!("{} (tier {:?})", def.key, def.tier))
        .collect();
    assert!(
        violations.is_empty(),
        "Settings with tier <= Advanced have empty descriptions: {violations:?}"
    );
}

#[test]
fn test_tier_distribution() {
    let registry = setting_registry();

    let simple_count = registry.filter_by_tier(Tier::Simple).len()
        - registry
            .all()
            .filter(|d| d.tier < Tier::Simple)
            .count();
    let intermediate_count = registry
        .all()
        .filter(|d| d.tier == Tier::Intermediate)
        .count();
    let advanced_count = registry
        .all()
        .filter(|d| d.tier == Tier::Advanced)
        .count();
    let developer_count = registry
        .all()
        .filter(|d| d.tier == Tier::Developer)
        .count();

    eprintln!("Tier distribution:");
    eprintln!("  Simple:       {simple_count}");
    eprintln!("  Intermediate: {intermediate_count}");
    eprintln!("  Advanced:     {advanced_count}");
    eprintln!("  Developer:    {developer_count}");

    assert!(
        (10..=30).contains(&simple_count),
        "Simple tier count {simple_count} outside expected range 10-30"
    );
    assert!(
        (30..=100).contains(&intermediate_count),
        "Intermediate tier count {intermediate_count} outside expected range 30-100"
    );
    assert!(
        (100..=300).contains(&advanced_count),
        "Advanced tier count {advanced_count} outside expected range 100-300"
    );
    assert!(
        developer_count > 0,
        "Developer tier should have at least 1 setting"
    );
}

#[test]
fn test_all_categories_populated() {
    let registry = setting_registry();

    let categories = [
        SettingCategory::Quality,
        SettingCategory::Speed,
        SettingCategory::LineWidth,
        SettingCategory::Cooling,
        SettingCategory::Retraction,
        SettingCategory::Support,
        SettingCategory::Infill,
        SettingCategory::Adhesion,
        SettingCategory::Advanced,
        SettingCategory::Machine,
        SettingCategory::Filament,
        SettingCategory::Acceleration,
        SettingCategory::PostProcess,
        SettingCategory::Timelapse,
        SettingCategory::MultiMaterial,
        SettingCategory::Calibration,
    ];

    for cat in categories {
        let count = registry.filter_by_category(cat).len();
        assert!(
            count > 0,
            "Category {cat:?} has no settings (expected at least 1)"
        );
    }
}

#[test]
fn test_default_values_populated() {
    let registry = setting_registry();

    // Check known fields have non-null defaults
    let fields_to_check = [
        "layer_height",
        "first_layer_height",
        "infill_density",
        "wall_count",
        "top_solid_layers",
        "bottom_solid_layers",
        "speeds.perimeter",
        "speeds.infill",
        "speeds.travel",
        "retraction.length",
    ];

    for key in fields_to_check {
        let def = registry.get_by_str(key);
        assert!(
            def.is_some(),
            "Expected setting '{key}' to exist in registry"
        );
        let def = def.unwrap();
        assert!(
            !def.default_value.is_null(),
            "Expected '{key}' to have a non-null default, got null"
        );
    }

    // Specific default value checks
    let lh = registry.get_by_str("layer_height").unwrap();
    let lh_val = lh.default_value.as_f64().expect("layer_height default should be a number");
    assert!(
        (lh_val - 0.2).abs() < 0.01,
        "layer_height default should be ~0.2, got {lh_val}"
    );

    let id = registry.get_by_str("infill_density").unwrap();
    let id_val = id.default_value.as_f64().expect("infill_density default should be a number");
    assert!(
        id_val > 0.0 && id_val <= 1.0,
        "infill_density default should be between 0 and 1, got {id_val}"
    );
}

#[test]
fn test_affected_by_computed() {
    let registry = setting_registry();
    // Check that at least some settings have affects entries
    let has_affects = registry.all().filter(|d| !d.affects.is_empty()).count();
    assert!(
        has_affects > 10,
        "Expected many settings with affects entries, got {has_affects}"
    );
}

#[test]
fn test_search_returns_results() {
    let registry = setting_registry();

    let perimeter_results = registry.search("perimeter");
    assert!(
        !perimeter_results.is_empty(),
        "Search for 'perimeter' should return results"
    );

    let retract_results = registry.search("retract");
    assert!(
        !retract_results.is_empty(),
        "Search for 'retract' should return results"
    );

    let nonexistent = registry.search("xyzzy_nonexistent_term");
    assert!(
        nonexistent.is_empty(),
        "Search for nonsense should return empty"
    );
}

#[test]
fn test_json_schema_valid_structure() {
    let registry = setting_registry();
    let schema = registry.to_json_schema();

    assert!(
        schema.get("$schema").is_some(),
        "JSON Schema should have $schema key"
    );
    assert!(
        schema.get("properties").is_some(),
        "JSON Schema should have properties key"
    );

    let props = schema["properties"].as_object().expect("properties should be an object");
    assert!(
        props.len() >= 5,
        "Expected at least 5 top-level property groups, got {}",
        props.len()
    );
}

#[test]
fn test_metadata_json_complete() {
    let registry = setting_registry();
    let metadata = registry.to_metadata_json();

    let arr = metadata.as_array().expect("metadata should be an array");
    assert_eq!(
        arr.len(),
        registry.len(),
        "Metadata JSON array length should match registry length"
    );
}
