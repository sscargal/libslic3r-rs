//! Integration tests for BambuStudio batch conversion pipeline.
//!
//! Tests 1-3 are synthetic and always run.
//! Tests 4-6 require real BambuStudio data at `/home/steve/slicer-analysis/`
//! and are gated with `#[ignore]`. Run them manually with:
//!   cargo test -p slicecore-engine --test integration_profile_library_bambu -- --ignored

use std::path::Path;

use slicecore_engine::config::PrintConfig;
use slicecore_engine::{
    batch_convert_profiles, load_index, write_index, write_merged_index, ProfileIndex,
    ProfileIndexEntry,
};

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
// 1. test_bambustudio_batch_convert_synthetic
// ---------------------------------------------------------------------------

/// Create a temp directory mimicking BambuStudio's vendor/type structure with
/// filament, machine, and process profiles. Verify batch conversion counts,
/// output files, and round-trip TOML loading.
#[test]
fn test_bambustudio_batch_convert_synthetic() {
    let source = TempDir::new().unwrap();
    let output = TempDir::new().unwrap();

    // Instantiated filament profile.
    let filament_json = r#"{
        "type": "filament",
        "name": "Bambu PLA Basic",
        "instantiation": "true",
        "nozzle_temperature": ["220"],
        "hot_plate_temp": ["60"],
        "filament_type": ["PLA"]
    }"#;
    write_json_profile(
        source.path(),
        "BBL",
        "filament",
        "Bambu_PLA_Basic.json",
        filament_json,
    );

    // Non-instantiated base filament (should be skipped).
    let base_json = r#"{
        "type": "filament",
        "name": "Bambu PLA Base",
        "instantiation": "false",
        "nozzle_temperature": ["210"],
        "hot_plate_temp": ["55"]
    }"#;
    write_json_profile(
        source.path(),
        "BBL",
        "filament",
        "Bambu_PLA_Base.json",
        base_json,
    );

    // Instantiated machine profile.
    let machine_json = r#"{
        "type": "machine",
        "name": "Bambu Lab X1C",
        "instantiation": "true",
        "nozzle_diameter": ["0.4"],
        "machine_max_speed_x": ["500"]
    }"#;
    write_json_profile(
        source.path(),
        "BBL",
        "machine",
        "Bambu_Lab_X1C.json",
        machine_json,
    );

    // Instantiated process profile.
    let process_json = r#"{
        "type": "process",
        "name": "0.20mm Standard @BBL X1C",
        "instantiation": "true",
        "layer_height": "0.2",
        "wall_loops": "3",
        "sparse_infill_density": "15%"
    }"#;
    write_json_profile(
        source.path(),
        "BBL",
        "process",
        "0.20mm_Standard_BBL_X1C.json",
        process_json,
    );

    let result = batch_convert_profiles(source.path(), output.path(), "bambustudio").unwrap();

    // Verify conversion counts.
    assert_eq!(
        result.converted, 3,
        "Should convert 3 instantiated profiles, got {}",
        result.converted
    );
    assert_eq!(
        result.skipped, 1,
        "Should skip 1 base profile, got {}",
        result.skipped
    );
    assert!(
        result.errors.is_empty(),
        "No errors expected, got: {:?}",
        result.errors
    );

    // Verify output directory structure.
    let filament_dir = output.path().join("BBL").join("filament");
    assert!(filament_dir.exists(), "BBL/filament directory should exist");

    let machine_dir = output.path().join("BBL").join("machine");
    assert!(machine_dir.exists(), "BBL/machine directory should exist");

    let process_dir = output.path().join("BBL").join("process");
    assert!(process_dir.exists(), "BBL/process directory should exist");

    // Verify TOML files exist.
    let filament_files: Vec<_> = std::fs::read_dir(&filament_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
        .collect();
    assert_eq!(
        filament_files.len(),
        1,
        "Should have 1 filament TOML (base skipped), got {}",
        filament_files.len()
    );

    // Verify each TOML file loads into PrintConfig without error.
    let config = PrintConfig::from_file(&filament_files[0].path()).unwrap();
    assert!(
        (config.filament.nozzle_temp() - 220.0).abs() < 1e-6,
        "nozzle_temp should be 220.0, got {}",
        config.filament.nozzle_temp()
    );
    assert!(
        (config.filament.bed_temp() - 60.0).abs() < 1e-6,
        "bed_temp should be 60.0, got {}",
        config.filament.bed_temp()
    );

    // Verify machine TOML loads.
    let machine_files: Vec<_> = std::fs::read_dir(&machine_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
        .collect();
    assert_eq!(machine_files.len(), 1);
    let machine_config = PrintConfig::from_file(&machine_files[0].path()).unwrap();
    assert!(
        (machine_config.machine.nozzle_diameter() - 0.4).abs() < 1e-6,
        "nozzle_diameter should be 0.4, got {}",
        machine_config.machine.nozzle_diameter()
    );

    // Verify process TOML loads.
    let process_files: Vec<_> = std::fs::read_dir(&process_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
        .collect();
    assert_eq!(process_files.len(), 1);
    let process_config = PrintConfig::from_file(&process_files[0].path()).unwrap();
    assert!(
        (process_config.layer_height - 0.2).abs() < 1e-6,
        "layer_height should be 0.2, got {}",
        process_config.layer_height
    );

    // Verify index entries.
    assert_eq!(result.index.profiles.len(), 3);
    for entry in &result.index.profiles {
        assert_eq!(entry.source, "bambustudio");
        assert_eq!(entry.vendor, "BBL");
    }
}

// ---------------------------------------------------------------------------
// 2. test_bambustudio_three_source_index_merge
// ---------------------------------------------------------------------------

/// Create an initial index with OrcaSlicer and PrusaSlicer entries, then add
/// BambuStudio entries via write_merged_index. Verify all 6 entries exist
/// with correct sources.
#[test]
fn test_bambustudio_three_source_index_merge() {
    let dir = TempDir::new().unwrap();

    // Write initial index with 2 OrcaSlicer entries.
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

    // Merge PrusaSlicer entries.
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

    // Merge BambuStudio entries.
    let bambu_index = ProfileIndex {
        version: 1,
        generated: "2026-01-03T00:00:00Z".to_string(),
        profiles: vec![
            ProfileIndexEntry {
                id: "bambustudio/BBL/filament/Bambu_ABS_BBL_X1C".to_string(),
                name: "Bambu ABS @BBL X1C".to_string(),
                source: "bambustudio".to_string(),
                vendor: "BBL".to_string(),
                profile_type: "filament".to_string(),
                material: Some("ABS".to_string()),
                nozzle_size: None,
                printer_model: Some("BBL X1C".to_string()),
                path: "bambustudio/BBL/filament/Bambu_ABS_BBL_X1C.toml".to_string(),
                layer_height: None,
                quality: None,
            },
            ProfileIndexEntry {
                id: "bambustudio/BBL/machine/Bambu_Lab_H2C".to_string(),
                name: "Bambu Lab H2C".to_string(),
                source: "bambustudio".to_string(),
                vendor: "BBL".to_string(),
                profile_type: "machine".to_string(),
                material: None,
                nozzle_size: None,
                printer_model: None,
                path: "bambustudio/BBL/machine/Bambu_Lab_H2C.toml".to_string(),
                layer_height: None,
                quality: None,
            },
        ],
    };
    write_merged_index(&bambu_index, dir.path()).unwrap();

    // Load merged index and verify.
    let merged = load_index(dir.path()).unwrap();
    assert_eq!(
        merged.profiles.len(),
        6,
        "Merged index should have 6 entries (2 orca + 2 prusa + 2 bambu), got {}",
        merged.profiles.len()
    );

    // Verify source counts.
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
    let bambu_count = merged
        .profiles
        .iter()
        .filter(|p| p.source == "bambustudio")
        .count();

    assert_eq!(orca_count, 2, "Should have 2 OrcaSlicer entries");
    assert_eq!(prusa_count, 2, "Should have 2 PrusaSlicer entries");
    assert_eq!(bambu_count, 2, "Should have 2 BambuStudio entries");

    // Verify all IDs are present.
    let ids: Vec<&str> = merged.profiles.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"orcaslicer/BBL/filament/Bambu_PLA"));
    assert!(ids.contains(&"orcaslicer/BBL/process/0.20mm_Standard"));
    assert!(ids.contains(&"prusaslicer/PrusaResearch/filament/Prusament_PLA"));
    assert!(ids.contains(&"prusaslicer/PrusaResearch/process/0.20mm_NORMAL"));
    assert!(ids.contains(&"bambustudio/BBL/filament/Bambu_ABS_BBL_X1C"));
    assert!(ids.contains(&"bambustudio/BBL/machine/Bambu_Lab_H2C"));
}

// ---------------------------------------------------------------------------
// 3. test_bambustudio_profile_loads_into_printconfig
// ---------------------------------------------------------------------------

/// Create a BambuStudio filament JSON with known field values.
/// Run batch_convert_profiles. Load the resulting TOML via PrintConfig::from_file.
/// Verify nozzle_temp, bed_temp, and extrusion_multiplier match.
#[test]
fn test_bambustudio_profile_loads_into_printconfig() {
    let source = TempDir::new().unwrap();
    let output = TempDir::new().unwrap();

    let filament_json = r#"{
        "type": "filament",
        "name": "Test Bambu PLA",
        "instantiation": "true",
        "nozzle_temperature": ["210"],
        "hot_plate_temp": ["60"],
        "filament_flow_ratio": ["0.95"],
        "filament_type": ["PLA"]
    }"#;
    write_json_profile(
        source.path(),
        "BBL",
        "filament",
        "Test_Bambu_PLA.json",
        filament_json,
    );

    let result = batch_convert_profiles(source.path(), output.path(), "bambustudio").unwrap();

    assert_eq!(result.converted, 1);
    assert!(result.errors.is_empty());

    // Find and load the converted TOML.
    let toml_dir = output.path().join("BBL").join("filament");
    let toml_files: Vec<_> = std::fs::read_dir(&toml_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
        .collect();
    assert_eq!(toml_files.len(), 1);

    let config = PrintConfig::from_file(&toml_files[0].path()).unwrap();

    // Verify mapped field values.
    assert!(
        (config.filament.nozzle_temp() - 210.0).abs() < 1e-6,
        "nozzle_temp should be 210.0, got {}",
        config.filament.nozzle_temp()
    );
    assert!(
        (config.filament.bed_temp() - 60.0).abs() < 1e-6,
        "bed_temp should be 60.0, got {}",
        config.filament.bed_temp()
    );
    assert!(
        (config.extrusion_multiplier - 0.95).abs() < 1e-6,
        "extrusion_multiplier should be 0.95, got {}",
        config.extrusion_multiplier
    );
}

// ---------------------------------------------------------------------------
// Real-data tests (gated with #[ignore])
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 4. test_real_bambustudio_batch_convert
// ---------------------------------------------------------------------------

/// Run batch_convert_profiles on the actual BambuStudio source directory.
/// Verify converted > 2000, errors < 50, BBL subdirectories exist.
#[test]
#[ignore]
fn test_real_bambustudio_batch_convert() {
    let source_dir = Path::new("/home/steve/slicer-analysis/BambuStudio/resources/profiles");
    assert!(
        source_dir.is_dir(),
        "BambuStudio profiles directory not found: {}",
        source_dir.display()
    );

    let output = TempDir::new().unwrap();
    let result = batch_convert_profiles(source_dir, output.path(), "bambustudio").unwrap();

    eprintln!(
        "BambuStudio batch convert: converted={}, skipped={}, errors={}",
        result.converted,
        result.skipped,
        result.errors.len()
    );
    if !result.errors.is_empty() {
        for (i, e) in result.errors.iter().enumerate().take(10) {
            eprintln!("  error[{}]: {}", i, e);
        }
    }

    assert!(
        result.converted > 2000,
        "Should convert >2000 BambuStudio profiles, got {}",
        result.converted
    );
    assert!(
        result.errors.len() < 50,
        "Should have <50 errors, got {}",
        result.errors.len()
    );

    // Verify BBL vendor directories exist.
    assert!(
        output.path().join("BBL").join("filament").is_dir(),
        "BBL/filament should exist"
    );
    assert!(
        output.path().join("BBL").join("machine").is_dir(),
        "BBL/machine should exist"
    );
    assert!(
        output.path().join("BBL").join("process").is_dir(),
        "BBL/process should exist"
    );

    // Verify at least 12 vendor directories.
    let vendor_count = std::fs::read_dir(output.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .count();
    assert!(
        vendor_count >= 12,
        "Should have at least 12 vendor directories, got {}",
        vendor_count
    );
}

// ---------------------------------------------------------------------------
// 5. test_real_bambustudio_combined_index
// ---------------------------------------------------------------------------

/// After converting all three sources (OrcaSlicer, PrusaSlicer, BambuStudio),
/// verify the merged index contains >15000 total profiles with all three sources.
#[test]
#[ignore]
fn test_real_bambustudio_combined_index() {
    let orca_source = Path::new("/home/steve/slicer-analysis/OrcaSlicer/resources/profiles");
    let prusa_source = Path::new("/home/steve/slicer-analysis/PrusaSlicer/resources/profiles");
    let bambu_source = Path::new("/home/steve/slicer-analysis/BambuStudio/resources/profiles");

    assert!(orca_source.is_dir(), "OrcaSlicer profiles not found");
    assert!(prusa_source.is_dir(), "PrusaSlicer profiles not found");
    assert!(bambu_source.is_dir(), "BambuStudio profiles not found");

    let output = TempDir::new().unwrap();

    // Convert OrcaSlicer.
    let orca_result =
        batch_convert_profiles(orca_source, &output.path().join("orcaslicer"), "orcaslicer")
            .unwrap();
    write_merged_index(&orca_result.index, output.path()).unwrap();

    // Convert PrusaSlicer.
    let prusa_result = slicecore_engine::batch_convert_prusaslicer_profiles(
        prusa_source,
        &output.path().join("prusaslicer"),
        "prusaslicer",
    )
    .unwrap();
    write_merged_index(&prusa_result.index, output.path()).unwrap();

    // Convert BambuStudio.
    let bambu_result = batch_convert_profiles(
        bambu_source,
        &output.path().join("bambustudio"),
        "bambustudio",
    )
    .unwrap();
    write_merged_index(&bambu_result.index, output.path()).unwrap();

    // Load merged index.
    let index = load_index(output.path()).unwrap();
    eprintln!("Combined index: {} total profiles", index.profiles.len());

    // Verify all three sources present.
    let sources: std::collections::HashSet<&str> =
        index.profiles.iter().map(|p| p.source.as_str()).collect();
    assert!(
        sources.contains("orcaslicer"),
        "Should contain orcaslicer entries"
    );
    assert!(
        sources.contains("prusaslicer"),
        "Should contain prusaslicer entries"
    );
    assert!(
        sources.contains("bambustudio"),
        "Should contain bambustudio entries"
    );

    // Verify total count.
    assert!(
        index.profiles.len() > 15000,
        "Combined index should have >15000 profiles, got {}",
        index.profiles.len()
    );

    // Verify per-source counts.
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
    let bambu_count = index
        .profiles
        .iter()
        .filter(|p| p.source == "bambustudio")
        .count();

    eprintln!(
        "Per-source: orcaslicer={}, prusaslicer={}, bambustudio={}",
        orca_count, prusa_count, bambu_count
    );

    assert!(orca_count > 1000, "OrcaSlicer should have >1000 entries");
    assert!(prusa_count > 3000, "PrusaSlicer should have >3000 entries");
    assert!(
        bambu_count > 2000,
        "BambuStudio should have >2000 entries, got {}",
        bambu_count
    );
}

// ---------------------------------------------------------------------------
// 6. test_real_bambustudio_unique_profiles
// ---------------------------------------------------------------------------

/// Verify that BambuStudio-specific profiles exist (H2C, H2S, P2S printer
/// profiles that are not in OrcaSlicer).
#[test]
#[ignore]
fn test_real_bambustudio_unique_profiles() {
    let bambu_source = Path::new("/home/steve/slicer-analysis/BambuStudio/resources/profiles");
    assert!(
        bambu_source.is_dir(),
        "BambuStudio profiles not found: {}",
        bambu_source.display()
    );

    let output = TempDir::new().unwrap();
    let result = batch_convert_profiles(bambu_source, output.path(), "bambustudio").unwrap();

    assert!(
        result.converted > 2000,
        "Should convert >2000 profiles, got {}",
        result.converted
    );

    // Search for H2C profiles in the converted output.
    let h2c_profiles: Vec<_> = result
        .index
        .profiles
        .iter()
        .filter(|p| p.name.contains("H2C"))
        .collect();

    eprintln!("H2C profiles found: {}", h2c_profiles.len());
    assert!(
        !h2c_profiles.is_empty(),
        "Should find H2C profiles unique to BambuStudio"
    );

    // Search for P2S profiles.
    let p2s_profiles: Vec<_> = result
        .index
        .profiles
        .iter()
        .filter(|p| p.name.contains("P2S"))
        .collect();

    eprintln!("P2S profiles found: {}", p2s_profiles.len());
    // P2S may or may not exist depending on BambuStudio version.
    // H2C is the primary unique indicator.

    // Verify the H2C profiles have correct structure.
    for profile in &h2c_profiles {
        assert_eq!(profile.source, "bambustudio");
        assert_eq!(profile.vendor, "BBL");
        assert!(
            profile.name.contains("H2C"),
            "H2C profile should contain 'H2C' in name: {}",
            profile.name
        );
    }

    // Verify converted TOML files for H2C exist on disk.
    let bbl_filament_dir = output.path().join("BBL").join("filament");
    if bbl_filament_dir.exists() {
        let h2c_files: Vec<_> = std::fs::read_dir(&bbl_filament_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("H2C"))
            .collect();

        eprintln!("H2C TOML files in BBL/filament: {}", h2c_files.len());
        assert!(
            !h2c_files.is_empty(),
            "Should find H2C TOML files in BBL/filament"
        );

        // Spot-check: load one H2C profile.
        let first_h2c = &h2c_files[0];
        let config = PrintConfig::from_file(&first_h2c.path()).unwrap();
        assert!(
            config.filament.nozzle_temp() > 0.0,
            "H2C profile should have non-zero nozzle_temp"
        );
    }
}
