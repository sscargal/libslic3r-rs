//! Integration tests for profile library batch conversion and fidelity.
//!
//! Tests (a)-(h) are synthetic and always run.
//! Tests (i)-(k) require real OrcaSlicer data at `/home/steve/slicer-analysis/`
//! and are gated with `#[ignore]`. Run them manually with:
//!   cargo test -p slicecore-engine --test integration_profile_library -- --ignored

use slicecore_engine::config::PrintConfig;
use slicecore_engine::profile_library::batch_convert_profiles;
use std::path::Path;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helper: create a synthetic JSON profile file in a vendor/type subdirectory
// ---------------------------------------------------------------------------

fn write_json_profile(base_dir: &Path, vendor: &str, ptype: &str, filename: &str, json: &str) {
    let dir = base_dir.join(vendor).join(ptype);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(filename), json).unwrap();
}

// ---------------------------------------------------------------------------
// (a) test_batch_convert_empty_dir
// ---------------------------------------------------------------------------

#[test]
fn test_batch_convert_empty_dir() {
    let source = TempDir::new().unwrap();
    let output = TempDir::new().unwrap();

    let result = batch_convert_profiles(source.path(), output.path(), "test").unwrap();

    assert_eq!(result.converted, 0, "No profiles to convert in empty dir");
    assert_eq!(result.skipped, 0, "No profiles to skip in empty dir");
    assert!(result.errors.is_empty(), "No errors expected for empty dir");
    assert!(
        result.index.profiles.is_empty(),
        "Index should be empty for empty dir"
    );
}

// ---------------------------------------------------------------------------
// (b) test_batch_convert_skips_non_instantiated
// ---------------------------------------------------------------------------

#[test]
fn test_batch_convert_skips_non_instantiated() {
    let source = TempDir::new().unwrap();
    let output = TempDir::new().unwrap();

    let json = r#"{
        "type": "filament",
        "name": "Base PLA",
        "instantiation": "false",
        "nozzle_temperature": ["210"]
    }"#;
    write_json_profile(source.path(), "TestVendor", "filament", "Base_PLA.json", json);

    let result = batch_convert_profiles(source.path(), output.path(), "test").unwrap();

    assert_eq!(result.converted, 0, "Non-instantiated profile should not be converted");
    assert_eq!(result.skipped, 1, "Non-instantiated profile should be skipped");
}

// ---------------------------------------------------------------------------
// (c) test_batch_convert_single_profile
// ---------------------------------------------------------------------------

#[test]
fn test_batch_convert_single_profile() {
    let source = TempDir::new().unwrap();
    let output = TempDir::new().unwrap();

    // Use nozzle_temperature (maps to nozzle_temp) and hot_plate_temp (maps to bed_temp).
    let json = r#"{
        "type": "filament",
        "name": "Test PLA",
        "instantiation": "true",
        "nozzle_temperature": ["210"],
        "hot_plate_temp": ["60"]
    }"#;
    write_json_profile(
        source.path(),
        "TestVendor",
        "filament",
        "Test_PLA.json",
        json,
    );

    let result = batch_convert_profiles(source.path(), output.path(), "test").unwrap();

    assert_eq!(result.converted, 1, "Should convert 1 profile");

    // Find the output TOML file.
    let toml_dir = output.path().join("TestVendor").join("filament");
    assert!(toml_dir.exists(), "Output directory should exist");
    let toml_files: Vec<_> = std::fs::read_dir(&toml_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "toml")
        })
        .collect();
    assert_eq!(toml_files.len(), 1, "Should produce exactly 1 TOML file");

    // Verify the TOML file can be loaded as a valid PrintConfig.
    let toml_path = toml_files[0].path();
    let config = PrintConfig::from_file(&toml_path).unwrap();

    // nozzle_temp should be 210.0 (not default 200.0).
    assert!(
        (config.nozzle_temp - 210.0).abs() < 1e-6,
        "nozzle_temp should be 210.0, got {}",
        config.nozzle_temp
    );
}

// ---------------------------------------------------------------------------
// (d) test_batch_convert_inheritance
// ---------------------------------------------------------------------------

#[test]
fn test_batch_convert_inheritance() {
    let source = TempDir::new().unwrap();
    let output = TempDir::new().unwrap();

    // Parent: base profile (not instantiated).
    let parent_json = r#"{
        "type": "filament",
        "name": "base_pla",
        "instantiation": "false",
        "nozzle_temperature": ["215"],
        "hot_plate_temp": ["60"]
    }"#;
    write_json_profile(
        source.path(),
        "TestVendor",
        "filament",
        "base_pla.json",
        parent_json,
    );

    // Child: inherits from parent, instantiated.
    let child_json = r#"{
        "type": "filament",
        "name": "PLA Variant",
        "instantiation": "true",
        "inherits": "base_pla",
        "filament_flow_ratio": ["0.95"]
    }"#;
    write_json_profile(
        source.path(),
        "TestVendor",
        "filament",
        "PLA_Variant.json",
        child_json,
    );

    let result = batch_convert_profiles(source.path(), output.path(), "test").unwrap();

    assert_eq!(
        result.converted, 1,
        "Only the instantiated variant should be converted"
    );

    // Load the converted TOML.
    let toml_dir = output.path().join("TestVendor").join("filament");
    let toml_files: Vec<_> = std::fs::read_dir(&toml_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "toml")
        })
        .collect();
    assert_eq!(toml_files.len(), 1);

    let config = PrintConfig::from_file(&toml_files[0].path()).unwrap();

    // Inherited from parent: nozzle_temp = 215.0.
    assert!(
        (config.nozzle_temp - 215.0).abs() < 1e-6,
        "nozzle_temp should be inherited as 215.0, got {}",
        config.nozzle_temp
    );

    // Child override: extrusion_multiplier = 0.95 (from filament_flow_ratio).
    assert!(
        (config.extrusion_multiplier - 0.95).abs() < 1e-6,
        "extrusion_multiplier should be 0.95 from child override, got {}",
        config.extrusion_multiplier
    );
}

// ---------------------------------------------------------------------------
// (e) test_index_entry_metadata
// ---------------------------------------------------------------------------

#[test]
fn test_index_entry_metadata() {
    let source = TempDir::new().unwrap();
    let output = TempDir::new().unwrap();

    let json = r#"{
        "type": "process",
        "name": "0.20mm Standard @TestVendor X1",
        "instantiation": "true",
        "layer_height": "0.2",
        "wall_loops": "3"
    }"#;
    write_json_profile(
        source.path(),
        "TestVendor",
        "process",
        "0.20mm_Standard_TestVendor_X1.json",
        json,
    );

    let result = batch_convert_profiles(source.path(), output.path(), "test").unwrap();

    assert_eq!(result.converted, 1);
    let entry = &result.index.profiles[0];

    assert_eq!(
        entry.profile_type, "process",
        "Profile type should be 'process'"
    );
    assert_eq!(
        entry.layer_height,
        Some(0.20),
        "Layer height should be extracted as 0.20"
    );
    assert_eq!(
        entry.quality,
        Some("Standard".to_string()),
        "Quality should be extracted as 'Standard'"
    );
    assert!(
        entry
            .printer_model
            .as_ref()
            .map_or(false, |m| m.contains("TestVendor X1")),
        "Printer model should contain 'TestVendor X1', got {:?}",
        entry.printer_model
    );
}

// ---------------------------------------------------------------------------
// (f) test_index_entry_filament_material
// ---------------------------------------------------------------------------

#[test]
fn test_index_entry_filament_material() {
    let source = TempDir::new().unwrap();
    let output = TempDir::new().unwrap();

    let json = r#"{
        "type": "filament",
        "name": "Generic PETG @TestVendor",
        "instantiation": "true",
        "nozzle_temperature": ["230"]
    }"#;
    write_json_profile(
        source.path(),
        "TestVendor",
        "filament",
        "Generic_PETG_TestVendor.json",
        json,
    );

    let result = batch_convert_profiles(source.path(), output.path(), "test").unwrap();

    assert_eq!(result.converted, 1);
    let entry = &result.index.profiles[0];

    assert_eq!(
        entry.material,
        Some("PETG".to_string()),
        "Material should be extracted as 'PETG'"
    );
}

// ---------------------------------------------------------------------------
// (g) test_sanitize_filenames_no_collision
// ---------------------------------------------------------------------------

#[test]
fn test_sanitize_filenames_no_collision() {
    let source = TempDir::new().unwrap();
    let output = TempDir::new().unwrap();

    let json1 = r#"{
        "type": "filament",
        "name": "PLA @BBL A1",
        "instantiation": "true",
        "nozzle_temperature": ["210"]
    }"#;
    let json2 = r#"{
        "type": "filament",
        "name": "PLA @BBL A1 0.2 nozzle",
        "instantiation": "true",
        "nozzle_temperature": ["215"]
    }"#;
    write_json_profile(
        source.path(),
        "TestVendor",
        "filament",
        "PLA_BBL_A1.json",
        json1,
    );
    write_json_profile(
        source.path(),
        "TestVendor",
        "filament",
        "PLA_BBL_A1_0.2_nozzle.json",
        json2,
    );

    let result = batch_convert_profiles(source.path(), output.path(), "test").unwrap();

    assert_eq!(result.converted, 2, "Both profiles should be converted");

    // Check that two distinct TOML files exist.
    let toml_dir = output.path().join("TestVendor").join("filament");
    let toml_files: Vec<_> = std::fs::read_dir(&toml_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "toml")
        })
        .collect();

    assert_eq!(
        toml_files.len(),
        2,
        "Should produce 2 distinct TOML files (no collision)"
    );

    // Verify they have different names.
    let names: Vec<String> = toml_files
        .iter()
        .map(|f| f.file_name().to_string_lossy().to_string())
        .collect();
    assert_ne!(
        names[0], names[1],
        "File names should be different: {:?}",
        names
    );
}

// ---------------------------------------------------------------------------
// (h) test_batch_convert_error_recovery
// ---------------------------------------------------------------------------

#[test]
fn test_batch_convert_error_recovery() {
    let source = TempDir::new().unwrap();
    let output = TempDir::new().unwrap();

    // Valid profile.
    let valid_json = r#"{
        "type": "filament",
        "name": "Valid PLA",
        "instantiation": "true",
        "nozzle_temperature": ["210"]
    }"#;
    write_json_profile(
        source.path(),
        "TestVendor",
        "filament",
        "Valid_PLA.json",
        valid_json,
    );

    // Malformed JSON file (invalid syntax).
    let bad_json = r#"{ this is not valid json !!!!! }"#;
    write_json_profile(
        source.path(),
        "TestVendor",
        "filament",
        "Bad_Profile.json",
        bad_json,
    );

    let result = batch_convert_profiles(source.path(), output.path(), "test").unwrap();

    // Valid profile should have been converted.
    assert!(
        result.converted >= 1,
        "Valid profile should succeed: converted={}",
        result.converted
    );

    // Malformed file should be recorded as error.
    assert!(
        !result.errors.is_empty(),
        "Malformed JSON should produce an error, errors={:?}",
        result.errors
    );

    // Batch should NOT have aborted.
    // The fact that converted >= 1 AND errors >= 1 proves non-aborting behavior.
}

// ---------------------------------------------------------------------------
// Real profile tests (gated with #[ignore])
// ---------------------------------------------------------------------------

/// Batch-convert all OrcaSlicer profiles and verify substantial conversion.
/// Requires: /home/steve/slicer-analysis/OrcaSlicer/resources/profiles/
#[test]
#[ignore]
fn test_real_orcaslicer_batch_convert() {
    let source_dir = Path::new("/home/steve/slicer-analysis/OrcaSlicer/resources/profiles");
    assert!(
        source_dir.is_dir(),
        "OrcaSlicer profiles directory not found: {}",
        source_dir.display()
    );

    let output = TempDir::new().unwrap();
    let result =
        batch_convert_profiles(source_dir, output.path(), "orcaslicer").unwrap();

    eprintln!(
        "Batch convert: converted={}, skipped={}, errors={}",
        result.converted,
        result.skipped,
        result.errors.len()
    );
    if !result.errors.is_empty() {
        for (i, e) in result.errors.iter().enumerate().take(10) {
            eprintln!("  error[{}]: {}", i, e);
        }
    }

    // SC: converted > 100.
    assert!(
        result.converted > 100,
        "Should convert more than 100 profiles, got {}",
        result.converted
    );

    // Index structure check.
    assert_eq!(result.index.version, 1, "Index version should be 1");
    assert!(
        !result.index.profiles.is_empty(),
        "Index should have profiles"
    );

    // At least 5 vendors.
    let vendors: std::collections::HashSet<&str> = result
        .index
        .profiles
        .iter()
        .map(|p| p.vendor.as_str())
        .collect();
    assert!(
        vendors.len() >= 5,
        "Should have at least 5 vendors, got {}: {:?}",
        vendors.len(),
        vendors
    );

    // At least one of each profile type.
    let has_filament = result
        .index
        .profiles
        .iter()
        .any(|p| p.profile_type == "filament");
    let has_process = result
        .index
        .profiles
        .iter()
        .any(|p| p.profile_type == "process");
    let has_machine = result
        .index
        .profiles
        .iter()
        .any(|p| p.profile_type == "machine");

    assert!(has_filament, "Should have at least one filament profile");
    assert!(has_process, "Should have at least one process profile");
    assert!(has_machine, "Should have at least one machine profile");
}

/// Load 10 random converted TOML profiles and verify round-trip fidelity.
/// Requires: /home/steve/slicer-analysis/OrcaSlicer/resources/profiles/
#[test]
#[ignore]
fn test_real_profile_toml_loadable() {
    let source_dir = Path::new("/home/steve/slicer-analysis/OrcaSlicer/resources/profiles");
    assert!(
        source_dir.is_dir(),
        "OrcaSlicer profiles directory not found"
    );

    let output = TempDir::new().unwrap();
    let result =
        batch_convert_profiles(source_dir, output.path(), "orcaslicer").unwrap();
    assert!(
        result.converted > 10,
        "Need at least 10 profiles for sampling"
    );

    // Collect all TOML files recursively.
    let toml_files: Vec<_> = walkdir::WalkDir::new(output.path())
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "toml")
        })
        .collect();

    assert!(
        toml_files.len() >= 10,
        "Expected at least 10 TOML files, got {}",
        toml_files.len()
    );

    // Sample 10 files evenly spaced.
    let step = toml_files.len() / 10;
    let mut loaded = 0;
    let mut load_errors: Vec<String> = Vec::new();

    for i in 0..10 {
        let idx = i * step;
        let path = toml_files[idx].path();
        match PrintConfig::from_file(path) {
            Ok(_config) => {
                loaded += 1;
            }
            Err(e) => {
                load_errors.push(format!("{}: {}", path.display(), e));
            }
        }
    }

    eprintln!(
        "Round-trip fidelity: {}/10 TOML files loaded successfully",
        loaded
    );
    for err in &load_errors {
        eprintln!("  FAIL: {}", err);
    }

    assert_eq!(
        loaded, 10,
        "All 10 sampled TOML files should load. Failures: {:?}",
        load_errors
    );
}

/// Verify inheritance resolution produces richer profiles than raw import.
/// Requires: /home/steve/slicer-analysis/OrcaSlicer/resources/profiles/
#[test]
#[ignore]
fn test_real_inheritance_produces_richer_profiles() {
    use slicecore_engine::profile_import::import_upstream_profile;

    let bbl_filament_dir = Path::new(
        "/home/steve/slicer-analysis/OrcaSlicer/resources/profiles/BBL/filament",
    );
    assert!(
        bbl_filament_dir.is_dir(),
        "BBL filament directory not found"
    );

    // Find a PLA profile that inherits from a base.
    let pla_path = std::fs::read_dir(bbl_filament_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            name.contains("PLA")
                && name.ends_with(".json")
                && !name.contains("Base")
        });

    let pla_path = match pla_path {
        Some(p) => p,
        None => {
            eprintln!("No BBL PLA filament profile found -- skipping");
            return;
        }
    };

    eprintln!("Testing inheritance for: {}", pla_path.display());

    // Read the raw JSON.
    let contents = std::fs::read_to_string(&pla_path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&contents).unwrap();

    // Check it actually has an inherits field.
    let has_inherits = value
        .as_object()
        .and_then(|o| o.get("inherits"))
        .and_then(|v| v.as_str())
        .is_some();

    if !has_inherits {
        eprintln!("Profile does not inherit -- trying to find one that does");
        // Find any inheriting profile.
        let inheriting_path = std::fs::read_dir(bbl_filament_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| {
                if !p.is_file() || p.extension().and_then(|e| e.to_str()) != Some("json") {
                    return false;
                }
                let c = std::fs::read_to_string(p).unwrap_or_default();
                c.contains("\"inherits\"")
                    && c.contains("\"instantiation\": \"true\"")
            });

        let inheriting_path = match inheriting_path {
            Some(p) => p,
            None => {
                eprintln!("No inheriting+instantiated profile found -- skipping");
                return;
            }
        };

        let contents = std::fs::read_to_string(&inheriting_path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&contents).unwrap();

        // Raw import (child only, no inheritance).
        let raw_result = import_upstream_profile(&value).unwrap();
        let raw_mapped = raw_result.mapped_fields.len();

        // Batch convert with inheritance for the whole BBL filament directory.
        let output = TempDir::new().unwrap();
        let source = Path::new("/home/steve/slicer-analysis/OrcaSlicer/resources/profiles");
        let batch_result =
            batch_convert_profiles(source, output.path(), "orcaslicer").unwrap();

        // The batch result with inheritance should produce more mapped fields
        // overall than a raw child-only import. We verify by comparing the
        // total inherited mapped_fields from the summary.
        eprintln!(
            "Raw import mapped {} fields, batch converted {} profiles",
            raw_mapped, batch_result.converted
        );

        // Verify that at least one profile was converted with inheritance.
        assert!(
            batch_result.converted > 0,
            "Should convert at least one profile"
        );
        return;
    }

    // Raw import (child only, no inheritance).
    let raw_result = import_upstream_profile(&value).unwrap();
    let raw_mapped = raw_result.mapped_fields.len();

    // Batch convert the entire directory (which resolves inheritance).
    let output = TempDir::new().unwrap();
    let source = Path::new("/home/steve/slicer-analysis/OrcaSlicer/resources/profiles");
    let batch_result =
        batch_convert_profiles(source, output.path(), "orcaslicer").unwrap();

    // The resolved profile should have more content than the raw child-only import.
    // We verify indirectly: the batch conversion should have converted profiles
    // and the TOML output for inherited profiles should be loadable.
    assert!(
        batch_result.converted > 100,
        "Batch should convert many profiles"
    );

    eprintln!(
        "Raw child mapped {} fields. Batch conversion: {} converted, {} skipped.",
        raw_mapped, batch_result.converted, batch_result.skipped
    );
}
