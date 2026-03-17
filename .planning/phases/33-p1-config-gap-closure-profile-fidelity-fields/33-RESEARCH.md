# Phase 33: P1 Config Gap Closure -- Profile Fidelity Fields - Research

**Researched:** 2026-03-17
**Domain:** Rust config struct expansion, serde serialization, profile import field mapping
**Confidence:** HIGH

## Summary

Phase 33 adds ~30 P1-priority config fields to `PrintConfig` that OrcaSlicer/BambuStudio/PrusaSlicer profiles commonly set. This is a direct continuation of Phase 32's P0 config gap closure pattern: config + mapping only, no engine behavior wiring. The work involves creating 4 new sub-structs (FuzzySkinConfig, BrimSkirtConfig, InputShapingConfig, ToolChangeRetractionConfig), extending 5 existing sub-structs (AccelerationConfig, CoolingConfig, SpeedConfig, FilamentPropsConfig, MultiMaterialConfig), and adding ~5 top-level fields to PrintConfig.

All patterns are well-established from Phase 32 and Phase 20. The codebase has clear conventions for sub-struct creation (`#[serde(default)]`, `impl Default`, doc comments with units/range/OrcaSlicer key), field mapping in both `profile_import.rs` (JSON) and `profile_import_ini.rs` (INI), G-code template variable registration in `config_validate.rs`, and config validation. The main complexity is volume -- ~30 fields across 3 mapping files, 1 config file, 1 validation file, and tests.

**Primary recommendation:** Follow Phase 32 patterns exactly. Split into 4 plans: (1) new sub-structs + new enums, (2) extend existing sub-structs + top-level fields, (3) JSON/INI mapping + G-code template variables + validation, (4) tests + profile re-conversion.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Create new sub-structs: FuzzySkinConfig (3 fields), BrimSkirtConfig (4+ fields), InputShapingConfig (2 fields), ToolChangeRetractionConfig (2 fields)
- Extend existing: AccelerationConfig (+3), CoolingConfig (+2), SpeedConfig (+1), FilamentPropsConfig (+1), MultiMaterialConfig (+4)
- Top-level PrintConfig fields: precise_outer_wall, draft_shield, ooze_prevention, infill_combination, infill_anchor_max
- Arachne fields: min_bead_width, min_feature_size
- Support field: support_bottom_interface_layers
- BrimType enum: None, Outer, Inner, Both (Rust-idiomatic, mapper translates OrcaSlicer strings)
- Filament index fields use 0-based Option<usize>, mapper translates OrcaSlicer 1-based
- Config + mapping only -- no engine behavior changes
- Migrate from passthrough to typed, remove from passthrough once typed
- OrcaSlicer defaults as baseline
- Both JSON AND INI mappings added together
- Full Rust doc comments (units, range, description -- Phase 35 prep)
- TOML inline comments for self-documenting configs
- G-code template variables for all new fields
- Basic range validation per field
- Full re-conversion of ~21k profiles after adding mappings

### Claude's Discretion
- ToolChangeRetractionConfig placement (nested in MultiMaterialConfig vs top-level on PrintConfig)
- Arachne fields placement (top-level vs new ArachneConfig sub-struct)
- draft_shield type (bool vs DraftShieldMode enum)
- Exact field ordering within sub-structs
- G-code template variable naming for new fields
- Which existing brim/skirt fields to migrate into BrimSkirtConfig

### Deferred Ideas (OUT OF SCOPE)
- P2 tool-change retraction fields (cooling_tube_length, cooling_tube_retraction, parking_pos_retraction, extra_loading_move)
- Profile migration tooling (versioned config format)
- Vendor-specific config extensions
- Advanced fan curves
- Engine behavior for P1 fields (fuzzy skin generation, brim ears, input shaping, etc.)
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde | 1.x | Serialization/deserialization with `#[serde(default)]` | Already used throughout config.rs |
| serde_json | 1.x | JSON profile import parsing | Already used in profile_import.rs |
| toml | 0.8.x | TOML config serialization/deserialization | Already used for native config format |

No new dependencies needed. Phase 33 is purely extending existing structs and mapping tables.

## Architecture Patterns

### Existing Config Structure (extend this)
```
crates/slicecore-engine/src/
  config.rs              -- PrintConfig + all sub-structs + enums (~1500 lines)
  config_validate.rs     -- validate_config() + resolve_template_variables()
  profile_import.rs      -- OrcaSlicer/BambuStudio JSON field mapping
  profile_import_ini.rs  -- PrusaSlicer INI field mapping
```

### Pattern 1: New Sub-Struct (from Phase 32 DimensionalCompensationConfig)
**What:** Self-contained config group with `#[serde(default)]`, manual `Default` impl, doc comments
**When to use:** For each of FuzzySkinConfig, BrimSkirtConfig, InputShapingConfig, ToolChangeRetractionConfig

```rust
/// Fuzzy skin configuration.
///
/// Adds random displacement to outer wall perimeters for a textured
/// surface finish. All values in mm.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FuzzySkinConfig {
    /// Enable fuzzy skin effect on outer walls.
    /// OrcaSlicer: `fuzzy_skin`. PrusaSlicer: `fuzzy_skin`.
    /// Default: false.
    pub enabled: bool,
    /// Maximum random displacement amplitude in mm.
    /// OrcaSlicer: `fuzzy_skin_thickness`. PrusaSlicer: `fuzzy_skin_thickness`.
    /// Range: 0.0-1.0. Default: 0.3.
    pub thickness: f64,
    /// Distance between displacement points along the wall in mm.
    /// OrcaSlicer: `fuzzy_skin_point_dist`. PrusaSlicer: `fuzzy_skin_point_distance`.
    /// Range: 0.1-5.0. Default: 0.8.
    pub point_distance: f64,
}

impl Default for FuzzySkinConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            thickness: 0.3,
            point_distance: 0.8,
        }
    }
}
```

### Pattern 2: New Enum (from Phase 32 SurfacePattern, BedType)
**What:** Rust-idiomatic enum with `#[serde(rename_all = "snake_case")]`, `#[default]` variant
**When to use:** BrimType enum

```rust
/// Brim adhesion type.
///
/// Controls where brim lines are placed relative to the object outline.
/// Import mappers translate OrcaSlicer strings ("outer_only", "inner_only",
/// "outer_and_inner", "no_brim") to these variants.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrimType {
    /// No brim generated.
    #[default]
    None,
    /// Brim on outer contour only.
    Outer,
    /// Brim on inner holes only.
    Inner,
    /// Brim on both outer contour and inner holes.
    Both,
}
```

### Pattern 3: Enum Mapping Helper (from map_surface_pattern, map_bed_type)
**What:** `pub(crate)` function mapping upstream string values to our enum
**When to use:** BrimType mapping

```rust
pub(crate) fn map_brim_type(value: &str) -> Option<BrimType> {
    match value.to_lowercase().replace(' ', "").as_str() {
        "no_brim" | "nobrim" | "none" => Some(BrimType::None),
        "outer_only" | "outeronly" | "outer" => Some(BrimType::Outer),
        "inner_only" | "inneronly" | "inner" => Some(BrimType::Inner),
        "outer_and_inner" | "outerandinner" | "both" => Some(BrimType::Both),
        _ => None,
    }
}
```

### Pattern 4: Field Mapping in apply_field_mapping (JSON)
**What:** Match arm in the large match block, using `parse_and_set_f64`, `parse_and_set_u32`, or boolean parsing
**When to use:** Every new field needs a mapping arm in both JSON and INI mappers

```rust
// In apply_field_mapping (profile_import.rs):
"fuzzy_skin" => {
    config.fuzzy_skin.enabled = value == "1" || value.eq_ignore_ascii_case("true");
    true
}
"fuzzy_skin_thickness" => parse_and_set_f64(value, &mut config.fuzzy_skin.thickness),
"fuzzy_skin_point_dist" => parse_and_set_f64(value, &mut config.fuzzy_skin.point_distance),
```

### Pattern 5: G-code Template Variable (from config_validate.rs resolve_variable)
**What:** Match arm in `resolve_variable()` returning `Option<String>`
**When to use:** Every new field

```rust
"fuzzy_skin" => Some(format!("{}", u8::from(config.fuzzy_skin.enabled))),
"fuzzy_skin_thickness" => Some(format!("{}", config.fuzzy_skin.thickness)),
```

### Pattern 6: Config Validation (from validate_config)
**What:** Range check producing `ValidationIssue` with Warning or Error severity
**When to use:** Fields with meaningful range constraints

```rust
// Fuzzy skin thickness > 1mm is suspicious
if config.fuzzy_skin.enabled && config.fuzzy_skin.thickness > 1.0 {
    issues.push(ValidationIssue {
        field: "fuzzy_skin.thickness".into(),
        message: format!("fuzzy skin thickness ({:.2} mm) is unusually large", config.fuzzy_skin.thickness),
        severity: ValidationSeverity::Warning,
        value: format!("{}", config.fuzzy_skin.thickness),
    });
}
```

### Pattern 7: 1-based to 0-based Filament Index Mapping
**What:** OrcaSlicer uses 1-based filament indices, we use 0-based `Option<usize>`
**When to use:** wall_filament, solid_infill_filament, support_filament, support_interface_filament

```rust
"wall_filament" => {
    if let Ok(v) = value.parse::<usize>() {
        config.multi_material.wall_filament = if v > 0 { Some(v - 1) } else { None };
        true
    } else {
        false
    }
}
```

### Discretion Recommendations

**ToolChangeRetractionConfig placement:** Nest inside MultiMaterialConfig. Rationale: tool-change retraction only applies when multi-material is active. Keeps the top-level PrintConfig cleaner. Access would be `config.multi_material.tool_change_retraction.retraction_distance_when_cut`.

**Arachne fields placement:** Add to PrintConfig top-level alongside `arachne_enabled`. Rationale: there are only 2 fields, and they are tightly coupled with the existing `arachne_enabled` bool. A separate ArachneConfig for 2 fields + 1 existing bool is overkill. If future phases add more Arachne fields, they can be factored into a sub-struct then.

**draft_shield type:** Use a bool. Rationale: OrcaSlicer and PrusaSlicer both treat it as boolean (enabled/disabled). No slicer uses a multi-mode enum for this feature. Keep it simple.

**Existing brim/skirt fields to migrate into BrimSkirtConfig:** Migrate `skirt_loops`, `skirt_distance`, `brim_width` from PrintConfig top-level into BrimSkirtConfig alongside the new fields. This creates a coherent group. Use `#[serde(alias)]` for backward compatibility with existing TOML configs.

### Anti-Patterns to Avoid
- **Forgetting INI mapper:** Every JSON mapping MUST have a corresponding INI mapping. Phase 32 established this.
- **Missing passthrough removal:** When adding a typed field, the corresponding key must stop going to passthrough in the default `_` arm. This happens automatically in the JSON mapper (typed match arm intercepts before `_`), but verify INI mapper follows same pattern.
- **Incorrect OrcaSlicer defaults:** Check OrcaSlicer source for actual default values; don't assume.
- **Missing G-code template variable:** Every new field must be added to `resolve_variable()`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Enum serde mapping | Manual from_str/to_str | `#[serde(rename_all = "snake_case")]` | Handles TOML/JSON serialization automatically |
| Percentage parsing | Custom parser per field | `parse_percentage_or_f64()` (exists in profile_import.rs) | Already handles `%` suffix consistently |
| Bool parsing from upstream | Custom truthiness checker | `value == "1" \|\| value.eq_ignore_ascii_case("true")` | Established pattern, handles OrcaSlicer's "1"/"0" convention |
| Profile re-conversion | New script | Existing `import-profiles` pipeline | Already handles ~21k profiles with parallel conversion |

## Common Pitfalls

### Pitfall 1: Backward Compatibility with Existing TOML Configs
**What goes wrong:** Migrating `skirt_loops` into BrimSkirtConfig changes TOML nesting, breaking existing saved configs.
**Why it happens:** TOML uses `[brim_skirt]\nskirt_loops = 1` instead of flat `skirt_loops = 1`.
**How to avoid:** Use `#[serde(alias = "skirt_loops")]` at the top-level AND the nested struct. Or better: keep the flat fields as deprecated aliases using a custom deserializer, or simply do NOT migrate existing flat fields in this phase -- only add new fields to BrimSkirtConfig, leave existing `skirt_loops`, `skirt_distance`, `brim_width` at the top-level. This avoids a breaking change.
**Warning signs:** Existing TOML configs fail to parse after the change.

### Pitfall 2: OrcaSlicer's BrimType String Values
**What goes wrong:** Mapping the wrong string values to BrimType variants.
**Why it happens:** OrcaSlicer uses `"outer_only"`, `"inner_only"`, `"outer_and_inner"`, `"no_brim"` but BambuStudio may use slightly different strings.
**How to avoid:** Map all known variants (including old/alternative names) in the mapping function. Test with real profiles.

### Pitfall 3: Filament Index Off-by-One
**What goes wrong:** Forgetting OrcaSlicer uses 1-based indices, our config uses 0-based.
**Why it happens:** Natural to copy the value directly.
**How to avoid:** Always subtract 1 in the mapper, use `Option<usize>` where `None` means "use default extruder" (OrcaSlicer's 0 means "use default").

### Pitfall 4: PrusaSlicer Key Name Differences
**What goes wrong:** PrusaSlicer uses different key names than OrcaSlicer for the same concept.
**Why it happens:** Two separate codebases with divergent naming.
**How to avoid:** For each field, check both OrcaSlicer and PrusaSlicer key names. Known differences:
- `fuzzy_skin_point_dist` (OrcaSlicer) vs `fuzzy_skin_point_distance` (PrusaSlicer -- note the full "distance")
- `retraction_distances_when_cut` (OrcaSlicer, note plural) vs no PrusaSlicer equivalent
- `brim_type` is the same in both
- `skirt_height` is the same in both

### Pitfall 5: Not Migrating Fields OUT of Passthrough
**What goes wrong:** A field is mapped to a typed field but also ends up in passthrough, causing double storage.
**Why it happens:** The JSON mapper's default `_` arm stores in passthrough. But if you add a named match arm, that field gets intercepted before reaching `_`. Just verify.
**How to avoid:** After adding mappings, run the profile import test and verify the field appears in `mapped_fields` not `passthrough_fields`.

### Pitfall 6: Breaking BrimSkirtConfig Migration
**What goes wrong:** Moving `skirt_loops`, `skirt_distance`, `brim_width` into BrimSkirtConfig breaks all existing code that accesses `config.skirt_loops`.
**Why it happens:** Dozens of locations in the engine reference these fields.
**How to avoid:** Recommended approach: Keep existing fields at top-level. Add NEW fields (brim_type, brim_ears, brim_ears_max_angle, skirt_height) to a new BrimSkirtConfig. Defer migration of existing fields to a future refactoring phase. This minimizes the blast radius.

## Code Examples

### Complete Sub-Struct + PrintConfig Integration
```rust
// In config.rs:

// 1. Define the struct
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InputShapingConfig {
    /// Enable accel-to-decel factor for input shaping.
    /// OrcaSlicer: `accel_to_decel_enable`. Default: false.
    pub accel_to_decel_enable: bool,
    /// Accel-to-decel factor ratio (0.0-1.0).
    /// OrcaSlicer: `accel_to_decel_factor`. Range: 0.0-1.0. Default: 0.5.
    pub accel_to_decel_factor: f64,
}

impl Default for InputShapingConfig {
    fn default() -> Self {
        Self {
            accel_to_decel_enable: false,
            accel_to_decel_factor: 0.5,
        }
    }
}

// 2. Add to PrintConfig
pub struct PrintConfig {
    // ...existing fields...
    /// Input shaping configuration.
    pub input_shaping: InputShapingConfig,
}

// 3. In PrintConfig::default()
input_shaping: InputShapingConfig::default(),
```

### JSON Mapping for Multi-Material Filament Index
```rust
// In profile_import.rs apply_field_mapping:
"wall_filament" => {
    if let Ok(v) = value.parse::<usize>() {
        config.multi_material.wall_filament = if v > 0 { Some(v - 1) } else { None };
        true
    } else {
        false
    }
}
```

### INI Mapping (profile_import_ini.rs pattern)
```rust
// In apply_prusaslicer_field_mapping:
"fuzzy_skin" => {
    config.fuzzy_skin.enabled = value == "1" || value.eq_ignore_ascii_case("true");
    true
}
"fuzzy_skin_thickness" => parse_and_set_f64(value, &mut config.fuzzy_skin.thickness),
"fuzzy_skin_point_distance" => {  // Note: PrusaSlicer uses full "distance"
    parse_and_set_f64(value, &mut config.fuzzy_skin.point_distance)
}

// In prusaslicer_key_to_config_field:
"fuzzy_skin" => Some("fuzzy_skin"),
"fuzzy_skin_thickness" => Some("fuzzy_skin_thickness"),
"fuzzy_skin_point_distance" => Some("fuzzy_skin_point_distance"),
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Flat fields on PrintConfig | Sub-struct grouping | Phase 20 | Better organization, prepared for Phase 35 ConfigSchema |
| Unmapped fields lost | Passthrough BTreeMap | Phase 20 | Round-trip fidelity for unknown fields |
| JSON-only mapping | JSON + INI mapping | Phase 11 | Full PrusaSlicer support |
| P0 fields only | P0 + P1 fields | Phase 32 -> 33 | Profile import accuracy improvement |

## Open Questions

1. **BrimSkirtConfig migration scope**
   - What we know: CONTEXT.md says to create BrimSkirtConfig with 4+ fields and "which existing brim/skirt fields to migrate" is Claude's discretion.
   - What's unclear: Whether migrating `skirt_loops`, `skirt_distance`, `brim_width` is worth the breaking-change risk.
   - Recommendation: Do NOT migrate existing flat fields into BrimSkirtConfig. Only add new fields (brim_type, brim_ears, brim_ears_max_angle, skirt_height) to the new struct. This avoids touching dozens of engine references and TOML backward compatibility issues. The struct name still makes logical sense as the container for brim/skirt-related additions.

2. **OrcaSlicer default values**
   - What we know: Most defaults can be inferred from OrcaSlicer source code.
   - What's unclear: Some fields have vendor-specific defaults (Bambu vs generic).
   - Recommendation: Use OrcaSlicer's generic defaults (not Bambu-specific) since our target is vendor-neutral config.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml workspace |
| Quick run command | `cargo test -p slicecore-engine --lib -- p1_` |
| Full suite command | `cargo test -p slicecore-engine` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command |
|--------|----------|-----------|-------------------|
| P1-01 | FuzzySkinConfig defaults + round-trip | unit | `cargo test -p slicecore-engine -- p1_fuzzy_skin` |
| P1-02 | BrimSkirtConfig + BrimType enum | unit | `cargo test -p slicecore-engine -- p1_brim` |
| P1-03 | InputShapingConfig defaults | unit | `cargo test -p slicecore-engine -- p1_input_shaping` |
| P1-04 | ToolChangeRetractionConfig | unit | `cargo test -p slicecore-engine -- p1_tool_change` |
| P1-05 | AccelerationConfig extensions | unit | `cargo test -p slicecore-engine -- p1_accel` |
| P1-06 | CoolingConfig extensions | unit | `cargo test -p slicecore-engine -- p1_cooling` |
| P1-07 | MultiMaterialConfig filament indices | unit | `cargo test -p slicecore-engine -- p1_multi_material` |
| P1-08 | JSON import mapping for all 30 fields | integration | `cargo test -p slicecore-engine -- p1_json_import` |
| P1-09 | INI import mapping for all 30 fields | integration | `cargo test -p slicecore-engine -- p1_ini_import` |
| P1-10 | G-code template variables for new fields | unit | `cargo test -p slicecore-engine -- p1_template` |
| P1-11 | Range validation for new fields | unit | `cargo test -p slicecore-engine -- p1_validation` |
| P1-12 | Profile re-conversion runs clean | integration | `cargo test -p slicecore-engine -- profile_reconversion` |

### Wave 0 Gaps
None -- existing test infrastructure covers all phase requirements. Tests will be added alongside implementation (same pattern as Phase 32).

## Sources

### Primary (HIGH confidence)
- `crates/slicecore-engine/src/config.rs` -- Current PrintConfig with all sub-structs, Phase 32 DimensionalCompensationConfig pattern
- `crates/slicecore-engine/src/profile_import.rs` -- JSON field mapping pattern, apply_field_mapping match arms
- `crates/slicecore-engine/src/profile_import_ini.rs` -- INI field mapping pattern, PrusaSlicer key translation
- `crates/slicecore-engine/src/config_validate.rs` -- Validation + G-code template variable resolution
- `designDocs/CONFIG_PARITY_AUDIT.md` lines 388-424 -- Complete P1 field list with OrcaSlicer keys
- `.planning/phases/32-p0-config-gap-closure-critical-missing-fields/32-CONTEXT.md` -- Phase 32 patterns

### Secondary (MEDIUM confidence)
- OrcaSlicer default values inferred from typical profile import behavior (verified against real converted profiles in the codebase)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, pure extension of existing patterns
- Architecture: HIGH -- Phase 32 established all patterns; Phase 33 is mechanical repetition
- Pitfalls: HIGH -- identified from direct code inspection of existing config/mapping code

**Research date:** 2026-03-17
**Valid until:** 2026-04-17 (stable domain, no external dependency changes expected)
