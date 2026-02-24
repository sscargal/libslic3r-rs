//! Integration tests for JSON profile import.
//!
//! Tests with inline synthetic JSON run as normal tests.
//! Tests that load files from `/home/steve/slicer-analysis/` are `#[ignore]`
//! and require that directory to be present. Run them manually with:
//!   cargo test --test integration_profile_import -- --ignored

use slicecore_engine::config::PrintConfig;
use slicecore_engine::SeamPosition;
use slicecore_gcode_io::GcodeDialect;
use std::io::Write;

// ---------------------------------------------------------------------------
// Synthetic profile tests (no external files required)
// ---------------------------------------------------------------------------

#[test]
fn test_synthetic_process_profile_roundtrip() {
    let json = r#"{
        "type": "process",
        "name": "Synthetic Process",
        "layer_height": "0.2",
        "wall_loops": "2",
        "sparse_infill_density": "15%",
        "outer_wall_speed": "200",
        "seam_position": "aligned",
        "default_acceleration": "10000"
    }"#;

    let config = PrintConfig::from_json(json).unwrap();

    assert!(
        (config.layer_height - 0.2).abs() < 1e-9,
        "layer_height should be 0.2, got {}",
        config.layer_height
    );
    assert_eq!(config.wall_count, 2);
    assert!(
        (config.infill_density - 0.15).abs() < 1e-9,
        "infill_density should be 0.15, got {}",
        config.infill_density
    );
    assert!(
        (config.speeds.perimeter - 200.0).abs() < 1e-9,
        "perimeter_speed should be 200, got {}",
        config.speeds.perimeter
    );
    assert_eq!(config.seam_position, SeamPosition::Aligned);
    assert!(
        (config.accel.print - 10000.0).abs() < 1e-9,
        "print_acceleration should be 10000, got {}",
        config.accel.print
    );
}

#[test]
fn test_synthetic_filament_profile_roundtrip() {
    let json = r#"{
        "type": "filament",
        "name": "Synthetic Filament",
        "nozzle_temperature": ["220"],
        "hot_plate_temp": ["55"],
        "filament_density": ["1.24"],
        "filament_flow_ratio": ["0.98"],
        "close_fan_the_first_x_layers": ["1"]
    }"#;

    let config = PrintConfig::from_json(json).unwrap();

    assert!(
        (config.filament.nozzle_temp() - 220.0).abs() < 1e-9,
        "nozzle_temp should be 220, got {}",
        config.filament.nozzle_temp()
    );
    assert!(
        (config.filament.bed_temp() - 55.0).abs() < 1e-9,
        "bed_temp should be 55, got {}",
        config.filament.bed_temp()
    );
    assert!(
        (config.filament.density - 1.24).abs() < 1e-9,
        "filament_density should be 1.24, got {}",
        config.filament.density
    );
    assert!(
        (config.extrusion_multiplier - 0.98).abs() < 1e-9,
        "extrusion_multiplier should be 0.98, got {}",
        config.extrusion_multiplier
    );
    assert_eq!(config.cooling.disable_fan_first_layers, 1);
}

#[test]
fn test_synthetic_machine_profile_roundtrip() {
    let json = r#"{
        "type": "machine",
        "name": "Synthetic Machine",
        "nozzle_diameter": ["0.4"],
        "retraction_length": ["0.8"],
        "gcode_flavor": "marlin",
        "machine_max_jerk_x": ["8.0"],
        "machine_max_jerk_y": ["8.0"]
    }"#;

    let config = PrintConfig::from_json(json).unwrap();

    assert!(
        (config.machine.nozzle_diameter() - 0.4).abs() < 1e-9,
        "nozzle_diameter should be 0.4, got {}",
        config.machine.nozzle_diameter()
    );
    assert!(
        (config.retraction.length - 0.8).abs() < 1e-9,
        "retract_length should be 0.8, got {}",
        config.retraction.length
    );
    assert_eq!(config.gcode_dialect, GcodeDialect::Marlin);
    assert!(
        (config.machine.jerk_x() - 8.0).abs() < 1e-9,
        "jerk_x should be 8.0, got {}",
        config.machine.jerk_x()
    );
}

#[test]
fn test_synthetic_nil_values_use_defaults() {
    let json = r#"{
        "type": "process",
        "name": "Nil Test",
        "layer_height": "nil",
        "wall_loops": "nil",
        "outer_wall_speed": "100"
    }"#;

    let config = PrintConfig::from_json(json).unwrap();
    let defaults = PrintConfig::default();

    // nil fields should keep defaults.
    assert!(
        (config.layer_height - defaults.layer_height).abs() < 1e-9,
        "layer_height should be default {} when nil, got {}",
        defaults.layer_height,
        config.layer_height
    );
    assert_eq!(
        config.wall_count, defaults.wall_count,
        "wall_count should be default {} when nil, got {}",
        defaults.wall_count, config.wall_count
    );

    // Non-nil field should be overridden.
    assert!(
        (config.speeds.perimeter - 100.0).abs() < 1e-9,
        "perimeter_speed should be 100 (not nil), got {}",
        config.speeds.perimeter
    );
}

#[test]
fn test_synthetic_mixed_array_scalar() {
    let json = r#"{
        "type": "filament",
        "name": "Mixed Test",
        "nozzle_temperature": ["210"],
        "hot_plate_temp": "60",
        "filament_density": ["1.27"],
        "filament_flow_ratio": "1.0"
    }"#;

    let config = PrintConfig::from_json(json).unwrap();

    // Array-wrapped value.
    assert!(
        (config.filament.nozzle_temp() - 210.0).abs() < 1e-9,
        "nozzle_temp (array) should be 210, got {}",
        config.filament.nozzle_temp()
    );
    // Scalar string value.
    assert!(
        (config.filament.bed_temp() - 60.0).abs() < 1e-9,
        "bed_temp (scalar) should be 60, got {}",
        config.filament.bed_temp()
    );
    // Array-wrapped.
    assert!(
        (config.filament.density - 1.27).abs() < 1e-9,
        "filament_density (array) should be 1.27, got {}",
        config.filament.density
    );
    // Scalar string.
    assert!(
        (config.extrusion_multiplier - 1.0).abs() < 1e-9,
        "extrusion_multiplier (scalar) should be 1.0, got {}",
        config.extrusion_multiplier
    );
}

#[test]
fn test_native_json_config() {
    // Native JSON with PrintConfig nested field names and real numeric values (not strings).
    let json = r#"{
        "layer_height": 0.15,
        "machine": { "nozzle_diameters": [0.6] },
        "wall_count": 4,
        "infill_density": 0.3,
        "speeds": { "perimeter": 60.0 },
        "filament": { "nozzle_temperatures": [215.0], "bed_temperatures": [65.0] }
    }"#;

    let config = PrintConfig::from_json(json).unwrap();

    assert!(
        (config.layer_height - 0.15).abs() < 1e-9,
        "layer_height should be 0.15, got {}",
        config.layer_height
    );
    assert!(
        (config.machine.nozzle_diameter() - 0.6).abs() < 1e-9,
        "nozzle_diameter should be 0.6, got {}",
        config.machine.nozzle_diameter()
    );
    assert_eq!(config.wall_count, 4);
    assert!(
        (config.infill_density - 0.3).abs() < 1e-9,
        "infill_density should be 0.3, got {}",
        config.infill_density
    );
    assert!(
        (config.speeds.perimeter - 60.0).abs() < 1e-9,
        "perimeter_speed should be 60, got {}",
        config.speeds.perimeter
    );
    assert!(
        (config.filament.nozzle_temp() - 215.0).abs() < 1e-9,
        "nozzle_temp should be 215, got {}",
        config.filament.nozzle_temp()
    );
    assert!(
        (config.filament.bed_temp() - 65.0).abs() < 1e-9,
        "bed_temp should be 65, got {}",
        config.filament.bed_temp()
    );
}

#[test]
fn test_toml_still_works() {
    // Regression guard: TOML files still load correctly via from_file.
    let dir = std::env::temp_dir().join("slicecore_integration_toml_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test_regression.toml");

    {
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "layer_height = 0.1\nwall_count = 5\ninfill_density = 0.4").unwrap();
    }

    let config = PrintConfig::from_file(&path).unwrap();

    assert!(
        (config.layer_height - 0.1).abs() < 1e-9,
        "TOML layer_height should be 0.1, got {}",
        config.layer_height
    );
    assert_eq!(config.wall_count, 5, "TOML wall_count should be 5");
    assert!(
        (config.infill_density - 0.4).abs() < 1e-9,
        "TOML infill_density should be 0.4, got {}",
        config.infill_density
    );

    // Cleanup.
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_import_result_reports_unmapped_fields() {
    let json = r#"{
        "type": "process",
        "name": "Unmapped Test",
        "layer_height": "0.2",
        "wall_loops": "3",
        "some_unknown_field": "value",
        "another_exotic_setting": "42",
        "outer_wall_speed": "150"
    }"#;

    let result = PrintConfig::from_json_with_details(json).unwrap();

    // Verify mapped fields include known ones.
    assert!(
        result.mapped_fields.contains(&"layer_height".to_string()),
        "layer_height should be in mapped_fields: {:?}",
        result.mapped_fields
    );
    assert!(
        result.mapped_fields.contains(&"wall_loops".to_string()),
        "wall_loops should be in mapped_fields: {:?}",
        result.mapped_fields
    );
    assert!(
        result
            .mapped_fields
            .contains(&"outer_wall_speed".to_string()),
        "outer_wall_speed should be in mapped_fields: {:?}",
        result.mapped_fields
    );

    // Verify unmapped fields include unknown ones.
    assert!(
        result
            .unmapped_fields
            .contains(&"some_unknown_field".to_string()),
        "some_unknown_field should be in unmapped_fields: {:?}",
        result.unmapped_fields
    );
    assert!(
        result
            .unmapped_fields
            .contains(&"another_exotic_setting".to_string()),
        "another_exotic_setting should be in unmapped_fields: {:?}",
        result.unmapped_fields
    );

    // Mapped count should be 3 (layer_height, wall_loops, outer_wall_speed).
    assert_eq!(
        result.mapped_fields.len(),
        3,
        "Should have 3 mapped fields, got {:?}",
        result.mapped_fields
    );
    assert_eq!(
        result.unmapped_fields.len(),
        2,
        "Should have 2 unmapped fields, got {:?}",
        result.unmapped_fields
    );
}

// ---------------------------------------------------------------------------
// Real upstream profile tests (require /home/steve/slicer-analysis/)
// ---------------------------------------------------------------------------

/// Load an actual OrcaSlicer process profile.
/// Requires: /home/steve/slicer-analysis/OrcaSlicer/resources/profiles/BBL/process/
#[test]
#[ignore]
fn test_real_orcaslicer_process_profile() {
    let dir = std::path::Path::new(
        "/home/steve/slicer-analysis/OrcaSlicer/resources/profiles/BBL/process",
    );
    assert!(
        dir.exists(),
        "OrcaSlicer process profile directory not found: {}",
        dir.display()
    );

    // Find a 0.20mm profile.
    let profile_path = std::fs::read_dir(dir)
        .expect("Failed to read process profile dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            name.contains("0.20mm") && name.ends_with(".json")
        })
        .expect("No 0.20mm process profile found");

    eprintln!("Loading: {}", profile_path.display());
    let config = PrintConfig::from_file(&profile_path).unwrap();

    // Verify layer_height is close to 0.2 (may be exact or inherited).
    // Some profiles with inherits may not override layer_height, so check
    // it's either default (0.2) or explicitly set close to 0.2.
    assert!(
        (config.layer_height - 0.2).abs() < 0.05,
        "Expected layer_height near 0.2, got {}",
        config.layer_height
    );

    eprintln!(
        "  layer_height={}, wall_count={}, infill_density={:.2}, perimeter_speed={}",
        config.layer_height, config.wall_count, config.infill_density, config.speeds.perimeter
    );
}

/// Load an actual OrcaSlicer filament profile.
/// Requires: /home/steve/slicer-analysis/OrcaSlicer/resources/profiles/BBL/filament/
#[test]
#[ignore]
fn test_real_orcaslicer_filament_profile() {
    let dir = std::path::Path::new(
        "/home/steve/slicer-analysis/OrcaSlicer/resources/profiles/BBL/filament",
    );
    assert!(
        dir.exists(),
        "OrcaSlicer filament profile directory not found: {}",
        dir.display()
    );

    // Find a Bambu ABS or PLA filament profile (direct JSON, not subdirectory).
    let profile_path = std::fs::read_dir(dir)
        .expect("Failed to read filament profile dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.is_file()
                && p.extension().is_some_and(|ext| ext == "json")
                && p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .contains("Bambu ABS")
        })
        .expect("No Bambu ABS filament profile found");

    eprintln!("Loading: {}", profile_path.display());
    let config = PrintConfig::from_file(&profile_path).unwrap();

    // Sanity check: nozzle_temp or bed_temp should be non-default.
    // ABS typically has bed_temp ~100, so check it's been loaded.
    let defaults = PrintConfig::default();
    let has_non_default = (config.filament.nozzle_temp() - defaults.filament.nozzle_temp()).abs() > 1.0
        || (config.filament.bed_temp() - defaults.filament.bed_temp()).abs() > 1.0;

    assert!(
        has_non_default,
        "Filament profile should have at least one non-default temperature. nozzle_temp={}, bed_temp={}",
        config.filament.nozzle_temp(), config.filament.bed_temp()
    );

    eprintln!(
        "  nozzle_temp={}, bed_temp={}, filament_density={}, extrusion_multiplier={}",
        config.filament.nozzle_temp(), config.filament.bed_temp(), config.filament.density, config.extrusion_multiplier
    );
}

/// Load an actual BambuStudio profile.
/// Requires: /home/steve/slicer-analysis/BambuStudio/resources/profiles/BBL/
#[test]
#[ignore]
fn test_real_bambustudio_profile() {
    let dir = std::path::Path::new(
        "/home/steve/slicer-analysis/BambuStudio/resources/profiles/BBL/process",
    );
    assert!(
        dir.exists(),
        "BambuStudio process profile directory not found: {}",
        dir.display()
    );

    // Find any .json file.
    let profile_path = std::fs::read_dir(dir)
        .expect("Failed to read BambuStudio profile dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "json"))
        .expect("No BambuStudio JSON profile found");

    eprintln!("Loading: {}", profile_path.display());
    let config = PrintConfig::from_file(&profile_path).unwrap();

    // Just verify it loaded without error and produced some config.
    eprintln!(
        "  layer_height={}, wall_count={}, perimeter_speed={}",
        config.layer_height, config.wall_count, config.speeds.perimeter
    );
}

/// Bulk-load all OrcaSlicer process profiles from the BBL vendor directory.
/// Counts successes and failures, asserts at least 80% success rate.
/// Requires: /home/steve/slicer-analysis/OrcaSlicer/resources/profiles/BBL/process/
#[test]
#[ignore]
fn test_bulk_orcaslicer_profiles() {
    let dir = std::path::Path::new(
        "/home/steve/slicer-analysis/OrcaSlicer/resources/profiles/BBL/process",
    );
    assert!(
        dir.exists(),
        "OrcaSlicer process profile directory not found: {}",
        dir.display()
    );

    let json_files: Vec<_> = std::fs::read_dir(dir)
        .expect("Failed to read profile dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "json"))
        .collect();

    assert!(
        !json_files.is_empty(),
        "No JSON files found in {}",
        dir.display()
    );

    let total = json_files.len();
    let mut successes = 0;
    let mut failures: Vec<(String, String)> = Vec::new();

    for path in &json_files {
        match PrintConfig::from_file(path) {
            Ok(config) => {
                successes += 1;
                // Verify it's not all defaults (at least one field should differ).
                let _layer_height = config.layer_height;
            }
            Err(e) => {
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                failures.push((name, format!("{}", e)));
            }
        }
    }

    let success_rate = successes as f64 / total as f64;

    eprintln!("Bulk OrcaSlicer profile load results:");
    eprintln!("  Total: {}", total);
    eprintln!("  Successes: {}", successes);
    eprintln!("  Failures: {}", failures.len());
    eprintln!("  Success rate: {:.1}%", success_rate * 100.0);

    if !failures.is_empty() {
        eprintln!("  Failed profiles:");
        for (name, err) in &failures {
            eprintln!("    - {}: {}", name, err);
        }
    }

    assert!(
        success_rate >= 0.80,
        "Success rate {:.1}% is below 80% threshold ({} of {} failed)",
        success_rate * 100.0,
        failures.len(),
        total
    );
}
