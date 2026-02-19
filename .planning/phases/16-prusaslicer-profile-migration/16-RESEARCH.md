# Phase 16: PrusaSlicer Profile Migration - Research

**Researched:** 2026-02-19
**Domain:** PrusaSlicer INI profile parsing, batch conversion, field mapping to PrintConfig
**Confidence:** HIGH

## Summary

Phase 16 extends the Phase 15 profile library to include PrusaSlicer profiles. The critical finding is that PrusaSlicer uses **INI format** (not JSON like OrcaSlicer/BambuStudio), stored as monolithic per-vendor `.ini` files with 329 unique field names. The existing `batch_convert_profiles()` infrastructure assumes JSON input with OrcaSlicer-specific key names and an `"instantiation": "true"` field to distinguish leaf profiles. None of this applies to PrusaSlicer. A new INI parsing + conversion pipeline is needed.

The PrusaSlicer profile directory at `/home/steve/slicer-analysis/PrusaSlicer/resources/profiles/` contains 35 FFF vendor INI files (excluding 2 SLA files), with approximately 9,523 concrete profile sections across all vendors. The largest vendor is PrusaResearch with 6,761 sections (629 print, 5,874 filament, 224 printer). Profiles use `[section:name]` headers where the section type is `print`, `filament`, `printer`, `printer_model`, or `vendor`. Abstract/base profiles have asterisk-wrapped names (e.g., `[print:*common*]`, `[filament:*PLA*]`), while concrete profiles have plain names (e.g., `[print:0.20mm NORMAL]`, `[filament:Prusament PLA @MK4S HF0.4]`). The inheritance mechanism uses `inherits = parent_name` with semicolon-separated multi-inheritance (e.g., `inherits = *0.15mm*; *soluble_support*`).

PrusaSlicer key names are mostly different from OrcaSlicer. PrusaSlicer uses native Slic3r names (`perimeters`, `fill_density`, `temperature`, `bed_temperature`, `retract_length`, `retract_speed`, `retract_lift`, `perimeter_speed`, `infill_speed`), while OrcaSlicer uses renamed variants (`wall_loops`, `sparse_infill_density`, `nozzle_temperature`, `hot_plate_temp`, `retraction_length`, `retraction_speed`, `z_hop`, `outer_wall_speed`, `sparse_infill_speed`). The `apply_field_mapping()` function in `profile_import.rs` currently only handles OrcaSlicer key names, so a parallel PrusaSlicer-specific field mapping is needed.

**Primary recommendation:** Build an INI parser for PrusaSlicer vendor files that: (1) parses sections + key-value pairs, (2) resolves multi-level and multi-parent inheritance within each vendor file, (3) maps PrusaSlicer key names to PrintConfig fields via a new `apply_prusaslicer_field_mapping()`, (4) converts concrete (non-asterisk) profiles to TOML, (5) writes output to `profiles/prusaslicer/vendor/type/` using the same directory and index structure as OrcaSlicer, and (6) skips SLA vendor files entirely.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| toml | 0.8 (already in workspace) | TOML serialization of converted profiles | Already used for config and profile_convert |
| serde_json | 1.x (already in workspace) | JSON index manifest generation | Already used by Phase 15 index |
| serde | 1.x (already in workspace) | Serialize/Deserialize traits | Already used everywhere |
| walkdir | 2.x (already in workspace) | Directory traversal for vendor subdirectories | Already added in Phase 15 |
| clap | 4.5 (already in CLI crate) | Extend import-profiles CLI command | Already used for existing subcommands |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| (none needed) | - | INI parsing is simple enough to hand-roll | PrusaSlicer INI format is non-standard (`;` comments, multi-value inheritance) -- no crate handles it correctly |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-rolled INI parser | `configparser` or `ini` crate | PrusaSlicer INI has non-standard features: `[section:name]` typed headers, semicolon-separated multi-inheritance, `%` suffix in values, multi-line G-code values with `\n` escapes. Standard INI crates don't handle these. Hand-rolling is safer. |
| Separate `import_prusaslicer_ini()` function | Extending `import_upstream_profile()` | The existing function assumes JSON `serde_json::Value`. INI is fundamentally different -- separate function is cleaner. |
| Convert INI to intermediate JSON then use existing pipeline | Direct INI-to-PrintConfig mapping | Extra conversion step adds complexity without benefit. Direct mapping is simpler. |

## Architecture Patterns

### Recommended Project Structure
```
crates/slicecore-engine/src/
    profile_import.rs          # Existing OrcaSlicer JSON import (keep as-is)
    profile_import_ini.rs      # NEW: PrusaSlicer INI parsing + import
    profile_convert.rs         # Existing TOML conversion (reuse as-is)
    profile_library.rs         # Extend with INI batch conversion path
```

### Pattern 1: INI Section Parser
**What:** Parse a PrusaSlicer vendor INI file into a structured representation of typed sections with key-value maps.
**When to use:** For every vendor .ini file in the PrusaSlicer profiles directory.

```rust
// Source: Direct analysis of PrusaSlicer INI files

/// A parsed INI section with its type, name, and key-value pairs.
#[derive(Debug, Clone)]
pub struct IniSection {
    /// Section type: "vendor", "printer_model", "print", "filament", "printer"
    pub section_type: String,
    /// Section name (e.g., "PrusaResearch", "*common*", "0.20mm NORMAL")
    pub name: String,
    /// Key-value pairs in this section
    pub fields: HashMap<String, String>,
    /// Is this an abstract/base profile? (name wrapped in asterisks)
    pub is_abstract: bool,
}

/// Parse a PrusaSlicer vendor INI file into sections.
pub fn parse_prusaslicer_ini(content: &str) -> Vec<IniSection> {
    // Section headers: [type:name] or [type]
    // Keys: key = value
    // Comments: # or ;
    // Multi-line values: \n escape sequences (NOT actual newlines)
    // ...
}
```

### Pattern 2: INI Inheritance Resolution with Multi-Parent
**What:** Resolve `inherits = *base1*; *base2*` by merging parents left-to-right, then overlaying child fields.
**When to use:** For every concrete profile before conversion to TOML.
**Key difference from OrcaSlicer:** OrcaSlicer profiles inherit from a single parent file. PrusaSlicer profiles can inherit from multiple abstract bases separated by semicolons, AND from concrete profiles by name. All inheritance is resolved within the same .ini file.

```rust
// Example from PrusaResearch.ini:
// [print:0.05mm ULTRADETAIL @0.25 nozzle]
// inherits = *0.05mm*; *0.25nozzle*
//
// [filament:*PLAHF*]
// inherits = *PLAPG*; *PA_PLAHF*
//
// Resolution: merge *PLAPG* fields, overlay *PA_PLAHF* fields, overlay child fields.

pub fn resolve_ini_inheritance(
    section: &IniSection,
    all_sections: &HashMap<(String, String), IniSection>,  // (type, name) -> section
) -> HashMap<String, String> {
    // 1. Parse inherits field: split on "; " (semicolon + space)
    // 2. For each parent (left to right): recursively resolve, merge fields
    // 3. Overlay child's own fields on top
    // 4. Return flattened key-value map
}
```

### Pattern 3: PrusaSlicer Field Mapping
**What:** Map PrusaSlicer INI key names to PrintConfig fields. These differ significantly from OrcaSlicer JSON key names.
**When to use:** After inheritance resolution, when converting flattened key-value map to PrintConfig.

Key PrusaSlicer-to-PrintConfig mappings (verified from source analysis):

| PrusaSlicer Key | PrintConfig Field | Notes |
|-----------------|-------------------|-------|
| `layer_height` | `layer_height` | Same name |
| `first_layer_height` | `first_layer_height` | Same name |
| `perimeters` | `wall_count` | Different name |
| `fill_density` | `infill_density` | Different name; value has `%` suffix (e.g., "15%") |
| `fill_pattern` | `infill_pattern` | Different name |
| `top_solid_layers` | `top_solid_layers` | Same name |
| `bottom_solid_layers` | `bottom_solid_layers` | Same name |
| `perimeter_speed` | `perimeter_speed` | Same name |
| `infill_speed` | `infill_speed` | Same name |
| `travel_speed` | `travel_speed` | Same name |
| `first_layer_speed` | `first_layer_speed` | Same name; may have `%` suffix |
| `skirts` | `skirt_loops` | Different name |
| `skirt_distance` | `skirt_distance` | Same name |
| `brim_width` | `brim_width` | Same name |
| `seam_position` | `seam_position` | Same name |
| `default_acceleration` | `print_acceleration` | Different name |
| `temperature` | `nozzle_temp` | Different name |
| `first_layer_temperature` | `first_layer_nozzle_temp` | Different name |
| `bed_temperature` | `bed_temp` | Different name |
| `first_layer_bed_temperature` | `first_layer_bed_temp` | Different name |
| `filament_density` | `filament_density` | Same name |
| `filament_diameter` | `filament_diameter` | Same name |
| `filament_cost` | `filament_cost_per_kg` | Different name |
| `extrusion_multiplier` | `extrusion_multiplier` | Same name |
| `disable_fan_first_layers` | `disable_fan_first_layers` | Same name |
| `fan_below_layer_time` | `fan_below_layer_time` | Same name |
| `nozzle_diameter` | `nozzle_diameter` | Same name |
| `retract_length` | `retract_length` | Same name |
| `retract_speed` | `retract_speed` | Same name |
| `retract_lift` | `retract_z_hop` | Different name |
| `retract_before_travel` | `min_travel_for_retract` | Different name |
| `gcode_flavor` | `gcode_dialect` | Different name |
| `machine_max_jerk_x` | `jerk_x` | Same mapping as OrcaSlicer |
| `machine_max_jerk_y` | `jerk_y` | Same mapping as OrcaSlicer |
| `machine_max_jerk_z` | `jerk_z` | Same mapping as OrcaSlicer |

### Pattern 4: Profile Type Categorization
**What:** PrusaSlicer section types map to profile types differently from OrcaSlicer.
**Mapping:**

| INI Section Type | Our Profile Type | Notes |
|-----------------|-----------------|-------|
| `print` | `process` | Print settings (layer height, speeds, etc.) |
| `filament` | `filament` | Filament settings (temperature, fan, etc.) |
| `printer` | `machine` | Printer hardware settings |
| `vendor` | (metadata only) | Skip -- not a profile |
| `printer_model` | (metadata only) | Skip -- printer model definitions, not profiles |

### Anti-Patterns to Avoid
- **Trying to reuse `import_upstream_profile()` for INI data:** This function expects a `serde_json::Value`. Don't convert INI to JSON just to feed it through the existing function. Write a clean parallel path.
- **Parsing all vendors as a single file:** Each .ini file is independent. Parse each vendor file separately and maintain inheritance scope per vendor file.
- **Including SLA profiles:** Skip `AnycubicSLA.ini` and `PrusaResearchSLA.ini` entirely. This project is FDM only.
- **Including `Templates.ini` as a vendor:** The Templates vendor contains generic base profiles used by other vendors. Include it but label the vendor as "Templates" -- these are usable standalone profiles.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TOML conversion | Custom TOML serializer | Existing `convert_to_toml()` | Already works, produces clean output |
| Profile index | Custom search structure | Existing `ProfileIndex` + `write_index()` | Already provides search, list, show functionality |
| Filename sanitization | New sanitizer | Existing `sanitize_filename()` | Already handles spaces, `@`, parentheses |
| Metadata extraction | New extractors | Existing `extract_material_from_name()`, `extract_layer_height_from_name()`, etc. | PrusaSlicer profile names follow same conventions (`0.20mm NORMAL @MK4S`, `Prusament PLA @COREONE HF0.4`) |
| Quality name extraction | New quality matcher | Existing `extract_quality_from_name()` + additions | Need to add PrusaSlicer-specific terms: "OPTIMAL", "ULTRADETAIL", "DETAIL", "SPEED" |

**Key insight:** The output pipeline (TOML conversion, index generation, CLI commands) is fully reusable from Phase 15. The only new code is the input pipeline: INI parsing, INI inheritance resolution, and PrusaSlicer field mapping.

## Common Pitfalls

### Pitfall 1: Multi-Parent Inheritance Order Matters
**What goes wrong:** Merging parents in wrong order produces incorrect field values.
**Why it happens:** PrusaSlicer `inherits = *0.15mm*; *MK3*` means: start from `*0.15mm*` (which itself inherits from `*common*`), then overlay `*MK3*` on top. Left-to-right merge order is critical.
**How to avoid:** Process parents left-to-right. For each parent, recursively resolve ITS inheritance first, then overlay the next parent.
**Warning signs:** Print profiles with wrong acceleration or speed values; filament profiles with wrong temperature values.

### Pitfall 2: Percentage Values Need Special Handling
**What goes wrong:** Values like `fill_density = 15%` or `first_layer_speed = 50%` are stored with `%` suffix in INI files.
**Why it happens:** PrusaSlicer uses percentage strings that need the `%` stripped and value divided by 100 (for fill_density) or treated as a percentage of another value (for speed percentages).
**How to avoid:** Strip `%` suffix before parsing. For `fill_density`, divide by 100. For percentage speeds like `first_layer_speed = 50%`, note this means "50% of default speed" -- store the raw number as a percentage indicator or skip mapping (our PrintConfig uses absolute speeds).
**Warning signs:** `infill_density = 15.0` instead of `0.15`; `first_layer_speed = 50.0` when it should be a percentage.

### Pitfall 3: Concrete vs Abstract Profile Discrimination
**What goes wrong:** Converting abstract profiles like `[print:*common*]` that have no meaningful standalone use.
**Why it happens:** Unlike OrcaSlicer which has `"instantiation": "true"`, PrusaSlicer uses the asterisk naming convention.
**How to avoid:** Only convert sections where the name does NOT start and end with `*`. Also skip `printer_model` and `vendor` sections.
**Warning signs:** Output contains profiles named `*common*` or `*PLA*` that have incomplete settings.

### Pitfall 4: Same-File vs Cross-File Inheritance
**What goes wrong:** Trying to resolve inheritance across different vendor files.
**Why it happens:** All PrusaSlicer inheritance is self-contained within each vendor .ini file. There's no cross-vendor inheritance.
**How to avoid:** Parse each vendor .ini file independently. Build the inheritance lookup table per file, not globally.
**Warning signs:** "Parent not found" errors when resolving inheritance.

### Pitfall 5: G-Code Values with Embedded Newlines
**What goes wrong:** G-code fields like `start_gcode`, `end_gcode`, `start_filament_gcode` contain `\n` escape sequences that look like line breaks but are actually part of the value.
**Why it happens:** PrusaSlicer stores multi-line G-code as single INI values with `\n` literals.
**How to avoid:** When parsing INI values, preserve `\n` escape sequences as-is. These are metadata/G-code fields that won't be mapped to PrintConfig numeric fields anyway, but incorrect parsing could break section boundaries.
**Warning signs:** INI parser treats `\n` in a G-code value as a new line, corrupting subsequent key parsing.

### Pitfall 6: Profile Names with Special Characters
**What goes wrong:** PrusaSlicer profile names contain `&&`, `@`, `+`, parentheses, periods (e.g., `Original Prusa i3 MK3S && MK3S+`, `MK3.9`).
**Why it happens:** PrusaSlicer naming conventions are more varied than OrcaSlicer.
**How to avoid:** Use the existing `sanitize_filename()` but verify it handles `&&` and `+` characters. May need to extend it.
**Warning signs:** Filenames with `&&` causing shell issues or filesystem problems.

### Pitfall 7: Index Merge Across Sources
**What goes wrong:** Running `import-profiles --source-name prusaslicer` overwrites the `index.json` that already contains OrcaSlicer profiles.
**Why it happens:** `write_index()` writes a single source's index. Running for a second source replaces it.
**How to avoid:** Extend the index logic to either: (a) merge indexes from multiple sources, or (b) write per-source indexes and load all at query time, or (c) modify the batch convert to append to an existing index.
**Warning signs:** After importing PrusaSlicer profiles, OrcaSlicer profiles disappear from `list-profiles` output.

## Code Examples

Verified patterns from direct analysis of PrusaSlicer source files:

### INI Section Header Parsing
```rust
// PrusaSlicer INI section headers are: [type:name] or [type]
// Examples:
//   [vendor]                    -> type="vendor", name=""
//   [printer_model:MK4S]       -> type="printer_model", name="MK4S"
//   [print:*common*]            -> type="print", name="*common*" (abstract)
//   [print:0.20mm NORMAL]       -> type="print", name="0.20mm NORMAL" (concrete)
//   [filament:Prusament PLA @MK4S HF0.4] -> type="filament", name="Prusament PLA @MK4S HF0.4"
//   [printer:Original Prusa i3 MK3S && MK3S+] -> type="printer", name="..."

fn parse_section_header(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return None;
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    if let Some(colon_pos) = inner.find(':') {
        let section_type = inner[..colon_pos].to_string();
        let name = inner[colon_pos + 1..].to_string();
        Some((section_type, name))
    } else {
        Some((inner.to_string(), String::new()))
    }
}
```

### Multi-Parent Inheritance Resolution
```rust
// PrusaSlicer inheritance examples:
//   inherits = *common*                    -> single parent
//   inherits = *0.15mm*; *MK3*             -> two parents
//   inherits = *0.35mm*; *0.6nozzle*; *soluble_support*  -> three parents
//   inherits = 0.15mm SPEED @MK3; *soluble_support*      -> concrete + abstract parents
//   inherits = Prusament PLA @PG; *PA_BUDDY_PLA*          -> concrete filament + abstract

fn parse_inherits(value: &str) -> Vec<String> {
    value.split("; ")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
```

### PrusaSlicer-Specific Field Mapping
```rust
// Source: Direct analysis of PrusaSlicer INI files

fn apply_prusaslicer_field_mapping(config: &mut PrintConfig, key: &str, value: &str) -> bool {
    match key {
        // --- Print/Process fields ---
        "layer_height" => parse_and_set_f64(value, &mut config.layer_height),
        "first_layer_height" => parse_and_set_f64(value, &mut config.first_layer_height),
        "perimeters" => parse_and_set_u32(value, &mut config.wall_count),
        "fill_density" => {
            // PrusaSlicer: "15%" -> 0.15
            let cleaned = value.trim_end_matches('%');
            if let Ok(pct) = cleaned.parse::<f64>() {
                config.infill_density = pct / 100.0;
                true
            } else { false }
        }
        "fill_pattern" => map_infill_pattern_prusaslicer(value, config),
        "top_solid_layers" => parse_and_set_u32(value, &mut config.top_solid_layers),
        "bottom_solid_layers" => parse_and_set_u32(value, &mut config.bottom_solid_layers),
        "perimeter_speed" => parse_and_set_f64(value, &mut config.perimeter_speed),
        "infill_speed" => parse_and_set_f64(value, &mut config.infill_speed),
        "travel_speed" => parse_and_set_f64(value, &mut config.travel_speed),
        "first_layer_speed" => {
            // May have % suffix meaning "percentage of default" -- skip if percentage
            if value.ends_with('%') { return false; }
            parse_and_set_f64(value, &mut config.first_layer_speed)
        }
        "skirts" => parse_and_set_u32(value, &mut config.skirt_loops),
        "skirt_distance" => parse_and_set_f64(value, &mut config.skirt_distance),
        "brim_width" => parse_and_set_f64(value, &mut config.brim_width),
        "default_acceleration" => parse_and_set_f64(value, &mut config.print_acceleration),
        "seam_position" => map_seam_position_prusaslicer(value, config),

        // --- Filament fields ---
        "temperature" => parse_and_set_f64(value, &mut config.nozzle_temp),
        "first_layer_temperature" => parse_and_set_f64(value, &mut config.first_layer_nozzle_temp),
        "bed_temperature" => parse_and_set_f64(value, &mut config.bed_temp),
        "first_layer_bed_temperature" => parse_and_set_f64(value, &mut config.first_layer_bed_temp),
        "filament_density" => parse_and_set_f64(value, &mut config.filament_density),
        "filament_diameter" => parse_and_set_f64(value, &mut config.filament_diameter),
        "filament_cost" => parse_and_set_f64(value, &mut config.filament_cost_per_kg),
        "extrusion_multiplier" => parse_and_set_f64(value, &mut config.extrusion_multiplier),
        "disable_fan_first_layers" => parse_and_set_u32(value, &mut config.disable_fan_first_layers),
        "fan_below_layer_time" => parse_and_set_f64(value, &mut config.fan_below_layer_time),

        // --- Machine/Printer fields ---
        "nozzle_diameter" => {
            // May be comma-separated for multi-extruder: "0.4,0.4,0.4,0.4"
            // Take first value
            let first = value.split(',').next().unwrap_or(value);
            parse_and_set_f64(first.trim(), &mut config.nozzle_diameter)
        }
        "retract_length" => parse_and_set_f64(value, &mut config.retract_length),
        "retract_speed" => parse_and_set_f64(value, &mut config.retract_speed),
        "retract_lift" => parse_and_set_f64(value, &mut config.retract_z_hop),
        "retract_before_travel" => parse_and_set_f64(value, &mut config.min_travel_for_retract),
        "gcode_flavor" => map_gcode_dialect_prusaslicer(value, config),
        "machine_max_jerk_x" => parse_and_set_f64(value, &mut config.jerk_x),
        "machine_max_jerk_y" => parse_and_set_f64(value, &mut config.jerk_y),
        "machine_max_jerk_z" => parse_and_set_f64(value, &mut config.jerk_z),

        _ => false,
    }
}
```

### Batch Conversion Entry Point
```rust
// Extend the CLI to support PrusaSlicer source:
// slicecore import-profiles \
//   --source-dir /home/steve/slicer-analysis/PrusaSlicer/resources/profiles \
//   --source-name prusaslicer
//
// The batch_convert function needs a new INI path for PrusaSlicer:

pub fn batch_convert_prusaslicer_profiles(
    source_dir: &Path,    // .../PrusaSlicer/resources/profiles/
    output_dir: &Path,    // profiles/prusaslicer/
    source_name: &str,    // "prusaslicer"
) -> Result<BatchConvertResult, EngineError> {
    // 1. Walk *.ini files in source_dir (skip *SLA* files)
    // 2. For each .ini file: parse all sections
    // 3. Build section lookup map per file
    // 4. For each concrete profile section (non-asterisk name):
    //    a. Resolve inheritance chain
    //    b. Apply PrusaSlicer field mapping -> ImportResult
    //    c. Convert to TOML via convert_to_toml()
    //    d. Write to output_dir/vendor/type/sanitized_name.toml
    //    e. Add to ProfileIndex
    // 5. Return BatchConvertResult
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| OrcaSlicer JSON only | INI parsing for PrusaSlicer | Phase 16 (this phase) | Doubles the profile library coverage |
| Single-source `batch_convert_profiles()` | Source-specific conversion paths (JSON vs INI) | Phase 16 | Requires dispatch on source format |
| `index.json` from single source | Merged index across sources | Phase 16 | Index must aggregate OrcaSlicer + PrusaSlicer entries |

**PrusaSlicer profile format has been stable since Slic3r PE days.** The INI format with `[type:name]` headers and `inherits` field has not changed. The field names are the same as Slic3r/SuperSlicer. This format is not expected to change.

## Open Questions

1. **Index Merge Strategy**
   - What we know: Phase 15 writes `index.json` for one source. Phase 16 adds a second source.
   - What's unclear: Should we merge into one index, maintain per-source indexes, or load/merge at query time?
   - Recommendation: Modify `write_index()` to accept an optional existing index and merge new entries. This is simplest for the CLI commands that already read a single `index.json`.

2. **Percentage Speed Values**
   - What we know: PrusaSlicer uses `first_layer_speed = 50%` meaning 50% of default speed. Our PrintConfig uses absolute speeds.
   - What's unclear: How to handle percentage speeds -- skip them? Convert using a reference speed?
   - Recommendation: Skip percentage speed values during mapping (return `false` from field mapping). Log a warning. Only map absolute numeric speed values.

3. **Quality Name Mapping for PrusaSlicer**
   - What we know: PrusaSlicer uses quality terms: ULTRADETAIL, DETAIL, OPTIMAL, NORMAL, SPEED, FAST. OrcaSlicer uses: Extra Fine, Fine, Standard, Draft, Super Draft.
   - What's unclear: Whether `extract_quality_from_name()` needs updating.
   - Recommendation: Add PrusaSlicer quality terms to the extractor: ULTRADETAIL -> "Ultra Detail", DETAIL -> "Detail", OPTIMAL -> "Optimal", NORMAL -> "Normal", SPEED -> "Speed", FAST -> "Fast".

4. **Templates Vendor Handling**
   - What we know: `Templates.ini` contains generic base profiles usable by any printer.
   - What's unclear: Whether to include these as a separate "Templates" vendor or skip them.
   - Recommendation: Include them as vendor "Templates" -- they provide useful generic profiles (Generic PLA, Generic PETG, Generic ABS) that apply to any printer.

5. **Profile Count Expectations**
   - What we know: ~9,523 concrete profiles across 35 FFF vendors; largest vendor (PrusaResearch) has 6,341 concrete profiles.
   - What's unclear: How many will successfully map to PrintConfig (many fields are unmappable metadata like G-code snippets).
   - Recommendation: Convert all concrete profiles. Each will map a subset of fields (expect 5-15 mapped fields per profile, 50-200+ unmapped). This matches the OrcaSlicer experience.

## Sources

### Primary (HIGH confidence)
- Direct file analysis of `/home/steve/slicer-analysis/PrusaSlicer/resources/profiles/` (35 FFF vendor INI files)
- Direct analysis of `PrusaResearch.ini` (40,369 lines, 6,761 sections, 329 unique field names)
- Direct analysis of existing Phase 15 code: `profile_import.rs`, `profile_convert.rs`, `profile_library.rs`
- Direct analysis of existing CLI: `slicecore-cli/src/main.rs`

### Secondary (MEDIUM confidence)
- Phase 15 RESEARCH.md findings about OrcaSlicer structure and directory layout
- PrusaSlicer `ArchiveRepositoryManifest.json` for vendor repository structure

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries already in workspace, verified in code
- Architecture: HIGH - INI format fully analyzed from actual source files, field mapping verified
- Pitfalls: HIGH - All pitfalls derived from direct analysis of actual INI files and existing code
- Field mapping: MEDIUM - Core fields verified, edge cases (percentage speeds, multi-value nozzle_diameter) need validation during implementation

**Research date:** 2026-02-19
**Valid until:** 2026-03-19 (PrusaSlicer INI format is stable; profiles update but format doesn't change)
