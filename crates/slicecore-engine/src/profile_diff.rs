//! Profile diff engine for comparing two [`PrintConfig`] instances.
//!
//! Serializes configs to JSON, flattens nested objects to dotted keys,
//! compares every field, and enriches each difference with metadata from
//! the global [`setting_registry`].

use std::collections::BTreeMap;

use serde::Serialize;
use slicecore_config_schema::types::{SettingCategory, SettingKey, Tier};

use crate::config::PrintConfig;
use crate::setting_registry;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single entry comparing one setting key across two configs.
#[derive(Debug, Clone, Serialize)]
pub struct DiffEntry {
    /// Dotted setting key (e.g. `"layer_height"`).
    pub key: String,
    /// Human-readable display name (from registry, or raw key as fallback).
    pub display_name: String,
    /// Setting category from the registry, if known.
    pub category: Option<SettingCategory>,
    /// Progressive disclosure tier from the registry, if known.
    pub tier: Option<Tier>,
    /// Value in the left (base) config.
    pub left_value: serde_json::Value,
    /// Value in the right (comparison) config.
    pub right_value: serde_json::Value,
    /// Whether left and right values differ.
    pub changed: bool,
    /// Unit string for display (e.g. `"mm"`, `"mm/s"`).
    pub units: Option<String>,
    /// Keys of settings affected by this one.
    pub affects: Vec<SettingKey>,
    /// Description of what this setting controls.
    pub description: String,
}

/// Result of comparing two configs, with all entries and summary statistics.
#[derive(Debug, Clone, Serialize)]
pub struct DiffResult {
    /// Name/label for the left config.
    pub left_name: String,
    /// Name/label for the right config.
    pub right_name: String,
    /// All entries (both changed and unchanged).
    pub entries: Vec<DiffEntry>,
    /// Count of entries where `changed == true`.
    pub total_differences: usize,
    /// Per-category counts of changed entries.
    pub category_counts: BTreeMap<String, usize>,
}

// ---------------------------------------------------------------------------
// Core functions
// ---------------------------------------------------------------------------

/// Recursively flattens a JSON object into dotted-key entries.
///
/// Non-object values are inserted at the current prefix. Objects recurse
/// with `prefix.key` (or just `key` when prefix is empty).
fn flatten_value(
    prefix: &str,
    value: &serde_json::Value,
    out: &mut BTreeMap<String, serde_json::Value>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let full_key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_value(&full_key, v, out);
            }
        }
        _ => {
            out.insert(prefix.to_owned(), value.clone());
        }
    }
}

/// Compares two [`PrintConfig`] instances field-by-field.
///
/// Returns a [`DiffResult`] containing an entry for **every** key found in
/// either config. Each entry has a `changed` flag indicating whether the
/// values differ, enabling downstream filtering (e.g. `--all` mode).
///
/// # Examples
///
/// ```
/// use slicecore_engine::config::PrintConfig;
/// use slicecore_engine::profile_diff::diff_configs;
///
/// let a = PrintConfig::default();
/// let b = PrintConfig::default();
/// let result = diff_configs(&a, &b, "base", "compare");
/// assert_eq!(result.total_differences, 0);
/// ```
pub fn diff_configs(
    left: &PrintConfig,
    right: &PrintConfig,
    left_name: &str,
    right_name: &str,
) -> DiffResult {
    let left_json = serde_json::to_value(left).unwrap_or_default();
    let right_json = serde_json::to_value(right).unwrap_or_default();

    let mut left_flat = BTreeMap::new();
    let mut right_flat = BTreeMap::new();
    flatten_value("", &left_json, &mut left_flat);
    flatten_value("", &right_json, &mut right_flat);

    // Union of all keys from both sides
    let mut all_keys: Vec<String> = left_flat.keys().chain(right_flat.keys()).cloned().collect();
    all_keys.sort();
    all_keys.dedup();

    let null = serde_json::Value::Null;

    let mut entries: Vec<DiffEntry> = all_keys
        .into_iter()
        .map(|key| {
            let lv = left_flat.get(&key).unwrap_or(&null);
            let rv = right_flat.get(&key).unwrap_or(&null);
            DiffEntry {
                key: key.clone(),
                display_name: key.clone(),
                category: None,
                tier: None,
                left_value: lv.clone(),
                right_value: rv.clone(),
                changed: lv != rv,
                units: None,
                affects: Vec::new(),
                description: String::new(),
            }
        })
        .collect();

    // Enrich each entry from the registry
    for entry in &mut entries {
        enrich_entry(entry);
    }

    // Compute category counts (changed only)
    let mut category_counts = BTreeMap::new();
    for entry in &entries {
        if entry.changed {
            let cat_name = entry
                .category
                .map_or_else(|| "uncategorized".to_owned(), |c| c.as_str().to_owned());
            *category_counts.entry(cat_name).or_insert(0) += 1;
        }
    }

    let total_differences = entries.iter().filter(|e| e.changed).count();

    DiffResult {
        left_name: left_name.to_owned(),
        right_name: right_name.to_owned(),
        entries,
        total_differences,
        category_counts,
    }
}

/// Enriches a [`DiffEntry`] with metadata from the global setting registry.
///
/// If the key is found, populates display name, category, tier, units,
/// affects, and description. If not found, display name stays as the raw key.
fn enrich_entry(entry: &mut DiffEntry) {
    let registry = setting_registry();
    if let Some(def) = registry.get_by_str(&entry.key) {
        entry.display_name = def.display_name.clone();
        entry.category = Some(def.category);
        entry.tier = Some(def.tier);
        entry.units = def.units.clone();
        entry.affects = def.affects.clone();
        entry.description = def.description.clone();
    }
}

/// Formats a JSON value for human-readable display.
///
/// Numbers are formatted with optional units, strings are unquoted,
/// arrays become comma-separated lists, and null becomes `"(none)"`.
///
/// # Examples
///
/// ```
/// use slicecore_engine::profile_diff::format_value;
///
/// let v = serde_json::json!(45.0);
/// assert_eq!(format_value(&v, &Some("mm/s".to_owned())), "45 mm/s");
/// ```
#[must_use]
pub fn format_value(value: &serde_json::Value, units: &Option<String>) -> String {
    match value {
        serde_json::Value::Number(n) => {
            let num_str = if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(f) = n.as_f64() {
                // Strip trailing zeros for cleaner display
                let s = format!("{f}");
                s
            } else {
                n.to_string()
            };
            match units {
                Some(u) => format!("{num_str} {u}"),
                None => num_str,
            }
        }
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .map(|v| format_value(v, &None))
            .collect::<Vec<_>>()
            .join(", "),
        serde_json::Value::Null => "(none)".to_owned(),
        serde_json::Value::Object(_) => {
            serde_json::to_string(value).unwrap_or_else(|_| "(object)".to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PrintConfig;

    #[test]
    fn identical_configs_have_zero_differences() {
        let a = PrintConfig::default();
        let b = PrintConfig::default();
        let result = diff_configs(&a, &b, "base", "compare");
        assert_eq!(result.total_differences, 0);
        assert!(result.entries.iter().all(|e| !e.changed));
    }

    #[test]
    fn modified_field_detected_as_changed() {
        let a = PrintConfig::default();
        let mut b = PrintConfig::default();
        b.layer_height = 0.3;
        let result = diff_configs(&a, &b, "base", "compare");
        assert_eq!(result.total_differences, 1);

        let changed: Vec<_> = result.entries.iter().filter(|e| e.changed).collect();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].key, "layer_height");
        assert_ne!(changed[0].left_value, changed[0].right_value);
    }

    #[test]
    fn registry_enrichment_populates_metadata() {
        let a = PrintConfig::default();
        let b = PrintConfig::default();
        let result = diff_configs(&a, &b, "base", "compare");

        let lh = result.entries.iter().find(|e| e.key == "layer_height");
        assert!(lh.is_some(), "layer_height entry must exist");
        let lh = lh.unwrap();

        assert!(!lh.display_name.is_empty());
        assert!(lh.category.is_some());
        assert!(lh.tier.is_some());
        // layer_height has units = "mm"
        assert_eq!(lh.units.as_deref(), Some("mm"));
    }

    #[test]
    fn category_counts_reflect_changed_entries_only() {
        let a = PrintConfig::default();
        let mut b = PrintConfig::default();
        b.layer_height = 0.3;
        b.first_layer_height = 0.35;
        let result = diff_configs(&a, &b, "base", "compare");

        // Both are Quality category
        let total_counted: usize = result.category_counts.values().sum();
        assert_eq!(total_counted, result.total_differences);
        assert_eq!(result.total_differences, 2);
    }

    #[test]
    fn flatten_nested_objects() {
        let val = serde_json::json!({"speed": {"travel": 150, "perimeter": 60}});
        let mut out = BTreeMap::new();
        flatten_value("", &val, &mut out);

        assert!(out.contains_key("speed.travel"));
        assert!(out.contains_key("speed.perimeter"));
        assert_eq!(out["speed.travel"], serde_json::json!(150));
    }

    #[test]
    fn format_value_with_units() {
        let v = serde_json::json!(45.0);
        assert_eq!(format_value(&v, &Some("mm/s".to_owned())), "45 mm/s");

        let v2 = serde_json::json!(45.0);
        assert_eq!(format_value(&v2, &None), "45");

        let v3 = serde_json::json!("PLA");
        assert_eq!(format_value(&v3, &None), "PLA");

        let v4 = serde_json::json!(true);
        assert_eq!(format_value(&v4, &None), "true");

        let v5 = serde_json::json!([1, 2, 3]);
        assert_eq!(format_value(&v5, &None), "1, 2, 3");

        let v6 = serde_json::Value::Null;
        assert_eq!(format_value(&v6, &None), "(none)");
    }

    #[test]
    fn unknown_keys_handled_gracefully() {
        // Create a DiffEntry with a key not in the registry
        let mut entry = DiffEntry {
            key: "totally_fake_setting_xyz_999".to_owned(),
            display_name: "totally_fake_setting_xyz_999".to_owned(),
            category: None,
            tier: None,
            left_value: serde_json::Value::Null,
            right_value: serde_json::Value::Null,
            changed: false,
            units: None,
            affects: Vec::new(),
            description: String::new(),
        };
        enrich_entry(&mut entry);

        // Should keep raw key as display name and leave category/tier as None
        assert_eq!(entry.display_name, "totally_fake_setting_xyz_999");
        assert!(entry.category.is_none());
        assert!(entry.tier.is_none());
    }

    #[test]
    fn all_entries_returned_for_identical_configs() {
        let a = PrintConfig::default();
        let b = PrintConfig::default();
        let result = diff_configs(&a, &b, "base", "compare");

        // There should be many entries (all config fields) but zero differences
        assert!(
            result.entries.len() > result.total_differences,
            "entries ({}) should be > total_differences ({})",
            result.entries.len(),
            result.total_differences
        );
        assert_eq!(result.total_differences, 0);
        // Expect at least 10 entries (PrintConfig has many fields)
        assert!(
            result.entries.len() >= 10,
            "expected many entries, got {}",
            result.entries.len()
        );
    }
}
