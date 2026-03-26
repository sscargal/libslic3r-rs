//! Integration tests for profile conversion pipeline (JSON -> TOML -> PrintConfig).
//!
//! Tests with inline synthetic JSON run as normal tests.
//! Tests that load files from `/home/steve/slicer-analysis/` are `#[ignore]`
//! and require that directory to be present. Run them manually with:
//!   cargo test --test integration_profile_convert -- --ignored

use slicecore_engine::config::PrintConfig;
use slicecore_engine::profile_convert::{convert_to_toml, merge_import_results};
use slicecore_engine::profile_import::import_upstream_profile;
use slicecore_engine::SeamPosition;
use slicecore_gcode_io::GcodeDialect;

// ---------------------------------------------------------------------------
// 1. Round-trip: Process profile
// ---------------------------------------------------------------------------

#[test]
fn test_round_trip_process_profile() {
    let json = serde_json::json!({
        "type": "process",
        "name": "RT Process",
        "layer_height": "0.15",
        "wall_loops": "4",
        "sparse_infill_density": "25%",
        "outer_wall_speed": "180",
        "travel_speed": "400",
        "seam_position": "random",
        "sparse_infill_pattern": "grid",
        "top_shell_layers": "5",
        "bottom_shell_layers": "4",
        "initial_layer_print_height": "0.25"
    });

    let import = import_upstream_profile(&json).unwrap();
    let converted = convert_to_toml(&import);

    // Parse the TOML back into a PrintConfig.
    let roundtrip = PrintConfig::from_toml(&converted.toml_output).unwrap();

    // Verify all mapped fields survive the round-trip.
    assert!(
        (roundtrip.layer_height - 0.15).abs() < 1e-6,
        "layer_height: expected 0.15, got {}",
        roundtrip.layer_height
    );
    assert_eq!(roundtrip.wall_count, 4, "wall_count should be 4");
    assert!(
        (roundtrip.infill_density - 0.25).abs() < 1e-6,
        "infill_density: expected 0.25, got {}",
        roundtrip.infill_density
    );
    assert!(
        (roundtrip.speeds.perimeter - 180.0).abs() < 1e-6,
        "perimeter_speed: expected 180, got {}",
        roundtrip.speeds.perimeter
    );
    assert!(
        (roundtrip.speeds.travel - 400.0).abs() < 1e-6,
        "travel_speed: expected 400, got {}",
        roundtrip.speeds.travel
    );
    assert_eq!(
        roundtrip.seam_position,
        SeamPosition::Random,
        "seam_position should be Random"
    );
    assert_eq!(
        roundtrip.infill_pattern,
        slicecore_engine::InfillPattern::Grid,
        "infill_pattern should be Grid"
    );
    assert_eq!(
        roundtrip.top_solid_layers, 5,
        "top_solid_layers should be 5"
    );
    assert_eq!(
        roundtrip.bottom_solid_layers, 4,
        "bottom_solid_layers should be 4"
    );
    assert!(
        (roundtrip.first_layer_height - 0.25).abs() < 1e-6,
        "first_layer_height: expected 0.25, got {}",
        roundtrip.first_layer_height
    );
}

// ---------------------------------------------------------------------------
// 2. Round-trip: Filament profile
// ---------------------------------------------------------------------------

#[test]
fn test_round_trip_filament_profile() {
    let json = serde_json::json!({
        "type": "filament",
        "name": "RT Filament",
        "nozzle_temperature": ["235"],
        "hot_plate_temp": ["80"],
        "filament_density": ["1.08"],
        "filament_diameter": ["1.75"],
        "filament_flow_ratio": ["0.95"],
        "filament_cost": ["30"]
    });

    let import = import_upstream_profile(&json).unwrap();
    let converted = convert_to_toml(&import);
    let roundtrip = PrintConfig::from_toml(&converted.toml_output).unwrap();

    assert!(
        (roundtrip.filament.nozzle_temp() - 235.0).abs() < 1e-6,
        "nozzle_temp: expected 235, got {}",
        roundtrip.filament.nozzle_temp()
    );
    assert!(
        (roundtrip.filament.bed_temp() - 80.0).abs() < 1e-6,
        "bed_temp: expected 80, got {}",
        roundtrip.filament.bed_temp()
    );
    assert!(
        (roundtrip.filament.density - 1.08).abs() < 1e-6,
        "filament_density: expected 1.08, got {}",
        roundtrip.filament.density
    );
    assert!(
        (roundtrip.filament.diameter - 1.75).abs() < 1e-6,
        "filament_diameter: expected 1.75, got {}",
        roundtrip.filament.diameter
    );
    assert!(
        (roundtrip.extrusion_multiplier - 0.95).abs() < 1e-6,
        "extrusion_multiplier: expected 0.95, got {}",
        roundtrip.extrusion_multiplier
    );
    assert!(
        (roundtrip.filament.cost_per_kg - 30.0).abs() < 1e-6,
        "filament_cost_per_kg: expected 30, got {}",
        roundtrip.filament.cost_per_kg
    );
}

// ---------------------------------------------------------------------------
// 3. Round-trip: Machine profile
// ---------------------------------------------------------------------------

#[test]
fn test_round_trip_machine_profile() {
    let json = serde_json::json!({
        "type": "machine",
        "name": "RT Machine",
        "nozzle_diameter": ["0.6"],
        "retraction_length": ["1.0"],
        "retraction_speed": ["60"],
        "z_hop": ["0.3"],
        "gcode_flavor": "klipper"
    });

    let import = import_upstream_profile(&json).unwrap();
    let converted = convert_to_toml(&import);
    let roundtrip = PrintConfig::from_toml(&converted.toml_output).unwrap();

    assert!(
        (roundtrip.machine.nozzle_diameter() - 0.6).abs() < 1e-6,
        "nozzle_diameter: expected 0.6, got {}",
        roundtrip.machine.nozzle_diameter()
    );
    assert!(
        (roundtrip.retraction.length - 1.0).abs() < 1e-6,
        "retract_length: expected 1.0, got {}",
        roundtrip.retraction.length
    );
    assert!(
        (roundtrip.retraction.speed - 60.0).abs() < 1e-6,
        "retract_speed: expected 60, got {}",
        roundtrip.retraction.speed
    );
    assert!(
        (roundtrip.z_hop.height - 0.3).abs() < 1e-6,
        "z_hop.height: expected 0.3, got {}",
        roundtrip.z_hop.height
    );
    assert_eq!(
        roundtrip.gcode_dialect,
        GcodeDialect::Klipper,
        "gcode_dialect should be Klipper"
    );
}

// ---------------------------------------------------------------------------
// 4. Merge: Process + Filament
// ---------------------------------------------------------------------------

#[test]
fn test_merge_process_and_filament() {
    let process_json = serde_json::json!({
        "type": "process",
        "name": "Merge Process",
        "layer_height": "0.15",
        "wall_loops": "4",
        "sparse_infill_density": "30%"
    });

    let filament_json = serde_json::json!({
        "type": "filament",
        "name": "Merge Filament",
        "nozzle_temperature": ["240"],
        "hot_plate_temp": ["90"],
        "filament_density": ["1.08"]
    });

    let process_result = import_upstream_profile(&process_json).unwrap();
    let filament_result = import_upstream_profile(&filament_json).unwrap();
    let merged = merge_import_results(&[process_result, filament_result]);

    // Convert merged result and parse back.
    let converted = convert_to_toml(&merged);
    let roundtrip = PrintConfig::from_toml(&converted.toml_output).unwrap();

    // Fields from process profile.
    assert!(
        (roundtrip.layer_height - 0.15).abs() < 1e-6,
        "layer_height from process: expected 0.15, got {}",
        roundtrip.layer_height
    );
    assert_eq!(
        roundtrip.wall_count, 4,
        "wall_count from process should be 4"
    );
    assert!(
        (roundtrip.infill_density - 0.3).abs() < 1e-6,
        "infill_density from process: expected 0.3, got {}",
        roundtrip.infill_density
    );

    // Fields from filament profile.
    assert!(
        (roundtrip.filament.nozzle_temp() - 240.0).abs() < 1e-6,
        "nozzle_temp from filament: expected 240, got {}",
        roundtrip.filament.nozzle_temp()
    );
    assert!(
        (roundtrip.filament.bed_temp() - 90.0).abs() < 1e-6,
        "bed_temp from filament: expected 90, got {}",
        roundtrip.filament.bed_temp()
    );

    // Merged mapped_fields should contain fields from both sources (deduplicated).
    assert!(
        merged.mapped_fields.contains(&"layer_height".to_string()),
        "merged should have layer_height in mapped_fields"
    );
    assert!(
        merged
            .mapped_fields
            .contains(&"nozzle_temperature".to_string()),
        "merged should have nozzle_temperature in mapped_fields"
    );
}

// ---------------------------------------------------------------------------
// 5. Merge: Process + Filament + Machine
// ---------------------------------------------------------------------------

#[test]
fn test_merge_three_profiles() {
    let process_json = serde_json::json!({
        "type": "process",
        "name": "Three-P",
        "layer_height": "0.1",
        "wall_loops": "3"
    });

    let filament_json = serde_json::json!({
        "type": "filament",
        "name": "Three-F",
        "nozzle_temperature": ["250"],
        "hot_plate_temp": ["100"]
    });

    let machine_json = serde_json::json!({
        "type": "machine",
        "name": "Three-M",
        "retraction_length": ["1.2"],
        "nozzle_diameter": ["0.6"],
        "gcode_flavor": "klipper"
    });

    let p = import_upstream_profile(&process_json).unwrap();
    let f = import_upstream_profile(&filament_json).unwrap();
    let m = import_upstream_profile(&machine_json).unwrap();
    let merged = merge_import_results(&[p, f, m]);

    let converted = convert_to_toml(&merged);
    let roundtrip = PrintConfig::from_toml(&converted.toml_output).unwrap();

    // Process fields.
    assert!(
        (roundtrip.layer_height - 0.1).abs() < 1e-6,
        "layer_height from process"
    );
    assert_eq!(roundtrip.wall_count, 3, "wall_count from process");

    // Filament fields.
    assert!(
        (roundtrip.filament.nozzle_temp() - 250.0).abs() < 1e-6,
        "nozzle_temp from filament"
    );
    assert!(
        (roundtrip.filament.bed_temp() - 100.0).abs() < 1e-6,
        "bed_temp from filament"
    );

    // Machine fields.
    assert!(
        (roundtrip.retraction.length - 1.2).abs() < 1e-6,
        "retract_length from machine"
    );
    assert!(
        (roundtrip.machine.nozzle_diameter() - 0.6).abs() < 1e-6,
        "nozzle_diameter from machine"
    );
    assert_eq!(
        roundtrip.gcode_dialect,
        GcodeDialect::Klipper,
        "gcode_dialect from machine"
    );
}

// ---------------------------------------------------------------------------
// 6. Selective output: only non-default fields
// ---------------------------------------------------------------------------

#[test]
fn test_selective_output_no_defaults() {
    let json = serde_json::json!({
        "type": "process",
        "name": "Selective Test",
        "layer_height": "0.15",
        "wall_loops": "4"
    });

    let import = import_upstream_profile(&json).unwrap();
    let converted = convert_to_toml(&import);
    let toml = &converted.toml_output;

    // Should contain the two non-default fields.
    assert!(
        toml.contains("layer_height"),
        "TOML should contain layer_height"
    );
    assert!(
        toml.contains("wall_count"),
        "TOML should contain wall_count"
    );

    // Should NOT contain default-only fields (not in the import at all).
    assert!(
        !toml.contains("fan_speed"),
        "TOML should not contain default-only fan_speed"
    );
    // "support" appears in header comments context ("Unmapped fields: 0"),
    // but should not appear as a TOML key assignment.
    let body_lines: Vec<&str> = toml
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .collect();
    for line in &body_lines {
        assert!(
            !line.starts_with("ironing"),
            "TOML body should not have ironing defaults: {}",
            line
        );
        assert!(
            !line.starts_with("scarf_joint"),
            "TOML body should not have scarf_joint defaults: {}",
            line
        );
        assert!(
            !line.starts_with("multi_material"),
            "TOML body should not have multi_material defaults: {}",
            line
        );
    }

    // Verify conciseness: non-comment lines should be under 20.
    assert!(
        body_lines.len() < 20,
        "TOML body should be concise (under 20 non-comment lines), got {} lines",
        body_lines.len()
    );
}

// ---------------------------------------------------------------------------
// 7. Unmapped fields appear in TOML comments
// ---------------------------------------------------------------------------

#[test]
fn test_unmapped_fields_in_output() {
    // bridge_speed and gap_infill_speed are now mapped to typed fields (Phase 20).
    // Use truly unknown fields to test passthrough/unmapped reporting.
    let json = serde_json::json!({
        "type": "process",
        "name": "Unmapped Test",
        "layer_height": "0.2",
        "ams_drying_temperature": "55",
        "scan_first_layer": "1"
    });

    let import = import_upstream_profile(&json).unwrap();
    let converted = convert_to_toml(&import);

    // Passthrough fields (no typed mapping) should appear as comments in TOML output.
    assert!(
        converted.toml_output.contains("ams_drying_temperature"),
        "TOML should mention unmapped ams_drying_temperature"
    );
    assert!(
        converted.toml_output.contains("scan_first_layer"),
        "TOML should mention unmapped scan_first_layer"
    );

    // ConvertResult should report them via unmapped_fields (backward compat).
    assert!(
        converted
            .unmapped_fields
            .contains(&"ams_drying_temperature".to_string()),
        "unmapped_fields should contain ams_drying_temperature"
    );
    assert!(
        converted
            .unmapped_fields
            .contains(&"scan_first_layer".to_string()),
        "unmapped_fields should contain scan_first_layer"
    );
}

// ---------------------------------------------------------------------------
// 8. Float precision: percentage -> clean TOML output
// ---------------------------------------------------------------------------

#[test]
fn test_percentage_float_clean_output() {
    let json = serde_json::json!({
        "type": "process",
        "name": "Float Test",
        "sparse_infill_density": "15%"
    });

    let import = import_upstream_profile(&json).unwrap();
    let converted = convert_to_toml(&import);

    // The TOML should contain a clean 0.15, not IEEE 754 noise.
    assert!(
        !converted.toml_output.contains("0.15000000000000002"),
        "TOML should not contain IEEE 754 noise"
    );

    // infill_density = 0.15 differs from default (0.2), so it should appear.
    assert!(
        converted.toml_output.contains("infill_density = 0.15"),
        "TOML should contain clean infill_density = 0.15, got:\n{}",
        converted.toml_output
    );
}

// ---------------------------------------------------------------------------
// 9. Real OrcaSlicer profile conversion (gated with #[ignore])
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn test_real_orcaslicer_profile_conversion() {
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

    eprintln!("Loading real profile: {}", profile_path.display());

    let data = std::fs::read_to_string(&profile_path).unwrap();
    let json_value: serde_json::Value = serde_json::from_str(&data).unwrap();
    let import = import_upstream_profile(&json_value).unwrap();
    let converted = convert_to_toml(&import);

    // Parse the TOML back.
    let roundtrip = PrintConfig::from_toml(&converted.toml_output).unwrap();

    // Verify reasonable values (real profile should set these).
    assert!(
        roundtrip.layer_height > 0.0,
        "layer_height should be positive, got {}",
        roundtrip.layer_height
    );
    assert!(
        roundtrip.speeds.perimeter > 0.0,
        "perimeter_speed should be positive, got {}",
        roundtrip.speeds.perimeter
    );

    eprintln!(
        "  Round-trip successful: layer_height={}, perimeter_speed={}, mapped={}, unmapped={}",
        roundtrip.layer_height,
        roundtrip.speeds.perimeter,
        converted.mapped_count,
        converted.unmapped_fields.len()
    );
}

// ---------------------------------------------------------------------------
// 10. Real multi-file merge (gated with #[ignore])
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn test_real_multi_file_merge() {
    let process_dir = std::path::Path::new(
        "/home/steve/slicer-analysis/OrcaSlicer/resources/profiles/BBL/process",
    );
    let filament_dir = std::path::Path::new(
        "/home/steve/slicer-analysis/OrcaSlicer/resources/profiles/BBL/filament",
    );
    let machine_dir = std::path::Path::new(
        "/home/steve/slicer-analysis/OrcaSlicer/resources/profiles/BBL/machine",
    );

    assert!(process_dir.exists(), "Process directory not found");
    assert!(filament_dir.exists(), "Filament directory not found");
    assert!(machine_dir.exists(), "Machine directory not found");

    // Find one profile of each type.
    let find_json = |dir: &std::path::Path, hint: &str| -> std::path::PathBuf {
        std::fs::read_dir(dir)
            .expect("Failed to read dir")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| {
                p.is_file()
                    && p.extension().is_some_and(|ext| ext == "json")
                    && (hint.is_empty()
                        || p.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .contains(hint))
            })
            .unwrap_or_else(|| {
                // Fallback: any JSON file in the directory.
                std::fs::read_dir(dir)
                    .unwrap()
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .find(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "json"))
                    .expect("No JSON files found in directory")
            })
    };

    let process_path = find_json(process_dir, "0.20mm");
    let filament_path = find_json(filament_dir, "Bambu PLA");
    let machine_path = find_json(machine_dir, "");

    eprintln!("Process:  {}", process_path.display());
    eprintln!("Filament: {}", filament_path.display());
    eprintln!("Machine:  {}", machine_path.display());

    let load_import = |path: &std::path::Path| {
        let data = std::fs::read_to_string(path).unwrap();
        let json_value: serde_json::Value = serde_json::from_str(&data).unwrap();
        import_upstream_profile(&json_value).unwrap()
    };

    let p = load_import(&process_path);
    let f = load_import(&filament_path);
    let m = load_import(&machine_path);

    let merged = merge_import_results(&[p, f, m]);
    let converted = convert_to_toml(&merged);
    let roundtrip = PrintConfig::from_toml(&converted.toml_output).unwrap();

    // Verify the merged config has populated fields from all three sources.
    assert!(
        roundtrip.layer_height > 0.0,
        "layer_height should be populated"
    );
    // At least one of the temperature fields should differ from default.
    let defaults = PrintConfig::default();
    let has_filament_data =
        (roundtrip.filament.nozzle_temp() - defaults.filament.nozzle_temp()).abs() > 1.0
            || (roundtrip.filament.bed_temp() - defaults.filament.bed_temp()).abs() > 1.0
            || (roundtrip.filament.density - defaults.filament.density).abs() > 0.01;
    assert!(
        has_filament_data,
        "Merged config should have some filament data differing from defaults"
    );

    eprintln!(
        "  Merged: layer_height={}, nozzle_temp={}, nozzle_diameter={}, mapped={}, unmapped={}",
        roundtrip.layer_height,
        roundtrip.filament.nozzle_temp(),
        roundtrip.machine.nozzle_diameter(),
        converted.mapped_count,
        converted.unmapped_fields.len()
    );
    eprintln!("  TOML output ({} bytes):", converted.toml_output.len());
    for line in converted.toml_output.lines().take(15) {
        eprintln!("    {}", line);
    }
}
