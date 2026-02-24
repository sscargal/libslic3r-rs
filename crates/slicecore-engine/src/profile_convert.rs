//! Profile conversion module for transforming imported profiles to native TOML format.
//!
//! This module converts [`ImportResult`] data from the profile import system into
//! clean TOML output containing only non-default fields. It also supports merging
//! multiple import results into a single unified configuration.
//!
//! # Features
//!
//! - **Selective output**: Only fields that differ from [`PrintConfig::default()`] are
//!   included in the TOML, keeping converted profiles minimal and readable.
//! - **Multi-file merge**: Overlay multiple [`ImportResult`] values (e.g., process +
//!   filament + machine profiles) into a single unified config.
//! - **Float precision**: Floating-point values are rounded to 6 decimal places to
//!   avoid IEEE 754 artifacts (e.g., `0.15000000000000002` becomes `0.15`).
//! - **Unmapped field reporting**: Fields from the source profile that have no
//!   [`PrintConfig`] equivalent are listed as comments at the end of the TOML output.
//!
//! # Usage
//!
//! ```ignore
//! use slicecore_engine::profile_import::import_upstream_profile;
//! use slicecore_engine::profile_convert::{convert_to_toml, merge_import_results};
//!
//! let result = import_upstream_profile(&json_value)?;
//! let converted = convert_to_toml(&result);
//! println!("{}", converted.toml_output);
//! ```

use crate::config::PrintConfig;
use crate::profile_import::ImportResult;

/// Result of converting an imported profile to TOML format.
#[derive(Debug, Clone)]
pub struct ConvertResult {
    /// Generated TOML string (with header comments and unmapped field comments).
    pub toml_output: String,
    /// Number of fields mapped from source.
    pub mapped_count: usize,
    /// Fields with no PrintConfig equivalent.
    pub unmapped_fields: Vec<String>,
    /// Profile name from metadata.
    pub source_name: Option<String>,
    /// Profile type from metadata.
    pub source_type: Option<String>,
}

/// Convert an [`ImportResult`] to clean TOML format with only non-default fields.
///
/// Builds a TOML string with:
/// 1. Header comments showing source metadata (name, type, inherits, mapped/unmapped counts).
/// 2. TOML body containing only fields that differ from [`PrintConfig::default()`].
/// 3. Trailing comments listing unmapped fields from the source.
///
/// Float values are rounded to 6 decimal places before comparison and output.
pub fn convert_to_toml(result: &ImportResult) -> ConvertResult {
    let default_config = PrintConfig::default();

    // Serialize both configs to toml::Value::Table for comparison.
    let result_value = toml::Value::try_from(&result.config)
        .expect("PrintConfig should serialize to toml::Value");
    let default_value = toml::Value::try_from(&default_config)
        .expect("PrintConfig default should serialize to toml::Value");

    let result_table = match result_value {
        toml::Value::Table(t) => t,
        _ => unreachable!("PrintConfig serializes to a table"),
    };
    let default_table = match default_value {
        toml::Value::Table(t) => t,
        _ => unreachable!("PrintConfig default serializes to a table"),
    };

    // Filter: keep only keys where the value differs from default.
    let mut filtered = toml::map::Map::new();
    for (key, val) in &result_table {
        if let Some(default_val) = default_table.get(key) {
            let mut val_rounded = val.clone();
            let mut def_rounded = default_val.clone();
            round_floats_in_value(&mut val_rounded);
            round_floats_in_value(&mut def_rounded);
            if val_rounded != def_rounded {
                filtered.insert(key.clone(), val_rounded);
            }
        } else {
            // Key not in default -- include it.
            let mut val_rounded = val.clone();
            round_floats_in_value(&mut val_rounded);
            filtered.insert(key.clone(), val_rounded);
        }
    }

    // Generate TOML body from the filtered table.
    let toml_body = if filtered.is_empty() {
        String::from("# All fields match defaults -- no overrides needed.\n")
    } else {
        toml::to_string_pretty(&toml::Value::Table(filtered))
            .unwrap_or_else(|_| String::from("# Error serializing TOML\n"))
    };

    // Build header comments.
    let mut header = String::new();
    header.push_str("# Converted from upstream slicer profile\n");
    if let Some(ref name) = result.metadata.name {
        header.push_str(&format!("# Source: {}\n", name));
    }
    if let Some(ref ptype) = result.metadata.profile_type {
        header.push_str(&format!("# Type: {}\n", ptype));
    }
    if let Some(ref inherits) = result.metadata.inherits {
        header.push_str(&format!("# Inherits: {}\n", inherits));
    }
    header.push_str(&format!(
        "# Mapped fields: {}\n",
        result.mapped_fields.len()
    ));
    header.push_str(&format!(
        "# Unmapped fields: {}\n",
        result.unmapped_fields.len()
    ));
    header.push('\n');

    // Build unmapped fields section as trailing comments.
    let mut footer = String::new();
    if !result.unmapped_fields.is_empty() {
        footer.push('\n');
        footer.push_str(
            "# Unmapped fields from source (no equivalent in PrintConfig):\n",
        );
        for field in &result.unmapped_fields {
            footer.push_str(&format!("# - {}\n", field));
        }
    }

    let toml_output = format!("{}{}{}", header, toml_body, footer);

    ConvertResult {
        toml_output,
        mapped_count: result.mapped_fields.len(),
        unmapped_fields: result.unmapped_fields.clone(),
        source_name: result.metadata.name.clone(),
        source_type: result.metadata.profile_type.clone(),
    }
}

/// Merge multiple [`ImportResult`] values into a single unified result.
///
/// Starting from [`PrintConfig::default()`], each result is overlaid in order:
/// only fields that differ from the default in each result are applied to the
/// running merged config. Later results override earlier ones for shared fields.
///
/// Metadata is merged: names are joined with " + ", and type/inherits use the
/// last result's values. Mapped and unmapped field lists are deduplicated unions
/// of all input results.
pub fn merge_import_results(results: &[ImportResult]) -> ImportResult {
    if results.is_empty() {
        return ImportResult {
            config: PrintConfig::default(),
            mapped_fields: Vec::new(),
            unmapped_fields: Vec::new(),
            passthrough_fields: Vec::new(),
            metadata: crate::profile_import::ProfileMetadata::default(),
        };
    }

    if results.len() == 1 {
        return results[0].clone();
    }

    let default_config = PrintConfig::default();
    let default_value = toml::Value::try_from(&default_config)
        .expect("PrintConfig default should serialize to toml::Value");
    let default_table = match default_value {
        toml::Value::Table(t) => t,
        _ => unreachable!(),
    };

    // Start with the default table as our merged base.
    let mut merged_table = default_table.clone();

    // Collect all mapped and unmapped fields (deduplicated).
    let mut all_mapped: Vec<String> = Vec::new();
    let mut all_unmapped: Vec<String> = Vec::new();

    for result in results {
        let result_value = toml::Value::try_from(&result.config)
            .expect("PrintConfig should serialize to toml::Value");
        let result_table = match result_value {
            toml::Value::Table(t) => t,
            _ => unreachable!(),
        };

        // Find keys where the result differs from default, and overlay.
        for (key, val) in &result_table {
            if let Some(default_val) = default_table.get(key) {
                let mut val_rounded = val.clone();
                let mut def_rounded = default_val.clone();
                round_floats_in_value(&mut val_rounded);
                round_floats_in_value(&mut def_rounded);
                if val_rounded != def_rounded {
                    merged_table.insert(key.clone(), val_rounded);
                }
            }
        }

        // Collect field names (deduplicated).
        for field in &result.mapped_fields {
            if !all_mapped.contains(field) {
                all_mapped.push(field.clone());
            }
        }
        for field in &result.unmapped_fields {
            if !all_unmapped.contains(field) {
                all_unmapped.push(field.clone());
            }
        }
    }

    // Deserialize the merged table back into PrintConfig.
    let merged_config: PrintConfig =
        toml::Value::Table(merged_table).try_into().unwrap_or_else(|e| {
            eprintln!("Warning: failed to deserialize merged config: {}", e);
            PrintConfig::default()
        });

    // Merge metadata: join names with " + ", use last result for type/inherits.
    let names: Vec<String> = results
        .iter()
        .filter_map(|r| r.metadata.name.clone())
        .collect();
    let merged_name = if names.is_empty() {
        None
    } else {
        Some(names.join(" + "))
    };

    let last = results.last().unwrap();
    let metadata = crate::profile_import::ProfileMetadata {
        name: merged_name,
        profile_type: last.metadata.profile_type.clone(),
        inherits: last.metadata.inherits.clone(),
    };

    // Merge passthrough_fields from all results.
    let mut all_passthrough: Vec<String> = Vec::new();
    for r in results {
        for f in &r.passthrough_fields {
            if !all_passthrough.contains(f) {
                all_passthrough.push(f.clone());
            }
        }
    }

    ImportResult {
        config: merged_config,
        mapped_fields: all_mapped,
        unmapped_fields: all_unmapped,
        passthrough_fields: all_passthrough,
        metadata,
    }
}

/// Recursively round float values in a [`toml::Value`] tree to 6 decimal places.
///
/// This prevents TOML output like `infill_density = 0.15000000000000002` caused
/// by IEEE 754 floating-point representation artifacts.
pub fn round_floats_in_value(value: &mut toml::Value) {
    match value {
        toml::Value::Float(f) => {
            *f = (*f * 1_000_000.0).round() / 1_000_000.0;
        }
        toml::Value::Table(table) => {
            let keys: Vec<String> = table.keys().cloned().collect();
            for key in keys {
                if let Some(val) = table.get_mut(&key) {
                    round_floats_in_value(val);
                }
            }
        }
        toml::Value::Array(arr) => {
            for val in arr.iter_mut() {
                round_floats_in_value(val);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_import::{import_upstream_profile, ProfileMetadata};
    use serde_json::json;

    #[test]
    fn test_convert_basic_process_profile() {
        let json_val = json!({
            "type": "process",
            "name": "0.20mm Standard",
            "layer_height": "0.2",
            "wall_loops": "3",
            "sparse_infill_density": "15%",
            "outer_wall_speed": "200",
            "travel_speed": "500",
            "seam_position": "aligned",
            "unknown_field_1": "value1",
            "unknown_field_2": "value2"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let converted = convert_to_toml(&result);

        // Header comments should be present.
        assert!(converted.toml_output.contains("# Source: 0.20mm Standard"));
        assert!(converted.toml_output.contains("# Type: process"));

        // TOML should contain mapped non-default fields.
        // wall_count = 3 (default is 2, so it should appear).
        assert!(converted.toml_output.contains("wall_count"));
        // perimeter_speed = 200 (default is 45, so it should appear).
        assert!(converted.toml_output.contains("perimeter_speed"));
        // travel_speed = 500 (default is 150, so it should appear).
        assert!(converted.toml_output.contains("travel_speed"));

        // layer_height = 0.2 matches default, so it should NOT appear in the body.
        // Check that it's not in the TOML body (only in comments).
        let body_start = converted.toml_output.find("\n\n").unwrap() + 2;
        let body = &converted.toml_output[body_start..];
        // The body should not have `layer_height = 0.2` as a TOML key.
        assert!(
            !body.contains("layer_height = 0.2"),
            "Default layer_height should be excluded from TOML body"
        );

        // Verify mapped/unmapped counts.
        assert!(converted.mapped_count > 0);
        assert!(!converted.unmapped_fields.is_empty());
    }

    #[test]
    fn test_convert_selective_output() {
        // Create an ImportResult with only 3 non-default fields.
        let mut config = PrintConfig::default();
        config.wall_count = 4;
        config.infill_density = 0.5;
        config.perimeter_speed = 100.0;

        let result = ImportResult {
            config,
            mapped_fields: vec![
                "wall_loops".into(),
                "sparse_infill_density".into(),
                "outer_wall_speed".into(),
            ],
            unmapped_fields: vec![],
            passthrough_fields: vec![],
            metadata: ProfileMetadata {
                name: Some("Test Selective".into()),
                profile_type: Some("process".into()),
                inherits: None,
            },
        };

        let converted = convert_to_toml(&result);

        // Should contain only the 3 overridden fields (not 86 defaults).
        assert!(converted.toml_output.contains("wall_count = 4"));
        assert!(converted.toml_output.contains("infill_density = 0.5"));
        assert!(converted.toml_output.contains("perimeter_speed = 100.0"));

        // Should NOT contain default fields.
        assert!(!converted.toml_output.contains("nozzle_diameter"));
        assert!(!converted.toml_output.contains("retract_length"));
        assert!(!converted.toml_output.contains("bed_temp"));
    }

    #[test]
    fn test_merge_two_profiles() {
        let process_json = json!({
            "type": "process",
            "name": "0.20mm Standard",
            "layer_height": "0.2",
            "wall_loops": "3",
            "sparse_infill_density": "15%"
        });

        let filament_json = json!({
            "type": "filament",
            "name": "Generic PLA",
            "nozzle_temperature": ["220"],
            "hot_plate_temp": ["55"],
            "filament_density": ["1.24"]
        });

        let process_result = import_upstream_profile(&process_json).unwrap();
        let filament_result = import_upstream_profile(&filament_json).unwrap();

        let merged = merge_import_results(&[process_result, filament_result]);

        // Merged config should have fields from both.
        assert_eq!(merged.config.wall_count, 3); // from process
        assert!((merged.config.infill_density - 0.15).abs() < 1e-6); // from process
        assert!((merged.config.nozzle_temp - 220.0).abs() < 1e-6); // from filament
        assert!((merged.config.bed_temp - 55.0).abs() < 1e-6); // from filament

        // Metadata should be merged.
        assert_eq!(
            merged.metadata.name.as_deref(),
            Some("0.20mm Standard + Generic PLA")
        );

        // Mapped fields should contain fields from both.
        assert!(merged.mapped_fields.contains(&"wall_loops".to_string()));
        assert!(merged
            .mapped_fields
            .contains(&"nozzle_temperature".to_string()));
    }

    #[test]
    fn test_float_rounding() {
        // A percentage like 15% becomes 0.15 in f64, which can have IEEE 754 noise.
        let mut config = PrintConfig::default();
        // Simulate the floating-point noise that can occur.
        config.infill_density = 0.15000000000000002;

        let result = ImportResult {
            config,
            mapped_fields: vec!["sparse_infill_density".into()],
            unmapped_fields: vec![],
            passthrough_fields: vec![],
            metadata: ProfileMetadata::default(),
        };

        let converted = convert_to_toml(&result);

        // The output should NOT contain the noisy representation.
        assert!(
            !converted.toml_output.contains("0.15000000000000002"),
            "TOML output should not contain floating-point noise"
        );

        // It should contain a clean value.
        assert!(
            converted.toml_output.contains("infill_density = 0.15"),
            "TOML output should contain clean float: {}",
            converted.toml_output
        );
    }

    #[test]
    fn test_unmapped_fields_in_comments() {
        let json_val = json!({
            "type": "process",
            "name": "Test",
            "layer_height": "0.2",
            "ams_drying_temperature": "55",
            "scan_first_layer": "1"
        });

        let result = import_upstream_profile(&json_val).unwrap();
        let converted = convert_to_toml(&result);

        // Passthrough fields (stored in config.passthrough, also in unmapped_fields)
        // should appear as comments.
        assert!(converted
            .toml_output
            .contains("# Unmapped fields from source"));
        assert!(converted
            .toml_output
            .contains("# - ams_drying_temperature"));
        assert!(converted.toml_output.contains("# - scan_first_layer"));
    }

    #[test]
    fn test_round_floats_in_value() {
        let mut val = toml::Value::Float(0.15000000000000002);
        round_floats_in_value(&mut val);
        assert_eq!(val, toml::Value::Float(0.15));

        let mut val = toml::Value::Float(1.0);
        round_floats_in_value(&mut val);
        assert_eq!(val, toml::Value::Float(1.0));

        // Test nested table rounding.
        let mut table = toml::map::Map::new();
        table.insert("a".into(), toml::Value::Float(0.200000000000000012));
        table.insert("b".into(), toml::Value::Integer(42));
        let mut val = toml::Value::Table(table);
        round_floats_in_value(&mut val);
        if let toml::Value::Table(t) = &val {
            assert_eq!(t["a"], toml::Value::Float(0.2));
            assert_eq!(t["b"], toml::Value::Integer(42));
        }
    }

    #[test]
    fn test_merge_empty_results() {
        let merged = merge_import_results(&[]);
        assert!((merged.config.layer_height - 0.2).abs() < 1e-9);
        assert!(merged.mapped_fields.is_empty());
        assert!(merged.unmapped_fields.is_empty());
    }

    #[test]
    fn test_merge_single_result() {
        let json_val = json!({
            "type": "process",
            "name": "Single",
            "wall_loops": "5"
        });
        let result = import_upstream_profile(&json_val).unwrap();
        let merged = merge_import_results(&[result.clone()]);

        assert_eq!(merged.config.wall_count, 5);
        assert_eq!(merged.metadata.name.as_deref(), Some("Single"));
    }
}
