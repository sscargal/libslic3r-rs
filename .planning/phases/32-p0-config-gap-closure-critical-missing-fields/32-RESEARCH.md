# Phase 32: P0 Config Gap Closure - Critical Missing Fields - Research

**Researched:** 2026-03-16
**Domain:** Rust config struct expansion, profile import mapping, serde serialization
**Confidence:** HIGH

## Summary

Phase 32 adds ~16 critical config fields to PrintConfig that upstream slicers (OrcaSlicer, BambuStudio, PrusaSlicer) commonly set and that affect print quality. This is a config-only phase -- fields are stored, serialized, round-tripped, and exposed as G-code template variables, but NOT wired into engine behavior.

The existing codebase has well-established patterns for all required work: sub-struct organization with `#[serde(default)]`, field mapping in `profile_import.rs` via `apply_field_mapping()` match arms, PrusaSlicer INI mapping in `profile_import_ini.rs` via `apply_prusaslicer_field_mapping()` match arms, enum creation following `InfillPattern`/`WallOrder` patterns, template variable resolution in `config_validate.rs`, and validation in the same module.

The main complexity lies in: (1) creating a new `DimensionalCompensationConfig` sub-struct and migrating `elefant_foot_compensation` into it (breaking TOML change), (2) creating `SurfacePattern` and `BedType` enums with upstream mapping, (3) the dual-representation fields (`chamber_temperature` in both MachineConfig and FilamentPropsConfig, `z_offset` as global + per-filament).

**Primary recommendation:** Follow the established Phase 20 patterns exactly. Group work by: new types/enums first, then config struct additions, then JSON mapping, then INI mapping, then template variables + validation, then tests, then profile re-conversion.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Config + mapping only -- fields are stored, serialized, round-tripped, but NOT wired into the slicing engine pipeline
- Engine behavior (e.g., actually applying xy_compensation to geometry) comes in a later phase
- Emit G-code comments for all new fields (visible in output for verification)
- Emit standard M-codes where obvious: chamber_temp -> M141, z_offset -> M851/start G-code
- All new fields available as G-code template variables (e.g., `{chamber_temperature}` in start/end G-code)
- All new fields included in the G-code reproduce command
- Migrate fields from passthrough (BTreeMap) to typed -- once typed, remove from passthrough
- Default values match OrcaSlicer defaults (imported profiles behave the same as in OrcaSlicer)
- Vec<f64> where upstream stores arrays (per Phase 20 pattern)
- Our own Rust naming convention -- profile_import mapper handles upstream->ours translation
- Unified semantics we define -- both OrcaSlicer and PrusaSlicer mappers translate to our canonical meaning
- Basic range validation per field (warn on out-of-range, error on dangerous values per Phase 30 model)
- Full Rust doc comments on every new field (units, range, description) -- prepares for Phase 35 ConfigSchema derive macro
- TOML serialization includes inline comments for all fields (self-documenting saved configs)
- Both OrcaSlicer JSON AND PrusaSlicer INI mappings added in this phase (all three import paths in sync)
- Full re-conversion of all ~21k profiles using existing import-profiles pipeline after adding new mappings
- Re-conversion is a separate plan (can be re-run independently)
- DimensionalCompensationConfig groups: xy_hole_compensation, xy_contour_compensation, elephant_foot_compensation (migrated)
- elephant_foot_compensation migration is a breaking TOML format change (acceptable per Phase 20 precedent)
- SurfacePattern enum: Rectilinear, Monotonic, MonotonicLine, Concentric, Hilbert, Archimedean (default: Monotonic)
- SurfacePattern is separate from InfillPattern (unsuitable patterns excluded)
- BedType enum: CoolPlate, EngineeringPlate, HighTempPlate, TexturedPEI, SmoothPEI, SatinPEI
- Our own snake_case naming for BedType -- import mapper translates from OrcaSlicer strings
- Per-type temperature fields in FilamentPropsConfig
- Auto-resolve bed_temperature from per-type temps + selected bed type
- Keep both resolved values AND raw per-type temps in final config
- InternalBridgeMode enum: Off, Auto, Always (not bool)

### Claude's Discretion
- Placement of surface pattern fields (top-level vs sub-struct)
- Exact SurfacePattern enum variant names
- Internal bridge support field placement
- Exact min_length acceleration field names and count (check OrcaSlicer source)
- Per-type temperature field naming in FilamentPropsConfig
- Order of fields within sub-structs

### Deferred Ideas (OUT OF SCOPE)
- Profile management CLI -- `slicecore profiles` subcommand grouping
- Upstream sync workflow -- tooling to pull latest profiles from upstream repos
- Config migration system -- versioned config format with automatic migration
- Engine behavior for P0 fields -- wiring xy_compensation into geometry, filament_shrink into scaling, etc.
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde | 1.x | Serialize/Deserialize derive for all config structs | Already used throughout config.rs |
| serde_json | 1.x | JSON profile parsing in profile_import.rs | Already used for upstream import |
| toml | 0.8.x | TOML config serialization/deserialization | Already used for native config format |

### Supporting
No new dependencies required. All work uses existing crate infrastructure.

## Architecture Patterns

### Existing Config Sub-Struct Pattern (MUST follow)

Every new sub-struct follows this exact pattern from `config.rs`:

```rust
/// Doc comment with description, units, and value semantics.
///
/// [Detailed explanation of field relationships]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DimensionalCompensationConfig {
    /// XY hole compensation in mm. Negative values shrink holes (typical).
    /// Range: -2.0 to 2.0. Default: 0.0 (no compensation).
    pub xy_hole_compensation: f64,
    /// XY contour compensation in mm. Positive values expand contours.
    /// Range: -2.0 to 2.0. Default: 0.0 (no compensation).
    pub xy_contour_compensation: f64,
    /// Elephant foot compensation in mm (first layer shrinkage correction).
    /// Range: 0.0 to 2.0. Default: 0.0.
    /// Migrated from PrintConfig.elefant_foot_compensation.
    pub elephant_foot_compensation: f64,
}

impl Default for DimensionalCompensationConfig {
    fn default() -> Self {
        Self {
            xy_hole_compensation: 0.0,
            xy_contour_compensation: 0.0,
            elephant_foot_compensation: 0.0,
        }
    }
}
```

Key points:
- `#[derive(Debug, Clone, Serialize, Deserialize)]` -- always all four
- `#[serde(default)]` -- always present for backward-compatible deserialization
- Explicit `impl Default` (not `#[derive(Default)]`) for documenting default values
- Full doc comments with units, range, and description on every field

### Existing Enum Pattern (for SurfacePattern, BedType, InternalBridgeMode)

Follow the `WallOrder` / `InfillPattern` pattern exactly:

```rust
/// Fill pattern for solid surfaces (top, bottom, internal solid).
///
/// This is separate from [`InfillPattern`] because solid surfaces
/// use a restricted subset of patterns suitable for dense fills.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfacePattern {
    /// Parallel lines (alternating direction per layer).
    Rectilinear,
    /// Unidirectional monotonic lines for smooth surfaces.
    #[default]
    Monotonic,
    /// Monotonic lines (single-line variant).
    MonotonicLine,
    /// Concentric inward-spiraling pattern.
    Concentric,
    /// Hilbert space-filling curve pattern.
    Hilbert,
    /// Archimedean spiral pattern.
    Archimedean,
}
```

Key points:
- `#[serde(rename_all = "snake_case")]` for TOML/JSON compatibility
- `#[default]` attribute on the default variant
- All derives: `Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize`

### Existing Field Mapping Pattern (profile_import.rs)

New scalar fields are added to the `apply_field_mapping()` match:

```rust
// In apply_field_mapping():
"xy_hole_compensation" => parse_and_set_f64(value, &mut config.dimensional_compensation.xy_hole_compensation),
"xy_contour_compensation" => parse_and_set_f64(value, &mut config.dimensional_compensation.xy_contour_compensation),
"extra_perimeters_on_overhangs" => {
    config.extra_perimeters_on_overhangs = value == "1" || value == "true";
    true
}
```

Enum fields need mapping functions (like `map_infill_pattern`):

```rust
fn map_surface_pattern(value: &str) -> Option<SurfacePattern> {
    match value.to_lowercase().as_str() {
        "rectilinear" | "zig-zag" => Some(SurfacePattern::Rectilinear),
        "monotonic" => Some(SurfacePattern::Monotonic),
        "monotonicline" | "monotonic_line" => Some(SurfacePattern::MonotonicLine),
        "concentric" => Some(SurfacePattern::Concentric),
        "hilbertcurve" | "hilbert" => Some(SurfacePattern::Hilbert),
        "archimedeanchords" | "archimedean" => Some(SurfacePattern::Archimedean),
        _ => None,
    }
}
```

### Existing INI Mapping Pattern (profile_import_ini.rs)

Two-stage mapping:
1. `prusaslicer_key_to_config_field()` -- maps INI key names to our field names
2. `apply_prusaslicer_field_mapping()` -- actually sets values on PrintConfig

New fields need entries in both functions.

### Template Variable Pattern (config_validate.rs)

Add to `resolve_variable()` match:

```rust
"chamber_temperature" => Some(format!("{}", config.filament.chamber_temperature)),
"z_offset" => Some(format!("{}", config.z_offset)),
"xy_hole_compensation" => Some(format!("{}", config.dimensional_compensation.xy_hole_compensation)),
```

### Recommended Project Structure for Changes

```
crates/slicecore-engine/src/
  config.rs                    # Add new sub-structs, enums, fields to PrintConfig
  config_validate.rs           # Add validation rules + template variables
  profile_import.rs            # Add OrcaSlicer JSON field mappings
  profile_import_ini.rs        # Add PrusaSlicer INI field mappings
  gcode_gen.rs                 # Add M-code emission + G-code comments
crates/slicecore-engine/tests/
  config_integration.rs        # Update existing tests
  golden_tests.rs              # May need updates if G-code header changes
```

### Anti-Patterns to Avoid
- **Adding fields to PrintConfig top-level when a sub-struct exists:** Always group related fields in sub-structs (e.g., xy_hole_compensation goes in DimensionalCompensationConfig, not top-level)
- **Using #[derive(Default)] for config structs:** Always use explicit `impl Default` so default values are documented and visible
- **Forgetting `#[serde(default)]`:** Every config struct MUST have this or deserialization of old configs will fail
- **Breaking the passthrough cleanup:** When a field moves from passthrough to typed, the mapping arm must NOT also store in passthrough. Remove the old passthrough storage.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Value parsing | Custom f64/u32 parsers | Existing `parse_and_set_f64`, `parse_and_set_u32` | Already handle edge cases |
| Percentage parsing | Custom % handling | Existing `parse_percentage_or_f64` | Handles both "50%" and "50" |
| Bool from string | Custom bool parser | `value == "1" \|\| value == "true"` pattern | Matches upstream "1"/"0"/"true"/"false" |
| Array field extraction | Custom JSON array parser | Existing `extract_string_value` + `apply_array_field_mapping` | Handles nil sentinels, array wrapping |
| INI value parsing | Custom INI parsers | Existing `parse_comma_separated_f64`, `parse_bool` | Already handle PrusaSlicer quirks |

**Key insight:** All parsing infrastructure exists. The work is purely adding match arms and struct fields, not building new parsing machinery.

## Common Pitfalls

### Pitfall 1: elephant_foot Migration Breaking Deserialization
**What goes wrong:** Moving `elefant_foot_compensation` from `PrintConfig` top-level to `DimensionalCompensationConfig` breaks existing TOML configs that have `elefant_foot_compensation = 0.2` at the top level.
**Why it happens:** Serde expects the field inside `[dimensional_compensation]` table now.
**How to avoid:** Add `#[serde(alias = "elefant_foot_compensation")]` to the new field OR add a custom deserializer OR accept the break (CONTEXT.md says "acceptable per Phase 20 precedent"). Since the field name also changes from `elefant_foot` to `elephant_foot`, consider keeping a deprecated alias.
**Warning signs:** Existing tests that set `elefant_foot_compensation` directly will fail.

### Pitfall 2: Passthrough Pollution After Typed Migration
**What goes wrong:** Fields like `xy_hole_compensation` currently go to passthrough when imported from upstream profiles. After adding a typed field, the mapping arm handles it, but if the passthrough default arm is also reached (e.g., slight key name mismatch), the value ends up in both places.
**Why it happens:** The match arms need exact key names matching upstream.
**How to avoid:** Check that the upstream key names are exact. Test that after import, the field is in the typed struct AND NOT in passthrough.
**Warning signs:** `passthrough_fields` list still contains keys that should now be typed.

### Pitfall 3: BedType Temperature Resolution Complexity
**What goes wrong:** The bed type system has 6+ per-type temperature fields that need to auto-resolve to the final `bed_temperatures` based on `curr_bed_type`. This resolution logic can silently produce wrong temperatures.
**Why it happens:** OrcaSlicer stores per-type temps separately (`hot_plate_temp`, `cool_plate_temp`, etc.) and resolves at slice time.
**How to avoid:** Implement resolution as an explicit method on the config (e.g., `resolve_bed_temperatures()`) called after profile merge, not during import. Keep raw per-type temps always available. Add tests for each bed type variant.
**Warning signs:** Wrong bed temperatures in G-code output.

### Pitfall 4: Forgetting to Update All Three Import Paths
**What goes wrong:** Adding a field to config.rs and profile_import.rs but forgetting profile_import_ini.rs (or vice versa), causing PrusaSlicer profiles to miss the field.
**Why it happens:** Three files must stay in sync: config.rs, profile_import.rs, profile_import_ini.rs.
**How to avoid:** Use a checklist per field. Add integration tests that verify all three paths.
**Warning signs:** Field coverage differs between JSON and INI import results.

### Pitfall 5: SurfacePattern vs InfillPattern Confusion
**What goes wrong:** Using InfillPattern for solid surface patterns, which includes unsuitable patterns like Lightning and Gyroid.
**Why it happens:** Both are "fill patterns" but solid surfaces need a restricted subset.
**How to avoid:** SurfacePattern is a separate enum per CONTEXT.md. Never use InfillPattern for top/bottom/solid_infill pattern fields.
**Warning signs:** Lightning or Gyroid appearing in top/bottom surface pattern options.

## Code Examples

### New Sub-Struct Addition (verified pattern from config.rs)

```rust
// Source: crates/slicecore-engine/src/config.rs lines 44-75
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DimensionalCompensationConfig {
    /// XY hole compensation in mm. Negative shrinks holes for tighter fits.
    /// OrcaSlicer: xy_hole_compensation. PrusaSlicer: xy_size_compensation (partial).
    /// Range: -2.0 to 2.0 mm. Default: 0.0.
    pub xy_hole_compensation: f64,
    /// XY contour compensation in mm. Positive expands outer contours.
    /// OrcaSlicer: xy_contour_compensation. PrusaSlicer: xy_size_compensation.
    /// Range: -2.0 to 2.0 mm. Default: 0.0.
    pub xy_contour_compensation: f64,
    /// Elephant foot compensation in mm (first layer inward offset).
    /// Migrated from PrintConfig.elefant_foot_compensation.
    /// OrcaSlicer: elefant_foot_compensation. PrusaSlicer: elefant_foot_compensation.
    /// Range: 0.0 to 2.0 mm. Default: 0.0.
    pub elephant_foot_compensation: f64,
}
```

### Field Mapping Addition (verified pattern from profile_import.rs)

```rust
// Source: crates/slicecore-engine/src/profile_import.rs lines 544-954
// In apply_field_mapping() match block:
"xy_hole_compensation" => {
    parse_and_set_f64(value, &mut config.dimensional_compensation.xy_hole_compensation)
}
"xy_contour_compensation" => {
    parse_and_set_f64(value, &mut config.dimensional_compensation.xy_contour_compensation)
}
// Note: elefant_foot_compensation already mapped -- redirect to new location:
"elefant_foot_compensation" => {
    parse_and_set_f64(value, &mut config.dimensional_compensation.elephant_foot_compensation)
}
"chamber_temperature" => {
    parse_and_set_f64(value, &mut config.filament.chamber_temperature)
}
```

### Enum Mapping Function (verified pattern from profile_import.rs)

```rust
// Source: crates/slicecore-engine/src/profile_import.rs lines 982-993
fn map_surface_pattern(value: &str) -> Option<SurfacePattern> {
    match value.to_lowercase().as_str() {
        "rectilinear" | "zig-zag" | "line" => Some(SurfacePattern::Rectilinear),
        "monotonic" => Some(SurfacePattern::Monotonic),
        "monotonicline" => Some(SurfacePattern::MonotonicLine),
        "concentric" => Some(SurfacePattern::Concentric),
        "hilbertcurve" => Some(SurfacePattern::Hilbert),
        "archimedeanchords" => Some(SurfacePattern::Archimedean),
        _ => None,
    }
}
```

### Template Variable Addition (verified pattern from config_validate.rs)

```rust
// Source: crates/slicecore-engine/src/config_validate.rs lines 233-242
fn resolve_variable(name: &str, config: &PrintConfig) -> Option<String> {
    match name {
        // ... existing variables ...
        "chamber_temperature" => Some(format!("{}", config.filament.chamber_temperature)),
        "z_offset" => Some(format!("{}", config.z_offset)),
        "xy_hole_compensation" => {
            Some(format!("{}", config.dimensional_compensation.xy_hole_compensation))
        }
        // ... more new variables ...
        _ => None,
    }
}
```

## Field-by-Field Implementation Reference

### Complete P0 Field List with Implementation Details

| # | Our Field Name | Type | Sub-struct | OrcaSlicer Key | PrusaSlicer Key | Default |
|---|---------------|------|-----------|----------------|-----------------|---------|
| 1 | `chamber_temperature` | f64 | MachineConfig (max) + FilamentPropsConfig (desired) | `chamber_temperature` | N/A (not in PrusaSlicer) | 0.0 |
| 2 | `xy_hole_compensation` | f64 | DimensionalCompensationConfig (NEW) | `xy_hole_compensation` | `xy_size_compensation` (partial) | 0.0 |
| 3 | `xy_contour_compensation` | f64 | DimensionalCompensationConfig (NEW) | `xy_contour_compensation` | `xy_size_compensation` | 0.0 |
| 4 | `elephant_foot_compensation` | f64 | DimensionalCompensationConfig (MIGRATE) | `elefant_foot_compensation` | `elefant_foot_compensation` / `elephant_foot_compensation` | 0.0 |
| 5 | `extra_perimeters_on_overhangs` | bool | PrintConfig top-level | `extra_perimeters_on_overhangs` | `extra_perimeters_over_overhangs` | false |
| 6 | `top_surface_pattern` | SurfacePattern | PrintConfig top-level (recommended) | `top_surface_pattern` | `top_fill_pattern` | Monotonic |
| 7 | `bottom_surface_pattern` | SurfacePattern | PrintConfig top-level (recommended) | `bottom_surface_pattern` | `bottom_fill_pattern` | Monotonic |
| 8 | `solid_infill_pattern` | SurfacePattern | PrintConfig top-level (recommended) | `internal_solid_infill_pattern` | `solid_fill_pattern` | Monotonic |
| 9 | `internal_bridge_speed` | f64 | SpeedConfig | `internal_bridge_speed` | N/A | 0.0 |
| 10 | `internal_bridge_support` | InternalBridgeMode | PrintConfig or relevant sub | `internal_bridge_support_enabled` | N/A | Off |
| 11 | `filament_shrink` | f64 | FilamentPropsConfig | `filament_shrinkage_compensation` | N/A | 100.0 (100% = no shrink) |
| 12 | `z_offset` | f64 | PrintConfig (global) + FilamentPropsConfig (per-filament additive) | `z_offset` | `z_offset` | 0.0 |
| 13 | `curr_bed_type` | BedType | MachineConfig | `curr_bed_type` | N/A | TexturedPEI |
| 14 | `min_length` accel fields | f64 set | AccelerationConfig | Various `min_length_*` | N/A | 0.0 |
| 15 | `precise_z_height` | bool | PrintConfig top-level | `precise_z_height` | N/A | false |

### BedType Per-Type Temperature Fields in FilamentPropsConfig

OrcaSlicer filament profiles store per-bed-type temperatures as arrays:

| Our Field Name | OrcaSlicer Key | Description |
|---------------|----------------|-------------|
| `hot_plate_temp` | `hot_plate_temp` | Smooth PEI / high-temp plate temps |
| `cool_plate_temp` | `cool_plate_temp` | Cool plate temps |
| `eng_plate_temp` | `eng_plate_temp` | Engineering plate temps |
| `textured_plate_temp` | `textured_plate_temp` | Textured PEI plate temps |
| `hot_plate_temp_initial_layer` | `hot_plate_temp_initial_layer` | First layer smooth PEI temps |
| `cool_plate_temp_initial_layer` | `cool_plate_temp_initial_layer` | First layer cool plate temps |
| `eng_plate_temp_initial_layer` | `eng_plate_temp_initial_layer` | First layer engineering plate temps |
| `textured_plate_temp_initial_layer` | `textured_plate_temp_initial_layer` | First layer textured PEI temps |

These are Vec<f64> (multi-extruder arrays). The existing `bed_temperatures` and `first_layer_bed_temperatures` remain as the "resolved" values.

### Acceleration min_length Fields

Based on OrcaSlicer wiki and source, the acceleration section has "minimum segment length" fields that prevent acceleration changes on very short segments. The exact field set in OrcaSlicer:

- `min_length_factor` -- global scaling factor (percentage, applied to all min_length values)

This is a single field, not a full set. If additional min_length variants exist in OrcaSlicer source, they should be discovered during implementation. Conservative recommendation: add `min_length_factor` as f64 to AccelerationConfig, default 0.0.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Top-level flat fields in PrintConfig | Grouped sub-structs (Phase 20) | Phase 20 | All new fields go in sub-structs |
| Unmapped fields silently lost | Passthrough BTreeMap catch-all (Phase 20) | Phase 20 | Unmapped fields preserved for round-trip |
| Single elephant_foot field name | Both `elefant_foot` and `elephant_foot` accepted | Phase 20 | New canonical name: `elephant_foot_compensation` in DimensionalCompensationConfig |

## Open Questions

1. **Exact min_length acceleration field names in OrcaSlicer**
   - What we know: There is at least `min_length_factor` as a percentage
   - What's unclear: Whether there are per-feature `min_length_*` variants or just the one global factor
   - Recommendation: During implementation, check OrcaSlicer source code or a real OrcaSlicer profile JSON for all `min_length` keys. Start with the known field, add others if discovered.

2. **PrusaSlicer xy_size_compensation mapping**
   - What we know: PrusaSlicer has `xy_size_compensation` which applies uniformly to both holes and contours
   - What's unclear: Best mapping strategy when our model splits into two fields (hole vs contour)
   - Recommendation: Map PrusaSlicer `xy_size_compensation` to `xy_contour_compensation` (its primary use case). Leave `xy_hole_compensation` at default for PrusaSlicer imports.

3. **Reproduce command format**
   - What we know: G-code contains `; Reproduce: slicecore slice` header (from cli_slice_profiles test)
   - What's unclear: Whether new fields automatically appear or need explicit inclusion
   - Recommendation: Investigate how the reproduce command is generated (likely in slice_workflow.rs) and ensure new fields are included.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in Rust test framework) |
| Config file | Cargo.toml per crate |
| Quick run command | `cargo test -p slicecore-engine -- config` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| P32-01 | New fields exist with correct defaults | unit | `cargo test -p slicecore-engine -- config::tests` | Yes (update) |
| P32-02 | TOML round-trip for new fields | unit | `cargo test -p slicecore-engine -- config::tests` | Yes (update) |
| P32-03 | JSON import maps new OrcaSlicer keys | unit | `cargo test -p slicecore-engine -- profile_import::tests` | Yes (update) |
| P32-04 | INI import maps new PrusaSlicer keys | unit | `cargo test -p slicecore-engine -- profile_import_ini::tests` | Yes (update) |
| P32-05 | Template variables resolve for new fields | unit | `cargo test -p slicecore-engine -- config_validate::tests` | Yes (update) |
| P32-06 | Validation warns on out-of-range values | unit | `cargo test -p slicecore-engine -- config_validate::tests` | Yes (update) |
| P32-07 | Passthrough cleanup (typed fields not in passthrough) | integration | `cargo test -p slicecore-engine -- profile_import` | Yes (update) |
| P32-08 | elephant_foot migration works | unit | `cargo test -p slicecore-engine -- config::tests` | Wave 0 |
| P32-09 | BedType temperature resolution | unit | `cargo test -p slicecore-engine -- config::tests` | Wave 0 |
| P32-10 | Profile re-conversion with new fields | integration | CLI import-profiles pipeline | Manual |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-engine -- config profile_import config_validate`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] Test for elephant_foot migration (old TOML -> new sub-struct path)
- [ ] Test for BedType temperature auto-resolution
- [ ] Test for each new enum variant serialization round-trip
- [ ] Tests for per-bed-type temperature import from OrcaSlicer JSON

## Sources

### Primary (HIGH confidence)
- `crates/slicecore-engine/src/config.rs` -- Current PrintConfig structure, all sub-structs, Default impls (~1500 lines reviewed)
- `crates/slicecore-engine/src/profile_import.rs` -- OrcaSlicer JSON field mapping, `apply_field_mapping()` pattern (~1000 lines reviewed)
- `crates/slicecore-engine/src/profile_import_ini.rs` -- PrusaSlicer INI mapping, `apply_prusaslicer_field_mapping()` pattern (~1000 lines reviewed)
- `crates/slicecore-engine/src/config_validate.rs` -- Template variable resolution and validation rules
- `designDocs/CONFIG_PARITY_AUDIT.md` -- Complete field inventory, P0/P1/P2 gap analysis
- `.planning/phases/32-p0-config-gap-closure-critical-missing-fields/32-CONTEXT.md` -- User decisions and field list

### Secondary (MEDIUM confidence)
- [OrcaSlicer print config reference (r6e.dev)](https://r6e.dev/orcaslicer/config_reference/print.html) -- Field names and types for xy_hole_compensation, xy_contour_compensation, extra_perimeters_on_overhangs, top/bottom_surface_pattern, internal_bridge_speed, internal_solid_infill_pattern
- [OrcaSlicer bed types wiki](https://github.com/OrcaSlicer/OrcaSlicer/wiki/bed-types) -- Bed type system and per-type temperature fields
- [OrcaSlicer acceleration wiki](https://github.com/OrcaSlicer/OrcaSlicer/wiki/speed_settings_acceleration) -- Acceleration configuration fields
- [Mastering Bed Types in OrcaSlicer (minimal3dp.com)](https://minimal3dp.com/blog/orcaslicer-bed-types/) -- curr_bed_type usage, per-type temperature variable names

### Tertiary (LOW confidence)
- min_length acceleration fields -- exact OrcaSlicer field names not fully verified from source. Implementation should check a real OrcaSlicer process profile JSON for all `min_length` keys.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies, all patterns established
- Architecture: HIGH - follows exact patterns from Phase 20 config expansion
- Pitfalls: HIGH - identified from direct code analysis of existing implementation
- Field mapping details: MEDIUM - OrcaSlicer field names verified from wiki/docs but min_length fields need source verification

**Research date:** 2026-03-16
**Valid until:** 2026-04-16 (stable domain, no external dependencies changing)
