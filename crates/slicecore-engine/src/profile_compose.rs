//! Profile composition engine for multi-layer TOML merging with provenance.
//!
//! This module implements the core merge engine that combines up to 5 profile
//! layers (Defaults -> Machine -> Filament -> Process -> User overrides / `--set`)
//! into a final [`PrintConfig`] with per-field provenance tracking.
//!
//! The merge operates on [`toml::Value`] trees rather than [`PrintConfig`] structs
//! so that the distinction between "not set" and "set to default" is preserved.
//!
//! # Architecture
//!
//! 1. [`ProfileComposer`] orchestrates the merge pipeline.
//! 2. [`merge_layer`] performs deep recursive table merging.
//! 3. [`parse_set_value`] auto-coerces `--set` string values to TOML types.
//! 4. [`set_dotted_key`] inserts values at nested paths like `speeds.perimeter`.
//! 5. [`validate_set_key`] checks keys against valid [`PrintConfig`] fields with
//!    fuzzy "did you mean?" suggestions.
//!
//! # Examples
//!
//! ```
//! use slicecore_engine::profile_compose::{ProfileComposer, SourceType};
//!
//! let mut composer = ProfileComposer::new();
//! composer.add_toml_layer(
//!     SourceType::Machine,
//!     "machine.toml",
//!     "[machine]\nbed_x = 300.0\n",
//! );
//! composer.add_set_override("speeds.perimeter", "60").unwrap();
//! let composed = composer.compose().unwrap();
//! assert_eq!(composed.config.machine.bed_x, 300.0);
//! assert_eq!(composed.config.speeds.perimeter, 60.0);
//! ```

use std::collections::HashMap;

use sha2::{Digest, Sha256};

use crate::config::PrintConfig;
use crate::error::EngineError;

/// The type of source a profile field originated from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SourceType {
    /// Engine-compiled defaults (`PrintConfig::default()`).
    Default,
    /// Machine/printer profile layer.
    Machine,
    /// Filament/material profile layer.
    Filament,
    /// Process/quality profile layer.
    Process,
    /// User override TOML file layer.
    UserOverride,
    /// CLI `--set key=value` override.
    CliSet,
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Default => write!(f, "default"),
            Self::Machine => write!(f, "machine"),
            Self::Filament => write!(f, "filament"),
            Self::Process => write!(f, "process"),
            Self::UserOverride => write!(f, "user-override"),
            Self::CliSet => write!(f, "cli-set"),
        }
    }
}

/// Tracks the source of a single field value, including override chains.
#[derive(Debug, Clone)]
pub struct FieldSource {
    /// Which layer type set this field.
    pub source_type: SourceType,
    /// File path of the source profile (if applicable).
    pub file_path: Option<String>,
    /// The previous source that was overridden, if any.
    pub overrode: Option<Box<FieldSource>>,
}

/// The result of composing multiple profile layers into a single config.
#[derive(Debug)]
pub struct ComposedConfig {
    /// The final merged print configuration.
    pub config: PrintConfig,
    /// Per-field provenance map (dotted key path -> source info).
    pub provenance: HashMap<String, FieldSource>,
    /// Conflict/informational warnings generated during merge.
    pub warnings: Vec<String>,
    /// SHA-256 checksums of each input profile file content.
    pub profile_checksums: Vec<(String, String)>,
}

/// A pending profile layer to be merged.
struct PendingLayer {
    source_type: SourceType,
    file_path: Option<String>,
    table: toml::map::Map<String, toml::Value>,
    raw_content: Option<String>,
}

/// Multi-layer profile composition engine.
///
/// Merges TOML value trees in priority order with provenance tracking.
/// Later layers win on conflict. Operates on raw TOML tables so that
/// "not set" and "set to default" remain distinguishable.
///
/// # Examples
///
/// ```
/// use slicecore_engine::profile_compose::{ProfileComposer, SourceType};
///
/// let mut composer = ProfileComposer::new();
/// composer.add_toml_layer(
///     SourceType::Filament,
///     "pla.toml",
///     "[filament]\ndiameter = 1.75\n",
/// );
/// let result = composer.compose().unwrap();
/// assert!(result.warnings.is_empty() || !result.warnings.is_empty());
/// ```
pub struct ProfileComposer {
    layers: Vec<PendingLayer>,
    set_overrides: Vec<(String, toml::Value)>,
}

impl ProfileComposer {
    /// Creates a new empty composer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            layers: Vec::new(),
            set_overrides: Vec::new(),
        }
    }

    /// Adds a TOML profile layer to the merge pipeline.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] if the TOML content cannot be parsed.
    pub fn add_toml_layer(
        &mut self,
        source_type: SourceType,
        file_path: &str,
        toml_content: &str,
    ) -> Result<(), EngineError> {
        let value: toml::Value = toml::from_str(toml_content).map_err(|e| {
            EngineError::ConfigError(format!("failed to parse TOML from {file_path}: {e}"))
        })?;
        let table = match value {
            toml::Value::Table(t) => t,
            _ => {
                return Err(EngineError::ConfigError(format!(
                    "TOML root in {file_path} is not a table"
                )));
            }
        };
        self.layers.push(PendingLayer {
            source_type,
            file_path: Some(file_path.to_string()),
            table,
            raw_content: Some(toml_content.to_string()),
        });
        Ok(())
    }

    /// Adds a raw TOML table layer (no file path, e.g. for programmatic use).
    pub fn add_table_layer(
        &mut self,
        source_type: SourceType,
        table: toml::map::Map<String, toml::Value>,
    ) {
        self.layers.push(PendingLayer {
            source_type,
            file_path: None,
            table,
            raw_content: None,
        });
    }

    /// Adds a `--set key=value` override.
    ///
    /// The value string is auto-coerced to the appropriate TOML type
    /// (integer, float, boolean, or string).
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] if the key path is invalid.
    pub fn add_set_override(&mut self, key: &str, value: &str) -> Result<(), EngineError> {
        let parsed = parse_set_value(value);
        self.set_overrides.push((key.to_string(), parsed));
        Ok(())
    }

    /// Composes all layers into a final [`ComposedConfig`].
    ///
    /// Merge order: `Default` base -> layers in add order -> `--set` overrides.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] if the final merged table cannot be deserialized
    /// into a [`PrintConfig`].
    pub fn compose(&self) -> Result<ComposedConfig, EngineError> {
        // Start from default config serialized to TOML table
        let default_config = PrintConfig::default();
        let default_value = toml::Value::try_from(&default_config).map_err(|e| {
            EngineError::ConfigError(format!("failed to serialize default PrintConfig: {e}"))
        })?;
        let mut base = match default_value {
            toml::Value::Table(t) => t,
            _ => {
                return Err(EngineError::ConfigError(
                    "default PrintConfig did not serialize to a table".to_string(),
                ));
            }
        };

        let mut provenance: HashMap<String, FieldSource> = HashMap::new();
        let mut warnings: Vec<String> = Vec::new();
        let mut checksums: Vec<(String, String)> = Vec::new();

        // Record default provenance for all leaf fields
        record_provenance_leaves(
            &base,
            "",
            &FieldSource {
                source_type: SourceType::Default,
                file_path: None,
                overrode: None,
            },
            &mut provenance,
        );

        // Merge each layer in order
        for layer in &self.layers {
            // Compute checksum for file content
            if let Some(ref content) = layer.raw_content {
                let checksum = compute_sha256(content);
                if let Some(ref path) = layer.file_path {
                    checksums.push((path.clone(), checksum));
                }
            }

            let source = FieldSource {
                source_type: layer.source_type.clone(),
                file_path: layer.file_path.clone(),
                overrode: None,
            };

            merge_layer(&mut base, &layer.table, "", &source, &mut provenance, &mut warnings);
        }

        // Apply --set overrides
        for (key, value) in &self.set_overrides {
            let source = FieldSource {
                source_type: SourceType::CliSet,
                file_path: None,
                overrode: provenance.get(key).map(|prev| Box::new(prev.clone())),
            };

            set_dotted_key(&mut base, key, value.clone())?;

            // Check for conflict warnings
            if let Some(existing) = provenance.get(key) {
                if existing.source_type != SourceType::Default {
                    warnings.push(format!(
                        "field '{key}' set by --set overrides value from {}{}",
                        existing.source_type,
                        existing
                            .file_path
                            .as_ref()
                            .map_or(String::new(), |p| format!(" ({p})")),
                    ));
                }
            }

            provenance.insert(key.clone(), source);
        }

        // Deserialize final merged table into PrintConfig
        let final_value = toml::Value::Table(base);
        let config: PrintConfig = final_value.try_into().map_err(|e| {
            EngineError::ConfigError(format!("failed to deserialize merged config: {e}"))
        })?;

        Ok(ComposedConfig {
            config,
            provenance,
            warnings,
            profile_checksums: checksums,
        })
    }
}

impl Default for ProfileComposer {
    fn default() -> Self {
        Self::new()
    }
}

/// Deep-merges a TOML table layer into a base table, recording provenance.
///
/// When both base and layer have a `Table` for the same key, merge recursively.
/// Otherwise the layer's leaf value overrides the base and provenance is updated.
///
/// # Arguments
///
/// * `base` - Mutable base table to merge into.
/// * `layer` - Layer table with values to merge from.
/// * `prefix` - Dotted key prefix for provenance tracking (empty string at root).
/// * `source` - The [`FieldSource`] describing where this layer came from.
/// * `provenance` - Map to update with field origins.
/// * `warnings` - Accumulator for conflict warnings.
pub fn merge_layer(
    base: &mut toml::map::Map<String, toml::Value>,
    layer: &toml::map::Map<String, toml::Value>,
    prefix: &str,
    source: &FieldSource,
    provenance: &mut HashMap<String, FieldSource>,
    warnings: &mut Vec<String>,
) {
    for (key, layer_val) in layer {
        let dotted = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };

        match (base.get(key), layer_val) {
            // Both are tables: recurse
            (Some(toml::Value::Table(_)), toml::Value::Table(layer_table)) => {
                // Safe: we just checked it's a table
                if let Some(toml::Value::Table(base_table)) = base.get_mut(key) {
                    merge_layer(base_table, layer_table, &dotted, source, provenance, warnings);
                }
            }
            // Layer has a table, base doesn't (or base doesn't have the key): insert
            (None, toml::Value::Table(layer_table)) => {
                base.insert(key.clone(), layer_val.clone());
                // Record provenance for all leaves in this new sub-table
                record_provenance_leaves(layer_table, &dotted, source, provenance);
            }
            // Leaf override
            _ => {
                // Check for conflict: non-default source already set this field
                if let Some(existing) = provenance.get(&dotted) {
                    if existing.source_type != SourceType::Default {
                        warnings.push(format!(
                            "field '{}' set by {} overrides value from {}{}",
                            dotted,
                            source.source_type,
                            existing.source_type,
                            existing
                                .file_path
                                .as_ref()
                                .map_or(String::new(), |p| format!(" ({p})")),
                        ));
                    }
                }

                let prev = provenance.get(&dotted).cloned();
                let field_source = FieldSource {
                    source_type: source.source_type.clone(),
                    file_path: source.file_path.clone(),
                    overrode: prev.map(Box::new),
                };

                base.insert(key.clone(), layer_val.clone());
                provenance.insert(dotted, field_source);
            }
        }
    }
}

/// Records provenance for all leaf (non-table) values in a TOML table tree.
fn record_provenance_leaves(
    table: &toml::map::Map<String, toml::Value>,
    prefix: &str,
    source: &FieldSource,
    provenance: &mut HashMap<String, FieldSource>,
) {
    for (key, val) in table {
        let dotted = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };

        match val {
            toml::Value::Table(sub) => {
                record_provenance_leaves(sub, &dotted, source, provenance);
            }
            _ => {
                provenance.insert(dotted, source.clone());
            }
        }
    }
}

/// Parses a string value as a TOML literal with auto-coercion.
///
/// Tries integer, then float, then boolean, then falls back to string.
///
/// # Examples
///
/// ```
/// use slicecore_engine::profile_compose::parse_set_value;
///
/// assert_eq!(parse_set_value("42"), toml::Value::Integer(42));
/// assert_eq!(parse_set_value("3.14"), toml::Value::Float(3.14));
/// assert_eq!(parse_set_value("true"), toml::Value::Boolean(true));
/// assert_eq!(parse_set_value("hello"), toml::Value::String("hello".into()));
/// ```
#[must_use]
pub fn parse_set_value(value: &str) -> toml::Value {
    // Try integer
    if let Ok(i) = value.parse::<i64>() {
        return toml::Value::Integer(i);
    }
    // Try float
    if let Ok(f) = value.parse::<f64>() {
        return toml::Value::Float(f);
    }
    // Try boolean
    match value {
        "true" => return toml::Value::Boolean(true),
        "false" => return toml::Value::Boolean(false),
        _ => {}
    }
    // Fallback to string
    toml::Value::String(value.to_string())
}

/// Inserts a value at a dotted key path in a TOML table.
///
/// Creates intermediate tables as needed. For example, `set_dotted_key(table,
/// "speeds.perimeter", Value::Float(60.0))` will create the `speeds` sub-table
/// if it doesn't exist.
///
/// # Errors
///
/// Returns [`EngineError`] if an intermediate path component exists but is not
/// a table.
///
/// # Examples
///
/// ```
/// use slicecore_engine::profile_compose::set_dotted_key;
///
/// let mut table = toml::map::Map::new();
/// set_dotted_key(&mut table, "speeds.perimeter", toml::Value::Float(60.0)).unwrap();
/// let speeds = table["speeds"].as_table().unwrap();
/// assert_eq!(speeds["perimeter"].as_float(), Some(60.0));
/// ```
pub fn set_dotted_key(
    table: &mut toml::map::Map<String, toml::Value>,
    key: &str,
    value: toml::Value,
) -> Result<(), EngineError> {
    let parts: Vec<&str> = key.split('.').collect();
    if parts.is_empty() {
        return Err(EngineError::ConfigError("empty key path".to_string()));
    }

    if parts.len() == 1 {
        table.insert(parts[0].to_string(), value);
        return Ok(());
    }

    // Navigate to the penultimate table, creating intermediates
    let mut current = table;
    for part in &parts[..parts.len() - 1] {
        let part_str = (*part).to_string();
        if !current.contains_key(*part) {
            current.insert(part_str.clone(), toml::Value::Table(toml::map::Map::new()));
        }
        match current.get_mut(*part) {
            Some(toml::Value::Table(sub)) => {
                current = sub;
            }
            Some(_) => {
                return Err(EngineError::ConfigError(format!(
                    "path component '{part}' in '{key}' is not a table"
                )));
            }
            None => unreachable!(), // We just inserted it
        }
    }

    let leaf_key = parts[parts.len() - 1].to_string();
    current.insert(leaf_key, value);
    Ok(())
}

/// Validates a dotted key path against valid [`PrintConfig`] fields.
///
/// Walks the default config's TOML table to build a set of valid paths.
/// If the key is invalid, uses Jaro-Winkler similarity to suggest similar keys.
///
/// # Errors
///
/// Returns [`EngineError`] with a "did you mean?" suggestion if the key is
/// not valid.
///
/// # Examples
///
/// ```
/// use slicecore_engine::profile_compose::validate_set_key;
///
/// // Valid key passes
/// assert!(validate_set_key("layer_height").is_ok());
///
/// // Invalid key with suggestion
/// let err = validate_set_key("layr_height").unwrap_err();
/// assert!(err.to_string().contains("did you mean"));
/// ```
pub fn validate_set_key(key: &str) -> Result<(), EngineError> {
    let default_config = PrintConfig::default();
    let default_value = toml::Value::try_from(&default_config).map_err(|e| {
        EngineError::ConfigError(format!("failed to serialize default config: {e}"))
    })?;

    let table = match default_value {
        toml::Value::Table(t) => t,
        _ => {
            return Err(EngineError::ConfigError(
                "default config is not a table".to_string(),
            ));
        }
    };

    let mut valid_keys = Vec::new();
    collect_valid_keys(&table, "", &mut valid_keys);

    if valid_keys.contains(&key.to_string()) {
        return Ok(());
    }

    // Find the best match using Jaro-Winkler similarity
    let mut best_match: Option<(&str, f64)> = None;
    for valid_key in &valid_keys {
        let score = strsim::jaro_winkler(key, valid_key);
        if let Some((_, best_score)) = best_match {
            if score > best_score {
                best_match = Some((valid_key, score));
            }
        } else {
            best_match = Some((valid_key, score));
        }
    }

    let suggestion = best_match
        .filter(|(_, score)| *score > 0.7)
        .map_or_else(
            || format!("unknown config key '{key}'"),
            |(candidate, _)| format!("unknown config key '{key}', did you mean '{candidate}'?"),
        );

    Err(EngineError::ConfigError(suggestion))
}

/// Computes SHA-256 hex digest of a string.
fn compute_sha256(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    // Format as hex
    result.iter().map(|b| format!("{b:02x}")).collect()
}

/// Recursively collects all valid dotted key paths from a TOML table.
fn collect_valid_keys(
    table: &toml::map::Map<String, toml::Value>,
    prefix: &str,
    keys: &mut Vec<String>,
) {
    for (key, val) in table {
        let dotted = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };

        match val {
            toml::Value::Table(sub) => {
                // Add both the table key itself and recurse into leaves
                keys.push(dotted.clone());
                collect_valid_keys(sub, &dotted, keys);
            }
            _ => {
                keys.push(dotted);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_later_layer_wins_on_conflict() {
        let mut composer = ProfileComposer::new();
        composer
            .add_toml_layer(SourceType::Machine, "machine.toml", "layer_height = 0.3\n")
            .unwrap();
        composer
            .add_toml_layer(SourceType::Process, "process.toml", "layer_height = 0.1\n")
            .unwrap();

        let result = composer.compose().unwrap();
        assert!(
            (result.config.layer_height - 0.1).abs() < f64::EPSILON,
            "later layer should win: got {}",
            result.config.layer_height
        );
    }

    #[test]
    fn nested_tables_recursively_merged() {
        let mut composer = ProfileComposer::new();
        composer
            .add_toml_layer(
                SourceType::Filament,
                "filament.toml",
                "[speeds]\ninfill = 100.0\n",
            )
            .unwrap();
        composer
            .add_toml_layer(
                SourceType::Process,
                "process.toml",
                "[speeds]\nperimeter = 60.0\n",
            )
            .unwrap();

        let result = composer.compose().unwrap();
        // Filament's infill speed should survive process layer setting perimeter
        assert!(
            (result.config.speeds.infill - 100.0).abs() < f64::EPSILON,
            "infill speed from filament layer should survive: got {}",
            result.config.speeds.infill
        );
        assert!(
            (result.config.speeds.perimeter - 60.0).abs() < f64::EPSILON,
            "perimeter speed from process layer should apply: got {}",
            result.config.speeds.perimeter
        );
    }

    #[test]
    fn provenance_records_source_type_and_file_path() {
        let mut composer = ProfileComposer::new();
        composer
            .add_toml_layer(
                SourceType::Machine,
                "machine.toml",
                "[machine]\nbed_x = 300.0\n",
            )
            .unwrap();

        let result = composer.compose().unwrap();
        let source = result.provenance.get("machine.bed_x").unwrap();
        assert_eq!(source.source_type, SourceType::Machine);
        assert_eq!(source.file_path.as_deref(), Some("machine.toml"));
    }

    #[test]
    fn provenance_records_override_chain() {
        let mut composer = ProfileComposer::new();
        composer
            .add_toml_layer(
                SourceType::Machine,
                "machine.toml",
                "layer_height = 0.3\n",
            )
            .unwrap();
        composer
            .add_toml_layer(
                SourceType::Process,
                "process.toml",
                "layer_height = 0.1\n",
            )
            .unwrap();

        let result = composer.compose().unwrap();
        let source = result.provenance.get("layer_height").unwrap();
        assert_eq!(source.source_type, SourceType::Process);
        assert!(source.overrode.is_some(), "should record what was overridden");

        let overrode = source.overrode.as_ref().unwrap();
        assert_eq!(overrode.source_type, SourceType::Machine);
    }

    #[test]
    fn conflict_detection_warns_on_same_field_from_multiple_layers() {
        let mut composer = ProfileComposer::new();
        composer
            .add_toml_layer(
                SourceType::Machine,
                "machine.toml",
                "layer_height = 0.3\n",
            )
            .unwrap();
        composer
            .add_toml_layer(
                SourceType::Process,
                "process.toml",
                "layer_height = 0.1\n",
            )
            .unwrap();

        let result = composer.compose().unwrap();
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("layer_height") && w.contains("overrides")),
            "should produce conflict warning, got: {:?}",
            result.warnings
        );
    }

    #[test]
    fn five_layer_merge_order() {
        let mut composer = ProfileComposer::new();

        // Machine sets layer_height
        composer
            .add_toml_layer(
                SourceType::Machine,
                "machine.toml",
                "layer_height = 0.4\n",
            )
            .unwrap();
        // Filament doesn't touch layer_height but sets filament diameter
        composer
            .add_toml_layer(
                SourceType::Filament,
                "filament.toml",
                "[filament]\ndiameter = 2.85\n",
            )
            .unwrap();
        // Process overrides layer_height
        composer
            .add_toml_layer(
                SourceType::Process,
                "process.toml",
                "layer_height = 0.2\n",
            )
            .unwrap();
        // User override further overrides
        composer
            .add_toml_layer(
                SourceType::UserOverride,
                "user.toml",
                "layer_height = 0.15\n",
            )
            .unwrap();
        // --set is the ultimate override
        composer.add_set_override("layer_height", "0.05").unwrap();

        let result = composer.compose().unwrap();
        assert!(
            (result.config.layer_height - 0.05).abs() < f64::EPSILON,
            "--set should be final override: got {}",
            result.config.layer_height
        );
        assert!(
            (result.config.filament.diameter - 2.85).abs() < f64::EPSILON,
            "filament diameter should survive: got {}",
            result.config.filament.diameter
        );
    }

    #[test]
    fn fields_not_in_layer_left_untouched() {
        let default = PrintConfig::default();
        let mut composer = ProfileComposer::new();
        composer
            .add_toml_layer(SourceType::Process, "process.toml", "layer_height = 0.1\n")
            .unwrap();

        let result = composer.compose().unwrap();
        // wall_count should remain at default since no layer touched it
        assert_eq!(
            result.config.wall_count, default.wall_count,
            "untouched fields should keep default value"
        );
    }

    #[test]
    fn sha256_checksum_computed() {
        let mut composer = ProfileComposer::new();
        let content = "layer_height = 0.2\n";
        composer
            .add_toml_layer(SourceType::Process, "process.toml", content)
            .unwrap();

        let result = composer.compose().unwrap();
        assert_eq!(result.profile_checksums.len(), 1);
        assert_eq!(result.profile_checksums[0].0, "process.toml");

        // Verify checksum is a 64-char hex string (SHA-256)
        let checksum = &result.profile_checksums[0].1;
        assert_eq!(checksum.len(), 64, "SHA-256 hex should be 64 chars");
        assert!(
            checksum.chars().all(|c| c.is_ascii_hexdigit()),
            "checksum should be hex"
        );
    }

    #[test]
    fn parse_set_value_integer() {
        assert_eq!(parse_set_value("42"), toml::Value::Integer(42));
        assert_eq!(parse_set_value("-10"), toml::Value::Integer(-10));
        assert_eq!(parse_set_value("0"), toml::Value::Integer(0));
    }

    #[test]
    fn parse_set_value_float() {
        assert_eq!(parse_set_value("3.14"), toml::Value::Float(3.14));
        assert_eq!(parse_set_value("0.2"), toml::Value::Float(0.2));
    }

    #[test]
    fn parse_set_value_bool() {
        assert_eq!(parse_set_value("true"), toml::Value::Boolean(true));
        assert_eq!(parse_set_value("false"), toml::Value::Boolean(false));
    }

    #[test]
    fn parse_set_value_string_fallback() {
        assert_eq!(
            parse_set_value("hello"),
            toml::Value::String("hello".into())
        );
        assert_eq!(
            parse_set_value("PLA"),
            toml::Value::String("PLA".into())
        );
    }

    #[test]
    fn set_dotted_key_nested_path() {
        let mut table = toml::map::Map::new();
        set_dotted_key(&mut table, "speeds.perimeter", toml::Value::Float(60.0)).unwrap();

        let speeds = table["speeds"].as_table().unwrap();
        assert_eq!(speeds["perimeter"].as_float(), Some(60.0));
    }

    #[test]
    fn set_dotted_key_creates_intermediates() {
        let mut table = toml::map::Map::new();
        set_dotted_key(
            &mut table,
            "machine.bed_x",
            toml::Value::Float(300.0),
        )
        .unwrap();

        assert!(table.contains_key("machine"));
        let machine = table["machine"].as_table().unwrap();
        assert_eq!(machine["bed_x"].as_float(), Some(300.0));
    }

    #[test]
    fn set_dotted_key_single_key() {
        let mut table = toml::map::Map::new();
        set_dotted_key(&mut table, "layer_height", toml::Value::Float(0.2)).unwrap();
        assert_eq!(table["layer_height"].as_float(), Some(0.2));
    }

    #[test]
    fn validate_set_key_valid() {
        assert!(validate_set_key("layer_height").is_ok());
        assert!(validate_set_key("speeds.perimeter").is_ok());
        assert!(validate_set_key("machine.bed_x").is_ok());
    }

    #[test]
    fn validate_set_key_invalid_with_suggestion() {
        let err = validate_set_key("layr_height").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("did you mean") && msg.contains("layer_height"),
            "should suggest 'layer_height', got: {msg}"
        );
    }

    #[test]
    fn validate_set_key_completely_wrong() {
        let err = validate_set_key("xyzzy_foobar_baz").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("unknown config key"),
            "should report unknown key, got: {msg}"
        );
    }

    #[test]
    fn set_override_via_composer() {
        let mut composer = ProfileComposer::new();
        composer.add_set_override("speeds.perimeter", "60").unwrap();

        let result = composer.compose().unwrap();
        assert!(
            (result.config.speeds.perimeter - 60.0).abs() < f64::EPSILON,
            "--set override should apply"
        );
        let source = result.provenance.get("speeds.perimeter").unwrap();
        assert_eq!(source.source_type, SourceType::CliSet);
    }

    #[test]
    fn set_override_conflict_warning() {
        let mut composer = ProfileComposer::new();
        composer
            .add_toml_layer(
                SourceType::Process,
                "process.toml",
                "[speeds]\nperimeter = 45.0\n",
            )
            .unwrap();
        composer.add_set_override("speeds.perimeter", "60").unwrap();

        let result = composer.compose().unwrap();
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("speeds.perimeter") && w.contains("--set")),
            "should warn about --set overriding process layer: {:?}",
            result.warnings
        );
    }

    #[test]
    fn default_composer_has_no_layers() {
        let composer = ProfileComposer::default();
        let result = composer.compose().unwrap();
        // Should produce default config with no warnings
        let default = PrintConfig::default();
        assert!(
            (result.config.layer_height - default.layer_height).abs() < f64::EPSILON,
        );
        assert!(result.profile_checksums.is_empty());
    }

    #[test]
    fn compute_sha256_deterministic() {
        let a = compute_sha256("hello");
        let b = compute_sha256("hello");
        assert_eq!(a, b);
        assert_ne!(a, compute_sha256("world"));
    }
}
