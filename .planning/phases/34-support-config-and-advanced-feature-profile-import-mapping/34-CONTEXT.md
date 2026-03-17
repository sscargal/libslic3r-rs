# Phase 34: Support Config and Advanced Feature Profile Import Mapping - Context

**Gathered:** 2026-03-17
**Status:** Ready for planning

<domain>
## Phase Boundary

Map ALL remaining unmapped config sections from upstream profiles (OrcaSlicer/BambuStudio/PrusaSlicer) to achieve 100% typed field coverage for every field with an upstream equivalent. Covers the five 0%-coverage sections (SupportConfig, ScarfJointConfig, MultiMaterialConfig, CustomGcodeHooks, PostProcessConfig), all ~20 P2 niche fields, stragglers from partially-mapped sections, passthrough-to-typed promotions, and any new fields discovered by live profile scanning. Also adds G-code template variable translation, import coverage reporting, and comprehensive validation. Config + mapping only — no engine behavior changes.

</domain>

<decisions>
## Implementation Decisions

### Scope — Full Clean Sweep
- Map all 5 sections at 0% upstream coverage: SupportConfig+subs (27 fields), ScarfJointConfig (13), MultiMaterialConfig (7), CustomGcodeHooks (5), PostProcessConfig+subs (12)
- Include ALL ~20 P2 niche fields from audit (timelapse_type, thumbnails, silent_mode, slicing_tolerance, post_process, etc.)
- Pick up stragglers from partially-mapped sections (e.g., ironing_angle in IroningConfig, unmapped SequentialConfig fields)
- Add NEW fields to existing sections where upstream has fields we haven't defined yet — after Phase 34, our config can represent anything an imported profile contains
- Promote passthrough keys to typed fields where they have enough structure
- Target: 100% mapping coverage for all fields with upstream equivalents. Passthrough only for truly exotic/unknown keys
- Meta/inheritance fields (compatible_printers_condition_cummulative, inherits_group) mapped as typed for future profile management

### Audit-First Approach
- First plan is a comprehensive field inventory before any implementation:
  - Grep all upstream profile keys across OrcaSlicer, BambuStudio, PrusaSlicer
  - Diff against our currently mapped fields
  - Scan actual imported profile JSON/INI files to discover keys not in the audit doc
  - Check passthrough BTreeMap contents for promotable keys
  - Specifically enumerate ALL support-related keys from both slicers (not just what's in current SupportConfig)
  - Validate critical support params (contact distance, pattern, density, interface pattern) are in the inventory
  - Produce a definitive "map these" field list

### Support Config Mapping
- Unified superset approach: our TreeSupportConfig is the superset, both slicers map into it filling what they can
- Define our own SupportType enum (None, Normal, Tree, Organic) covering both slicers — import mapper translates from each slicer's vocabulary
- Our custom fields (quality_preset, conflict_resolution) stay at defaults during import — don't reverse-engineer upstream intent
- Map bridge params (speed, fan_speed, flow_ratio, acceleration, line_width_ratio) from BOTH OrcaSlicer JSON and PrusaSlicer INI
- Both top and bottom interface layer counts fully mapped as typed fields
- Dedicated round-trip tests for support-heavy, bridge-heavy, and tree-support profiles

### Slicer Divergence Strategy
- Default + document pattern for unmappable fields: sensible defaults, doc comments note "No upstream equivalent — set manually or via AI"
- Same pattern applied consistently across all sections (ScarfJoint, MultiMaterial, PostProcess)
- Map simple upstream fields, default complex our-only structures (fan override rules, custom G-code rules)
- PrusaSlicer post_process scripts mapped as Vec<String> into PostProcessConfig

### G-code Template Handling
- Dual storage: `start_gcode` (translated to our syntax) + `start_gcode_original` (verbatim from upstream) — separate fields for each G-code hook
- Data-driven mapping table for variable translation (OrcaSlicer variable names → ours), not hardcoded
- Mapping table is configurable and auditable, easy to extend

### Import Coverage Report
- After importing a profile, optionally output mapping coverage summary: N fields mapped, M defaulted, K in passthrough
- Available as both CLI output during import and as a feature for diagnostic use

### Re-conversion
- Full sweep of ~21k profiles (same as Phase 32/33)
- Validation step: compare before/after for each profile, flag regressions (fields that were mapped before but lost values)
- Coverage improvement report: both CLI output AND committed MAPPING_COVERAGE_REPORT.md in designDocs/
- Update CONFIG_PARITY_AUDIT.md Section 4 with final coverage numbers (single source of truth)
- Re-conversion plan designed as independently re-runnable for future upstream profile updates
- Integration test asserting passthrough is below threshold (<5% of upstream keys) for representative profiles

### Patterns Carried Forward from Phase 32/33
- Config + mapping only — fields stored, serialized, round-tripped, NOT wired into engine
- Migrate from passthrough → typed, remove from passthrough once typed
- OrcaSlicer defaults as baseline
- Both OrcaSlicer JSON AND PrusaSlicer INI mappings added together
- Full Rust doc comments on every field (units, range, description — Phase 35 prep)
- TOML inline comments for self-documenting configs
- G-code template variables for all new fields
- G-code comments emitting new field values
- Basic range validation per field (warn on out-of-range)

### Claude's Discretion
- Exact field ordering within sub-structs
- Which passthrough keys qualify for typed promotion (based on audit findings)
- G-code template variable naming for new fields
- Exact SupportType enum variant names and mapping logic
- How to structure the mapping table data format
- Test profile selection for round-trip and threshold tests
- Report formatting details

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Config audit & field inventory
- `designDocs/CONFIG_PARITY_AUDIT.md` — Complete field-by-field comparison, P0/P1/P2 gap categorization, mapping coverage stats (Section 4), recommended phases (Section 5)
- `designDocs/CONFIG_PARITY_AUDIT.md` §P2 (line 425-451) — All 20 P2 niche fields with descriptions and categories
- `designDocs/01-PRODUCT_REQUIREMENTS.md` §7 — SettingDefinition schema (informs doc comment format for Phase 35 compatibility)

### Current config implementation
- `crates/slicecore-engine/src/config.rs` — PrintConfig and all sub-structs
- `crates/slicecore-engine/src/support/config.rs` — SupportConfig (~27 fields + BridgeConfig + TreeSupportConfig)
- `crates/slicecore-engine/src/profile_import.rs` — OrcaSlicer/BambuStudio JSON field mapping tables
- `crates/slicecore-engine/src/profile_import_ini.rs` — PrusaSlicer INI field mapping

### Prior config decisions
- `.planning/phases/32-p0-config-gap-closure-critical-missing-fields/32-CONTEXT.md` — Phase 32 P0 patterns (sub-struct organization, migration, validation, G-code emission, DimensionalCompensationConfig, SurfacePattern, BedType)
- `.planning/phases/33-p1-config-gap-closure-profile-fidelity-fields/33-CONTEXT.md` — Phase 33 P1 patterns (FuzzySkinConfig, BrimSkirtConfig, InputShapingConfig, ToolChangeRetractionConfig, extended sub-structs)
- `.planning/phases/20-expand-printconfig-field-coverage-and-profile-mapping/20-CONTEXT.md` — Phase 20 config expansion patterns (Vec arrays, passthrough catch-all, sub-config organization)
- `.planning/phases/30-cli-profile-composition-and-slice-workflow/30-CONTEXT.md` — Profile merge model, G-code template variables, reproduce command

### Config parity audit context
- `.planning/quick/4-config-parity-audit-and-phase-planning-f/4-CONTEXT.md` — Audit task context (parity strategy, gap categorization)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `PrintConfig` with nested sub-configs — established pattern for all new sub-struct additions
- `DimensionalCompensationConfig`, `FuzzySkinConfig`, `BrimSkirtConfig`, `InputShapingConfig`, `ToolChangeRetractionConfig` (Phase 32/33) — recent examples of sub-struct creation
- `SurfacePattern`, `BedType`, `BrimType` enums (Phase 32/33) — reference for new enums (SupportType)
- `profile_import.rs` apply_field_mapping + apply_array_field_mapping — pattern for upstream key → typed field mapping
- `profile_import_ini.rs` — PrusaSlicer INI mapping with same pattern
- `SupportConfig`, `BridgeConfig`, `TreeSupportConfig` — all exist with our internal fields, just need upstream mapping
- `ScarfJointConfig` (13 fields) — exists, needs `seam_slope_*` mapping from OrcaSlicer
- `MultiMaterialConfig` (7 fields) — exists, needs `wipe_tower_*` / `prime_*` mapping
- `CustomGcodeHooks` (5 fields) — exists, needs upstream G-code hook mapping
- `PostProcessConfig` + `TimelapseConfig` + `FanOverrideRule` + `CustomGcodeRule` — exist, need selective mapping
- G-code template variable system — extend for new fields + add variable translation table
- Config validation in profile merge — extend for new field range checks

### Established Patterns
- Sub-struct organization with `#[serde(default)]` for backward compatibility
- Vec<f64> for multi-extruder array fields
- `passthrough` BTreeMap<String,String> catch-all for unmapped keys
- Enum variants with `#[serde(rename = "...")]` for upstream string mapping
- Serde derive with default for backward-compatible deserialization
- Both JSON and INI mappers updated together for each new field

### Integration Points
- `config.rs` — add new fields, enums, possibly new sub-structs for newly-discovered field clusters
- `support/config.rs` — add upstream mapping for all 27 support fields
- `profile_import.rs` — add JSON field mappings for all remaining unmapped fields
- `profile_import_ini.rs` — add INI field mappings
- G-code generator — emit template variables and comments for new fields, add variable translation table
- Config validation — add range checks for new fields
- Existing tests — update snapshots, add field-specific tests, add round-trip tests, add passthrough threshold tests

</code_context>

<specifics>
## Specific Ideas

- The audit-first plan should scan real profile files (not just the audit doc) to catch upstream keys added since the audit was written
- G-code template translation uses a data-driven mapping table (HashMap<String, String>) from OrcaSlicer/PrusaSlicer variable names to ours
- Dual G-code storage: `start_gcode` (translated) + `start_gcode_original` (verbatim) pattern for all G-code hooks
- SupportType enum should cover both OrcaSlicer and PrusaSlicer's support taxonomy in a unified way
- Passthrough threshold test: assert <5% of upstream keys end up in passthrough after mapping
- Coverage report both in CLI and committed markdown, plus update to CONFIG_PARITY_AUDIT.md

</specifics>

<deferred>
## Deferred Ideas

### Profile Tooling (future phases)
- **Profile diff tool** — `slicecore profiles diff profile-a.toml profile-b.toml` showing field-by-field semantic differences
- **Mapping health dashboard** — `slicecore profiles health` showing per-slicer mapping coverage, passthrough usage, unusual patterns
- **Upstream profile sync CI** — GitHub Action fetching latest slicer profile repos, running import, comparing coverage, opening PRs for new unmapped keys
- **Profile validation linter** — `slicecore profiles lint profile.toml` checking out-of-range values, conflicting settings, missing required fields. Semantic checks beyond range validation
- **Unmapped key reporter** — Log passthrough keys during import, accumulate across imports to prioritize future mapping by real-world usage

### Profile Management (future phases)
- **Profile migration system** — Versioned config format with auto-migration when structs change. Eliminates manual re-conversion for users
- **Profile recommendation engine** — Given printer+filament combo, suggest closest matching profile from imported library. Leverages AI module + comprehensive field coverage
- **Profile compression/dedup** — Store profiles as deltas from base profile to reduce storage. Foundation for profile inheritance

### Config Infrastructure (future phases)
- **Slicer compatibility matrix** — Auto-generated matrix showing which config fields supported per slicer source. Published as doc or web page
- **Config field deprecation system** — Mark fields as deprecated with migration paths. Auto-migrate and warn when loading old configs
- **Profile test harness** — Automated test importing every profile from every slicer, validating parse, checking types, reporting stats. Regression safety net for 21k+ profiles

### Carried from Phase 32/33
- **Engine behavior for mapped fields** — Wiring support params into support generation, scarf joint into seam, multi-material into tool changes, etc. Separate phase after config mapping is complete
- **Cross-slicer profile converter** — Round-trip through our canonical format to convert between slicer formats

</deferred>

---

*Phase: 34-support-config-and-advanced-feature-profile-import-mapping*
*Context gathered: 2026-03-17*
