use slicecore_config_schema::{
    Constraint, HasSettingSchema, OverrideSafety, SettingCategory, Tier, ValueType,
};

// Test enum
#[allow(dead_code)]
#[derive(slicecore_config_derive::SettingSchema)]
enum TestWallOrder {
    #[setting(display = "Inner First", description = "Print inner walls first")]
    InnerFirst,
    #[setting(display = "Outer First", description = "Print outer wall first")]
    OuterFirst,
}

// Test struct with various attributes
#[allow(dead_code)]
#[derive(slicecore_config_derive::SettingSchema)]
#[setting(category = "Speed")]
struct TestSpeedConfig {
    #[setting(
        tier = 1,
        description = "Perimeter print speed",
        units = "mm/s",
        min = 1.0,
        max = 1000.0,
        affects = ["quality"]
    )]
    perimeter: f64,

    #[setting(tier = 2, description = "Infill speed", units = "mm/s")]
    infill: f64,

    // No attributes -- should get tier=Developer and empty description
    gap_fill: f64,

    #[setting(skip)]
    _internal_cache: f64,
}

// Test struct with flatten
#[allow(dead_code)]
#[derive(slicecore_config_derive::SettingSchema)]
struct TestParentConfig {
    #[setting(flatten)]
    speed: TestSpeedConfig,

    #[setting(tier = 1, description = "Layer height", units = "mm")]
    layer_height: f64,
}

#[test]
fn test_enum_derives() {
    let defs = TestWallOrder::setting_definitions("");
    assert_eq!(defs.len(), 1);
    match &defs[0].value_type {
        ValueType::Enum { variants } => {
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].value, "inner_first");
            assert_eq!(variants[0].display, "Inner First");
            assert_eq!(variants[0].description, "Print inner walls first");
            assert_eq!(variants[1].value, "outer_first");
            assert_eq!(variants[1].display, "Outer First");
        }
        other => panic!("expected ValueType::Enum, got {:?}", other),
    }
}

#[test]
fn test_struct_basic_fields() {
    let defs = TestSpeedConfig::setting_definitions("");
    // 3 fields: perimeter, infill, gap_fill (_internal_cache is skipped)
    assert_eq!(defs.len(), 3);
}

#[test]
fn test_field_attributes() {
    let defs = TestSpeedConfig::setting_definitions("");
    let perimeter = defs.iter().find(|d| d.key.0 == "perimeter").unwrap();
    assert_eq!(perimeter.tier, Tier::Simple);
    assert_eq!(perimeter.description, "Perimeter print speed");
    assert_eq!(perimeter.units, Some("mm/s".to_string()));
    assert_eq!(perimeter.category, SettingCategory::Speed);
}

#[test]
fn test_field_constraints() {
    let defs = TestSpeedConfig::setting_definitions("");
    let perimeter = defs.iter().find(|d| d.key.0 == "perimeter").unwrap();
    assert_eq!(perimeter.constraints.len(), 1);
    match &perimeter.constraints[0] {
        Constraint::Range { min, max } => {
            assert!((min - 1.0).abs() < f64::EPSILON);
            assert!((max - 1000.0).abs() < f64::EPSILON);
        }
        other => panic!("expected Constraint::Range, got {:?}", other),
    }
}

#[test]
fn test_field_affects() {
    let defs = TestSpeedConfig::setting_definitions("");
    let perimeter = defs.iter().find(|d| d.key.0 == "perimeter").unwrap();
    assert_eq!(perimeter.affects.len(), 1);
    assert_eq!(perimeter.affects[0].0, "quality");
}

#[test]
fn test_unannotated_field() {
    let defs = TestSpeedConfig::setting_definitions("");
    let gap_fill = defs.iter().find(|d| d.key.0 == "gap_fill").unwrap();
    assert_eq!(gap_fill.tier, Tier::Developer);
    assert_eq!(gap_fill.description, "");
}

#[test]
fn test_skip_field() {
    let defs = TestSpeedConfig::setting_definitions("");
    // _internal_cache should be excluded
    assert!(defs.iter().all(|d| !d.key.0.contains("_internal_cache")));
    assert_eq!(defs.len(), 3);
}

#[test]
fn test_flatten_prefix() {
    let defs = TestParentConfig::setting_definitions("");
    let keys: Vec<&str> = defs.iter().map(|d| d.key.0.as_str()).collect();
    assert!(
        keys.contains(&"speed.perimeter"),
        "missing speed.perimeter, got: {:?}",
        keys
    );
    assert!(
        keys.contains(&"speed.infill"),
        "missing speed.infill, got: {:?}",
        keys
    );
    assert!(
        keys.contains(&"speed.gap_fill"),
        "missing speed.gap_fill, got: {:?}",
        keys
    );
    assert!(
        keys.contains(&"layer_height"),
        "missing layer_height, got: {:?}",
        keys
    );
    // 3 from speed (skip excluded) + 1 layer_height = 4
    assert_eq!(defs.len(), 4);
}

#[test]
fn test_display_name_auto() {
    let defs = TestSpeedConfig::setting_definitions("");
    let gap_fill = defs.iter().find(|d| d.key.0 == "gap_fill").unwrap();
    assert_eq!(gap_fill.display_name, "Gap Fill");
}

// Test struct with override_safety attributes
#[allow(dead_code)]
#[derive(slicecore_config_derive::SettingSchema)]
struct TestOverrideSafetyConfig {
    #[setting(tier = 1, description = "Layer height", override_safety = "safe")]
    layer_height: f64,

    #[setting(tier = 2, description = "Bed temperature", override_safety = "warn")]
    bed_temperature: f64,

    #[setting(
        tier = 3,
        description = "Machine bed X size",
        override_safety = "ignored"
    )]
    bed_x: f64,

    #[setting(tier = 1, description = "Infill density (no explicit safety)")]
    infill_density: f64,
}

#[test]
fn test_override_safety_explicit_safe() {
    let defs = TestOverrideSafetyConfig::setting_definitions("");
    let lh = defs.iter().find(|d| d.key.0 == "layer_height").unwrap();
    assert_eq!(lh.override_safety, OverrideSafety::Safe);
}

#[test]
fn test_override_safety_warn() {
    let defs = TestOverrideSafetyConfig::setting_definitions("");
    let bt = defs.iter().find(|d| d.key.0 == "bed_temperature").unwrap();
    assert_eq!(bt.override_safety, OverrideSafety::Warn);
}

#[test]
fn test_override_safety_ignored() {
    let defs = TestOverrideSafetyConfig::setting_definitions("");
    let bx = defs.iter().find(|d| d.key.0 == "bed_x").unwrap();
    assert_eq!(bx.override_safety, OverrideSafety::Ignored);
}

#[test]
fn test_override_safety_default_is_safe() {
    let defs = TestOverrideSafetyConfig::setting_definitions("");
    let id = defs.iter().find(|d| d.key.0 == "infill_density").unwrap();
    assert_eq!(id.override_safety, OverrideSafety::Safe);
}
