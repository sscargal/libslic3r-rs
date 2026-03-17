//! Phase 20 integration tests: Expanded field coverage and profile mapping.
//!
//! These tests verify the Phase 20 success criteria:
//! - SC1: PrintConfig has all critical process fields
//! - SC2: PrintConfig has all critical machine fields
//! - SC3: PrintConfig has all critical filament fields
//! - SC4: JSON mapper maps 50+ upstream fields
//! - SC5: INI mapper maps the same expanded field set
//! - SC6: Re-converted X1C profiles contain comprehensive settings
//! - SC7: All existing tests pass with no regressions (implicit via `cargo test --workspace`)
//!
//! Tests that load real profiles from `/home/steve/slicer-analysis/` or
//! `profiles/` are gated with `#[ignore]` for CI compatibility.
//! Run them manually with:
//!   cargo test -p slicecore-engine --test integration_phase20 -- --ignored --nocapture

use slicecore_engine::config::PrintConfig;
use slicecore_engine::profile_import::import_upstream_profile;

// ---------------------------------------------------------------------------
// Test 1: Expanded JSON field coverage
// ---------------------------------------------------------------------------

/// Verify that the re-converted BambuStudio X1C process profile has 50+ mapped
/// fields (via batch conversion with inheritance resolution) and that its
/// sub-config sections (speeds, accel) contain non-default values.
///
/// The batch conversion resolves inheritance chains (X1C -> single_0.20 ->
/// common), so the converted TOML has more mapped fields than a single leaf
/// profile import.
#[test]
#[ignore] // Requires profiles/ directory from import-profiles
fn test_expanded_json_field_coverage() {
    // Verify the batch-converted profile has 50+ mapped fields
    let profile_path =
        "/home/steve/libslic3r-rs/profiles/bambustudio/BBL/process/0.20mm_Standard_BBL_X1C.toml";
    let toml_str = std::fs::read_to_string(profile_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", profile_path, e));

    // Extract mapped field count from TOML header comment
    let mapped_count = toml_str
        .lines()
        .find(|l| l.starts_with("# Mapped fields:"))
        .and_then(|l| l.strip_prefix("# Mapped fields: "))
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(0);

    assert!(
        mapped_count >= 50,
        "Expected 50+ mapped fields in converted X1C process profile, got {}",
        mapped_count
    );

    // Parse and verify sub-config values
    let config: PrintConfig = toml::from_str(&toml_str)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", profile_path, e));

    // Speeds sub-config has non-default values
    assert!(
        config.speeds.perimeter > 100.0,
        "speeds.perimeter should be > 100 (X1C is 200), got {}",
        config.speeds.perimeter
    );
    assert!(
        config.speeds.infill > 100.0,
        "speeds.infill should be > 100 (X1C is 270), got {}",
        config.speeds.infill
    );
    assert!(
        config.speeds.bridge > 0.0,
        "speeds.bridge should be > 0, got {}",
        config.speeds.bridge
    );
    assert!(
        config.speeds.inner_wall > 0.0,
        "speeds.inner_wall should be > 0, got {}",
        config.speeds.inner_wall
    );
    assert!(
        config.speeds.gap_fill > 0.0,
        "speeds.gap_fill should be > 0, got {}",
        config.speeds.gap_fill
    );
    assert!(
        config.speeds.travel > 100.0,
        "speeds.travel should be > 100 (X1C is 500), got {}",
        config.speeds.travel
    );

    // Acceleration sub-config has non-default values
    assert!(
        config.accel.print > 5000.0,
        "accel.print should be > 5000 (X1C is 10000), got {}",
        config.accel.print
    );
    assert!(
        config.accel.travel > 5000.0,
        "accel.travel should be > 5000 (X1C is 10000), got {}",
        config.accel.travel
    );
    assert!(
        config.accel.outer_wall > 0.0,
        "accel.outer_wall should be > 0, got {}",
        config.accel.outer_wall
    );

    // Process misc fields
    assert!(
        config.infill_density > 0.0,
        "infill_density should be > 0, got {}",
        config.infill_density
    );

    // Passthrough has unmapped fields preserved
    assert!(
        !config.passthrough.is_empty(),
        "passthrough should contain unmapped upstream fields"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Expanded INI field coverage
// ---------------------------------------------------------------------------

/// Verify that importing a real PrusaSlicer INI profile populates sub-config
/// fields with non-default values from the INI pipeline.
#[test]
#[ignore] // Requires /home/steve/slicer-analysis/PrusaSlicer/
fn test_expanded_ini_field_coverage() {
    // We test by looking at converted profiles. The INI pipeline is
    // exercised during batch_convert_prusaslicer_profiles which resolves
    // inheritance and maps fields. Check a converted profile from disk.
    let profile_path = std::path::Path::new("/home/steve/libslic3r-rs/profiles/prusaslicer");
    if !profile_path.exists() {
        panic!("profiles/prusaslicer/ not found. Run import-profiles first.");
    }

    // Find a print profile with speed settings -- Prusa MK4 profiles are good candidates
    let mut found_speeds = false;
    let mut found_retraction = false;
    let mut checked = 0;

    for entry in walkdir(profile_path, "process") {
        let toml_str = std::fs::read_to_string(&entry).unwrap();
        let config: PrintConfig = toml::from_str(&toml_str).unwrap_or_else(|e| {
            panic!("Failed to parse {}: {}", entry.display(), e);
        });

        // Check for non-default speed values from INI mapping
        if config.speeds.perimeter != 45.0 {
            found_speeds = true;
        }
        checked += 1;
        if checked >= 50 {
            break;
        }
    }

    for entry in walkdir(profile_path, "machine") {
        let toml_str = std::fs::read_to_string(&entry).unwrap();
        let config: PrintConfig = toml::from_str(&toml_str).unwrap_or_else(|e| {
            panic!("Failed to parse {}: {}", entry.display(), e);
        });

        // Check for non-default retraction values from INI mapping
        if config.retraction.length != 0.8 || config.retraction.speed != 45.0 {
            found_retraction = true;
            break;
        }
    }

    assert!(
        found_speeds,
        "Expected at least one PrusaSlicer print profile with non-default speed values"
    );
    assert!(
        found_retraction,
        "Expected at least one PrusaSlicer printer profile with non-default retraction values"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Passthrough storage
// ---------------------------------------------------------------------------

/// Verify that fields with no typed PrintConfig mapping are stored in the
/// passthrough BTreeMap and reported in passthrough_fields.
#[test]
fn test_passthrough_storage() {
    let json = serde_json::json!({
        "type": "process",
        "name": "Passthrough Test",
        "layer_height": "0.2",
        "outer_wall_speed": "200",
        // Fields with no typed PrintConfig equivalent
        "ams_drying_temperature": "45",
        "scan_first_layer": "1",
        "reduce_crossing_wall": "0",
        "detect_floating_vertical_shell": "1"
    });

    let result = import_upstream_profile(&json).unwrap();

    // Passthrough fields should be stored in config.passthrough
    assert!(
        !result.config.passthrough.is_empty(),
        "passthrough BTreeMap should contain unmapped fields"
    );

    // Check specific passthrough fields
    assert_eq!(
        result.config.passthrough.get("ams_drying_temperature"),
        Some(&"45".to_string()),
        "ams_drying_temperature should be in passthrough"
    );
    assert_eq!(
        result.config.passthrough.get("scan_first_layer"),
        Some(&"1".to_string()),
        "scan_first_layer should be in passthrough"
    );

    // Passthrough fields should be tracked
    assert!(
        !result.passthrough_fields.is_empty(),
        "passthrough_fields list should not be empty"
    );

    // Mapped fields should still work
    assert!(
        (result.config.speeds.perimeter - 200.0).abs() < 1e-9,
        "perimeter_speed should be 200, got {}",
        result.config.speeds.perimeter
    );
}

// ---------------------------------------------------------------------------
// Test 4: Converted TOML has nested sections
// ---------------------------------------------------------------------------

/// Verify that converted TOML profiles contain nested sub-config sections
/// like [speeds], [accel], [retraction], etc.
#[test]
#[ignore] // Requires profiles/ directory from import-profiles
fn test_converted_toml_has_nested_sections() {
    let process_path =
        "/home/steve/libslic3r-rs/profiles/bambustudio/BBL/process/0.20mm_Standard_BBL_X1C.toml";
    let machine_path = "/home/steve/libslic3r-rs/profiles/bambustudio/BBL/machine/Bambu_Lab_X1_Carbon_0.4_nozzle.toml";
    let filament_dir =
        std::path::Path::new("/home/steve/libslic3r-rs/profiles/bambustudio/BBL/filament");

    // Process profile should have [speeds] and [accel]
    let process_toml = std::fs::read_to_string(process_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", process_path, e));
    assert!(
        process_toml.contains("[speeds]"),
        "Process profile should contain [speeds] section"
    );
    assert!(
        process_toml.contains("[accel]"),
        "Process profile should contain [accel] section"
    );

    // Machine profile should have [machine] and [retraction]
    let machine_toml = std::fs::read_to_string(machine_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", machine_path, e));
    assert!(
        machine_toml.contains("[machine]"),
        "Machine profile should contain [machine] section"
    );
    assert!(
        machine_toml.contains("[retraction]"),
        "Machine profile should contain [retraction] section"
    );

    // Filament profiles should have [cooling] and [filament]
    let mut found_cooling = false;
    let mut found_filament = false;
    if filament_dir.exists() {
        for entry in std::fs::read_dir(filament_dir).unwrap().take(20) {
            let entry = entry.unwrap();
            if entry.path().extension().map_or(false, |e| e == "toml") {
                let content = std::fs::read_to_string(entry.path()).unwrap();
                if content.contains("[cooling]") {
                    found_cooling = true;
                }
                if content.contains("[filament]") {
                    found_filament = true;
                }
                if found_cooling && found_filament {
                    break;
                }
            }
        }
    }

    assert!(
        found_cooling,
        "At least one filament profile should contain [cooling] section"
    );
    assert!(
        found_filament,
        "At least one filament profile should contain [filament] section"
    );
}

// ---------------------------------------------------------------------------
// Test 5: X1C profiles have comprehensive settings (comparison-readiness)
// ---------------------------------------------------------------------------

/// Load the BambuStudio X1C 0.4mm nozzle + Bambu PLA + 0.20mm Standard
/// converted TOML profiles, merge them into a single PrintConfig, and verify
/// that key settings are populated for comparison readiness.
#[test]
#[ignore] // Requires profiles/ directory from import-profiles
fn test_x1c_profiles_have_comprehensive_settings() {
    let process_path =
        "/home/steve/libslic3r-rs/profiles/bambustudio/BBL/process/0.20mm_Standard_BBL_X1C.toml";
    let machine_path = "/home/steve/libslic3r-rs/profiles/bambustudio/BBL/machine/Bambu_Lab_X1_Carbon_0.4_nozzle.toml";

    // Find a PLA filament profile for X1C
    let filament_dir =
        std::path::Path::new("/home/steve/libslic3r-rs/profiles/bambustudio/BBL/filament");
    let filament_path = find_filament_profile(filament_dir, "PLA")
        .expect("Should find a PLA filament profile for BambuStudio");

    // Load each profile as PrintConfig
    let process_toml = std::fs::read_to_string(process_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", process_path, e));
    let machine_toml = std::fs::read_to_string(machine_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", machine_path, e));
    let filament_toml = std::fs::read_to_string(&filament_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", filament_path.display(), e));

    let process_config: PrintConfig =
        toml::from_str(&process_toml).unwrap_or_else(|e| panic!("Failed to parse process: {}", e));
    let machine_config: PrintConfig =
        toml::from_str(&machine_toml).unwrap_or_else(|e| panic!("Failed to parse machine: {}", e));
    let filament_config: PrintConfig = toml::from_str(&filament_toml)
        .unwrap_or_else(|e| panic!("Failed to parse filament: {}", e));

    // Merge: start with defaults, overlay machine, then filament, then process
    let mut merged = PrintConfig::default();

    // Machine settings
    merged.machine = machine_config.machine;
    merged.retraction = machine_config.retraction;

    // Filament settings
    merged.filament = filament_config.filament;
    merged.cooling = filament_config.cooling;
    merged.extrusion_multiplier = filament_config.extrusion_multiplier;

    // Process settings
    merged.speeds = process_config.speeds;
    merged.accel = process_config.accel;
    merged.infill_density = process_config.infill_density;
    merged.infill_pattern = process_config.infill_pattern;
    merged.top_solid_layers = process_config.top_solid_layers;
    merged.first_layer_height = process_config.first_layer_height;
    merged.brim_width = process_config.brim_width;

    // SC6 verification: comprehensive settings present
    // Speeds
    assert!(
        merged.speeds.bridge > 0.0,
        "speeds.bridge should be > 0 (from process), got {}",
        merged.speeds.bridge
    );
    assert!(
        merged.speeds.perimeter > 100.0,
        "speeds.perimeter should be > 100 (X1C), got {}",
        merged.speeds.perimeter
    );
    assert!(
        merged.speeds.travel > 100.0,
        "speeds.travel should be > 100 (X1C is 500), got {}",
        merged.speeds.travel
    );

    // Line widths (defaults are fine for process profiles that don't override)
    assert!(
        merged.machine.nozzle_diameter() > 0.0,
        "machine.nozzle_diameter should be > 0, got {}",
        merged.machine.nozzle_diameter()
    );

    // Machine settings
    assert!(
        merged.machine.bed_x > 0.0,
        "machine.bed_x should be > 0, got {}",
        merged.machine.bed_x
    );
    assert!(
        merged.machine.bed_y > 0.0,
        "machine.bed_y should be > 0, got {}",
        merged.machine.bed_y
    );
    assert!(
        !merged.machine.printer_model.is_empty(),
        "machine.printer_model should be non-empty"
    );
    assert_eq!(
        merged.machine.nozzle_type, "hardened_steel",
        "X1C should have hardened_steel nozzle"
    );

    // Retraction
    assert!(
        merged.retraction.length > 0.0,
        "retraction.length should be > 0, got {}",
        merged.retraction.length
    );
    assert!(
        merged.retraction.speed > 0.0,
        "retraction.speed should be > 0, got {}",
        merged.retraction.speed
    );
    assert!(
        merged.retraction.z_hop > 0.0,
        "retraction.z_hop should be > 0 (X1C = 0.4), got {}",
        merged.retraction.z_hop
    );

    // Filament temperatures
    assert!(
        merged.filament.nozzle_temp() > 190.0,
        "filament.nozzle_temp should be > 190 (PLA), got {}",
        merged.filament.nozzle_temp()
    );
    assert!(
        merged.filament.bed_temp() > 40.0,
        "filament.bed_temp should be > 40 (PLA), got {}",
        merged.filament.bed_temp()
    );

    // Cooling
    assert!(
        merged.cooling.fan_max_speed > 0.0,
        "cooling.fan_max_speed should be > 0, got {}",
        merged.cooling.fan_max_speed
    );

    // Acceleration
    assert!(
        merged.accel.print > 5000.0,
        "accel.print should be > 5000 (X1C is 10000), got {}",
        merged.accel.print
    );
}

// ---------------------------------------------------------------------------
// SC1-SC3: PrintConfig sub-config struct completeness
// ---------------------------------------------------------------------------

/// SC1: Verify PrintConfig has all critical process fields in sub-configs.
#[test]
fn test_sc1_critical_process_fields() {
    let config = PrintConfig::default();

    // SpeedConfig has all critical process speed fields
    let _ = config.speeds.perimeter;
    let _ = config.speeds.infill;
    let _ = config.speeds.travel;
    let _ = config.speeds.first_layer;
    let _ = config.speeds.bridge;
    let _ = config.speeds.inner_wall;
    let _ = config.speeds.gap_fill;
    let _ = config.speeds.top_surface;
    let _ = config.speeds.internal_solid_infill;
    let _ = config.speeds.support;
    let _ = config.speeds.support_interface;
    let _ = config.speeds.small_perimeter;
    let _ = config.speeds.overhang_1_4;
    let _ = config.speeds.overhang_2_4;
    let _ = config.speeds.overhang_3_4;
    let _ = config.speeds.overhang_4_4;

    // AccelerationConfig has all critical fields
    let _ = config.accel.print;
    let _ = config.accel.travel;
    let _ = config.accel.outer_wall;
    let _ = config.accel.inner_wall;
    let _ = config.accel.initial_layer;
    let _ = config.accel.top_surface;
    let _ = config.accel.sparse_infill;
    let _ = config.accel.bridge;

    // LineWidthConfig has all critical fields
    let _ = config.line_widths.outer_wall;
    let _ = config.line_widths.inner_wall;
    let _ = config.line_widths.infill;
    let _ = config.line_widths.top_surface;
    let _ = config.line_widths.initial_layer;
    let _ = config.line_widths.support;

    // Process misc fields
    let _ = config.bridge_flow;
    let _ = config.dimensional_compensation.elephant_foot_compensation;
    let _ = config.infill_direction;
    let _ = config.infill_wall_overlap;
    let _ = config.spiral_mode;
    let _ = config.only_one_wall_top;
    let _ = config.resolution;
    let _ = config.raft_layers;
    let _ = config.detect_thin_wall;
}

/// SC2: Verify PrintConfig has all critical machine fields.
#[test]
fn test_sc2_critical_machine_fields() {
    let config = PrintConfig::default();

    let _ = config.machine.bed_x;
    let _ = config.machine.bed_y;
    let _ = config.machine.printable_height;
    let _ = config.machine.max_acceleration_x;
    let _ = config.machine.max_acceleration_y;
    let _ = config.machine.max_acceleration_z;
    let _ = config.machine.max_acceleration_e;
    let _ = config.machine.max_speed_x;
    let _ = config.machine.max_speed_y;
    let _ = config.machine.max_speed_z;
    let _ = config.machine.max_speed_e;
    let _ = config.machine.nozzle_diameters;
    let _ = config.machine.jerk_values_x;
    let _ = config.machine.jerk_values_y;
    let _ = config.machine.jerk_values_z;
    let _ = config.machine.jerk_values_e;
    let _ = config.machine.start_gcode;
    let _ = config.machine.end_gcode;
    let _ = config.machine.layer_change_gcode;
    let _ = config.machine.nozzle_type;
    let _ = config.machine.printer_model;
    let _ = config.machine.min_layer_height;
    let _ = config.machine.max_layer_height;

    // RetractionConfig (machine-adjacent)
    let _ = config.retraction.length;
    let _ = config.retraction.speed;
    let _ = config.retraction.z_hop;
    let _ = config.retraction.min_travel;
    let _ = config.retraction.deretraction_speed;
    let _ = config.retraction.wipe;
    let _ = config.retraction.wipe_distance;
}

/// SC3: Verify PrintConfig has all critical filament fields.
#[test]
fn test_sc3_critical_filament_fields() {
    let config = PrintConfig::default();

    let _ = config.filament.diameter;
    let _ = config.filament.density;
    let _ = config.filament.cost_per_kg;
    let _ = config.filament.filament_type;
    let _ = config.filament.filament_vendor;
    let _ = config.filament.max_volumetric_speed;
    let _ = config.filament.nozzle_temperature_range_low;
    let _ = config.filament.nozzle_temperature_range_high;
    let _ = config.filament.nozzle_temperatures;
    let _ = config.filament.bed_temperatures;
    let _ = config.filament.first_layer_nozzle_temperatures;
    let _ = config.filament.first_layer_bed_temperatures;
    let _ = config.filament.filament_retraction_length;
    let _ = config.filament.filament_retraction_speed;
    let _ = config.filament.filament_start_gcode;
    let _ = config.filament.filament_end_gcode;

    // CoolingConfig (filament-adjacent)
    let _ = config.cooling.fan_speed;
    let _ = config.cooling.fan_below_layer_time;
    let _ = config.cooling.disable_fan_first_layers;
    let _ = config.cooling.fan_max_speed;
    let _ = config.cooling.fan_min_speed;
    let _ = config.cooling.slow_down_layer_time;
    let _ = config.cooling.slow_down_min_speed;
    let _ = config.cooling.overhang_fan_speed;
    let _ = config.cooling.slow_down_for_layer_cooling;
}

/// SC4: Verify JSON mapper maps 50+ fields via synthetic profile.
#[test]
fn test_sc4_json_mapper_50_plus_fields() {
    // Build a JSON object with many fields from all categories
    let mut obj = serde_json::Map::new();
    obj.insert("type".into(), "process".into());
    obj.insert("name".into(), "SC4 Comprehensive Test".into());

    // Process fields
    obj.insert("layer_height".into(), "0.2".into());
    obj.insert("initial_layer_print_height".into(), "0.3".into());
    obj.insert("wall_loops".into(), "3".into());
    obj.insert("sparse_infill_density".into(), "20%".into());
    obj.insert("top_shell_layers".into(), "4".into());
    obj.insert("bottom_shell_layers".into(), "3".into());
    obj.insert("sparse_infill_pattern".into(), "grid".into());
    obj.insert("skirt_loops".into(), "1".into());
    obj.insert("skirt_distance".into(), "5".into());
    obj.insert("brim_width".into(), "3".into());
    obj.insert("filament_flow_ratio".into(), serde_json::json!(["0.95"]));

    // Speed fields
    obj.insert("outer_wall_speed".into(), "200".into());
    obj.insert("inner_wall_speed".into(), "300".into());
    obj.insert("sparse_infill_speed".into(), "250".into());
    obj.insert("travel_speed".into(), "500".into());
    obj.insert("initial_layer_speed".into(), "50".into());
    obj.insert("bridge_speed".into(), "30".into());
    obj.insert("gap_infill_speed".into(), "200".into());
    obj.insert("top_surface_speed".into(), "150".into());
    obj.insert("internal_solid_infill_speed".into(), "180".into());
    obj.insert("initial_layer_infill_speed".into(), "80".into());
    obj.insert("support_speed".into(), "100".into());
    obj.insert("support_interface_speed".into(), "60".into());
    obj.insert("small_perimeter_speed".into(), "40".into());
    obj.insert("overhang_1_4_speed".into(), "0".into());
    obj.insert("overhang_2_4_speed".into(), "50".into());
    obj.insert("overhang_3_4_speed".into(), "30".into());
    obj.insert("overhang_4_4_speed".into(), "10".into());
    obj.insert("travel_speed_z".into(), "0".into());

    // Acceleration fields
    obj.insert("default_acceleration".into(), "10000".into());
    obj.insert("travel_acceleration".into(), "12000".into());
    obj.insert("outer_wall_acceleration".into(), "5000".into());
    obj.insert("inner_wall_acceleration".into(), "6000".into());
    obj.insert("initial_layer_acceleration".into(), "500".into());
    obj.insert("initial_layer_travel_acceleration".into(), "3000".into());
    obj.insert("top_surface_acceleration".into(), "2000".into());
    obj.insert("sparse_infill_acceleration".into(), "8000".into());

    // Line width fields
    obj.insert("outer_wall_line_width".into(), "0.42".into());
    obj.insert("inner_wall_line_width".into(), "0.45".into());
    obj.insert("sparse_infill_line_width".into(), "0.45".into());
    obj.insert("top_surface_line_width".into(), "0.42".into());
    obj.insert("initial_layer_line_width".into(), "0.5".into());
    obj.insert("internal_solid_infill_line_width".into(), "0.42".into());
    obj.insert("support_line_width".into(), "0.42".into());

    // Cooling fields
    obj.insert("fan_max_speed".into(), serde_json::json!(["100"]));
    obj.insert("fan_min_speed".into(), serde_json::json!(["35"]));
    obj.insert(
        "slow_down_for_layer_cooling".into(),
        serde_json::json!(["1"]),
    );
    obj.insert("slow_down_layer_time".into(), serde_json::json!(["8"]));
    obj.insert("slow_down_min_speed".into(), serde_json::json!(["10"]));
    obj.insert("overhang_fan_speed".into(), serde_json::json!(["100"]));

    // Process misc
    obj.insert("bridge_flow".into(), "0.95".into());
    obj.insert("elefant_foot_compensation".into(), "0.15".into());
    obj.insert("infill_direction".into(), "45".into());
    obj.insert("infill_wall_overlap".into(), "25%".into());
    obj.insert("spiral_mode".into(), "0".into());
    obj.insert("only_one_wall_top".into(), "1".into());
    obj.insert("resolution".into(), "0.012".into());
    obj.insert("raft_layers".into(), "0".into());
    obj.insert("detect_thin_wall".into(), "1".into());

    let json = serde_json::Value::Object(obj);
    let result = import_upstream_profile(&json).unwrap();

    assert!(
        result.mapped_fields.len() >= 50,
        "SC4: Expected 50+ mapped fields, got {}. Fields: {:?}",
        result.mapped_fields.len(),
        result.mapped_fields
    );
}

/// SC5: Verify INI mapper handles expanded field set.
#[test]
fn test_sc5_ini_mapper_expanded_fields() {
    use slicecore_engine::profile_import_ini::import_prusaslicer_ini_profile;

    // Simulate a PrusaSlicer INI section with many fields
    let mut fields = std::collections::HashMap::new();
    // Speed fields
    fields.insert("perimeter_speed".to_string(), "60".to_string());
    fields.insert("infill_speed".to_string(), "80".to_string());
    fields.insert("travel_speed".to_string(), "150".to_string());
    fields.insert("first_layer_speed".to_string(), "30".to_string());
    fields.insert("bridge_speed".to_string(), "25".to_string());
    fields.insert("support_material_speed".to_string(), "40".to_string());
    fields.insert(
        "support_material_interface_speed".to_string(),
        "30".to_string(),
    );
    fields.insert("top_solid_infill_speed".to_string(), "40".to_string());
    fields.insert("solid_infill_speed".to_string(), "60".to_string());
    fields.insert("small_perimeter_speed".to_string(), "25".to_string());
    fields.insert("gap_fill_speed".to_string(), "20".to_string());

    // Retraction fields
    fields.insert("retract_length".to_string(), "0.8".to_string());
    fields.insert("retract_speed".to_string(), "35".to_string());
    fields.insert("retract_lift".to_string(), "0.3".to_string());
    fields.insert("retract_before_travel".to_string(), "2.0".to_string());
    fields.insert("deretract_speed".to_string(), "25".to_string());

    // Machine fields
    fields.insert("nozzle_diameter".to_string(), "0.4".to_string());
    fields.insert(
        "bed_shape".to_string(),
        "0x0,250x0,250x210,0x210".to_string(),
    );
    fields.insert("start_gcode".to_string(), "G28\nG29".to_string());
    fields.insert("end_gcode".to_string(), "M104 S0".to_string());
    fields.insert("max_print_speed".to_string(), "200".to_string());
    fields.insert("machine_max_acceleration_x".to_string(), "5000".to_string());
    fields.insert("machine_max_acceleration_y".to_string(), "5000".to_string());
    fields.insert("machine_max_acceleration_z".to_string(), "100".to_string());
    fields.insert("machine_max_acceleration_e".to_string(), "5000".to_string());

    // Acceleration
    fields.insert("default_acceleration".to_string(), "1000".to_string());
    fields.insert("travel_acceleration".to_string(), "1500".to_string());
    fields.insert("first_layer_acceleration".to_string(), "500".to_string());

    // Filament props
    fields.insert("filament_diameter".to_string(), "1.75".to_string());
    fields.insert("filament_density".to_string(), "1.24".to_string());
    fields.insert("filament_cost".to_string(), "20".to_string());
    fields.insert("filament_type".to_string(), "PLA".to_string());
    fields.insert("filament_vendor".to_string(), "Generic".to_string());
    fields.insert("max_volumetric_speed".to_string(), "15".to_string());
    fields.insert("temperature".to_string(), "215".to_string());
    fields.insert("bed_temperature".to_string(), "60".to_string());
    fields.insert("first_layer_temperature".to_string(), "220".to_string());
    fields.insert("first_layer_bed_temperature".to_string(), "65".to_string());

    // Cooling
    fields.insert("min_fan_speed".to_string(), "35".to_string());
    fields.insert("max_fan_speed".to_string(), "100".to_string());
    fields.insert("slowdown_below_layer_time".to_string(), "5".to_string());
    fields.insert("min_print_speed".to_string(), "10".to_string());

    // Line widths
    fields.insert(
        "external_perimeter_extrusion_width".to_string(),
        "0.42".to_string(),
    );
    fields.insert("perimeter_extrusion_width".to_string(), "0.45".to_string());
    fields.insert("infill_extrusion_width".to_string(), "0.45".to_string());
    fields.insert("top_infill_extrusion_width".to_string(), "0.42".to_string());
    fields.insert("first_layer_extrusion_width".to_string(), "0.5".to_string());
    fields.insert(
        "solid_infill_extrusion_width".to_string(),
        "0.42".to_string(),
    );
    fields.insert(
        "support_material_extrusion_width".to_string(),
        "0.42".to_string(),
    );

    // Process misc
    fields.insert("layer_height".to_string(), "0.2".to_string());
    fields.insert("first_layer_height".to_string(), "0.3".to_string());
    fields.insert("perimeters".to_string(), "2".to_string());
    fields.insert("fill_density".to_string(), "20%".to_string());
    fields.insert("fill_pattern".to_string(), "grid".to_string());

    let result = import_prusaslicer_ini_profile(&fields, "SC5 Test", "print");

    // SC5: INI mapper should map a significant portion of these fields
    assert!(
        result.mapped_fields.len() >= 40,
        "SC5: Expected 40+ mapped fields from INI, got {}. Fields: {:?}",
        result.mapped_fields.len(),
        result.mapped_fields
    );

    // Verify key values were correctly mapped
    let config = &result.config;
    assert!(
        (config.speeds.perimeter - 60.0).abs() < 1e-9,
        "perimeter speed"
    );
    assert!(
        (config.retraction.length - 0.8).abs() < 1e-9,
        "retraction length"
    );
    assert!(
        (config.filament.density - 1.24).abs() < 1e-9,
        "filament density"
    );
}

/// SC7: Verify passthrough BTreeMap is serialized correctly in TOML.
#[test]
fn test_passthrough_serializes_in_toml() {
    let mut config = PrintConfig::default();
    config
        .passthrough
        .insert("custom_field_1".to_string(), "value1".to_string());
    config
        .passthrough
        .insert("custom_field_2".to_string(), "42".to_string());

    let toml_str = toml::to_string(&config).unwrap();

    assert!(
        toml_str.contains("[passthrough]"),
        "TOML should contain [passthrough] section"
    );
    assert!(
        toml_str.contains("custom_field_1"),
        "TOML should contain custom_field_1"
    );
    assert!(
        toml_str.contains("custom_field_2"),
        "TOML should contain custom_field_2"
    );

    // Verify round-trip
    let roundtrip: PrintConfig = toml::from_str(&toml_str).unwrap();
    assert_eq!(
        roundtrip.passthrough.get("custom_field_1"),
        Some(&"value1".to_string())
    );
    assert_eq!(
        roundtrip.passthrough.get("custom_field_2"),
        Some(&"42".to_string())
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Walk a profile directory and find TOML files under a specific subdirectory.
fn walkdir(base: &std::path::Path, subdir: &str) -> Vec<std::path::PathBuf> {
    let mut results = Vec::new();
    walk_recursive(base, subdir, &mut results);
    results
}

fn walk_recursive(
    dir: &std::path::Path,
    target_subdir: &str,
    results: &mut Vec<std::path::PathBuf>,
) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().map_or(false, |n| n == target_subdir) {
                    // Found target subdir, collect TOML files
                    collect_toml_files(&path, results);
                } else {
                    walk_recursive(&path, target_subdir, results);
                }
            }
        }
    }
}

fn collect_toml_files(dir: &std::path::Path, results: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "toml") {
                results.push(path);
            }
        }
    }
}

/// Find a filament profile containing a keyword in its filename.
fn find_filament_profile(dir: &std::path::Path, keyword: &str) -> Option<std::path::PathBuf> {
    if !dir.exists() {
        return None;
    }
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "toml") {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.contains(keyword) {
                    return Some(path);
                }
            }
        }
    }
    None
}
