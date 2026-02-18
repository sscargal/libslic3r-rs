# Phase 14: Profile Conversion Tool (JSON to TOML) - Research

**Researched:** 2026-02-18
**Domain:** Profile format conversion, TOML serialization, CLI subcommand design
**Confidence:** HIGH

## Summary

Phase 14 adds a profile conversion tool that converts OrcaSlicer/BambuStudio JSON profiles into the project's native TOML format. Phase 13 already implemented the hard part: content-based format detection, JSON field mapping with value conversion (string-to-number, percentage stripping, array unwrapping, nil sentinel handling), and the `ImportResult` struct that tracks which fields were mapped and which were unmapped. Phase 14 builds on this foundation by adding TOML output generation and a CLI subcommand.

The conversion tool takes a JSON profile (or multiple profiles), imports it via the existing `import_upstream_profile()` function, then serializes the resulting `PrintConfig` to TOML using `toml::to_string_pretty()`. Since `PrintConfig` already derives both `Serialize` and `Deserialize`, TOML serialization should work out of the box. The main challenges are: (1) producing human-readable TOML with comments explaining the source mapping, (2) handling the three-profile-type split (process + filament + machine) that OrcaSlicer uses vs our unified `PrintConfig`, (3) only emitting fields that were actually mapped (not spamming all 55+ defaults), and (4) providing a useful CLI experience with conversion reporting.

The scope is modest: a `convert-profile` CLI subcommand, a `profile_convert` module in slicecore-engine with the conversion logic, and integration tests. No new external dependencies are needed.

**Primary recommendation:** Add a `convert-profile` CLI subcommand and a `profile_convert` module that uses the existing `import_upstream_profile()` + `toml::to_string_pretty()` to produce clean, commented TOML output. Support both single-file and multi-file (merge process + filament + machine) conversion modes. Emit only non-default fields for clean output.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| toml | 0.8 (already in workspace) | TOML serialization via `to_string_pretty` | Already used for config parsing; serialization is the inverse |
| serde_json | 1.x (already in workspace) | JSON deserialization of upstream profiles | Already used by Phase 13 profile import |
| serde | 1.x (already in workspace) | Serialize trait for PrintConfig | Already derived on all config types |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| thiserror | 2.x (already in workspace) | Error types for conversion failures | Conversion-specific error variants |
| clap | (already in CLI crate) | CLI subcommand definition | New `convert-profile` subcommand |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `toml::to_string_pretty` (full config dump) | Manual TOML string construction | Manual construction allows comments and selective field emission but is more maintenance. Recommend hybrid: serialize to Value, then custom format. |
| Full `PrintConfig` serialization | Selective field serialization | Dumping all 55+ fields (most at defaults) creates noisy TOML. Better to emit only fields that were actually mapped from the source profile. |

**Installation:**
No new dependencies needed. All libraries are already in the workspace.

## Architecture Patterns

### Recommended Module Structure
```
crates/slicecore-engine/src/
    profile_import.rs          # Existing: import_upstream_profile, ImportResult
    profile_convert.rs         # NEW: convert_to_toml, ConvertResult, selective serialization
crates/slicecore-cli/src/
    main.rs                    # MODIFIED: add convert-profile subcommand
```

### Pattern 1: Selective TOML Output (Only Mapped Fields)
**What:** Instead of serializing the entire PrintConfig (55+ fields, most at defaults), only emit fields that were actually mapped from the source JSON profile.
**When to use:** Always -- the user wants to see what was converted, not 55 lines of defaults.
**Why this matters:** A full `toml::to_string_pretty(&config)` would produce a wall of text including all nested structs (support, ironing, scarf_joint, multi_material, sequential, custom_gcode). The user only cares about the fields that came from their JSON profile.

**Implementation approach:**
```rust
/// Convert an ImportResult to selective TOML output.
///
/// Only emits fields that were actually mapped from the source profile.
/// Default-valued fields are omitted for clean, minimal output.
pub fn convert_to_toml(result: &ImportResult) -> Result<String, EngineError> {
    let mut output = String::new();

    // Header comment with source metadata.
    if let Some(name) = &result.metadata.name {
        output.push_str(&format!("# Converted from: {}\n", name));
    }
    if let Some(ptype) = &result.metadata.profile_type {
        output.push_str(&format!("# Source type: {}\n", ptype));
    }
    if let Some(inherits) = &result.metadata.inherits {
        output.push_str(&format!("# Inherits: {} (not resolved)\n", inherits));
    }
    output.push_str(&format!(
        "# Mapped {} of {} fields\n\n",
        result.mapped_fields.len(),
        result.mapped_fields.len() + result.unmapped_fields.len()
    ));

    // Serialize only the mapped fields using toml::Value construction.
    let config = &result.config;
    let defaults = PrintConfig::default();

    // Build a toml::Value::Table with only non-default, mapped fields.
    // Then serialize that table to string.
    // ...
}
```

### Pattern 2: Multi-File Merge Conversion
**What:** OrcaSlicer splits settings across 3 profile types (process, filament, machine). Users often want to merge all three into a single TOML.
**When to use:** When converting a complete profile set for a specific printer/material combo.
**Example CLI usage:**
```bash
slicecore convert-profile \
    --process "0.20mm Standard @BBL X1C.json" \
    --filament "Bambu ABS @BBL A1.json" \
    --machine "Bambu Lab A1 0.4 nozzle.json" \
    --output my_profile.toml
```

**Implementation:**
```rust
/// Merge multiple ImportResults into a single PrintConfig.
///
/// Later profiles override earlier ones for conflicting fields.
/// The merge order is: process -> filament -> machine (process is base).
pub fn merge_import_results(results: &[ImportResult]) -> ImportResult {
    let mut merged_config = PrintConfig::default();
    let mut all_mapped = Vec::new();
    let mut all_unmapped = Vec::new();

    for result in results {
        // Apply each result's mapped fields onto the merged config.
        // This is essentially re-importing, but since the fields are
        // already in PrintConfig, we can just copy the non-default values.
        // ...
    }

    ImportResult {
        config: merged_config,
        mapped_fields: all_mapped,
        unmapped_fields: all_unmapped,
        metadata: ProfileMetadata::default(), // merged doesn't have single metadata
    }
}
```

### Pattern 3: Conversion Report
**What:** After conversion, report what was mapped, what was dropped, and any warnings.
**When to use:** Always -- users need to know what was and wasn't converted.
**Example output:**
```
Converted "0.20mm Standard @BBL X1C" (process profile)
  Mapped 12 fields: layer_height, wall_loops -> wall_count, ...
  Unmapped 36 fields (no PrintConfig equivalent): bridge_speed, gap_infill_speed, ...
  Inherits: fdm_process_single_0.20 (not resolved -- export resolved profile from OrcaSlicer for complete settings)
  Output: my_profile.toml
```

### Anti-Patterns to Avoid
- **Full PrintConfig dump:** Do NOT serialize all 55+ fields to TOML. Most will be defaults and create noise. Emit only fields that were actually set from the source profile.
- **Loss of information without reporting:** Do NOT silently drop unmapped fields. Always report what was and wasn't converted so users can manually add any critical settings.
- **Resolving inheritance chains:** Do NOT attempt to load parent profiles to resolve `"inherits"`. This requires knowledge of the vendor directory structure. Instead, clearly state the limitation and recommend users export resolved profiles from OrcaSlicer.
- **Overwriting existing files without confirmation:** The CLI should either require `--output` or use `--force` to overwrite. Default to stdout if no output file specified.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TOML serialization | Manual TOML string formatting | `toml::to_string_pretty` or `toml::Value::Table` construction + `toml::to_string_pretty` | Handles escaping, quoting, table formatting correctly |
| JSON parsing | Custom parser | `serde_json` (Phase 13 already handles this) | Battle-tested, all edge cases handled |
| Field mapping | New mapping logic | `import_upstream_profile()` from Phase 13 | Already handles string-to-number, percentages, arrays, nil, enum mapping |
| CLI argument parsing | Manual arg parsing | `clap` (already in CLI crate) | Consistent with existing subcommands |

**Key insight:** Phase 13 already built the hard part (JSON import with field mapping). Phase 14 is primarily about the output side (TOML serialization) and the CLI integration. The conversion pipeline is: `read JSON -> import_upstream_profile() -> selective TOML output`.

## Common Pitfalls

### Pitfall 1: toml::to_string_pretty Serializes Everything
**What goes wrong:** Calling `toml::to_string_pretty(&config)` on a full `PrintConfig` produces 80+ lines including all nested structs with defaults (support config, ironing config, scarf joint config, multi-material config, sequential config, custom gcode hooks, per-feature flow). The output is unusable.
**Why it happens:** `Serialize` derives on all fields mean every field is serialized.
**How to avoid:** Build a `toml::Value::Table` manually with only the fields that were actually mapped. Or serialize the full config, then strip fields where the value matches the default.
**Warning signs:** Output file has hundreds of lines for a profile that only set 10 fields.

### Pitfall 2: Enum Serialization Format Mismatch
**What goes wrong:** Some enums use `#[serde(rename_all = "snake_case")]` (like `InfillPattern`, `SeamPosition`, `SupportType`) while others don't (like `GcodeDialect`, which serializes as PascalCase: `"Marlin"` not `"marlin"`). The generated TOML may use inconsistent casing.
**Why it happens:** Different enums in the codebase have different serde configurations. `GcodeDialect` in `slicecore-gcode-io` has no `rename_all` attribute.
**How to avoid:** Use the same serde serialization that `from_toml` expects for deserialization. Since `PrintConfig` round-trips through serde, the `toml::to_string_pretty` output should deserialize back correctly via `PrintConfig::from_toml`. Verify round-trip correctness in tests.
**Warning signs:** Generated TOML fails to parse back into PrintConfig.

### Pitfall 3: Nested Table Ordering in TOML
**What goes wrong:** TOML tables (like `[support]`, `[ironing]`, `[scarf_joint]`) must come after all top-level key-value pairs. If the serializer interleaves them, the TOML is technically valid but hard to read.
**Why it happens:** `toml::to_string_pretty` handles this correctly, but manual string construction might not.
**How to avoid:** Use `toml::to_string_pretty` for the actual serialization, then add comments around sections. Do NOT manually construct TOML strings for nested structs.
**Warning signs:** TOML validation errors or confusing output ordering.

### Pitfall 4: Float Precision in TOML Output
**What goes wrong:** Values like `0.15` (from percentage conversion 15% / 100) may serialize as `0.15000000000000002` due to IEEE 754 floating-point representation.
**Why it happens:** The percentage `15%` is converted to `0.15` in f64, which is not exactly representable in binary floating point.
**How to avoid:** Post-process float values to round to a reasonable precision (e.g., 6 decimal places) before serialization. Or use `toml::Value::Float` directly with rounded values.
**Warning signs:** TOML output has values like `infill_density = 0.15000000000000002`.

### Pitfall 5: Plugin InfillPattern Variant
**What goes wrong:** `InfillPattern::Plugin(String)` serializes as `infill_pattern = { plugin = "name" }` which is correct TOML table syntax. However, if this shows up in a converted profile, it means the source specified a plugin pattern (unlikely for upstream profiles).
**Why it happens:** The `Plugin` variant is an internally-tagged enum with a string payload.
**How to avoid:** Upstream OrcaSlicer profiles should never produce a `Plugin` variant (the mapping function returns standard patterns only). But verify this doesn't cause issues in round-trip tests.
**Warning signs:** `infill_pattern = { plugin = "..." }` appearing in converted output.

## Code Examples

### Selective TOML Generation (Recommended Approach)
```rust
// Source: Derived from codebase analysis of PrintConfig, ImportResult, and toml crate

use crate::config::PrintConfig;
use crate::profile_import::ImportResult;
use toml::Value;

/// Result of converting a JSON profile to TOML.
pub struct ConvertResult {
    /// The generated TOML string.
    pub toml_output: String,
    /// Number of fields successfully mapped and emitted.
    pub mapped_count: usize,
    /// Fields that had no PrintConfig equivalent.
    pub unmapped_fields: Vec<String>,
    /// Source profile metadata.
    pub source_name: Option<String>,
    pub source_type: Option<String>,
}

/// Convert an ImportResult to a TOML string with only mapped fields.
pub fn convert_to_toml(result: &ImportResult) -> ConvertResult {
    let mut output = String::new();
    let config = &result.config;
    let defaults = PrintConfig::default();

    // Header comments.
    write_header_comments(&mut output, result);

    // Serialize full config to toml::Value, then filter to only non-default fields.
    let full_value = toml::Value::try_from(config).unwrap_or(Value::Table(Default::default()));
    let default_value = toml::Value::try_from(&defaults).unwrap_or(Value::Table(Default::default()));

    if let (Value::Table(full), Value::Table(default)) = (full_value, default_value) {
        let mut filtered = toml::map::Map::new();
        for (key, val) in &full {
            if default.get(key) != Some(val) {
                filtered.insert(key.clone(), val.clone());
            }
        }
        let filtered_val = Value::Table(filtered);
        match toml::to_string_pretty(&filtered_val) {
            Ok(toml_str) => output.push_str(&toml_str),
            Err(_) => {
                // Fallback: serialize full config.
                if let Ok(toml_str) = toml::to_string_pretty(config) {
                    output.push_str(&toml_str);
                }
            }
        }
    }

    ConvertResult {
        toml_output: output,
        mapped_count: result.mapped_fields.len(),
        unmapped_fields: result.unmapped_fields.clone(),
        source_name: result.metadata.name.clone(),
        source_type: result.metadata.profile_type.clone(),
    }
}
```

### CLI Subcommand Integration
```rust
// Source: Derived from existing CLI pattern in main.rs

/// Convert an OrcaSlicer/BambuStudio JSON profile to TOML format.
///
/// Reads one or more JSON profile files, maps fields to PrintConfig,
/// and outputs TOML to file or stdout.
ConvertProfile {
    /// Input JSON profile file(s) to convert.
    /// Multiple files are merged (process + filament + machine).
    #[arg(required = true)]
    input: Vec<PathBuf>,

    /// Output TOML file path (default: stdout).
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Show detailed conversion report.
    #[arg(short, long)]
    verbose: bool,
}
```

### Round-Trip Verification Test
```rust
#[test]
fn test_json_to_toml_round_trip() {
    let json = r#"{
        "type": "process",
        "name": "Test Profile",
        "layer_height": "0.2",
        "wall_loops": "3",
        "sparse_infill_density": "20%",
        "outer_wall_speed": "150",
        "seam_position": "aligned"
    }"#;

    // Import from JSON.
    let result = PrintConfig::from_json_with_details(json).unwrap();
    let imported_config = result.config.clone();

    // Convert to TOML.
    let convert_result = convert_to_toml(&result);
    let toml_str = &convert_result.toml_output;

    // Parse TOML back to PrintConfig.
    let round_tripped = PrintConfig::from_toml(toml_str).unwrap();

    // Verify key fields survive the round-trip.
    assert!((round_tripped.layer_height - imported_config.layer_height).abs() < 1e-9);
    assert_eq!(round_tripped.wall_count, imported_config.wall_count);
    assert!((round_tripped.infill_density - imported_config.infill_density).abs() < 1e-9);
    assert!((round_tripped.perimeter_speed - imported_config.perimeter_speed).abs() < 1e-9);
    assert_eq!(round_tripped.seam_position, imported_config.seam_position);
}
```

## Existing Infrastructure Analysis

### What Phase 13 Already Built (HIGH confidence - verified from source)

1. **`profile_import.rs`** (441 lines): Contains `detect_config_format()`, `import_upstream_profile()`, `ImportResult`, `ProfileMetadata`, field extraction helpers (`extract_string_value`, `extract_f64`, etc.), `apply_field_mapping()` with 32+ field mappings, and enum mapping helpers (`map_infill_pattern`, `map_seam_position`, `map_gcode_dialect`).

2. **`config.rs`**: `PrintConfig` with `Serialize`/`Deserialize` derives, `from_json()`, `from_json_with_details()`, `from_file()` (auto-detect), `from_toml()`, `from_toml_file()`. All 55+ fields have defaults.

3. **`lib.rs`**: Re-exports `detect_config_format`, `ConfigFormat`, `ImportResult`, `ProfileMetadata`.

4. **CLI (`main.rs`)**: `--config` flag already uses `from_file()` for auto-detection. No `convert-profile` subcommand exists yet.

5. **Integration tests**: `integration_profile_import.rs` with 8 synthetic tests + 4 ignored real-profile tests. Verifies round-trip, nil handling, mixed array/scalar, unmapped field reporting.

### What Phase 14 Needs to Add

1. **`profile_convert.rs`** (new module): Conversion logic, selective TOML output, `ConvertResult` struct, optional multi-file merge.

2. **CLI `convert-profile` subcommand**: Accept one or more JSON files, output TOML to file or stdout, show conversion report.

3. **Integration tests**: Round-trip (JSON -> PrintConfig -> TOML -> PrintConfig), real upstream profile conversion, multi-file merge, selective output verification.

### PrintConfig Field Count Analysis (verified from source)

Top-level scalar fields: ~30 (layer_height, wall_count, speeds, temps, retraction, etc.)
Nested struct fields:
- `scarf_joint: ScarfJointConfig` (13 fields)
- `support: SupportConfig` (15+ fields, with nested `bridge` and `tree`)
- `ironing: IroningConfig` (5 fields)
- `per_feature_flow: PerFeatureFlow` (8+ fields)
- `custom_gcode: CustomGcodeHooks` (5 fields)
- `multi_material: MultiMaterialConfig` (7 fields)
- `sequential: SequentialConfig` (3 fields)

Total: ~86 fields when fully expanded. Only ~32 are mapped from upstream JSON. Selective output is essential.

### TOML Serialization Verification Points

- `PrintConfig` derives `Serialize` -- `toml::to_string_pretty` should work.
- All enum types derive `Serialize` with `#[serde(rename_all = "snake_case")]` except `GcodeDialect` (PascalCase).
- `InfillPattern::Plugin(String)` serializes as a tagged map `{ plugin = "name" }`.
- Nested structs serialize as TOML tables: `[support]`, `[ironing]`, `[scarf_joint]`, etc.
- `CustomGcodeHooks` has a `Vec<(f64, String)>` field that serializes as `custom_gcode_per_z = [[5.0, "M600"]]`.
- `Vec<ToolConfig>` in `MultiMaterialConfig` serializes as `[[multi_material.tools]]`.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| TOML-only config | Auto-detect TOML/JSON, import upstream profiles | Phase 13 | Users can import existing profiles |
| No conversion tool | JSON -> TOML conversion with field reporting | Phase 14 (planned) | Users can migrate to native format |

**Deprecated/outdated:**
- Nothing deprecated. Phase 14 builds on Phase 13 without deprecating anything.

## Open Questions

1. **Should the converter emit ALL fields or only mapped fields?**
   - What we know: Full serialization produces ~86 fields. Only ~32 are mapped from upstream JSON.
   - What's unclear: Users might want all fields (as a complete config template) or just mapped fields (minimal).
   - Recommendation: Default to selective (only non-default fields). Add a `--full` flag for complete output.

2. **Should we add TOML comments describing each field?**
   - What we know: The example profile `pla_standard_0.2mm.toml` has extensive comments. `toml::to_string_pretty` does not add comments.
   - What's unclear: How important are comments in converted output?
   - Recommendation: Add section header comments (like "# Layer geometry", "# Speeds") and a top-level comment with source metadata. Skip per-field comments for now (the example profile can serve as documentation).

3. **Should multi-file merge be in the initial implementation?**
   - What we know: OrcaSlicer uses 3 separate profiles (process + filament + machine). Merging them produces a complete config.
   - What's unclear: Is this a common user workflow or edge case?
   - Recommendation: Include it. The merge logic is straightforward (load each file, import each, overlay configs). It's the natural way to build a complete TOML from an OrcaSlicer setup. Accept multiple `--input` files.

4. **Should output go to stdout or file by default?**
   - What we know: Unix convention is stdout for pipe-friendly operation. CLI tools like `jq` and `yq` output to stdout by default.
   - Recommendation: Default to stdout. Use `--output/-o` for file output. This allows piping: `slicecore convert-profile input.json > output.toml`.

## Sources

### Primary (HIGH confidence)
- **Existing codebase** -- Direct inspection of:
  - `/home/steve/libslic3r-rs/crates/slicecore-engine/src/profile_import.rs` (Phase 13 import logic, 441 lines)
  - `/home/steve/libslic3r-rs/crates/slicecore-engine/src/config.rs` (PrintConfig struct, 1031 lines)
  - `/home/steve/libslic3r-rs/crates/slicecore-engine/src/lib.rs` (re-exports)
  - `/home/steve/libslic3r-rs/crates/slicecore-cli/src/main.rs` (existing CLI pattern)
  - `/home/steve/libslic3r-rs/crates/slicecore-engine/tests/integration_profile_import.rs` (test patterns)
  - `/home/steve/libslic3r-rs/examples/profiles/pla_standard_0.2mm.toml` (TOML output format example)
- **Actual OrcaSlicer profiles** -- `/home/steve/slicer-analysis/OrcaSlicer/resources/profiles/BBL/` (verified JSON structure, field counts, real-world content)
- **Phase 13 research** -- `/home/steve/libslic3r-rs/.planning/phases/13-json-profile-support/13-RESEARCH.md` (field mapping tables, upstream schema analysis)

### Secondary (MEDIUM confidence)
- **toml crate** -- `toml` 0.8 in workspace, verified `Serialize` derives on all config types. `to_string_pretty` produces human-readable TOML.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- All libraries already in workspace, no new dependencies
- Architecture: HIGH -- Straightforward extension of Phase 13 infrastructure; conversion is the inverse of import
- Selective output: MEDIUM -- The approach of filtering `toml::Value::Table` entries by comparing against defaults needs validation. May need refinement for nested struct comparison.
- CLI integration: HIGH -- Follows established pattern from existing subcommands (slice, validate, analyze, ai-suggest)
- Pitfalls: HIGH -- Identified from direct code inspection (enum serde format mismatches, float precision, full-dump noise)

**Research date:** 2026-02-18
**Valid until:** 2026-03-18 (stable domain -- no external dependencies changing)
