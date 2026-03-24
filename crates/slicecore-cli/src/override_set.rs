//! Override set CRUD CLI subcommands.
//!
//! Provides the `slicecore override-set` command group with subcommands for
//! creating, listing, showing, editing, deleting, renaming, and diffing named
//! override sets. Override sets are stored as TOML files in
//! `~/.slicecore/override-sets/`.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use clap::Subcommand;
use slicecore_engine::profile_compose::{parse_set_value, validate_set_key};

/// Override set management subcommands.
#[derive(Subcommand)]
pub enum OverrideSetCommands {
    /// List all override sets with field counts.
    List {
        /// Output as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Show an override set with metadata.
    Show {
        /// Override set name.
        name: String,
        /// Output as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Create a new override set.
    Create {
        /// Name for the new override set.
        name: String,
        /// Set key=value pairs (repeatable).
        #[arg(long = "set")]
        set_overrides: Vec<String>,
        /// Output as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Open an override set in $EDITOR.
    Edit {
        /// Override set name.
        name: String,
    },
    /// Delete an override set.
    Delete {
        /// Override set name.
        name: String,
        /// Skip confirmation prompt.
        #[arg(long)]
        force: bool,
    },
    /// Rename an override set.
    Rename {
        /// Current name.
        old: String,
        /// New name.
        new: String,
    },
    /// Compare two override sets side-by-side.
    Diff {
        /// First override set name.
        set_a: String,
        /// Second override set name.
        set_b: String,
        /// Output as JSON.
        #[arg(long)]
        json: bool,
    },
}

/// Resolve the override-sets storage directory, creating it if absent.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined or the
/// directory cannot be created.
fn override_sets_dir() -> Result<PathBuf, anyhow::Error> {
    let home =
        home::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    let dir = home.join(".slicecore").join("override-sets");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Path for a named override set.
fn set_path(name: &str) -> Result<PathBuf, anyhow::Error> {
    let dir = override_sets_dir()?;
    Ok(dir.join(format!("{name}.toml")))
}

/// Load a named override set from disk.
///
/// # Errors
///
/// Returns an error if the file does not exist or cannot be parsed.
fn load_set(name: &str) -> Result<toml::map::Map<String, toml::Value>, anyhow::Error> {
    let path = set_path(name)?;
    if !path.exists() {
        let available = list_sets()?;
        let suggestion = fuzzy_suggest(name, &available);
        let mut msg = format!("Override set '{name}' not found.");
        if let Some(s) = suggestion {
            msg.push_str(&format!(" Did you mean '{s}'?"));
        }
        anyhow::bail!("{msg}");
    }
    let content = fs::read_to_string(&path)?;
    let table: toml::map::Map<String, toml::Value> = toml::from_str(&content)?;
    Ok(table)
}

/// Save a named override set to disk.
fn save_set(name: &str, table: &toml::map::Map<String, toml::Value>) -> Result<(), anyhow::Error> {
    let path = set_path(name)?;
    let content = toml::to_string_pretty(&toml::Value::Table(table.clone()))?;
    fs::write(&path, content)?;
    Ok(())
}

/// List all override set names in the storage directory.
fn list_sets() -> Result<Vec<String>, anyhow::Error> {
    let dir = override_sets_dir()?;
    let mut names = Vec::new();
    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "toml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
    }
    names.sort();
    Ok(names)
}

/// Simple fuzzy suggestion: find the closest match by substring or prefix.
fn fuzzy_suggest(name: &str, available: &[String]) -> Option<String> {
    let lower = name.to_lowercase();
    // Exact prefix match first
    if let Some(m) = available
        .iter()
        .find(|a| a.to_lowercase().starts_with(&lower))
    {
        return Some(m.clone());
    }
    // Substring match
    if let Some(m) = available.iter().find(|a| a.to_lowercase().contains(&lower)) {
        return Some(m.clone());
    }
    // Reverse substring
    available
        .iter()
        .find(|a| lower.contains(&a.to_lowercase()))
        .cloned()
}

/// Flatten a TOML table into dotted key paths for display.
fn flatten_table(
    table: &toml::map::Map<String, toml::Value>,
    prefix: &str,
) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for (key, value) in table {
        let full_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match value {
            toml::Value::Table(sub) => {
                result.extend(flatten_table(sub, &full_key));
            }
            other => {
                result.push((full_key, format_value(other)));
            }
        }
    }
    result
}

/// Format a TOML value for display.
fn format_value(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => format!("\"{s}\""),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => format!("{f}"),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Array(a) => {
            let items: Vec<String> = a.iter().map(format_value).collect();
            format!("[{}]", items.join(", "))
        }
        toml::Value::Table(_) => "<table>".to_string(),
        toml::Value::Datetime(d) => d.to_string(),
    }
}

/// Execute an override-set subcommand.
///
/// # Errors
///
/// Returns an error if any filesystem or validation operation fails.
#[allow(clippy::too_many_lines)]
pub fn run_override_set(cmd: OverrideSetCommands) -> Result<(), anyhow::Error> {
    match cmd {
        OverrideSetCommands::List { json } => {
            let names = list_sets()?;
            if json {
                let entries: Vec<serde_json::Value> = names
                    .iter()
                    .map(|name| {
                        let count = load_set(name)
                            .map(|t| flatten_table(&t, "").len())
                            .unwrap_or(0);
                        serde_json::json!({ "name": name, "field_count": count })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else if names.is_empty() {
                println!("No override sets found.");
                println!("Create one with: slicecore override-set create <name> --set key=value");
            } else {
                println!("{:<30} Fields", "Name");
                println!("{}", "-".repeat(40));
                for name in &names {
                    let count = load_set(name)
                        .map(|t| flatten_table(&t, "").len())
                        .unwrap_or(0);
                    println!("{name:<30} {count}");
                }
            }
        }

        OverrideSetCommands::Show { name, json } => {
            let table = load_set(&name)?;
            let fields = flatten_table(&table, "");
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&toml::Value::Table(table))?
                );
            } else {
                println!("Override set: {name}");
                println!("{}", "-".repeat(50));
                if fields.is_empty() {
                    println!("(empty)");
                } else {
                    for (key, value) in &fields {
                        println!("  {key:<35} = {value}");
                    }
                }
            }
        }

        OverrideSetCommands::Create {
            name,
            set_overrides,
            json,
        } => {
            let path = set_path(&name)?;
            if path.exists() {
                anyhow::bail!("Override set '{name}' already exists. Use 'edit' to modify it.");
            }

            let mut table = toml::map::Map::new();
            for kv in &set_overrides {
                let (key, raw_value) = kv
                    .split_once('=')
                    .ok_or_else(|| anyhow::anyhow!("Invalid key=value format: '{kv}'"))?;
                validate_set_key(key)?;
                let value = parse_set_value(raw_value);
                slicecore_engine::profile_compose::set_dotted_key(&mut table, key, value)?;
            }

            save_set(&name, &table)?;

            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "created": name,
                        "fields": flatten_table(&table, "").len(),
                    }))?
                );
            } else {
                let count = flatten_table(&table, "").len();
                println!("Created override set '{name}' with {count} field(s).");
            }
        }

        OverrideSetCommands::Edit { name } => {
            let path = set_path(&name)?;
            if !path.exists() {
                anyhow::bail!("Override set '{name}' not found.");
            }

            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
            let status = std::process::Command::new(&editor).arg(&path).status()?;
            if !status.success() {
                anyhow::bail!("Editor exited with non-zero status.");
            }

            // Re-validate after edit
            let content = fs::read_to_string(&path)?;
            let table: toml::map::Map<String, toml::Value> = toml::from_str(&content)
                .map_err(|e| anyhow::anyhow!("Invalid TOML after edit: {e}"))?;
            let fields = flatten_table(&table, "");
            let mut errors = Vec::new();
            for (key, _) in &fields {
                if let Err(e) = validate_set_key(key) {
                    errors.push(format!("  {key}: {e}"));
                }
            }
            if errors.is_empty() {
                println!("Override set '{name}' saved ({} fields).", fields.len());
            } else {
                eprintln!("Warning: some field names may be invalid:");
                for err in &errors {
                    eprintln!("{err}");
                }
                eprintln!("The file was saved but may not work correctly.");
            }
        }

        OverrideSetCommands::Delete { name, force } => {
            let path = set_path(&name)?;
            if !path.exists() {
                anyhow::bail!("Override set '{name}' not found.");
            }

            if !force {
                eprint!("Delete override set '{name}'? [y/N] ");
                let mut response = String::new();
                std::io::stdin().read_line(&mut response)?;
                if !response.trim().eq_ignore_ascii_case("y") {
                    println!("Cancelled.");
                    return Ok(());
                }
            }

            fs::remove_file(&path)?;
            println!("Deleted override set '{name}'.");
        }

        OverrideSetCommands::Rename { old, new } => {
            let old_path = set_path(&old)?;
            let new_path = set_path(&new)?;

            if !old_path.exists() {
                anyhow::bail!("Override set '{old}' not found.");
            }
            if new_path.exists() {
                anyhow::bail!("Override set '{new}' already exists.");
            }

            fs::rename(&old_path, &new_path)?;
            println!("Renamed override set '{old}' -> '{new}'.");
        }

        OverrideSetCommands::Diff { set_a, set_b, json } => {
            let table_a = load_set(&set_a)?;
            let table_b = load_set(&set_b)?;
            let fields_a = flatten_table(&table_a, "");
            let fields_b = flatten_table(&table_b, "");

            let map_a: BTreeMap<String, String> = fields_a.into_iter().collect();
            let map_b: BTreeMap<String, String> = fields_b.into_iter().collect();

            let mut all_keys: Vec<String> = map_a.keys().chain(map_b.keys()).cloned().collect();
            all_keys.sort();
            all_keys.dedup();

            if json {
                let mut only_a = Vec::new();
                let mut only_b = Vec::new();
                let mut different = Vec::new();
                let mut same = Vec::new();

                for key in &all_keys {
                    match (map_a.get(key), map_b.get(key)) {
                        (Some(va), None) => {
                            only_a.push(serde_json::json!({ "key": key, "value": va }))
                        }
                        (None, Some(vb)) => {
                            only_b.push(serde_json::json!({ "key": key, "value": vb }))
                        }
                        (Some(va), Some(vb)) if va != vb => {
                            different.push(serde_json::json!({ "key": key, "a": va, "b": vb }));
                        }
                        (Some(v), Some(_)) => {
                            same.push(serde_json::json!({ "key": key, "value": v }))
                        }
                        (None, None) => {}
                    }
                }

                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "set_a": set_a,
                        "set_b": set_b,
                        "only_in_a": only_a,
                        "only_in_b": only_b,
                        "different": different,
                        "same": same,
                    }))?
                );
            } else {
                println!("Diff: {set_a} vs {set_b}");
                println!("{}", "-".repeat(70));

                let mut has_diff = false;
                for key in &all_keys {
                    match (map_a.get(key), map_b.get(key)) {
                        (Some(va), None) => {
                            println!("  - {key:<35} = {va}  (only in {set_a})");
                            has_diff = true;
                        }
                        (None, Some(vb)) => {
                            println!("  + {key:<35} = {vb}  (only in {set_b})");
                            has_diff = true;
                        }
                        (Some(va), Some(vb)) if va != vb => {
                            println!("  ~ {key:<35} = {va} -> {vb}");
                            has_diff = true;
                        }
                        _ => {}
                    }
                }

                if !has_diff {
                    println!("  No differences found.");
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create override sets in a temp directory and override the home.
    fn with_temp_sets_dir<F: FnOnce(PathBuf)>(f: F) {
        let tmp = tempfile::TempDir::new().unwrap();
        let sets_dir = tmp.path().join(".slicecore").join("override-sets");
        fs::create_dir_all(&sets_dir).unwrap();

        // We test the low-level helpers by operating on paths directly
        f(sets_dir);
    }

    #[test]
    fn override_set_create_load_roundtrip() {
        with_temp_sets_dir(|dir| {
            let mut table = toml::map::Map::new();
            table.insert("layer_height".to_string(), toml::Value::Float(0.1));
            table.insert("wall_count".to_string(), toml::Value::Integer(4));

            let path = dir.join("test-set.toml");
            let content = toml::to_string_pretty(&toml::Value::Table(table.clone())).unwrap();
            fs::write(&path, &content).unwrap();

            // Read back
            let read_content = fs::read_to_string(&path).unwrap();
            let loaded: toml::map::Map<String, toml::Value> =
                toml::from_str(&read_content).unwrap();
            assert_eq!(loaded["layer_height"].as_float(), Some(0.1));
            assert_eq!(loaded["wall_count"].as_integer(), Some(4));
        });
    }

    #[test]
    fn override_set_rename_works() {
        with_temp_sets_dir(|dir| {
            let old_path = dir.join("old-name.toml");
            let new_path = dir.join("new-name.toml");

            let mut table = toml::map::Map::new();
            table.insert("layer_height".to_string(), toml::Value::Float(0.2));
            let content = toml::to_string_pretty(&toml::Value::Table(table)).unwrap();
            fs::write(&old_path, &content).unwrap();

            assert!(old_path.exists());
            assert!(!new_path.exists());

            fs::rename(&old_path, &new_path).unwrap();

            assert!(!old_path.exists());
            assert!(new_path.exists());

            let loaded: toml::map::Map<String, toml::Value> =
                toml::from_str(&fs::read_to_string(&new_path).unwrap()).unwrap();
            assert_eq!(loaded["layer_height"].as_float(), Some(0.2));
        });
    }

    #[test]
    fn override_set_diff_shows_differences() {
        let mut table_a = toml::map::Map::new();
        table_a.insert("layer_height".to_string(), toml::Value::Float(0.1));
        table_a.insert("wall_count".to_string(), toml::Value::Integer(4));

        let mut table_b = toml::map::Map::new();
        table_b.insert("layer_height".to_string(), toml::Value::Float(0.2));
        table_b.insert("infill_density".to_string(), toml::Value::Float(0.5));

        let fields_a = flatten_table(&table_a, "");
        let fields_b = flatten_table(&table_b, "");

        let map_a: BTreeMap<String, String> = fields_a.into_iter().collect();
        let map_b: BTreeMap<String, String> = fields_b.into_iter().collect();

        // layer_height differs
        assert_ne!(map_a.get("layer_height"), map_b.get("layer_height"));
        // wall_count only in A
        assert!(map_a.contains_key("wall_count"));
        assert!(!map_b.contains_key("wall_count"));
        // infill_density only in B
        assert!(!map_a.contains_key("infill_density"));
        assert!(map_b.contains_key("infill_density"));
    }

    #[test]
    fn flatten_table_handles_nested() {
        let mut inner = toml::map::Map::new();
        inner.insert("perimeter".to_string(), toml::Value::Float(60.0));
        let mut table = toml::map::Map::new();
        table.insert("speeds".to_string(), toml::Value::Table(inner));
        table.insert("layer_height".to_string(), toml::Value::Float(0.2));

        let fields = flatten_table(&table, "");
        let map: BTreeMap<String, String> = fields.into_iter().collect();
        assert_eq!(map.get("speeds.perimeter"), Some(&"60".to_string()));
        assert_eq!(map.get("layer_height"), Some(&"0.2".to_string()));
    }

    #[test]
    fn fuzzy_suggest_finds_prefix_match() {
        let available = vec!["high-detail".to_string(), "fast-draft".to_string()];
        assert_eq!(
            fuzzy_suggest("high", &available),
            Some("high-detail".to_string())
        );
    }

    #[test]
    fn fuzzy_suggest_finds_substring_match() {
        let available = vec!["high-detail".to_string(), "fast-draft".to_string()];
        assert_eq!(
            fuzzy_suggest("draft", &available),
            Some("fast-draft".to_string())
        );
    }

    #[test]
    fn validate_set_key_rejects_invalid() {
        let result = validate_set_key("not_a_real_setting_xyz_abc");
        assert!(result.is_err());
    }

    #[test]
    fn validate_set_key_accepts_layer_height() {
        let result = validate_set_key("layer_height");
        assert!(result.is_ok());
    }
}
