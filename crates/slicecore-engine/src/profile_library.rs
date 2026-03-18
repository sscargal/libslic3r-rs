//! Batch profile conversion and library management.
//!
//! This module provides infrastructure for batch-converting upstream OrcaSlicer
//! and BambuStudio JSON profile directories into native TOML format. It handles:
//!
//! - **Inheritance resolution**: Walking the inherits chain within each vendor/type
//!   directory and merging from root ancestor to leaf profile.
//! - **Batch conversion**: Recursively processing vendor directories, skipping
//!   non-instantiated (base/parent) profiles, and writing TOML output files.
//! - **Index generation**: Building a searchable JSON manifest (`index.json`) with
//!   metadata extracted from profile names.
//!
//! # Usage
//!
//! ```ignore
//! use slicecore_engine::profile_library::{batch_convert_profiles, write_index};
//!
//! let result = batch_convert_profiles(
//!     Path::new("/path/to/OrcaSlicer/resources/profiles"),
//!     Path::new("profiles/orcaslicer"),
//!     "orcaslicer",
//! )?;
//! write_index(&result.index, Path::new("profiles"))?;
//! ```

use std::collections::HashMap;
use std::path::Path;

use crate::error::EngineError;
use crate::profile_convert::convert_to_toml;
use crate::profile_import::{import_upstream_profile, ImportResult};
use crate::profile_import_ini::{
    build_section_lookup, import_prusaslicer_ini_profile, parse_prusaslicer_ini,
    resolve_ini_inheritance,
};

/// An entry in the profile index, containing searchable metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProfileIndexEntry {
    /// Unique identifier, e.g. "orcaslicer/BBL/filament/Bambu_PLA_Basic_BBL_A1".
    pub id: String,
    /// Original profile name from the source JSON.
    pub name: String,
    /// Source slicer name, e.g. "orcaslicer" or "bambustudio".
    pub source: String,
    /// Vendor directory name, e.g. "BBL", "Creality".
    pub vendor: String,
    /// Profile type: "filament", "process", or "machine".
    pub profile_type: String,
    /// Material type extracted from name (for filament profiles).
    pub material: Option<String>,
    /// Nozzle size extracted from name.
    pub nozzle_size: Option<f64>,
    /// Printer model extracted from `@` suffix in name.
    pub printer_model: Option<String>,
    /// Relative path to the converted TOML file.
    pub path: String,
    /// Layer height extracted from process profile name.
    pub layer_height: Option<f64>,
    /// Quality level extracted from name.
    pub quality: Option<String>,
}

/// The profile index manifest, serialized to `index.json`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProfileIndex {
    /// Schema version (always 1).
    pub version: u32,
    /// ISO 8601 timestamp when the index was generated.
    pub generated: String,
    /// All converted profile entries.
    pub profiles: Vec<ProfileIndexEntry>,
}

/// Result of a batch conversion operation.
#[derive(Debug)]
pub struct BatchConvertResult {
    /// Number of profiles successfully converted.
    pub converted: usize,
    /// Number of profiles skipped (non-instantiated base profiles).
    pub skipped: usize,
    /// Error messages from individual profile conversion failures.
    pub errors: Vec<String>,
    /// The generated profile index.
    pub index: ProfileIndex,
}

// ---------------------------------------------------------------------------
// Inheritance resolution
// ---------------------------------------------------------------------------

/// Maximum inheritance depth to guard against circular references.
const MAX_INHERITANCE_DEPTH: usize = 10;

/// Resolve the inheritance chain for a profile within its vendor/type directory.
///
/// Loads the profile and all its ancestors, merging from root to leaf.
/// Results are cached in `cache` to avoid redundant file reads.
fn resolve_inheritance(
    profile_path: &Path,
    type_dir: &Path,
    cache: &mut HashMap<String, ImportResult>,
) -> Result<ImportResult, EngineError> {
    resolve_inheritance_depth(profile_path, type_dir, cache, 0)
}

fn resolve_inheritance_depth(
    profile_path: &Path,
    type_dir: &Path,
    cache: &mut HashMap<String, ImportResult>,
    depth: usize,
) -> Result<ImportResult, EngineError> {
    if depth > MAX_INHERITANCE_DEPTH {
        return Err(EngineError::ConfigError(format!(
            "Inheritance depth exceeds {} for '{}'",
            MAX_INHERITANCE_DEPTH,
            profile_path.display()
        )));
    }

    // Check cache by filename stem.
    let stem = profile_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    if let Some(cached) = cache.get(&stem) {
        return Ok(cached.clone());
    }

    // Read and parse the JSON file.
    let contents = std::fs::read_to_string(profile_path).map_err(|e| {
        EngineError::ConfigError(format!(
            "Failed to read '{}': {}",
            profile_path.display(),
            e
        ))
    })?;

    let value: serde_json::Value = serde_json::from_str(&contents).map_err(|e| {
        EngineError::ConfigError(format!(
            "Failed to parse JSON '{}': {}",
            profile_path.display(),
            e
        ))
    })?;

    // Import this profile's fields.
    let child_result = import_upstream_profile(&value)?;

    // Check for inherits field.
    let inherits = value
        .as_object()
        .and_then(|obj| obj.get("inherits"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let resolved = if let Some(parent_name) = inherits {
        // Find parent JSON file in the same type directory.
        let parent_path = type_dir.join(format!("{}.json", parent_name));

        if parent_path.exists() {
            let parent_result =
                resolve_inheritance_depth(&parent_path, type_dir, cache, depth + 1)?;
            // Merge: start from parent, overlay child's explicit fields.
            merge_inheritance(parent_result, child_result)
        } else {
            // Parent not found -- use child as-is (start from defaults).
            child_result
        }
    } else {
        // No inheritance -- use as-is.
        child_result
    };

    // Cache the resolved result.
    cache.insert(stem, resolved.clone());

    Ok(resolved)
}

/// Merge a child profile onto a parent for inheritance resolution.
///
/// Unlike `merge_import_results` (which uses default-comparison), this function
/// starts from the parent's config and overlays the child's explicitly-set fields.
/// This correctly handles the case where a child sets a field to the same value
/// as the global default but different from the parent.
fn merge_inheritance(parent: ImportResult, child: ImportResult) -> ImportResult {
    use crate::profile_convert::round_floats_in_value;

    // Start from parent's config as TOML table.
    let parent_value =
        toml::Value::try_from(&parent.config).expect("PrintConfig should serialize to toml::Value");
    let mut merged_table = match parent_value {
        toml::Value::Table(t) => t,
        _ => unreachable!(),
    };

    // Serialize child's config to get its field values.
    let child_value =
        toml::Value::try_from(&child.config).expect("PrintConfig should serialize to toml::Value");
    let child_table = match child_value {
        toml::Value::Table(t) => t,
        _ => unreachable!(),
    };

    // Serialize default config for comparison against child.
    let default_config = crate::config::PrintConfig::default();
    let default_value = toml::Value::try_from(&default_config)
        .expect("PrintConfig default should serialize to toml::Value");
    let default_table = match default_value {
        toml::Value::Table(t) => t,
        _ => unreachable!(),
    };

    // For each key in the child's table: if the child's value differs from the
    // default OR if the child's mapped_fields contains the corresponding upstream
    // key, apply it to the merged table. We use the simpler heuristic: overlay
    // any field where the child's value differs from default, AND also overlay
    // any field that the child explicitly mapped.
    //
    // To handle the "child sets field to default value" case, we check
    // mapped_fields. The mapped_fields list contains the upstream JSON key names,
    // not the PrintConfig field names. We need to overlay based on what fields
    // the child import actually touched.

    // Build a set of PrintConfig field names that the child touched.
    // We do this by comparing child config to default: any difference means touched.
    // PLUS: we check if the child's mapped_fields is non-empty -- if the child had
    // any mapped fields, all non-default child values are overlaid.
    for (key, val) in &child_table {
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

    // Now handle the tricky case: child explicitly maps a field to a value that
    // equals the global default but differs from the parent. We only overlay
    // fields that the child's JSON explicitly contained (tracked in mapped_fields).
    //
    // Build a set of PrintConfig field names that the child actually touched,
    // using the upstream-key-to-config-field mapping.
    let child_touched_fields: std::collections::HashSet<&str> = child
        .mapped_fields
        .iter()
        .filter_map(|upstream_key| {
            crate::profile_import::upstream_key_to_config_field(upstream_key)
        })
        .collect();

    let parent_table_for_cmp = match toml::Value::try_from(&parent.config) {
        Ok(toml::Value::Table(t)) => t,
        _ => unreachable!(),
    };

    for (key, val) in &child_table {
        // Only overlay fields the child explicitly mapped.
        if !child_touched_fields.contains(key.as_str()) {
            continue;
        }
        if let Some(parent_val) = parent_table_for_cmp.get(key) {
            let mut val_rounded = val.clone();
            let mut par_rounded = parent_val.clone();
            round_floats_in_value(&mut val_rounded);
            round_floats_in_value(&mut par_rounded);
            if val_rounded != par_rounded {
                merged_table.insert(key.clone(), val_rounded);
            }
        }
    }

    // Deserialize back to PrintConfig.
    let merged_config: crate::config::PrintConfig = toml::Value::Table(merged_table)
        .try_into()
        .unwrap_or_else(|e| {
            eprintln!("Warning: failed to deserialize merged config: {}", e);
            crate::config::PrintConfig::default()
        });

    // Merge field lists.
    let mut all_mapped = parent.mapped_fields;
    for f in &child.mapped_fields {
        if !all_mapped.contains(f) {
            all_mapped.push(f.clone());
        }
    }
    let mut all_unmapped = parent.unmapped_fields;
    for f in &child.unmapped_fields {
        if !all_unmapped.contains(f) {
            all_unmapped.push(f.clone());
        }
    }
    let mut all_passthrough = parent.passthrough_fields;
    for f in &child.passthrough_fields {
        if !all_passthrough.contains(f) {
            all_passthrough.push(f.clone());
        }
    }

    // Use child's metadata (the leaf profile).
    ImportResult {
        config: merged_config,
        mapped_fields: all_mapped,
        unmapped_fields: all_unmapped,
        passthrough_fields: all_passthrough,
        metadata: child.metadata,
    }
}

// ---------------------------------------------------------------------------
// Batch conversion
// ---------------------------------------------------------------------------

/// Convert all profiles in a source directory tree to native TOML format.
///
/// Walks `source_dir` looking for vendor directories (direct children that are
/// directories). Within each vendor, processes `filament/`, `process/`, and
/// `machine/` subdirectories. Only profiles with `"instantiation": "true"` are
/// converted; base/parent profiles are skipped.
///
/// Converted TOML files are written to `output_dir/vendor/type/name.toml`.
/// Individual profile errors are collected but do not abort the batch.
pub fn batch_convert_profiles(
    source_dir: &Path,
    output_dir: &Path,
    source_name: &str,
) -> Result<BatchConvertResult, EngineError> {
    let mut converted: usize = 0;
    let mut skipped: usize = 0;
    let mut errors: Vec<String> = Vec::new();
    let mut entries: Vec<ProfileIndexEntry> = Vec::new();

    // Verify source directory exists.
    if !source_dir.is_dir() {
        return Err(EngineError::ConfigError(format!(
            "Source directory '{}' does not exist or is not a directory",
            source_dir.display()
        )));
    }

    // Walk direct children of source_dir to find vendor directories.
    let vendor_dirs = match std::fs::read_dir(source_dir) {
        Ok(rd) => rd,
        Err(e) => {
            return Err(EngineError::ConfigError(format!(
                "Failed to read source directory '{}': {}",
                source_dir.display(),
                e
            )));
        }
    };

    for vendor_entry in vendor_dirs {
        let vendor_entry = match vendor_entry {
            Ok(e) => e,
            Err(e) => {
                errors.push(format!("Failed to read vendor entry: {}", e));
                continue;
            }
        };

        let vendor_path = vendor_entry.path();
        if !vendor_path.is_dir() {
            continue; // Skip non-directory entries (.json files, .py scripts, etc.)
        }

        let vendor_name = vendor_entry.file_name().to_string_lossy().to_string();

        // Process each profile type subdirectory.
        for profile_type in &["filament", "process", "machine"] {
            let type_dir = vendor_path.join(profile_type);
            if !type_dir.is_dir() {
                continue;
            }

            // Inheritance cache per vendor/type directory.
            let mut cache: HashMap<String, ImportResult> = HashMap::new();

            // Walk all .json files in the type directory.
            for entry in walkdir::WalkDir::new(&type_dir)
                .min_depth(1)
                .max_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }

                // Read the file to check instantiation field.
                let contents = match std::fs::read_to_string(path) {
                    Ok(c) => c,
                    Err(e) => {
                        errors.push(format!("Failed to read '{}': {}", path.display(), e));
                        continue;
                    }
                };

                let value: serde_json::Value = match serde_json::from_str(&contents) {
                    Ok(v) => v,
                    Err(e) => {
                        errors.push(format!("Failed to parse JSON '{}': {}", path.display(), e));
                        continue;
                    }
                };

                // Check instantiation field -- skip non-instantiated profiles.
                let instantiation = value
                    .as_object()
                    .and_then(|obj| obj.get("instantiation"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if instantiation != "true" {
                    skipped += 1;
                    continue;
                }

                // Resolve inheritance chain.
                let resolved = match resolve_inheritance(path, &type_dir, &mut cache) {
                    Ok(r) => r,
                    Err(e) => {
                        errors.push(format!(
                            "Failed to resolve inheritance for '{}': {}",
                            path.display(),
                            e
                        ));
                        continue;
                    }
                };

                // Convert to TOML.
                let convert_result = convert_to_toml(&resolved);

                // Determine output filename.
                let profile_name = resolved.metadata.name.clone().unwrap_or_else(|| {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string()
                });

                let sanitized = sanitize_filename(&profile_name);
                let out_dir = output_dir.join(&vendor_name).join(profile_type);

                // Create output directory.
                if let Err(e) = std::fs::create_dir_all(&out_dir) {
                    errors.push(format!(
                        "Failed to create directory '{}': {}",
                        out_dir.display(),
                        e
                    ));
                    continue;
                }

                let out_file = out_dir.join(format!("{}.toml", sanitized));

                // Write TOML file.
                if let Err(e) = std::fs::write(&out_file, &convert_result.toml_output) {
                    errors.push(format!("Failed to write '{}': {}", out_file.display(), e));
                    continue;
                }

                // Build index entry.
                let relative_path = format!(
                    "{}/{}/{}/{}.toml",
                    source_name, vendor_name, profile_type, sanitized
                );
                let id = format!(
                    "{}/{}/{}/{}",
                    source_name, vendor_name, profile_type, sanitized
                );

                let entry = ProfileIndexEntry {
                    id,
                    name: profile_name.clone(),
                    source: source_name.to_string(),
                    vendor: vendor_name.clone(),
                    profile_type: profile_type.to_string(),
                    material: extract_material_from_name(&profile_name),
                    nozzle_size: extract_nozzle_size_from_name(&profile_name),
                    printer_model: extract_printer_model(&profile_name),
                    path: relative_path,
                    layer_height: extract_layer_height_from_name(&profile_name),
                    quality: extract_quality_from_name(&profile_name),
                };

                entries.push(entry);
                converted += 1;
            }
        }
    }

    let index = ProfileIndex {
        version: 1,
        generated: chrono_timestamp(),
        profiles: entries,
    };

    Ok(BatchConvertResult {
        converted,
        skipped,
        errors,
        index,
    })
}

// ---------------------------------------------------------------------------
// PrusaSlicer batch conversion
// ---------------------------------------------------------------------------

/// Convert all PrusaSlicer profiles from INI vendor files to native TOML format.
///
/// Walks `source_dir` looking for `*.ini` files (not subdirectories). Skips files
/// containing "SLA" in the filename. For each INI file, parses sections, resolves
/// inheritance, and converts concrete profiles to TOML.
///
/// Output structure: `output_dir/vendor_name/profile_type/sanitized_name.toml`.
/// Individual profile errors are collected but do not abort the batch.
pub fn batch_convert_prusaslicer_profiles(
    source_dir: &Path,
    output_dir: &Path,
    source_name: &str,
) -> Result<BatchConvertResult, EngineError> {
    let mut converted: usize = 0;
    let mut skipped: usize = 0;
    let mut errors: Vec<String> = Vec::new();
    let mut entries: Vec<ProfileIndexEntry> = Vec::new();

    // Verify source directory exists.
    if !source_dir.is_dir() {
        return Err(EngineError::ConfigError(format!(
            "Source directory '{}' does not exist or is not a directory",
            source_dir.display()
        )));
    }

    // Walk *.ini files directly in source_dir (not subdirectories).
    let dir_entries = match std::fs::read_dir(source_dir) {
        Ok(rd) => rd,
        Err(e) => {
            return Err(EngineError::ConfigError(format!(
                "Failed to read source directory '{}': {}",
                source_dir.display(),
                e
            )));
        }
    };

    for dir_entry in dir_entries {
        let dir_entry = match dir_entry {
            Ok(e) => e,
            Err(e) => {
                errors.push(format!("Failed to read directory entry: {}", e));
                continue;
            }
        };

        let path = dir_entry.path();

        // Only process .ini files.
        if path.extension().and_then(|e| e.to_str()) != Some("ini") {
            continue;
        }

        // Skip SLA vendor files.
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if filename.contains("SLA") {
            skipped += 1;
            continue;
        }

        // Derive vendor name from filename stem.
        let vendor_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Read and parse the INI file.
        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                errors.push(format!("Failed to read '{}': {}", path.display(), e));
                continue;
            }
        };

        let sections = parse_prusaslicer_ini(&contents);
        let lookup = build_section_lookup(&sections);

        // Profile types to convert.
        const CONVERTIBLE_TYPES: &[&str] = &["print", "filament", "printer"];

        // Process each concrete section.
        for (idx, section) in sections.iter().enumerate() {
            // Skip abstract profiles.
            if section.is_abstract {
                skipped += 1;
                continue;
            }

            // Skip non-profile section types (vendor, printer_model).
            if !CONVERTIBLE_TYPES.contains(&section.section_type.as_str()) {
                continue;
            }

            // Skip sections with empty names.
            if section.name.is_empty() {
                continue;
            }

            // Resolve inheritance chain.
            let resolved = resolve_ini_inheritance(&sections[idx], &sections, &lookup, 0);

            // Convert to ImportResult.
            let import_result =
                import_prusaslicer_ini_profile(&resolved, &section.name, &section.section_type);

            // Convert to TOML.
            let convert_result = convert_to_toml(&import_result);

            // Map section type to profile type directory name.
            let profile_type = match section.section_type.as_str() {
                "print" => "process",
                "filament" => "filament",
                "printer" => "machine",
                _ => &section.section_type,
            };

            let sanitized = sanitize_filename(&section.name);
            let out_dir = output_dir.join(&vendor_name).join(profile_type);

            // Create output directory.
            if let Err(e) = std::fs::create_dir_all(&out_dir) {
                errors.push(format!(
                    "Failed to create directory '{}': {}",
                    out_dir.display(),
                    e
                ));
                continue;
            }

            let out_file = out_dir.join(format!("{}.toml", sanitized));

            // Write TOML file.
            if let Err(e) = std::fs::write(&out_file, &convert_result.toml_output) {
                errors.push(format!("Failed to write '{}': {}", out_file.display(), e));
                continue;
            }

            // Build index entry.
            let relative_path = format!(
                "{}/{}/{}/{}.toml",
                source_name, vendor_name, profile_type, sanitized
            );
            let id = format!(
                "{}/{}/{}/{}",
                source_name, vendor_name, profile_type, sanitized
            );

            let entry = ProfileIndexEntry {
                id,
                name: section.name.clone(),
                source: source_name.to_string(),
                vendor: vendor_name.clone(),
                profile_type: profile_type.to_string(),
                material: extract_material_from_name(&section.name),
                nozzle_size: extract_nozzle_size_from_name(&section.name),
                printer_model: extract_printer_model(&section.name),
                path: relative_path,
                layer_height: extract_layer_height_from_name(&section.name),
                quality: extract_quality_from_name(&section.name),
            };

            entries.push(entry);
            converted += 1;
        }
    }

    let index = ProfileIndex {
        version: 1,
        generated: chrono_timestamp(),
        profiles: entries,
    };

    Ok(BatchConvertResult {
        converted,
        skipped,
        errors,
        index,
    })
}

// ---------------------------------------------------------------------------
// Index I/O
// ---------------------------------------------------------------------------

/// Write the profile index to `output_dir/index.json` as pretty-printed JSON.
pub fn write_index(index: &ProfileIndex, output_dir: &Path) -> Result<(), EngineError> {
    std::fs::create_dir_all(output_dir).map_err(|e| {
        EngineError::ConfigError(format!(
            "Failed to create directory '{}': {}",
            output_dir.display(),
            e
        ))
    })?;

    let json = serde_json::to_string_pretty(index)
        .map_err(|e| EngineError::ConfigError(format!("Failed to serialize index: {}", e)))?;

    let path = output_dir.join("index.json");
    std::fs::write(&path, json).map_err(|e| {
        EngineError::ConfigError(format!("Failed to write index '{}': {}", path.display(), e))
    })?;

    Ok(())
}

/// Load a profile index from `profiles_dir/index.json`.
pub fn load_index(profiles_dir: &Path) -> Result<ProfileIndex, EngineError> {
    let path = profiles_dir.join("index.json");
    let contents = std::fs::read_to_string(&path).map_err(|e| {
        EngineError::ConfigError(format!("Failed to read index '{}': {}", path.display(), e))
    })?;

    let index: ProfileIndex = serde_json::from_str(&contents).map_err(|e| {
        EngineError::ConfigError(format!("Failed to parse index '{}': {}", path.display(), e))
    })?;

    Ok(index)
}

/// Write a profile index, merging with any existing index at `output_dir/index.json`.
///
/// If an existing `index.json` exists, loads it and merges the new entries:
/// - New entries with the same `id` replace existing ones.
/// - Entries from the existing index with different IDs are preserved.
///
/// If no existing index exists, writes the new index as-is.
pub fn write_merged_index(new_index: &ProfileIndex, output_dir: &Path) -> Result<(), EngineError> {
    let index_path = output_dir.join("index.json");

    let merged = if index_path.exists() {
        // Load existing index.
        let existing = load_index(output_dir)?;

        // Build a set of IDs in the new index for fast lookup.
        let new_ids: std::collections::HashSet<&str> =
            new_index.profiles.iter().map(|p| p.id.as_str()).collect();

        // Keep existing entries whose IDs are not in the new index.
        let mut merged_profiles: Vec<ProfileIndexEntry> = existing
            .profiles
            .into_iter()
            .filter(|p| !new_ids.contains(p.id.as_str()))
            .collect();

        // Append all new entries.
        merged_profiles.extend(new_index.profiles.clone());

        ProfileIndex {
            version: 1,
            generated: chrono_timestamp(),
            profiles: merged_profiles,
        }
    } else {
        new_index.clone()
    };

    write_index(&merged, output_dir)
}

// ---------------------------------------------------------------------------
// Metadata extraction helpers
// ---------------------------------------------------------------------------

/// Sanitize a profile name for use as a filename.
///
/// Replaces spaces with `_`, removes `@`, removes parentheses,
/// replaces `/` with `_`, replaces `&&` with `_and_`.
pub(crate) fn sanitize_filename(name: &str) -> String {
    // First, replace multi-character sequences.
    let name = name.replace("&&", "_and_");
    name.chars()
        .filter_map(|c| match c {
            ' ' => Some('_'),
            '@' => None,
            '(' | ')' => None,
            '/' => Some('_'),
            _ => Some(c),
        })
        .collect()
}

/// Extract material type from a profile name.
///
/// Matches against known material names, checking longest matches first to
/// avoid "PLA" matching before "PLA-CF".
pub(crate) fn extract_material_from_name(name: &str) -> Option<String> {
    // Order: longest match first to avoid prefix collisions.
    const MATERIALS: &[&str] = &[
        "PLA-CF", "PLA+", "PETG-CF", "PA-CF", "PLA", "PETG", "ABS", "ASA", "PA", "PC", "TPU",
        "PVA", "HIPS", "PP",
    ];

    let upper = name.to_uppercase();
    for mat in MATERIALS {
        if upper.contains(mat) {
            return Some(mat.to_string());
        }
    }
    None
}

/// Extract layer height from a profile name.
///
/// Matches patterns like `0.20mm` or `0.08mm` at the start of the name.
pub(crate) fn extract_layer_height_from_name(name: &str) -> Option<f64> {
    let trimmed = name.trim();
    // Look for a pattern like "0.XXmm" at the start.
    if let Some(mm_pos) = trimmed.find("mm") {
        // Find the start of the number (walk backwards from mm_pos).
        let before = &trimmed[..mm_pos];
        // Try to find a float-like sequence ending at mm_pos.
        // Use char_indices to get the correct byte offset after a multi-byte character.
        let start = before
            .char_indices()
            .rev()
            .find(|&(_, c)| !c.is_ascii_digit() && c != '.')
            .map(|(p, c)| p + c.len_utf8())
            .unwrap_or(0);
        let num_str = &before[start..];
        if !num_str.is_empty() {
            return num_str.parse::<f64>().ok();
        }
    }
    None
}

/// Extract nozzle size from a profile name.
///
/// Matches patterns like `0.4 nozzle` or `0.6 nozzle`.
pub(crate) fn extract_nozzle_size_from_name(name: &str) -> Option<f64> {
    let lower = name.to_lowercase();
    if let Some(nozzle_pos) = lower.find("nozzle") {
        let before = lower[..nozzle_pos].trim_end();
        // Find the last number before "nozzle".
        // Use char_indices to get the correct byte offset after a multi-byte character.
        let start = before
            .char_indices()
            .rev()
            .find(|&(_, c)| !c.is_ascii_digit() && c != '.')
            .map(|(p, c)| p + c.len_utf8())
            .unwrap_or(0);
        let num_str = &before[start..];
        if !num_str.is_empty() {
            return num_str.parse::<f64>().ok();
        }
    }
    None
}

/// Extract printer model from the `@` suffix in a profile name.
///
/// For example, `"Bambu ABS @BBL X1C"` returns `Some("BBL X1C")`.
pub(crate) fn extract_printer_model(name: &str) -> Option<String> {
    if let Some(at_pos) = name.find('@') {
        let model = name[at_pos + 1..].trim();
        if !model.is_empty() {
            return Some(model.to_string());
        }
    }
    None
}

/// Extract quality level from a profile name.
///
/// Matches (case-insensitive) OrcaSlicer terms: "Extra Fine", "Fine",
/// "High Quality", "Standard", "Draft", "Super Draft".
///
/// Also matches PrusaSlicer terms: "Ultra Detail", "Detail", "Optimal",
/// "Normal", "Speed", "Fast".
///
/// Longest matches are checked first to avoid prefix collisions
/// (e.g., "ultradetail" before "detail", "super draft" before "draft").
pub(crate) fn extract_quality_from_name(name: &str) -> Option<String> {
    let lower = name.to_lowercase();
    // Check longest matches first.
    if lower.contains("super draft") {
        Some("Super Draft".to_string())
    } else if lower.contains("extra fine") {
        Some("Extra Fine".to_string())
    } else if lower.contains("high quality") {
        Some("High Quality".to_string())
    } else if lower.contains("ultradetail") {
        Some("Ultra Detail".to_string())
    } else if lower.contains("standard") {
        Some("Standard".to_string())
    } else if lower.contains("draft") {
        Some("Draft".to_string())
    } else if lower.contains("fine") {
        Some("Fine".to_string())
    } else if lower.contains("detail") {
        Some("Detail".to_string())
    } else if lower.contains("optimal") {
        Some("Optimal".to_string())
    } else if lower.contains("normal") {
        Some("Normal".to_string())
    } else if lower.contains("speed") {
        Some("Speed".to_string())
    } else if lower.contains("fast") {
        Some("Fast".to_string())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Generate an ISO 8601 timestamp string (simplified, no external dependency).
fn chrono_timestamp() -> String {
    // Use a fixed format. In a real implementation you'd use chrono or time crate.
    // We approximate with std::time.
    let now = std::time::SystemTime::now();
    let since_epoch = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = since_epoch.as_secs();
    // Simple UTC timestamp without external crate.
    // Format: YYYY-MM-DDTHH:MM:SSZ (approximate from epoch seconds).
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Calculate year/month/day from days since epoch (1970-01-01).
    let (year, month, day) = days_to_ymd(days);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm based on Howard Hinnant's civil_from_days.
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64 + era * 400) as u64;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(
            sanitize_filename("Bambu PLA Basic @BBL A1"),
            "Bambu_PLA_Basic_BBL_A1"
        );
        assert_eq!(
            sanitize_filename("Generic (PLA) Profile"),
            "Generic_PLA_Profile"
        );
        assert_eq!(sanitize_filename("Profile/SubName"), "Profile_SubName");
        assert_eq!(sanitize_filename("simple"), "simple");
    }

    #[test]
    fn test_extract_material() {
        assert_eq!(
            extract_material_from_name("Bambu PLA Basic @BBL A1"),
            Some("PLA".to_string())
        );
        assert_eq!(
            extract_material_from_name("Generic PETG"),
            Some("PETG".to_string())
        );
        // PLA-CF must match before PLA.
        assert_eq!(
            extract_material_from_name("Bambu PLA-CF @BBL X1C"),
            Some("PLA-CF".to_string())
        );
        assert_eq!(
            extract_material_from_name("Generic ABS @BBL X1C"),
            Some("ABS".to_string())
        );
        assert_eq!(extract_material_from_name("Some Unknown Material"), None);
        assert_eq!(
            extract_material_from_name("Bambu TPU 95A @BBL A1"),
            Some("TPU".to_string())
        );
    }

    #[test]
    fn test_extract_layer_height() {
        assert_eq!(
            extract_layer_height_from_name("0.20mm Standard"),
            Some(0.20)
        );
        assert_eq!(
            extract_layer_height_from_name("0.08mm Extra Fine @BBL X1C"),
            Some(0.08)
        );
        assert_eq!(extract_layer_height_from_name("0.28mm Initial"), Some(0.28));
        assert_eq!(extract_layer_height_from_name("Standard Profile"), None);
    }

    #[test]
    fn test_extract_nozzle_size() {
        assert_eq!(extract_nozzle_size_from_name("0.4 nozzle"), Some(0.4));
        assert_eq!(
            extract_nozzle_size_from_name("Profile for 0.6 nozzle @BBL X1C"),
            Some(0.6)
        );
        assert_eq!(extract_nozzle_size_from_name("no nozzle info"), None);
    }

    #[test]
    fn test_extract_quality() {
        assert_eq!(
            extract_quality_from_name("0.20mm Standard @BBL X1C"),
            Some("Standard".to_string())
        );
        assert_eq!(
            extract_quality_from_name("0.08mm Extra Fine"),
            Some("Extra Fine".to_string())
        );
        assert_eq!(
            extract_quality_from_name("0.30mm Draft"),
            Some("Draft".to_string())
        );
        assert_eq!(
            extract_quality_from_name("0.40mm Super Draft"),
            Some("Super Draft".to_string())
        );
        assert_eq!(
            extract_quality_from_name("0.12mm Fine"),
            Some("Fine".to_string())
        );
        assert_eq!(extract_quality_from_name("Generic PLA"), None);
    }

    #[test]
    fn test_extract_printer_model() {
        assert_eq!(
            extract_printer_model("Bambu ABS @BBL X1C"),
            Some("BBL X1C".to_string())
        );
        assert_eq!(
            extract_printer_model("Generic PLA @Creality K1"),
            Some("Creality K1".to_string())
        );
        assert_eq!(extract_printer_model("Generic PLA"), None);
    }

    #[test]
    fn test_index_serialization() {
        let index = ProfileIndex {
            version: 1,
            generated: "2026-01-01T00:00:00Z".to_string(),
            profiles: vec![
                ProfileIndexEntry {
                    id: "orcaslicer/BBL/filament/Bambu_PLA_Basic".to_string(),
                    name: "Bambu PLA Basic".to_string(),
                    source: "orcaslicer".to_string(),
                    vendor: "BBL".to_string(),
                    profile_type: "filament".to_string(),
                    material: Some("PLA".to_string()),
                    nozzle_size: None,
                    printer_model: None,
                    path: "orcaslicer/BBL/filament/Bambu_PLA_Basic.toml".to_string(),
                    layer_height: None,
                    quality: None,
                },
                ProfileIndexEntry {
                    id: "orcaslicer/BBL/process/0.20mm_Standard_BBL_X1C".to_string(),
                    name: "0.20mm Standard @BBL X1C".to_string(),
                    source: "orcaslicer".to_string(),
                    vendor: "BBL".to_string(),
                    profile_type: "process".to_string(),
                    material: None,
                    nozzle_size: None,
                    printer_model: Some("BBL X1C".to_string()),
                    path: "orcaslicer/BBL/process/0.20mm_Standard_BBL_X1C.toml".to_string(),
                    layer_height: Some(0.20),
                    quality: Some("Standard".to_string()),
                },
            ],
        };

        // Serialize to JSON.
        let json = serde_json::to_string_pretty(&index).unwrap();

        // Deserialize back.
        let loaded: ProfileIndex = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.generated, "2026-01-01T00:00:00Z");
        assert_eq!(loaded.profiles.len(), 2);
        assert_eq!(
            loaded.profiles[0].id,
            "orcaslicer/BBL/filament/Bambu_PLA_Basic"
        );
        assert_eq!(loaded.profiles[0].material, Some("PLA".to_string()));
        assert_eq!(loaded.profiles[1].layer_height, Some(0.20));
        assert_eq!(loaded.profiles[1].quality, Some("Standard".to_string()));
        assert_eq!(
            loaded.profiles[1].printer_model,
            Some("BBL X1C".to_string())
        );
    }

    #[test]
    fn test_resolve_inheritance_simple() {
        // Create a temp directory with parent + child profiles.
        let dir = std::env::temp_dir().join("slicecore_test_inheritance");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Parent profile (base, not instantiated).
        let parent = serde_json::json!({
            "type": "filament",
            "name": "Generic PLA",
            "nozzle_temperature": ["210"],
            "hot_plate_temp": ["55"],
            "filament_density": ["1.24"]
        });
        std::fs::write(
            dir.join("Generic PLA.json"),
            serde_json::to_string_pretty(&parent).unwrap(),
        )
        .unwrap();

        // Child profile that inherits from parent.
        let child = serde_json::json!({
            "type": "filament",
            "name": "Bambu PLA Basic",
            "inherits": "Generic PLA",
            "instantiation": "true",
            "nozzle_temperature": ["220"],
            "hot_plate_temp": ["60"]
        });
        std::fs::write(
            dir.join("Bambu PLA Basic.json"),
            serde_json::to_string_pretty(&child).unwrap(),
        )
        .unwrap();

        let mut cache = HashMap::new();
        let result =
            resolve_inheritance(&dir.join("Bambu PLA Basic.json"), &dir, &mut cache).unwrap();

        // Child overrides parent for nozzle_temperature and hot_plate_temp.
        assert!((result.config.filament.nozzle_temp() - 220.0).abs() < 1e-6);
        assert!((result.config.filament.bed_temp() - 60.0).abs() < 1e-6);
        // Parent's filament_density is inherited.
        assert!((result.config.filament.density - 1.24).abs() < 1e-6);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_batch_convert_empty_dir() {
        let dir = std::env::temp_dir().join("slicecore_test_batch_empty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let out_dir = std::env::temp_dir().join("slicecore_test_batch_out");
        let _ = std::fs::remove_dir_all(&out_dir);

        let result = batch_convert_profiles(&dir, &out_dir, "test").unwrap();

        assert_eq!(result.converted, 0);
        assert_eq!(result.skipped, 0);
        assert!(result.errors.is_empty());
        assert!(result.index.profiles.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn test_write_and_load_index() {
        let dir = std::env::temp_dir().join("slicecore_test_index_io");
        let _ = std::fs::remove_dir_all(&dir);

        let index = ProfileIndex {
            version: 1,
            generated: "2026-01-15T10:30:00Z".to_string(),
            profiles: vec![ProfileIndexEntry {
                id: "test/vendor/filament/test_pla".to_string(),
                name: "Test PLA".to_string(),
                source: "test".to_string(),
                vendor: "vendor".to_string(),
                profile_type: "filament".to_string(),
                material: Some("PLA".to_string()),
                nozzle_size: None,
                printer_model: None,
                path: "test/vendor/filament/test_pla.toml".to_string(),
                layer_height: None,
                quality: None,
            }],
        };

        write_index(&index, &dir).unwrap();
        let loaded = load_index(&dir).unwrap();

        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.profiles.len(), 1);
        assert_eq!(loaded.profiles[0].name, "Test PLA");
        assert_eq!(loaded.profiles[0].material, Some("PLA".to_string()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_sanitize_filename_with_ampersands() {
        assert_eq!(
            sanitize_filename("Original Prusa i3 MK3S && MK3S+"),
            "Original_Prusa_i3_MK3S__and__MK3S+"
        );
        assert_eq!(sanitize_filename("MK3.9 && MK3.9+"), "MK3.9__and__MK3.9+");
        // Single & should be preserved.
        assert_eq!(sanitize_filename("A & B"), "A_&_B");
    }

    #[test]
    fn test_extract_quality_prusaslicer_terms() {
        assert_eq!(
            extract_quality_from_name("0.05mm ULTRADETAIL @0.25 nozzle"),
            Some("Ultra Detail".to_string())
        );
        assert_eq!(
            extract_quality_from_name("0.10mm DETAIL @MK4S"),
            Some("Detail".to_string())
        );
        assert_eq!(
            extract_quality_from_name("0.15mm OPTIMAL @MK4S"),
            Some("Optimal".to_string())
        );
        assert_eq!(
            extract_quality_from_name("0.20mm NORMAL"),
            Some("Normal".to_string())
        );
        assert_eq!(
            extract_quality_from_name("0.30mm SPEED @MK4S"),
            Some("Speed".to_string())
        );
        assert_eq!(
            extract_quality_from_name("0.35mm FAST"),
            Some("Fast".to_string())
        );
        // Ensure "ultradetail" matches before "detail".
        assert_eq!(
            extract_quality_from_name("ULTRADETAIL profile"),
            Some("Ultra Detail".to_string())
        );
    }

    #[test]
    fn test_write_merged_index_new() {
        let dir = std::env::temp_dir().join("slicecore_test_merged_new");
        let _ = std::fs::remove_dir_all(&dir);

        // No existing index -- should write as-is.
        let index = ProfileIndex {
            version: 1,
            generated: "2026-01-01T00:00:00Z".to_string(),
            profiles: vec![ProfileIndexEntry {
                id: "prusaslicer/Prusa/process/test".to_string(),
                name: "Test Profile".to_string(),
                source: "prusaslicer".to_string(),
                vendor: "Prusa".to_string(),
                profile_type: "process".to_string(),
                material: None,
                nozzle_size: None,
                printer_model: None,
                path: "prusaslicer/Prusa/process/test.toml".to_string(),
                layer_height: None,
                quality: None,
            }],
        };

        write_merged_index(&index, &dir).unwrap();
        let loaded = load_index(&dir).unwrap();
        assert_eq!(loaded.profiles.len(), 1);
        assert_eq!(loaded.profiles[0].source, "prusaslicer");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_write_merged_index_preserves_existing() {
        let dir = std::env::temp_dir().join("slicecore_test_merged_existing");
        let _ = std::fs::remove_dir_all(&dir);

        // Write an initial "orcaslicer" index.
        let orca_index = ProfileIndex {
            version: 1,
            generated: "2026-01-01T00:00:00Z".to_string(),
            profiles: vec![ProfileIndexEntry {
                id: "orcaslicer/BBL/filament/PLA".to_string(),
                name: "PLA".to_string(),
                source: "orcaslicer".to_string(),
                vendor: "BBL".to_string(),
                profile_type: "filament".to_string(),
                material: Some("PLA".to_string()),
                nozzle_size: None,
                printer_model: None,
                path: "orcaslicer/BBL/filament/PLA.toml".to_string(),
                layer_height: None,
                quality: None,
            }],
        };
        write_index(&orca_index, &dir).unwrap();

        // Now merge a "prusaslicer" index.
        let prusa_index = ProfileIndex {
            version: 1,
            generated: "2026-01-02T00:00:00Z".to_string(),
            profiles: vec![ProfileIndexEntry {
                id: "prusaslicer/Prusa/filament/PrusaPLA".to_string(),
                name: "Prusament PLA".to_string(),
                source: "prusaslicer".to_string(),
                vendor: "Prusa".to_string(),
                profile_type: "filament".to_string(),
                material: Some("PLA".to_string()),
                nozzle_size: None,
                printer_model: None,
                path: "prusaslicer/Prusa/filament/PrusaPLA.toml".to_string(),
                layer_height: None,
                quality: None,
            }],
        };
        write_merged_index(&prusa_index, &dir).unwrap();

        let loaded = load_index(&dir).unwrap();
        // Should have both OrcaSlicer and PrusaSlicer entries.
        assert_eq!(loaded.profiles.len(), 2);

        let sources: Vec<&str> = loaded.profiles.iter().map(|p| p.source.as_str()).collect();
        assert!(sources.contains(&"orcaslicer"));
        assert!(sources.contains(&"prusaslicer"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
