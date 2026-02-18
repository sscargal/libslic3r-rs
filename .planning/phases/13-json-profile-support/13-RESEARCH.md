# Phase 13: JSON Profile Support - Research

**Researched:** 2026-02-18
**Domain:** Slicer profile import (JSON/TOML format detection, field mapping, upstream compatibility)
**Confidence:** HIGH

## Summary

This phase adds the ability to load printer/filament profiles from OrcaSlicer and BambuStudio JSON format files, in addition to the existing TOML support. The upstream JSON format is shared between OrcaSlicer (9,533 profile files) and BambuStudio (3,005 profile files) -- they use the identical schema (BambuStudio is the upstream fork). PrusaSlicer uses INI format (36 vendor bundles); INI support is deferred as the user scope specifies JSON.

The critical design challenge is that OrcaSlicer/BambuStudio profiles use a **fundamentally different schema** from our PrintConfig: values are stored as strings (often in single-element arrays), field names differ substantially, profiles use an inheritance chain (`"inherits": "parent_name"`), and profiles are split across three types (filament, machine, process) rather than unified into a single config struct. The implementation requires a translation layer -- not direct serde deserialization -- to convert upstream profile fields into PrintConfig values.

Format auto-detection is straightforward: TOML files never start with `{`, JSON files always do. A content-sniffing approach (check first non-whitespace character) is reliable and does not depend on file extensions.

**Primary recommendation:** Build a `ProfileImporter` module in slicecore-engine that deserializes upstream JSON into `serde_json::Value`, extracts and maps known fields to PrintConfig, and handles the string-to-number conversion quirk. Do NOT attempt to model the full 716-field OrcaSlicer schema -- map only the ~32 fields that correspond to existing PrintConfig fields, and log/report unmapped fields for user awareness.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde_json | 1.x (workspace) | JSON deserialization | Already in workspace, standard Rust JSON |
| serde | 1.x (workspace) | Serialization framework | Already used everywhere in codebase |
| toml | 0.8 (workspace) | TOML deserialization | Already used for config loading |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| thiserror | 2.x (workspace) | Error types | Profile import error variants |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| serde_json::Value (dynamic) | Typed structs for upstream schema | Dynamic is better: upstream schema has 716 fields, we only need ~32. Typed structs would be enormous maintenance burden for negligible benefit. |
| Content sniffing for format detection | File extension check | Extension check is fragile (users rename files). Content sniffing is reliable: JSON starts with `{`, TOML never does. Use both: extension as hint, content as authority. |

**Installation:**
No new dependencies needed. All libraries are already in the workspace.

## Architecture Patterns

### Recommended Module Structure
```
crates/slicecore-engine/src/
├── config.rs               # Existing: PrintConfig, from_toml, from_toml_file
├── profile_import.rs       # NEW: ProfileImporter, format detection, JSON loading
├── profile_import/
│   ├── mod.rs              # Format detection, ConfigFormat enum, load_config_file
│   ├── orca_json.rs        # OrcaSlicer/BambuStudio JSON -> PrintConfig mapping
│   └── field_map.rs        # Field name mapping tables and value conversion
└── ...
```

Alternative (simpler, recommended for initial implementation):
```
crates/slicecore-engine/src/
├── config.rs               # Existing + new from_json, from_file (auto-detect)
├── profile_import.rs       # NEW: single module with all import logic
└── ...
```

### Pattern 1: Content-Based Format Detection
**What:** Detect config file format by inspecting content, not extension
**When to use:** Loading any config file (CLI --config flag)
**Example:**
```rust
/// Detected configuration file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Toml,
    Json,
}

/// Detect config format from file content.
///
/// JSON files start with `{` (after optional whitespace/BOM).
/// TOML files never start with `{`.
pub fn detect_config_format(data: &[u8]) -> Result<ConfigFormat, EngineError> {
    // Skip UTF-8 BOM if present
    let data = if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &data[3..]
    } else {
        data
    };

    // Find first non-whitespace byte
    for &byte in data {
        match byte {
            b' ' | b'\t' | b'\n' | b'\r' => continue,
            b'{' => return Ok(ConfigFormat::Json),
            _ => return Ok(ConfigFormat::Toml),
        }
    }

    // Empty file defaults to TOML (empty TOML = all defaults)
    Ok(ConfigFormat::Toml)
}
```

### Pattern 2: Dynamic JSON Field Extraction with Mapping Table
**What:** Use serde_json::Value for upstream profiles, extract known fields via mapping table
**When to use:** Converting OrcaSlicer/BambuStudio JSON to PrintConfig
**Example:**
```rust
use serde_json::Value;

/// Result of importing an upstream slicer profile.
pub struct ImportResult {
    /// The mapped PrintConfig (partial -- unmapped fields use defaults).
    pub config: PrintConfig,
    /// Fields from the source that were successfully mapped.
    pub mapped_fields: Vec<String>,
    /// Fields from the source that have no PrintConfig equivalent.
    pub unmapped_fields: Vec<String>,
    /// Source profile metadata (name, type, inherits).
    pub metadata: ProfileMetadata,
}

pub struct ProfileMetadata {
    pub name: String,
    pub profile_type: String,  // "filament", "machine", "process"
    pub inherits: Option<String>,
    pub source_format: String,  // "orcaslicer", "bambustudio"
}

/// Extract a string value from OrcaSlicer JSON.
///
/// OrcaSlicer stores most values as either:
/// - Plain strings: `"layer_height": "0.2"`
/// - Single-element arrays of strings: `"nozzle_temperature": ["200"]`
/// - Multi-element arrays (multi-extruder): `"retraction_length": ["3", "3"]`
/// - Sentinel "nil" for "inherit from parent"
fn extract_string_value(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if s == "nil" => None,
        Value::String(s) => Some(s.clone()),
        Value::Array(arr) if !arr.is_empty() => {
            match &arr[0] {
                Value::String(s) if s == "nil" => None,
                Value::String(s) => Some(s.clone()),
                _ => None,
            }
        }
        _ => None,
    }
}

fn extract_f64(value: &Value) -> Option<f64> {
    extract_string_value(value)?.parse::<f64>().ok()
}
```

### Pattern 3: Field Mapping Table (Data-Driven)
**What:** Define field mappings as data, not code, for maintainability
**When to use:** Mapping upstream field names to PrintConfig fields
**Example:**
```rust
/// Mapping from an upstream JSON field to a PrintConfig field.
struct FieldMapping {
    /// Field name in the upstream JSON (OrcaSlicer/BambuStudio key).
    upstream_key: &'static str,
    /// Which profile type this field appears in.
    profile_type: ProfileType,
    /// Function to apply the extracted value to PrintConfig.
    apply: fn(&mut PrintConfig, &str) -> bool,
}

enum ProfileType {
    Process,   // Print settings (layer height, speeds, etc.)
    Filament,  // Material settings (temps, density, flow)
    Machine,   // Printer settings (bed size, retraction, nozzle)
}

// Example mapping entries:
const PROCESS_MAPPINGS: &[(&str, fn(&mut PrintConfig, &str) -> bool)] = &[
    ("layer_height", |c, v| { c.layer_height = v.parse().ok()?; Some(()) }.is_some()),
    ("initial_layer_print_height", |c, v| { c.first_layer_height = v.parse().ok()?; Some(()) }.is_some()),
    ("wall_loops", |c, v| { c.wall_count = v.parse().ok()?; Some(()) }.is_some()),
    ("sparse_infill_density", |c, v| {
        // OrcaSlicer uses percentage string like "20%" or plain "20"
        let cleaned = v.trim_end_matches('%');
        if let Ok(pct) = cleaned.parse::<f64>() {
            c.infill_density = pct / 100.0;
            true
        } else {
            false
        }
    }),
    ("outer_wall_speed", |c, v| { c.perimeter_speed = v.parse().ok()?; Some(()) }.is_some()),
    ("sparse_infill_speed", |c, v| { c.infill_speed = v.parse().ok()?; Some(()) }.is_some()),
    ("travel_speed", |c, v| { c.travel_speed = v.parse().ok()?; Some(()) }.is_some()),
    ("initial_layer_speed", |c, v| { c.first_layer_speed = v.parse().ok()?; Some(()) }.is_some()),
    // ... more mappings
];
```

### Anti-Patterns to Avoid
- **Full upstream schema modeling:** Do NOT create Rust structs mirroring all 716 OrcaSlicer settings. Map only what PrintConfig uses (~32 fields). The rest should be reported as unmapped.
- **Inheritance chain resolution:** Do NOT implement the `"inherits"` chain resolution. Profiles should be loaded as flat files. If a user wants the full resolved profile, they should export it from OrcaSlicer first. Document this limitation clearly.
- **Extension-only format detection:** Do NOT rely solely on `.json` vs `.toml` file extensions. Users rename files. Content sniffing is the authority.
- **Lossy silent conversion:** Do NOT silently drop fields. Report every unmapped field so users know what was and wasn't imported.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON parsing | Custom JSON parser | serde_json (already in workspace) | Battle-tested, handles all edge cases |
| String-to-number conversion | Custom float parser | str::parse::<f64>() | Handles all float formats correctly |
| Percentage handling | Regex-based parsing | Simple trim_end_matches('%') + parse | OrcaSlicer percentages are always `"N%"` format |
| Config format sniffing | Complex heuristics | First-non-whitespace-byte check | JSON always starts with `{`, TOML never does |

**Key insight:** The complexity is in the field mapping, not the parsing. serde_json handles all JSON parsing; the real work is mapping OrcaSlicer's 716 field names to our 55 PrintConfig fields.

## Common Pitfalls

### Pitfall 1: Values Are Strings, Not Numbers
**What goes wrong:** Attempting `serde_json::from_str::<PrintConfig>(json)` will fail because OrcaSlicer stores numeric values as strings (`"0.2"` not `0.2`).
**Why it happens:** OrcaSlicer's C++ config system stores all values as strings internally. The JSON export preserves this.
**How to avoid:** Always extract as string first, then parse to the target type. Never attempt direct serde deserialization into PrintConfig from upstream JSON.
**Warning signs:** "expected f64, found string" serde errors.

### Pitfall 2: Array vs Scalar Inconsistency
**What goes wrong:** Some fields are plain strings (`"layer_height": "0.2"`), others are single-element arrays (`"nozzle_temperature": ["200"]`), and multi-extruder fields are multi-element arrays (`"retraction_length": ["3", "3"]`).
**Why it happens:** In OrcaSlicer's C++ code, `coFloat` fields serialize as strings, `coFloats`/`coInts`/`coStrings` serialize as arrays. The JSON format preserves this type distinction.
**How to avoid:** The extraction function must handle both `Value::String` and `Value::Array` transparently. For multi-element arrays, take the first element (index 0) for the primary extruder.
**Warning signs:** "expected string, found array" or vice versa.

### Pitfall 3: The "nil" Sentinel
**What goes wrong:** Fields with value `"nil"` or `["nil"]` mean "inherit from parent profile." Treating "nil" as a real value will cause parse errors.
**Why it happens:** OrcaSlicer's inheritance system uses "nil" to mark fields that should be resolved from the parent profile.
**How to avoid:** Check for "nil" before any parsing. When "nil" is found, skip the field (let PrintConfig's default apply).
**Warning signs:** "invalid float literal" errors on `"nil".parse::<f64>()`.

### Pitfall 4: Percentage Strings
**What goes wrong:** Some values include a `%` suffix (`"infill_wall_overlap": "25%"`, `"overhang_fan_threshold": "95%"`, `"ironing_flow": "10%"`). Parsing these as numbers fails.
**Why it happens:** OrcaSlicer has a `coFloatOrPercent` type that serializes as `"25%"` when the value is percentage-based.
**How to avoid:** Strip the `%` suffix before parsing. Decide per-field whether the percentage needs conversion (e.g., 25% -> 0.25 vs keeping as 25.0).
**Warning signs:** "invalid digit found in string" parse errors.

### Pitfall 5: Field Name Mapping Is Not 1:1
**What goes wrong:** Assuming OrcaSlicer field names match PrintConfig field names.
**Why it happens:** Different naming conventions. OrcaSlicer uses `wall_loops`, we use `wall_count`. OrcaSlicer uses `sparse_infill_density`, we use `infill_density`. OrcaSlicer splits settings across 3 profile types (process/filament/machine), we combine them into one struct.
**How to avoid:** Use an explicit mapping table. Never assume names match.
**Warning signs:** All fields showing up as "unmapped" despite having equivalents.

### Pitfall 6: Bed Temperature Is Per-Plate-Type in OrcaSlicer
**What goes wrong:** There is no single `bed_temp` field. OrcaSlicer has `cool_plate_temp`, `eng_plate_temp`, `hot_plate_temp` (6 fields total with initial layer variants).
**Why it happens:** OrcaSlicer supports multiple bed plate types (Cool Plate, Engineering Plate, High Temp Plate, etc.).
**How to avoid:** Default to `hot_plate_temp` for `bed_temp` and `hot_plate_temp_initial_layer` for `first_layer_bed_temp`, as high-temp plates are the most common for general use. Document this mapping choice.
**Warning signs:** Missing bed temperature in imported configs.

## Code Examples

### Config Format Detection and Unified Loading
```rust
// Source: Verified against existing codebase patterns in config.rs and detect.rs

impl PrintConfig {
    /// Load a PrintConfig from a file, auto-detecting format (TOML or JSON).
    ///
    /// Format detection uses content sniffing:
    /// - JSON files start with `{` (after optional whitespace/BOM)
    /// - Everything else is treated as TOML
    ///
    /// For JSON files, this supports both:
    /// - **Native JSON**: Direct serde_json deserialization (same field names as TOML)
    /// - **OrcaSlicer/BambuStudio JSON**: Upstream profile format with field mapping
    ///
    /// The format is distinguished by checking for the `"type"` field that all
    /// upstream profiles contain.
    pub fn from_file(path: &std::path::Path) -> Result<Self, EngineError> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| EngineError::ConfigIo(path.to_path_buf(), e))?;

        match detect_config_format(contents.as_bytes())? {
            ConfigFormat::Toml => Self::from_toml(&contents)
                .map_err(EngineError::ConfigParse),
            ConfigFormat::Json => Self::from_json(&contents),
        }
    }

    /// Parse a PrintConfig from a JSON string.
    ///
    /// Supports two JSON variants:
    /// 1. Native format (field names match PrintConfig) -- direct deserialization
    /// 2. OrcaSlicer/BambuStudio format (detected by "type" field) -- mapped import
    pub fn from_json(json_str: &str) -> Result<Self, EngineError> {
        // First, try to detect if this is an upstream slicer profile
        let value: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| EngineError::ConfigError(format!("JSON parse error: {}", e)))?;

        if value.get("type").is_some() {
            // Upstream slicer profile -- use mapping import
            import_upstream_profile(&value)
        } else {
            // Native JSON format -- direct deserialization
            serde_json::from_str(json_str)
                .map_err(|e| EngineError::ConfigError(format!("JSON config error: {}", e)))
        }
    }
}
```

### Upstream Profile Field Extraction
```rust
// Source: Derived from analysis of actual OrcaSlicer profile files

/// Import a single upstream slicer profile JSON into PrintConfig.
///
/// Returns a PrintConfig with defaults for any unmapped fields.
/// The `type` field determines which mapping table to use:
/// - "process" -> print settings (speeds, layers, infill)
/// - "filament" -> material settings (temps, density, flow)
/// - "machine" -> printer settings (bed size, retraction, nozzle)
fn import_upstream_profile(value: &serde_json::Value) -> Result<PrintConfig, EngineError> {
    let mut config = PrintConfig::default();
    let profile_type = value.get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let obj = value.as_object()
        .ok_or_else(|| EngineError::ConfigError("JSON profile must be an object".into()))?;

    for (key, val) in obj {
        // Skip metadata fields
        if matches!(key.as_str(), "type" | "name" | "inherits" | "from"
            | "setting_id" | "instantiation" | "compatible_printers"
            | "compatible_printers_condition" | "filament_id" | "description") {
            continue;
        }

        // Extract string value (handles both scalar and array forms)
        let string_val = match extract_string_value(val) {
            Some(s) => s,
            None => continue, // "nil" or unextractable
        };

        // Apply mapping based on profile type and field name
        apply_field_mapping(&mut config, profile_type, key, &string_val);
    }

    Ok(config)
}
```

### Test with Actual Upstream Profiles
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_orcaslicer_filament_profile() {
        let json = r#"{
            "type": "filament",
            "name": "Generic PLA",
            "from": "system",
            "instantiation": "true",
            "nozzle_temperature": ["220"],
            "nozzle_temperature_initial_layer": ["225"],
            "filament_density": ["1.24"],
            "filament_diameter": ["1.75"],
            "filament_flow_ratio": ["0.98"],
            "filament_cost": ["20"],
            "close_fan_the_first_x_layers": ["1"],
            "hot_plate_temp": ["55"],
            "hot_plate_temp_initial_layer": ["60"]
        }"#;

        let config = PrintConfig::from_json(json).unwrap();
        assert!((config.nozzle_temp - 220.0).abs() < 1e-9);
        assert!((config.first_layer_nozzle_temp - 225.0).abs() < 1e-9);
        assert!((config.filament_density - 1.24).abs() < 1e-9);
        assert!((config.filament_diameter - 1.75).abs() < 1e-9);
        assert!((config.extrusion_multiplier - 0.98).abs() < 1e-9);
        assert_eq!(config.disable_fan_first_layers, 1);
        assert!((config.bed_temp - 55.0).abs() < 1e-9);
        assert!((config.first_layer_bed_temp - 60.0).abs() < 1e-9);
    }

    #[test]
    fn import_orcaslicer_process_profile() {
        let json = r#"{
            "type": "process",
            "name": "0.20mm Standard",
            "from": "system",
            "layer_height": "0.2",
            "initial_layer_print_height": "0.28",
            "wall_loops": "2",
            "sparse_infill_density": "15%",
            "sparse_infill_pattern": "grid",
            "outer_wall_speed": "200",
            "sparse_infill_speed": "270",
            "travel_speed": "500",
            "initial_layer_speed": "50",
            "skirt_loops": "1",
            "skirt_distance": "2",
            "brim_width": "0",
            "seam_position": "aligned",
            "default_acceleration": "10000",
            "travel_acceleration": "12000"
        }"#;

        let config = PrintConfig::from_json(json).unwrap();
        assert!((config.layer_height - 0.2).abs() < 1e-9);
        assert!((config.first_layer_height - 0.28).abs() < 1e-9);
        assert_eq!(config.wall_count, 2);
        assert!((config.infill_density - 0.15).abs() < 1e-9);  // 15% -> 0.15
        assert!((config.perimeter_speed - 200.0).abs() < 1e-9);
        assert!((config.travel_speed - 500.0).abs() < 1e-9);
    }

    #[test]
    fn native_json_format_works() {
        // Our own JSON format (same field names as TOML)
        let json = r#"{
            "layer_height": 0.2,
            "nozzle_diameter": 0.4,
            "wall_count": 3,
            "infill_density": 0.2
        }"#;

        let config = PrintConfig::from_json(json).unwrap();
        assert!((config.layer_height - 0.2).abs() < 1e-9);
        assert_eq!(config.wall_count, 3);
    }

    #[test]
    fn format_detection_json() {
        assert_eq!(detect_config_format(b"{}"), Ok(ConfigFormat::Json));
        assert_eq!(detect_config_format(b"  {\"type\": \"filament\"}"), Ok(ConfigFormat::Json));
        assert_eq!(detect_config_format(b"\xEF\xBB\xBF{\"key\": 1}"), Ok(ConfigFormat::Json));
    }

    #[test]
    fn format_detection_toml() {
        assert_eq!(detect_config_format(b"layer_height = 0.2"), Ok(ConfigFormat::Toml));
        assert_eq!(detect_config_format(b"# comment\nlayer_height = 0.2"), Ok(ConfigFormat::Toml));
        assert_eq!(detect_config_format(b""), Ok(ConfigFormat::Toml));
    }
}
```

## Upstream Profile Format Analysis

### OrcaSlicer/BambuStudio JSON Schema (Verified from actual files)

**Profile counts (verified from filesystem):**
| Source | Filament | Machine | Process | Total |
|--------|----------|---------|---------|-------|
| OrcaSlicer | 5,129 | 1,379 | 2,961 | 9,533* |
| BambuStudio | 1,935 | 386 | 684 | 3,005 |

*Includes all vendor subdirectories (BBL, FLSun, Anycubic, etc.)

**Key structural observations:**

1. **Three profile types:** `filament`, `machine`, `process` -- identified by `"type"` field
2. **Inheritance chain:** `"inherits": "parent_name"` points to another profile in the same vendor directory
3. **Values are strings:** Numeric values stored as `"0.2"` not `0.2`
4. **Array wrapping:** Multi-extruder fields wrapped in arrays: `["200", "200"]`
5. **"nil" sentinel:** Means "inherit from parent" -- not a real value
6. **Percentage strings:** Some values include `%` suffix: `"25%"`, `"95%"`
7. **Metadata fields:** `type`, `name`, `from`, `setting_id`, `instantiation`, `inherits`, `compatible_printers`
8. **BambuStudio = same format:** Confirmed identical JSON schema to OrcaSlicer

### Field Mapping Reference (Process -> PrintConfig)

| OrcaSlicer Process Field | PrintConfig Field | Value Conversion |
|--------------------------|-------------------|------------------|
| `layer_height` | `layer_height` | parse f64 |
| `initial_layer_print_height` | `first_layer_height` | parse f64 |
| `wall_loops` | `wall_count` | parse u32 |
| `seam_position` | `seam_position` | enum mapping |
| `sparse_infill_pattern` | `infill_pattern` | enum mapping |
| `sparse_infill_density` | `infill_density` | strip %, /100.0 |
| `top_shell_layers` | `top_solid_layers` | parse u32 |
| `bottom_shell_layers` | `bottom_solid_layers` | parse u32 |
| `outer_wall_speed` | `perimeter_speed` | parse f64 |
| `sparse_infill_speed` | `infill_speed` | parse f64 |
| `travel_speed` | `travel_speed` | parse f64 |
| `initial_layer_speed` | `first_layer_speed` | parse f64 |
| `skirt_loops` | `skirt_loops` | parse u32 |
| `skirt_distance` | `skirt_distance` | parse f64 |
| `brim_width` | `brim_width` | parse f64 |
| `default_acceleration` | `print_acceleration` | parse f64 |
| `travel_acceleration` | `travel_acceleration` | parse f64 |
| `enable_arc_fitting` | `arc_fitting_enabled` | "1" -> true |
| `adaptive_layer_height` | `adaptive_layer_height` | "1" -> true |
| `ironing_type` | `ironing.enabled` | != "no ironing" |
| `ironing_flow` | `ironing.flow_rate` | strip %, /100.0 |
| `ironing_speed` | `ironing.speed` | parse f64 |
| `ironing_spacing` | `ironing.spacing` | parse f64 |
| `wall_generator` | `arachne_enabled` | "arachne" -> true |

### Field Mapping Reference (Filament -> PrintConfig)

| OrcaSlicer Filament Field | PrintConfig Field | Value Conversion |
|---------------------------|-------------------|------------------|
| `nozzle_temperature` | `nozzle_temp` | arr[0], parse f64 |
| `nozzle_temperature_initial_layer` | `first_layer_nozzle_temp` | arr[0], parse f64 |
| `hot_plate_temp` | `bed_temp` | arr[0], parse f64 |
| `hot_plate_temp_initial_layer` | `first_layer_bed_temp` | arr[0], parse f64 |
| `filament_density` | `filament_density` | arr[0], parse f64 |
| `filament_diameter` | `filament_diameter` | arr[0], parse f64 |
| `filament_cost` | `filament_cost_per_kg` | arr[0], parse f64 |
| `filament_flow_ratio` | `extrusion_multiplier` | arr[0], parse f64 |
| `close_fan_the_first_x_layers` | `disable_fan_first_layers` | arr[0], parse u32 |
| `fan_cooling_layer_time` | `fan_below_layer_time` | arr[0], parse f64 |

### Field Mapping Reference (Machine -> PrintConfig)

| OrcaSlicer Machine Field | PrintConfig Field | Value Conversion |
|--------------------------|-------------------|------------------|
| `nozzle_diameter` | `nozzle_diameter` | arr[0], parse f64 |
| `retraction_length` | `retract_length` | arr[0], parse f64 |
| `retraction_speed` | `retract_speed` | arr[0], parse f64 |
| `z_hop` | `retract_z_hop` | arr[0], parse f64 |
| `retraction_minimum_travel` | `min_travel_for_retract` | arr[0], parse f64 |
| `gcode_flavor` | `gcode_dialect` | enum mapping |
| `machine_max_jerk_x` | `jerk_x` | arr[0], parse f64 |
| `machine_max_jerk_y` | `jerk_y` | arr[0], parse f64 |
| `machine_max_jerk_z` | `jerk_z` | arr[0], parse f64 |

### Enum Mapping Reference

| OrcaSlicer Value | PrintConfig Enum |
|------------------|------------------|
| `"aligned"` | `SeamPosition::Aligned` |
| `"random"` | `SeamPosition::Random` |
| `"rear"` | `SeamPosition::Rear` |
| `"nearest"` | `SeamPosition::SmartHiding` |
| `"grid"` | `InfillPattern::Grid` |
| `"honeycomb"` | `InfillPattern::Honeycomb` |
| `"gyroid"` | `InfillPattern::Gyroid` |
| `"cubic"` | `InfillPattern::Cubic` |
| `"adaptivecubic"` | `InfillPattern::AdaptiveCubic` |
| `"lightning"` | `InfillPattern::Lightning` |
| `"zig-zag"`, `"rectilinear"` | `InfillPattern::Rectilinear` |
| `"monotonic"` | `InfillPattern::Monotonic` |
| `"marlin"` | `GcodeDialect::Marlin` |
| `"klipper"` | `GcodeDialect::Klipper` |
| `"reprapfirmware"` | `GcodeDialect::RepRapFirmware` |

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| TOML-only config loading | Auto-detect TOML/JSON, support upstream slicer profiles | Phase 13 | Users can import existing profiles |
| Single `from_toml_file()` | Unified `from_file()` with format detection | Phase 13 | CLI accepts any config format |

**Deprecated/outdated:**
- PrusaSlicer INI format: Still used by PrusaSlicer, but OrcaSlicer/BambuStudio (the dominant forks) have moved to JSON. INI support deferred.

## Open Questions

1. **Multiple Profile Merging**
   - What we know: OrcaSlicer uses 3 separate profiles (process + filament + machine) to build a complete config. Our PrintConfig is a single unified struct.
   - What's unclear: Should we support loading and merging 3 separate profile files in one operation?
   - Recommendation: For Phase 13, support loading individual profile files that map their known fields. A "merge 3 profiles" convenience function is a nice-to-have but not essential. Users can load a process profile and a filament profile separately and manually combine. Start simple, iterate.

2. **Inheritance Resolution**
   - What we know: Profiles chain via `"inherits"` (e.g., `"FLSun S1 PLA Generic"` -> `"FLSun Generic PLA"` -> `"fdm_filament_pla"` -> `"fdm_filament_common"`).
   - What's unclear: Should we resolve the full inheritance chain by loading parent profiles from disk?
   - Recommendation: No for Phase 13. Inheritance resolution requires knowing the vendor directory structure and loading multiple files. Instead, tell users to load the most-specific (instantiation=true) profile, which has the most fields overridden. Document this as a known limitation.

3. **Infill Pattern Enum Mapping Completeness**
   - What we know: OrcaSlicer has some infill patterns we support and some we don't (e.g., "3dhoneycomb", "concentric", "line", "hilbertcurve").
   - What's unclear: What to do when an unmapped pattern is encountered.
   - Recommendation: Fall back to `InfillPattern::Rectilinear` (our default) and report it as an unmapped field warning.

4. **Testing with Actual Upstream Files**
   - What we know: Thousands of profile files exist at `/home/steve/slicer-analysis/`.
   - What's unclear: How many should be integration tested? Licensing concerns for including them in the repo?
   - Recommendation: Create synthetic test profiles that mimic the upstream format (avoid licensing issues). Add a small number of integration tests that load actual upstream profiles from the slicer-analysis directory (marked with `#[ignore]` for CI, run manually).

## Sources

### Primary (HIGH confidence)
- **Actual OrcaSlicer profile files** - `/home/steve/slicer-analysis/OrcaSlicer/resources/profiles/` - Direct inspection of JSON structure, value types, field names, inheritance patterns
- **Actual BambuStudio profile files** - `/home/steve/slicer-analysis/BambuStudio/resources/profiles/` - Confirmed identical JSON schema to OrcaSlicer
- **Existing codebase** - `/home/steve/libslic3r-rs/crates/slicecore-engine/src/config.rs` - PrintConfig struct, field names, types, defaults, serde configuration
- **Existing format detection** - `/home/steve/libslic3r-rs/crates/slicecore-fileio/src/detect.rs` - Pattern for content-based format detection

### Secondary (MEDIUM confidence)
- **Slicer analysis docs** - `/home/steve/slicer-analysis/analysis/settings_orca.md` - 716 OrcaSlicer settings catalogued with types and categories
- **PrusaSlicer INI format** - `/home/steve/slicer-analysis/PrusaSlicer/resources/profiles/PrusaResearch.ini` - INI format structure (deferred, not in scope)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries already in workspace, no new dependencies needed
- Architecture: HIGH - Based on direct analysis of upstream profile files and existing codebase patterns
- Field mapping: HIGH - Derived from side-by-side comparison of actual upstream JSON files and PrintConfig fields
- Pitfalls: HIGH - Discovered from actual data inspection (string values, nil sentinels, array wrapping, percentage strings)

**Research date:** 2026-02-18
**Valid until:** 2026-03-18 (stable domain -- upstream JSON format has not changed in years)
