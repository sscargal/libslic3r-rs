# Phase 34: Support Config and Advanced Feature Profile Import Mapping - Research

**Researched:** 2026-03-17
**Domain:** Serde config structs, profile import field mapping, OrcaSlicer/PrusaSlicer upstream key translation
**Confidence:** HIGH

## Summary

Phase 34 is a "clean sweep" phase: map every remaining upstream slicer field to typed config fields, achieving 100% coverage of fields that have upstream equivalents. Five sub-structs (SupportConfig, ScarfJointConfig, MultiMaterialConfig, CustomGcodeHooks, PostProcessConfig) currently sit at 0% upstream mapping despite having fully defined internal types. Additionally, ~20 P2 niche fields need typed representation, and a G-code template variable translation table must be built.

The codebase already has robust, well-tested patterns for this exact work from Phases 20, 32, and 33. The `apply_field_mapping` match arm pattern in `profile_import.rs` (and its INI counterpart) is the standard mechanism. All five 0%-mapped sub-structs already exist as Rust types with defaults -- they just lack the `match` arms in the import mappers. The scope is large (~100+ new match arms across JSON and INI) but mechanically repetitive and low-risk.

**Primary recommendation:** Execute audit-first (scan real profiles for undiscovered keys), then batch new match arms by config section, adding both JSON and INI mappers together. The G-code variable translation table and coverage report are independent concerns that can be parallelized.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Map all 5 sections at 0% upstream coverage: SupportConfig+subs (27 fields), ScarfJointConfig (13), MultiMaterialConfig (7), CustomGcodeHooks (5), PostProcessConfig+subs (12)
- Include ALL ~20 P2 niche fields from audit (timelapse_type, thumbnails, silent_mode, slicing_tolerance, post_process, etc.)
- Pick up stragglers from partially-mapped sections (e.g., ironing_angle in IroningConfig, unmapped SequentialConfig fields)
- Add NEW fields to existing sections where upstream has fields we haven't defined yet
- Promote passthrough keys to typed fields where they have enough structure
- Target: 100% mapping coverage for all fields with upstream equivalents
- Meta/inheritance fields (compatible_printers_condition_cummulative, inherits_group) mapped as typed for future profile management
- Audit-first approach: comprehensive field inventory before implementation
- Unified superset approach for support: our TreeSupportConfig is the superset, both slicers map into it
- Define own SupportType enum covering both slicers (already exists: None, Auto, Traditional, Tree)
- Our custom fields (quality_preset, conflict_resolution) stay at defaults during import
- Map bridge params from BOTH OrcaSlicer JSON and PrusaSlicer INI
- Default + document pattern for unmappable fields
- Dual G-code storage: `start_gcode` (translated) + `start_gcode_original` (verbatim)
- Data-driven mapping table for variable translation
- Import coverage report (CLI + diagnostic)
- Full sweep of ~21k profiles with regression check
- Coverage improvement report in CLI + committed MAPPING_COVERAGE_REPORT.md
- Update CONFIG_PARITY_AUDIT.md Section 4
- Re-conversion plan designed as independently re-runnable
- Passthrough threshold test (<5% of upstream keys)
- Config + mapping only -- no engine behavior changes
- Patterns carried forward: OrcaSlicer defaults as baseline, both JSON + INI updated together, full doc comments, TOML inline comments, G-code template variables, G-code comments, basic range validation

### Claude's Discretion
- Exact field ordering within sub-structs
- Which passthrough keys qualify for typed promotion (based on audit findings)
- G-code template variable naming for new fields
- Exact SupportType enum variant names and mapping logic
- How to structure the mapping table data format
- Test profile selection for round-trip and threshold tests
- Report formatting details

### Deferred Ideas (OUT OF SCOPE)
- Profile diff tool
- Mapping health dashboard
- Upstream profile sync CI
- Profile validation linter
- Unmapped key reporter
- Profile migration system
- Profile recommendation engine
- Profile compression/dedup
- Slicer compatibility matrix
- Config field deprecation system
- Profile test harness
- Engine behavior for mapped fields
- Cross-slicer profile converter
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde | 1.x | Serialize/Deserialize derive for all config structs | Already used throughout; `#[serde(default)]` pattern established |
| serde_json | 1.x | JSON parsing for OrcaSlicer/BambuStudio profiles | Already used in profile_import.rs |
| toml | 0.8.x | TOML serialization/deserialization for native config | Already used for PrintConfig I/O |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| BTreeMap<String,String> | std | Passthrough catch-all for truly unknown keys | Already in PrintConfig.passthrough |
| HashMap<String,String> | std | G-code variable translation table | For the data-driven mapping table |

No new crate dependencies are needed. Phase 34 is purely additive Rust code using existing dependencies.

## Architecture Patterns

### Recommended Project Structure
```
crates/slicecore-engine/src/
  config.rs              # Add new P2 fields, new enums, extend existing sub-structs
  support/config.rs      # Already complete -- no structural changes, just mapping
  profile_import.rs      # Add ~50+ new match arms in apply_field_mapping
  profile_import_ini.rs  # Add ~40+ new match arms in INI mapper
  custom_gcode.rs        # Possibly add gcode_original fields
  gcode_template.rs      # NEW: G-code variable translation table (or in existing module)
```

### Pattern 1: Field Mapping Match Arm
**What:** Each upstream key maps to a typed field via a match arm in `apply_field_mapping`.
**When to use:** Every new field that has an upstream OrcaSlicer/PrusaSlicer equivalent.
**Example:**
```rust
// Source: crates/slicecore-engine/src/profile_import.rs
// Pattern already used 150+ times in the codebase
"enable_support" | "support_material" => {
    config.support.enabled = value == "1" || value.eq_ignore_ascii_case("true");
    true
}
"support_type" => {
    if let Some(st) = map_support_type(value) {
        config.support.support_type = st;
        true
    } else {
        false
    }
}
"support_threshold_angle" | "support_material_threshold" => {
    parse_and_set_f64(value, &mut config.support.overhang_angle)
}
```

### Pattern 2: Enum Mapping Helper
**What:** Private function translating upstream string values to our Rust enum variants.
**When to use:** For every enum field (SupportType, SupportPattern, InterfacePattern, TreeBranchStyle).
**Example:**
```rust
// Source: existing pattern from map_infill_pattern, map_surface_pattern, map_bed_type, map_brim_type
fn map_support_type(value: &str) -> Option<SupportType> {
    match value.to_lowercase().as_str() {
        "none" | "disable" => Some(SupportType::None),
        "normal" | "grid" => Some(SupportType::Traditional),
        "tree" | "tree(auto)" => Some(SupportType::Tree),
        "default" | "auto" => Some(SupportType::Auto),
        // OrcaSlicer uses different vocabulary:
        "tree_slim" | "organic" => Some(SupportType::Tree),
        _ => None,
    }
}
```

### Pattern 3: Derived Field Mapping (Density from Spacing)
**What:** Some upstream fields encode density as spacing distance rather than percentage.
**When to use:** `support_base_pattern_spacing` -> `support_density`, `support_interface_spacing` -> `interface_density`.
**Example:**
```rust
// OrcaSlicer encodes support density as line spacing in mm
// Convert: density = line_width / spacing (approximate)
"support_base_pattern_spacing" => {
    if let Ok(spacing) = value.parse::<f64>() {
        if spacing > 0.0 {
            // Approximate density: support_line_width / spacing
            let line_width = config.line_widths.support;
            config.support.support_density = (line_width / spacing).clamp(0.0, 1.0);
        }
        true
    } else {
        false
    }
}
```

### Pattern 4: Dual G-code Storage
**What:** Store both translated and original G-code templates.
**When to use:** For all G-code hook fields (start_gcode, end_gcode, before_layer_change, etc.).
**Example:**
```rust
// In CustomGcodeHooks or MachineConfig
pub start_gcode: String,           // Translated to our variable syntax
pub start_gcode_original: String,  // Verbatim from upstream profile
```

### Pattern 5: Data-Driven Variable Translation Table
**What:** A HashMap mapping upstream variable names to ours for G-code template translation.
**When to use:** When storing translated G-code templates.
**Example:**
```rust
fn build_variable_translation_table() -> HashMap<&'static str, &'static str> {
    let mut table = HashMap::new();
    // OrcaSlicer variables -> our variables
    table.insert("{initial_layer_print_height}", "{first_layer_height}");
    table.insert("{nozzle_temperature_initial_layer}", "{first_layer_nozzle_temp}");
    table.insert("{bed_temperature_initial_layer}", "{first_layer_bed_temp}");
    table.insert("{filament_flow_ratio}", "{extrusion_multiplier}");
    // ... etc
    table
}
```

### Anti-Patterns to Avoid
- **One-off field handling:** Do not write unique parsing logic for each field. Use `parse_and_set_f64`, `parse_and_set_u32`, boolean pattern, and enum mapper helpers consistently.
- **Mapping without INI counterpart:** Every JSON match arm MUST have a corresponding INI match arm (or explicit "no PrusaSlicer equivalent" documentation).
- **Testing per-field in isolation:** Test via round-trip profile import rather than individual field parse tests.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Upstream key enumeration | Manual audit from documentation | Grep actual profile files + passthrough BTreeMap contents | Docs are incomplete; real profiles have keys docs miss |
| SupportType mapping | Direct string comparison | Enum mapping helper (like `map_infill_pattern`) | Both OrcaSlicer and PrusaSlicer use different vocabularies for the same concept |
| Density-from-spacing conversion | Hardcoded formula | Parameterized helper taking line_width | Formula is reused for body density and interface density |
| G-code variable translation | String::replace chain | Data-driven table iterated in a loop | Maintainable, auditable, extensible |
| Coverage reporting | Ad-hoc counting | ImportResult already tracks mapped/unmapped/passthrough | All the infrastructure exists |

**Key insight:** 95% of this phase is mechanical application of existing patterns to new fields. The risk is in missing fields (solved by audit-first) and in slicer vocabulary differences (solved by testing against real profiles), not in code complexity.

## Common Pitfalls

### Pitfall 1: OrcaSlicer vs PrusaSlicer Key Name Divergence
**What goes wrong:** OrcaSlicer uses `support_threshold_angle`, PrusaSlicer uses `support_material_threshold`. Mapping only one leaves the other unmapped.
**Why it happens:** The two slicers forked from the same codebase but diverged key names.
**How to avoid:** For every field, identify BOTH the OrcaSlicer JSON key AND the PrusaSlicer INI key. Add both as match alternatives (e.g., `"support_threshold_angle" | "support_material_threshold" =>`).
**Warning signs:** Tests pass on OrcaSlicer profiles but fail on PrusaSlicer profiles.

### Pitfall 2: Spacing-vs-Density Encoding
**What goes wrong:** OrcaSlicer encodes support density as `support_base_pattern_spacing` (distance in mm) while we store it as a 0.0-1.0 fraction.
**Why it happens:** Different slicers use different representations for the same concept.
**How to avoid:** Document the conversion formula. Use a helper function. Add tests with known spacing -> density conversion values.
**Warning signs:** Support density values like 2.5 or 0.0 after import (clearly wrong).

### Pitfall 3: Array-Wrapped Single Values
**What goes wrong:** OrcaSlicer wraps some scalar values in arrays: `"support_type": ["tree"]` instead of `"support_type": "tree"`.
**Why it happens:** OrcaSlicer's JSON format is inconsistent between scalar and array encoding.
**How to avoid:** The `extract_string_value` helper already handles this. Use it consistently rather than direct `.as_str()`.
**Warning signs:** Fields parsing as None when the JSON clearly has a value.

### Pitfall 4: PrusaSlicer Boolean Encoding
**What goes wrong:** PrusaSlicer uses `1`/`0` for booleans, not `true`/`false`.
**Why it happens:** INI format convention.
**How to avoid:** Always use `value == "1" || value.eq_ignore_ascii_case("true")` pattern (already standard).
**Warning signs:** Boolean fields always false after PrusaSlicer import.

### Pitfall 5: Missing Regression Check on Re-conversion
**What goes wrong:** Adding new typed fields causes previously-passthrough fields to now be typed with wrong defaults, changing profile behavior.
**Why it happens:** A field that was preserved verbatim in passthrough now gets parsed, and the parse result differs from the passthrough value.
**How to avoid:** Compare before/after passthrough contents for each re-converted profile. Fields moving from passthrough to typed should have matching values.
**Warning signs:** Profile diff shows value changes for fields that should be identity-mapped.

### Pitfall 6: Support Type Vocabulary Mismatch
**What goes wrong:** OrcaSlicer's "normal" support type doesn't map cleanly to our enum.
**Why it happens:** OrcaSlicer has: `normal`, `tree(auto)`, `tree(manual)`, `hybrid(auto/manual)`. PrusaSlicer has: `0` (none), `1` (support_material). We have: `Auto`, `Traditional`, `Tree`, `None`.
**How to avoid:** Build the `map_support_type` function from real profile values, not documentation. Test with profiles that use each variant.
**Warning signs:** Imported profiles all end up with `Auto` support type.

### Pitfall 7: G-code Template Variable Collisions
**What goes wrong:** A simple find-and-replace on variable names causes double-replacement (e.g., replacing `{layer}` also matches within `{layer_height}`).
**Why it happens:** Naive string replacement without boundary checking.
**How to avoid:** Sort replacement table by key length (longest first), or use regex word boundaries, or use a single-pass replacement.
**Warning signs:** G-code templates have garbled variable references after translation.

## Code Examples

### Support Config Mapping (JSON)
```rust
// In apply_field_mapping match block
// --- Support config fields ---
"enable_support" | "support_material" => {
    config.support.enabled = value == "1" || value.eq_ignore_ascii_case("true");
    true
}
"support_type" | "support_material_type" => {
    if let Some(st) = map_support_type(value) {
        config.support.support_type = st;
        true
    } else {
        false
    }
}
"support_threshold_angle" | "support_material_threshold" => {
    parse_and_set_f64(value, &mut config.support.overhang_angle)
}
"support_base_pattern" | "support_material_pattern" => {
    if let Some(p) = map_support_pattern(value) {
        config.support.support_pattern = p;
        true
    } else {
        false
    }
}
"support_interface_top_layers" | "support_material_interface_layers" => {
    parse_and_set_u32(value, &mut config.support.interface_layers)
}
"support_interface_pattern" | "support_material_interface_pattern" => {
    if let Some(p) = map_interface_pattern(value) {
        config.support.interface_pattern = p;
        true
    } else {
        false
    }
}
"support_top_z_distance" | "support_material_contact_distance" => {
    parse_and_set_f64(value, &mut config.support.z_gap)
}
"support_object_xy_distance" | "support_material_xy_spacing" => {
    parse_and_set_f64(value, &mut config.support.xy_gap)
}
"support_on_build_plate_only" | "support_material_buildplate_only" => {
    config.support.build_plate_only = value == "1" || value.eq_ignore_ascii_case("true");
    true
}
```

### Scarf Joint Mapping (JSON)
```rust
// OrcaSlicer uses seam_slope_* prefix
"seam_slope_type" => {
    config.scarf_joint.enabled = value != "none" && value != "0";
    true
}
"seam_slope_conditional" => {
    config.scarf_joint.conditional_scarf = value == "1" || value.eq_ignore_ascii_case("true");
    true
}
"seam_slope_start_height" => {
    parse_and_set_f64(value, &mut config.scarf_joint.scarf_start_height)
}
"seam_slope_entire_loop" => {
    config.scarf_joint.scarf_around_entire_wall =
        value == "1" || value.eq_ignore_ascii_case("true");
    true
}
"seam_slope_min_length" => {
    parse_and_set_f64(value, &mut config.scarf_joint.scarf_length)
}
"seam_slope_steps" => {
    parse_and_set_u32(value, &mut config.scarf_joint.scarf_steps)
}
"seam_slope_inner_walls" => {
    config.scarf_joint.scarf_inner_walls =
        value == "1" || value.eq_ignore_ascii_case("true");
    true
}
"wipe_on_loops" => {
    config.scarf_joint.wipe_on_loop =
        value == "1" || value.eq_ignore_ascii_case("true");
    true
}
```

### Coverage Report Structure
```rust
pub struct CoverageReport {
    pub total_upstream_keys: usize,
    pub mapped_to_typed: usize,
    pub defaulted: usize,
    pub in_passthrough: usize,
    pub passthrough_ratio: f64,
}

impl std::fmt::Display for CoverageReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Import Coverage Report")?;
        writeln!(f, "  Total upstream keys:  {}", self.total_upstream_keys)?;
        writeln!(f, "  Mapped to typed:      {} ({:.1}%)",
            self.mapped_to_typed,
            100.0 * self.mapped_to_typed as f64 / self.total_upstream_keys as f64)?;
        writeln!(f, "  Passthrough:          {} ({:.1}%)",
            self.in_passthrough,
            100.0 * self.in_passthrough as f64 / self.total_upstream_keys as f64)?;
        Ok(())
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Support fields unmapped (passthrough) | Phase 34 adds all 27 match arms | Now | Support profile import finally works |
| ScarfJoint fields unmapped | Phase 34 maps seam_slope_* keys | Now | OrcaSlicer scarf joint profiles import correctly |
| Passthrough catch-all for unknown keys | Typed fields + passthrough only for truly exotic | Phases 32->33->34 | Import fidelity goes from ~60% to ~95%+ |

**Deprecated/outdated:**
- Nothing deprecated. This phase extends existing patterns, doesn't replace them.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml (each crate) |
| Quick run command | `cargo test -p slicecore-engine --lib -- support::config` |
| Full suite command | `cargo test -p slicecore-engine` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SUPPORT-MAP | 27 support fields import from JSON/INI | unit | `cargo test -p slicecore-engine -- profile_import::tests::support` | Wave 0 |
| SCARF-MAP | 13 scarf joint fields import from JSON | unit | `cargo test -p slicecore-engine -- profile_import::tests::scarf` | Wave 0 |
| MULTI-MAP | 7+ multi-material fields import | unit | `cargo test -p slicecore-engine -- profile_import::tests::multi_material` | Wave 0 |
| GCODE-MAP | 5 custom gcode hook fields import | unit | `cargo test -p slicecore-engine -- profile_import::tests::custom_gcode` | Wave 0 |
| POST-MAP | 12 post-process fields import (selective) | unit | `cargo test -p slicecore-engine -- profile_import::tests::post_process` | Wave 0 |
| P2-FIELDS | ~20 P2 niche fields added and mapped | unit | `cargo test -p slicecore-engine -- profile_import::tests::p2` | Wave 0 |
| GCODE-TRANSLATE | Variable translation table works | unit | `cargo test -p slicecore-engine -- gcode_template` | Wave 0 |
| PASSTHROUGH-THRESHOLD | <5% passthrough on representative profiles | integration | `cargo test -p slicecore-engine -- passthrough_threshold` | Wave 0 |
| ROUND-TRIP | Support-heavy, bridge-heavy, tree profiles round-trip | integration | `cargo test -p slicecore-engine -- round_trip` | Wave 0 |
| RECONVERT | Re-conversion completes without regression | manual+script | `cargo run -- convert ...` | Wave N |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-engine --lib`
- **Per wave merge:** `cargo test -p slicecore-engine`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] Support mapping test profiles (JSON + INI with all support fields populated)
- [ ] Scarf joint mapping test profiles
- [ ] Multi-material mapping test profiles
- [ ] G-code template translation tests
- [ ] Passthrough threshold integration test
- [ ] Round-trip tests for support-heavy and tree-support profiles

## Open Questions

1. **Exact OrcaSlicer support_type vocabulary**
   - What we know: OrcaSlicer uses strings like "normal", "tree(auto)". PrusaSlicer uses integer codes.
   - What's unclear: The full set of possible string values for each slicer.
   - Recommendation: The audit plan (Plan 1) should grep actual profile files for all distinct `support_type` values. Build the mapping from empirical data.

2. **Spacing-to-density conversion accuracy**
   - What we know: OrcaSlicer uses `support_base_pattern_spacing` (mm between lines) while we use a 0-1 density fraction.
   - What's unclear: The exact formula OrcaSlicer uses internally for the conversion.
   - Recommendation: Use `line_width / spacing` as approximation, validate by comparing with known profiles.

3. **G-code variable name discovery**
   - What we know: OrcaSlicer variables use `{key_name}` syntax in G-code templates.
   - What's unclear: The complete list of OrcaSlicer template variables.
   - Recommendation: Grep all G-code template fields in the profile corpus for `{variable}` patterns to build the translation table empirically.

4. **PrusaSlicer support field names**
   - What we know: PrusaSlicer prefixes support fields with `support_material_*`.
   - What's unclear: Whether all our support sub-fields have PrusaSlicer equivalents.
   - Recommendation: Grep PrusaSlicer vendor INI bundles for `support_material*` keys.

## Sources

### Primary (HIGH confidence)
- `crates/slicecore-engine/src/config.rs` -- All existing config structs, sub-structs, enums, and defaults
- `crates/slicecore-engine/src/support/config.rs` -- SupportConfig with 27 fields, BridgeConfig, TreeSupportConfig
- `crates/slicecore-engine/src/profile_import.rs` -- Complete field mapping implementation with 150+ match arms
- `crates/slicecore-engine/src/profile_import_ini.rs` -- PrusaSlicer INI mapping implementation
- `crates/slicecore-engine/src/custom_gcode.rs` -- CustomGcodeHooks with 5 fields
- `designDocs/CONFIG_PARITY_AUDIT.md` -- Comprehensive field-by-field gap analysis with P0/P1/P2 categorization

### Secondary (MEDIUM confidence)
- Phase 32 CONTEXT.md -- P0 patterns established (verified by reading actual code)
- Phase 33 CONTEXT.md -- P1 patterns established (verified by reading actual code)

### Tertiary (LOW confidence)
- OrcaSlicer support_type vocabulary -- needs empirical verification from actual profiles (Open Question 1)
- Spacing-to-density conversion formula -- needs validation (Open Question 2)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, purely using established patterns
- Architecture: HIGH -- patterns are proven across 150+ existing match arms in Phases 20/32/33
- Pitfalls: HIGH -- identified from direct code analysis and documented slicer divergences
- P2 field list: MEDIUM -- audit doc may be incomplete; audit-first plan will discover missing fields

**Research date:** 2026-03-17
**Valid until:** 2026-04-17 (stable domain, no upstream changes expected)
