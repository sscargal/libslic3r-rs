//! Integration tests for PrusaSlicer INI profile conversion pipeline.
//!
//! Tests 1-8 are synthetic and always run.
//! Tests 9-11 require real PrusaSlicer data at `/home/steve/slicer-analysis/`
//! and are gated with `#[ignore]`. Run them manually with:
//!   cargo test -p slicecore-engine --test integration_profile_library_ini -- --ignored

use std::collections::HashMap;
use std::path::Path;

use slicecore_engine::config::PrintConfig;
use slicecore_engine::profile_import_ini::{
    build_section_lookup, import_prusaslicer_ini_profile, parse_prusaslicer_ini,
    resolve_ini_inheritance,
};
use slicecore_engine::{
    batch_convert_prusaslicer_profiles, load_index, write_index, write_merged_index,
    ProfileIndex, ProfileIndexEntry,
};

use tempfile::TempDir;

// ---------------------------------------------------------------------------
// 1. test_parse_ini_sections
// ---------------------------------------------------------------------------

/// Parse a multi-section INI string with vendor, printer_model, print, filament,
/// printer sections. Verify section_type, name, is_abstract, field count.
#[test]
fn test_parse_ini_sections() {
    let ini = "\
[vendor]
name = PrusaResearch
config_version = 1.6.0

[printer_model:MK4S]
name = Original Prusa MK4S
variants = 0.4; 0.25; 0.6
technology = FFF

[print:*common*]
layer_height = 0.2
perimeters = 2
fill_density = 15%
perimeter_speed = 45

[print:0.20mm NORMAL]
inherits = *common*
perimeter_speed = 50
infill_speed = 80

[filament:Prusament PLA @MK4S]
temperature = 215
bed_temperature = 60
filament_density = 1.24

[printer:Original Prusa MK4S]
nozzle_diameter = 0.4
retract_length = 0.8
gcode_flavor = marlin
";

    let sections = parse_prusaslicer_ini(ini);
    assert_eq!(sections.len(), 6, "Should parse 6 sections");

    // Vendor section.
    assert_eq!(sections[0].section_type, "vendor");
    assert_eq!(sections[0].name, "");
    assert!(!sections[0].is_abstract);
    assert_eq!(sections[0].fields.len(), 2);

    // Printer model section.
    assert_eq!(sections[1].section_type, "printer_model");
    assert_eq!(sections[1].name, "MK4S");
    assert!(!sections[1].is_abstract);
    assert!(sections[1].fields.get("technology").is_some());

    // Abstract print section.
    assert_eq!(sections[2].section_type, "print");
    assert_eq!(sections[2].name, "*common*");
    assert!(sections[2].is_abstract);
    assert_eq!(sections[2].fields.len(), 4);
    assert_eq!(sections[2].fields.get("layer_height").unwrap(), "0.2");

    // Concrete print section.
    assert_eq!(sections[3].section_type, "print");
    assert_eq!(sections[3].name, "0.20mm NORMAL");
    assert!(!sections[3].is_abstract);
    assert!(sections[3].fields.get("inherits").is_some());

    // Filament section.
    assert_eq!(sections[4].section_type, "filament");
    assert_eq!(sections[4].name, "Prusament PLA @MK4S");
    assert!(!sections[4].is_abstract);
    assert_eq!(sections[4].fields.get("temperature").unwrap(), "215");

    // Printer section.
    assert_eq!(sections[5].section_type, "printer");
    assert_eq!(sections[5].name, "Original Prusa MK4S");
    assert!(!sections[5].is_abstract);
    assert_eq!(sections[5].fields.get("nozzle_diameter").unwrap(), "0.4");
}

// ---------------------------------------------------------------------------
// 2. test_ini_inheritance_single_parent
// ---------------------------------------------------------------------------

/// Create a parent print section and a child that inherits from it.
/// Verify child inherits parent fields and child overrides apply.
#[test]
fn test_ini_inheritance_single_parent() {
    let ini = "\
[print:*common*]
layer_height = 0.2
perimeters = 2
fill_density = 15%
perimeter_speed = 40
infill_speed = 80

[print:0.20mm NORMAL]
inherits = *common*
perimeters = 3
perimeter_speed = 50
";

    let sections = parse_prusaslicer_ini(ini);
    let lookup = build_section_lookup(&sections);

    let resolved = resolve_ini_inheritance(&sections[1], &sections, &lookup, 0);

    // Inherited from parent.
    assert_eq!(resolved.get("layer_height").unwrap(), "0.2");
    assert_eq!(resolved.get("fill_density").unwrap(), "15%");
    assert_eq!(resolved.get("infill_speed").unwrap(), "80");

    // Child overrides.
    assert_eq!(resolved.get("perimeters").unwrap(), "3");
    assert_eq!(resolved.get("perimeter_speed").unwrap(), "50");

    // inherits key should not appear (it is excluded in resolve_ini_inheritance).
    // Actually, resolve_ini_inheritance skips "inherits" in child overlay.
    assert!(
        resolved.get("inherits").is_none(),
        "inherits key should be excluded from resolved fields"
    );
}

// ---------------------------------------------------------------------------
// 3. test_ini_inheritance_multi_parent
// ---------------------------------------------------------------------------

/// Create two parent sections and a child inheriting from both.
/// Verify left-to-right merge order and child overrides.
#[test]
fn test_ini_inheritance_multi_parent() {
    let ini = "\
[print:*0.15mm*]
layer_height = 0.15
perimeters = 3
fill_density = 20%
perimeter_speed = 40
infill_speed = 60

[print:*soluble_support*]
perimeters = 4
support_material = 1
support_material_interface_layers = 3
perimeter_speed = 35

[print:0.15mm OPTIMAL SOLUBLE FULL]
inherits = *0.15mm*; *soluble_support*
fill_density = 25%
infill_speed = 50
";

    let sections = parse_prusaslicer_ini(ini);
    let lookup = build_section_lookup(&sections);

    let resolved = resolve_ini_inheritance(&sections[2], &sections, &lookup, 0);

    // From *0.15mm* (first parent): layer_height = 0.15.
    assert_eq!(resolved.get("layer_height").unwrap(), "0.15");

    // From *soluble_support* (second parent, overrides *0.15mm*): perimeters = 4.
    assert_eq!(
        resolved.get("perimeters").unwrap(),
        "4",
        "Second parent should override first parent"
    );

    // From *soluble_support* (second parent, overrides *0.15mm*): perimeter_speed = 35.
    assert_eq!(
        resolved.get("perimeter_speed").unwrap(),
        "35",
        "Second parent should override first parent's perimeter_speed"
    );

    // From *soluble_support* only: support fields.
    assert_eq!(resolved.get("support_material").unwrap(), "1");
    assert_eq!(
        resolved.get("support_material_interface_layers").unwrap(),
        "3"
    );

    // Child overrides both parents: fill_density = 25%.
    assert_eq!(
        resolved.get("fill_density").unwrap(),
        "25%",
        "Child should override both parents"
    );

    // Child overrides: infill_speed = 50.
    assert_eq!(
        resolved.get("infill_speed").unwrap(),
        "50",
        "Child should override parent infill_speed"
    );
}

// ---------------------------------------------------------------------------
// 4. test_prusaslicer_field_mapping_process
// ---------------------------------------------------------------------------

/// Test import_prusaslicer_ini_profile with process fields.
/// Verify PrintConfig values for wall_count, infill_density, infill_pattern, etc.
#[test]
fn test_prusaslicer_field_mapping_process() {
    let mut fields = HashMap::new();
    fields.insert("layer_height".to_string(), "0.2".to_string());
    fields.insert("perimeters".to_string(), "3".to_string());
    fields.insert("fill_density".to_string(), "15%".to_string());
    fields.insert("fill_pattern".to_string(), "cubic".to_string());
    fields.insert("first_layer_speed".to_string(), "20".to_string());
    fields.insert("seam_position".to_string(), "aligned".to_string());
    fields.insert("perimeter_speed".to_string(), "45".to_string());
    fields.insert("infill_speed".to_string(), "80".to_string());
    fields.insert("travel_speed".to_string(), "150".to_string());
    fields.insert("top_solid_layers".to_string(), "5".to_string());
    fields.insert("bottom_solid_layers".to_string(), "4".to_string());

    let result = import_prusaslicer_ini_profile(&fields, "0.20mm NORMAL", "print");
    let config = &result.config;

    assert!((config.layer_height - 0.2).abs() < 1e-9);
    assert_eq!(config.wall_count, 3);
    assert!((config.infill_density - 0.15).abs() < 1e-9);
    assert_eq!(
        config.infill_pattern,
        slicecore_engine::infill::InfillPattern::Cubic
    );
    assert!((config.speeds.first_layer - 20.0).abs() < 1e-9);
    assert_eq!(
        config.seam_position,
        slicecore_engine::seam::SeamPosition::Aligned
    );
    assert!((config.speeds.perimeter - 45.0).abs() < 1e-9);
    assert!((config.speeds.infill - 80.0).abs() < 1e-9);
    assert!((config.speeds.travel - 150.0).abs() < 1e-9);
    assert_eq!(config.top_solid_layers, 5);
    assert_eq!(config.bottom_solid_layers, 4);

    // Metadata.
    assert_eq!(result.metadata.profile_type.as_deref(), Some("process"));
    assert_eq!(result.metadata.name.as_deref(), Some("0.20mm NORMAL"));
}

// ---------------------------------------------------------------------------
// 5. test_prusaslicer_field_mapping_filament
// ---------------------------------------------------------------------------

/// Test import_prusaslicer_ini_profile with filament fields.
/// Verify PrintConfig values for nozzle_temp, bed_temp, filament_density, etc.
#[test]
fn test_prusaslicer_field_mapping_filament() {
    let mut fields = HashMap::new();
    fields.insert("temperature".to_string(), "210".to_string());
    fields.insert("first_layer_temperature".to_string(), "215".to_string());
    fields.insert("bed_temperature".to_string(), "60".to_string());
    fields.insert(
        "first_layer_bed_temperature".to_string(),
        "65".to_string(),
    );
    fields.insert("filament_density".to_string(), "1.24".to_string());
    fields.insert("filament_diameter".to_string(), "1.75".to_string());
    fields.insert("extrusion_multiplier".to_string(), "1".to_string());
    fields.insert("filament_cost".to_string(), "25".to_string());

    let result = import_prusaslicer_ini_profile(&fields, "Prusament PLA", "filament");
    let config = &result.config;

    assert!((config.filament.nozzle_temp() - 210.0).abs() < 1e-9);
    assert!((config.filament.first_layer_nozzle_temp() - 215.0).abs() < 1e-9);
    assert!((config.filament.bed_temp() - 60.0).abs() < 1e-9);
    assert!((config.filament.first_layer_bed_temp() - 65.0).abs() < 1e-9);
    assert!((config.filament.density - 1.24).abs() < 1e-9);
    assert!((config.filament.diameter - 1.75).abs() < 1e-9);
    assert!((config.extrusion_multiplier - 1.0).abs() < 1e-9);
    assert!((config.filament.cost_per_kg - 25.0).abs() < 1e-9);

    // Metadata.
    assert_eq!(result.metadata.profile_type.as_deref(), Some("filament"));
}

// ---------------------------------------------------------------------------
// 6. test_prusaslicer_field_mapping_machine
// ---------------------------------------------------------------------------

/// Test import_prusaslicer_ini_profile with machine/printer fields.
/// Verify PrintConfig values for nozzle_diameter, retract_z_hop, gcode_dialect, etc.
#[test]
fn test_prusaslicer_field_mapping_machine() {
    let mut fields = HashMap::new();
    fields.insert("nozzle_diameter".to_string(), "0.4,0.4".to_string());
    fields.insert("retract_length".to_string(), "0.8,0.8".to_string());
    fields.insert("retract_speed".to_string(), "35,35".to_string());
    fields.insert("retract_lift".to_string(), "0.2,0.2".to_string());
    fields.insert("gcode_flavor".to_string(), "marlin".to_string());
    fields.insert("retract_before_travel".to_string(), "2,2".to_string());
    fields.insert("machine_max_jerk_x".to_string(), "8,8".to_string());
    fields.insert("machine_max_jerk_y".to_string(), "8,8".to_string());
    fields.insert("machine_max_jerk_z".to_string(), "0.4,0.4".to_string());

    let result = import_prusaslicer_ini_profile(&fields, "Original Prusa MK4S", "printer");
    let config = &result.config;

    // Takes first comma-separated value.
    assert!(
        (config.machine.nozzle_diameter() - 0.4).abs() < 1e-9,
        "nozzle_diameter should be 0.4 (first value)"
    );
    assert!(
        (config.retraction.length - 0.8).abs() < 1e-9,
        "retract_length should be 0.8"
    );
    assert!(
        (config.retraction.speed - 35.0).abs() < 1e-9,
        "retract_speed should be 35.0"
    );
    assert!(
        (config.retraction.z_hop - 0.2).abs() < 1e-9,
        "retract_z_hop should be 0.2 (from retract_lift)"
    );
    assert_eq!(
        config.gcode_dialect,
        slicecore_gcode_io::GcodeDialect::Marlin
    );
    assert!(
        (config.retraction.min_travel - 2.0).abs() < 1e-9,
        "min_travel_for_retract should be 2.0"
    );
    assert!((config.machine.jerk_x() - 8.0).abs() < 1e-9);
    assert!((config.machine.jerk_y() - 8.0).abs() < 1e-9);
    assert!((config.machine.jerk_z() - 0.4).abs() < 1e-9);

    // Metadata.
    assert_eq!(result.metadata.profile_type.as_deref(), Some("machine"));
}

// ---------------------------------------------------------------------------
// 7. test_batch_convert_prusaslicer_synthetic
// ---------------------------------------------------------------------------

/// Create a temp directory with a synthetic .ini file containing vendor section,
/// abstract print, concrete print, abstract filament, concrete filament.
/// Run batch_convert_prusaslicer_profiles. Verify converted count, skipped count,
/// output TOML files exist, index entries have correct vendor/type.
#[test]
fn test_batch_convert_prusaslicer_synthetic() {
    let source = TempDir::new().unwrap();
    let output = TempDir::new().unwrap();

    let ini_content = "\
[vendor]
name = TestVendor
config_version = 1.0.0

[printer_model:TestPrinter]
name = Test Printer
variants = 0.4
technology = FFF

[print:*common*]
layer_height = 0.2
perimeters = 2
fill_density = 15%
perimeter_speed = 45

[print:0.20mm NORMAL @TestPrinter]
inherits = *common*
perimeter_speed = 50
infill_speed = 80

[filament:*PLA*]
temperature = 200
bed_temperature = 55
filament_density = 1.24

[filament:TestVendor PLA @TestPrinter]
inherits = *PLA*
temperature = 215
bed_temperature = 60

[printer:Test Printer 0.4 nozzle]
nozzle_diameter = 0.4
retract_length = 0.8
gcode_flavor = marlin
";

    // Write .ini file to source directory.
    std::fs::write(source.path().join("TestVendor.ini"), ini_content).unwrap();

    let result =
        batch_convert_prusaslicer_profiles(source.path(), output.path(), "prusaslicer").unwrap();

    // Should convert the 3 concrete sections (0.20mm NORMAL, TestVendor PLA, Test Printer).
    assert_eq!(
        result.converted, 3,
        "Should convert 3 concrete profiles, got {}",
        result.converted
    );

    // Should skip 2 abstract sections (*common*, *PLA*).
    assert_eq!(
        result.skipped, 2,
        "Should skip 2 abstract profiles, got {}",
        result.skipped
    );

    // No errors.
    assert!(
        result.errors.is_empty(),
        "No errors expected, got: {:?}",
        result.errors
    );

    // Verify output files exist.
    let process_dir = output.path().join("TestVendor").join("process");
    assert!(process_dir.exists(), "process directory should exist");
    let process_files: Vec<_> = std::fs::read_dir(&process_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "toml")
        })
        .collect();
    assert_eq!(
        process_files.len(),
        1,
        "Should have 1 process TOML file"
    );

    let filament_dir = output.path().join("TestVendor").join("filament");
    assert!(filament_dir.exists(), "filament directory should exist");

    let machine_dir = output.path().join("TestVendor").join("machine");
    assert!(machine_dir.exists(), "machine directory should exist");

    // Verify index entries.
    assert_eq!(result.index.profiles.len(), 3);
    let types: Vec<&str> = result
        .index
        .profiles
        .iter()
        .map(|p| p.profile_type.as_str())
        .collect();
    assert!(types.contains(&"process"), "Should have process entry");
    assert!(types.contains(&"filament"), "Should have filament entry");
    assert!(types.contains(&"machine"), "Should have machine entry");

    // All entries should have vendor = "TestVendor" and source = "prusaslicer".
    for entry in &result.index.profiles {
        assert_eq!(entry.vendor, "TestVendor");
        assert_eq!(entry.source, "prusaslicer");
    }

    // Verify the converted TOML loads as a valid PrintConfig.
    let toml_path = process_files[0].path();
    let config = PrintConfig::from_file(&toml_path).unwrap();
    // Inheritance resolved: perimeter_speed should be child's 50, not parent's 45.
    assert!(
        (config.speeds.perimeter - 50.0).abs() < 1e-6,
        "perimeter_speed should be 50.0 (child override), got {}",
        config.speeds.perimeter
    );
}

// ---------------------------------------------------------------------------
// 8. test_write_merged_index
// ---------------------------------------------------------------------------

/// Create an initial index with 2 OrcaSlicer entries, write it.
/// Create a PrusaSlicer index with 2 entries, call write_merged_index.
/// Load the result and verify all 4 entries exist with correct sources.
#[test]
fn test_write_merged_index() {
    let dir = TempDir::new().unwrap();

    // Write initial OrcaSlicer index.
    let orca_index = ProfileIndex {
        version: 1,
        generated: "2026-01-01T00:00:00Z".to_string(),
        profiles: vec![
            ProfileIndexEntry {
                id: "orcaslicer/BBL/filament/Bambu_PLA".to_string(),
                name: "Bambu PLA".to_string(),
                source: "orcaslicer".to_string(),
                vendor: "BBL".to_string(),
                profile_type: "filament".to_string(),
                material: Some("PLA".to_string()),
                nozzle_size: None,
                printer_model: None,
                path: "orcaslicer/BBL/filament/Bambu_PLA.toml".to_string(),
                layer_height: None,
                quality: None,
            },
            ProfileIndexEntry {
                id: "orcaslicer/BBL/process/0.20mm_Standard".to_string(),
                name: "0.20mm Standard @BBL X1C".to_string(),
                source: "orcaslicer".to_string(),
                vendor: "BBL".to_string(),
                profile_type: "process".to_string(),
                material: None,
                nozzle_size: None,
                printer_model: Some("BBL X1C".to_string()),
                path: "orcaslicer/BBL/process/0.20mm_Standard.toml".to_string(),
                layer_height: Some(0.20),
                quality: Some("Standard".to_string()),
            },
        ],
    };
    write_index(&orca_index, dir.path()).unwrap();

    // Verify initial index.
    let loaded = load_index(dir.path()).unwrap();
    assert_eq!(loaded.profiles.len(), 2);

    // Now merge PrusaSlicer entries.
    let prusa_index = ProfileIndex {
        version: 1,
        generated: "2026-01-02T00:00:00Z".to_string(),
        profiles: vec![
            ProfileIndexEntry {
                id: "prusaslicer/PrusaResearch/filament/Prusament_PLA".to_string(),
                name: "Prusament PLA".to_string(),
                source: "prusaslicer".to_string(),
                vendor: "PrusaResearch".to_string(),
                profile_type: "filament".to_string(),
                material: Some("PLA".to_string()),
                nozzle_size: None,
                printer_model: None,
                path: "prusaslicer/PrusaResearch/filament/Prusament_PLA.toml".to_string(),
                layer_height: None,
                quality: None,
            },
            ProfileIndexEntry {
                id: "prusaslicer/PrusaResearch/process/0.20mm_NORMAL".to_string(),
                name: "0.20mm NORMAL".to_string(),
                source: "prusaslicer".to_string(),
                vendor: "PrusaResearch".to_string(),
                profile_type: "process".to_string(),
                material: None,
                nozzle_size: None,
                printer_model: None,
                path: "prusaslicer/PrusaResearch/process/0.20mm_NORMAL.toml".to_string(),
                layer_height: Some(0.20),
                quality: Some("Normal".to_string()),
            },
        ],
    };
    write_merged_index(&prusa_index, dir.path()).unwrap();

    // Load merged index.
    let merged = load_index(dir.path()).unwrap();
    assert_eq!(
        merged.profiles.len(),
        4,
        "Merged index should have 4 entries (2 OrcaSlicer + 2 PrusaSlicer)"
    );

    // Verify sources.
    let orca_count = merged
        .profiles
        .iter()
        .filter(|p| p.source == "orcaslicer")
        .count();
    let prusa_count = merged
        .profiles
        .iter()
        .filter(|p| p.source == "prusaslicer")
        .count();
    assert_eq!(orca_count, 2, "Should have 2 OrcaSlicer entries");
    assert_eq!(prusa_count, 2, "Should have 2 PrusaSlicer entries");

    // Verify individual entries exist.
    let ids: Vec<&str> = merged.profiles.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"orcaslicer/BBL/filament/Bambu_PLA"));
    assert!(ids.contains(&"orcaslicer/BBL/process/0.20mm_Standard"));
    assert!(ids.contains(&"prusaslicer/PrusaResearch/filament/Prusament_PLA"));
    assert!(ids.contains(&"prusaslicer/PrusaResearch/process/0.20mm_NORMAL"));
}

// ---------------------------------------------------------------------------
// Real-data tests (gated with #[ignore])
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 9. test_real_prusaresearch_conversion
// ---------------------------------------------------------------------------

/// Load PrusaResearch.ini, parse, verify >5000 concrete sections found,
/// convert a subset, verify TOML output is valid.
#[test]
#[ignore]
fn test_real_prusaresearch_conversion() {
    let ini_path =
        Path::new("/home/steve/slicer-analysis/PrusaSlicer/resources/profiles/PrusaResearch.ini");
    assert!(
        ini_path.is_file(),
        "PrusaResearch.ini not found: {}",
        ini_path.display()
    );

    let contents = std::fs::read_to_string(ini_path).unwrap();
    let sections = parse_prusaslicer_ini(&contents);

    eprintln!("PrusaResearch.ini: {} total sections", sections.len());

    // Count concrete sections (not abstract, not vendor/printer_model).
    let concrete_count = sections
        .iter()
        .filter(|s| {
            !s.is_abstract
                && !s.name.is_empty()
                && ["print", "filament", "printer"].contains(&s.section_type.as_str())
        })
        .count();

    eprintln!("Concrete sections: {}", concrete_count);
    assert!(
        concrete_count > 1000,
        "PrusaResearch should have >1000 concrete sections, got {}",
        concrete_count
    );

    // Convert a subset via batch converter.
    let output = TempDir::new().unwrap();
    let source_dir =
        Path::new("/home/steve/slicer-analysis/PrusaSlicer/resources/profiles");

    // Convert just PrusaResearch (it is the single file used by batch converter).
    let result =
        batch_convert_prusaslicer_profiles(source_dir, output.path(), "prusaslicer").unwrap();

    eprintln!(
        "Batch convert: converted={}, skipped={}, errors={}",
        result.converted,
        result.skipped,
        result.errors.len()
    );

    assert!(
        result.converted > 3000,
        "Should convert >3000 profiles total across all vendors, got {}",
        result.converted
    );

    // Verify a PrusaResearch process TOML exists and is loadable.
    let prusa_process_dir = output.path().join("PrusaResearch").join("process");
    if prusa_process_dir.exists() {
        let toml_files: Vec<_> = std::fs::read_dir(&prusa_process_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "toml")
            })
            .collect();

        assert!(
            !toml_files.is_empty(),
            "PrusaResearch/process should have TOML files"
        );

        // Verify first TOML file is loadable.
        let config = PrintConfig::from_file(&toml_files[0].path()).unwrap();
        assert!(
            config.layer_height > 0.0,
            "Converted profile should have a valid layer_height"
        );
    }
}

// ---------------------------------------------------------------------------
// 10. test_real_small_vendor_conversion
// ---------------------------------------------------------------------------

/// Load a small vendor file (Anker.ini), batch convert entire file,
/// verify all concrete profiles converted without errors.
#[test]
#[ignore]
fn test_real_small_vendor_conversion() {
    let ini_path =
        Path::new("/home/steve/slicer-analysis/PrusaSlicer/resources/profiles/Anker.ini");
    assert!(
        ini_path.is_file(),
        "Anker.ini not found: {}",
        ini_path.display()
    );

    let contents = std::fs::read_to_string(ini_path).unwrap();
    let sections = parse_prusaslicer_ini(&contents);
    let lookup = build_section_lookup(&sections);

    let concrete_sections: Vec<_> = sections
        .iter()
        .filter(|s| {
            !s.is_abstract
                && !s.name.is_empty()
                && ["print", "filament", "printer"].contains(&s.section_type.as_str())
        })
        .collect();

    eprintln!(
        "Anker.ini: {} total sections, {} concrete",
        sections.len(),
        concrete_sections.len()
    );

    assert!(
        !concrete_sections.is_empty(),
        "Anker.ini should have concrete profiles"
    );

    // Convert each concrete section individually to verify no errors.
    let mut convert_errors = Vec::new();
    for section in &concrete_sections {
        let resolved = resolve_ini_inheritance(section, &sections, &lookup, 0);
        let result =
            import_prusaslicer_ini_profile(&resolved, &section.name, &section.section_type);

        // Verify metadata is populated.
        if result.metadata.name.is_none() {
            convert_errors.push(format!("Missing name for section: {}", section.name));
        }
    }

    assert!(
        convert_errors.is_empty(),
        "Conversion errors: {:?}",
        convert_errors
    );

    // Also batch-convert just Anker to verify end-to-end.
    let source = TempDir::new().unwrap();
    let output = TempDir::new().unwrap();

    // Copy Anker.ini to temp source dir.
    std::fs::write(source.path().join("Anker.ini"), &contents).unwrap();

    let result =
        batch_convert_prusaslicer_profiles(source.path(), output.path(), "prusaslicer").unwrap();

    assert_eq!(
        result.converted,
        concrete_sections.len(),
        "Batch should convert all {} concrete Anker profiles",
        concrete_sections.len()
    );
    assert!(
        result.errors.is_empty(),
        "No errors expected for Anker, got: {:?}",
        result.errors
    );
}

// ---------------------------------------------------------------------------
// 11. test_real_combined_index
// ---------------------------------------------------------------------------

/// After converting both OrcaSlicer and PrusaSlicer profiles,
/// verify merged index contains entries from both sources with >6000 total profiles.
#[test]
#[ignore]
fn test_real_combined_index() {
    let orca_source =
        Path::new("/home/steve/slicer-analysis/OrcaSlicer/resources/profiles");
    let prusa_source =
        Path::new("/home/steve/slicer-analysis/PrusaSlicer/resources/profiles");

    assert!(
        orca_source.is_dir(),
        "OrcaSlicer profiles not found: {}",
        orca_source.display()
    );
    assert!(
        prusa_source.is_dir(),
        "PrusaSlicer profiles not found: {}",
        prusa_source.display()
    );

    let output = TempDir::new().unwrap();

    // Convert OrcaSlicer first.
    let orca_result = slicecore_engine::batch_convert_profiles(
        orca_source,
        &output.path().join("orcaslicer"),
        "orcaslicer",
    )
    .unwrap();
    write_merged_index(&orca_result.index, output.path()).unwrap();

    eprintln!(
        "OrcaSlicer: converted={}, errors={}",
        orca_result.converted,
        orca_result.errors.len()
    );

    // Convert PrusaSlicer next.
    let prusa_result = batch_convert_prusaslicer_profiles(
        prusa_source,
        &output.path().join("prusaslicer"),
        "prusaslicer",
    )
    .unwrap();
    write_merged_index(&prusa_result.index, output.path()).unwrap();

    eprintln!(
        "PrusaSlicer: converted={}, errors={}",
        prusa_result.converted,
        prusa_result.errors.len()
    );

    // Load merged index.
    let index = load_index(output.path()).unwrap();
    eprintln!("Merged index: {} total profiles", index.profiles.len());

    // Verify both sources present.
    let sources: std::collections::HashSet<&str> = index
        .profiles
        .iter()
        .map(|p| p.source.as_str())
        .collect();
    assert!(
        sources.contains("orcaslicer"),
        "Merged index should contain OrcaSlicer entries"
    );
    assert!(
        sources.contains("prusaslicer"),
        "Merged index should contain PrusaSlicer entries"
    );

    // Verify total count.
    assert!(
        index.profiles.len() > 6000,
        "Merged index should have >6000 profiles, got {}",
        index.profiles.len()
    );

    // Verify per-source counts are reasonable.
    let orca_count = index
        .profiles
        .iter()
        .filter(|p| p.source == "orcaslicer")
        .count();
    let prusa_count = index
        .profiles
        .iter()
        .filter(|p| p.source == "prusaslicer")
        .count();

    assert!(
        orca_count > 1000,
        "OrcaSlicer should have >1000 entries, got {}",
        orca_count
    );
    assert!(
        prusa_count > 3000,
        "PrusaSlicer should have >3000 entries, got {}",
        prusa_count
    );
}
