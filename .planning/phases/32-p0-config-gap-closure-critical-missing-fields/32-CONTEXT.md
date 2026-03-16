# Phase 32: P0 Config Gap Closure — Critical Missing Fields - Context

**Gathered:** 2026-03-16
**Status:** Ready for planning

<domain>
## Phase Boundary

Add ~15 critical config fields to PrintConfig that upstream slicers (OrcaSlicer/BambuStudio/PrusaSlicer) commonly set and that affect print quality. Add upstream profile mappings (both JSON and INI) for each new field. Re-run profile import to regenerate all ~21k profiles with new typed fields. Config-only — no engine behavior changes in this phase.

</domain>

<decisions>
## Implementation Decisions

### Engine Integration Depth
- Config + mapping only — fields are stored, serialized, round-tripped, but NOT wired into the slicing engine pipeline
- Engine behavior (e.g., actually applying xy_compensation to geometry) comes in a later phase
- Emit G-code comments for all new fields (visible in output for verification)
- Emit standard M-codes where obvious: chamber_temp → M141, z_offset → M851/start G-code
- All new fields available as G-code template variables (e.g., `{chamber_temperature}` in start/end G-code)
- All new fields included in the G-code reproduce command

### Field Handling Patterns
- Migrate fields from passthrough (BTreeMap) to typed — once typed, remove from passthrough
- Default values match OrcaSlicer defaults (imported profiles behave the same as in OrcaSlicer)
- Vec<f64> where upstream stores arrays (per Phase 20 pattern)
- Our own Rust naming convention — profile_import mapper handles upstream→ours translation
- Unified semantics we define — both OrcaSlicer and PrusaSlicer mappers translate to our canonical meaning
- Basic range validation per field (warn on out-of-range, error on dangerous values per Phase 30 model)
- Full Rust doc comments on every new field (units, range, description) — prepares for Phase 35 ConfigSchema derive macro
- TOML serialization includes inline comments for all fields (self-documenting saved configs)

### Profile Mapping
- Both OrcaSlicer JSON AND PrusaSlicer INI mappings added in this phase (all three import paths in sync)
- Full re-conversion of all ~21k profiles using existing import-profiles pipeline after adding new mappings
- Re-conversion is a separate plan (can be re-run independently)

### Testing
- Update existing tests (golden tests, config integration) to reflect new fields
- Add dedicated tests for each new field's mapping, default, and validation

### P0 Field List (13 core + 3 additions = 16 fields)

| # | Field | Type | Sub-struct | Notes |
|---|-------|------|-----------|-------|
| 1 | `chamber_temperature` | f64 | MachineConfig (max capability) + FilamentPropsConfig (desired) | Auto-resolve: filament desired validated against machine max |
| 2 | `xy_hole_compensation` | f64 | DimensionalCompensationConfig (NEW) | Shrinks holes for dimensional accuracy |
| 3 | `xy_contour_compensation` | f64 | DimensionalCompensationConfig (NEW) | Shrink/expand outer contours |
| 4 | `elephant_foot_compensation` | f64 | DimensionalCompensationConfig (MIGRATE from top-level) | Breaking change — migrated into sub-struct |
| 5 | `extra_perimeters_on_overhangs` | bool | PrintConfig top-level | Simple on/off matching OrcaSlicer |
| 6 | `top_surface_pattern` | SurfacePattern | Claude's discretion | New enum, default: Monotonic |
| 7 | `bottom_surface_pattern` | SurfacePattern | Claude's discretion | New enum, default: Monotonic |
| 8 | `solid_infill_pattern` | SurfacePattern | Claude's discretion | Internal solid layers pattern |
| 9 | `internal_bridge_speed` | f64 | SpeedConfig | Speed for internal bridges (mm/s) |
| 10 | `internal_bridge_support` | InternalBridgeMode (Off/Auto/Always) | PrintConfig or relevant sub-struct | Enum, not bool |
| 11 | `filament_shrink` | f64 | FilamentPropsConfig | Single percentage (isotropic). Vec<f64> if upstream is array. |
| 12 | `z_offset` | f64 | PrintConfig top-level (global) + FilamentPropsConfig (per-filament, additive) | Both global and per-filament |
| 13 | `curr_bed_type` | BedType | MachineConfig | Extended enum: CoolPlate, EngineeringPlate, HighTempPlate, TexturedPEI, SmoothPEI, SatinPEI |
| 14 | `min_length_*` acceleration fields | f64 (full set) | AccelerationConfig | Full set matching OrcaSlicer variants |
| 15 | `precise_z_height` | bool | PrintConfig top-level | Simple toggle |

### DimensionalCompensationConfig (NEW sub-struct)
- Groups: xy_hole_compensation, xy_contour_compensation, elephant_foot_compensation (migrated)
- Room for future compensation fields
- elephant_foot_compensation migration is a breaking TOML format change (acceptable per Phase 20 precedent)

### SurfacePattern Enum (NEW)
- Limited set for solid surfaces: Rectilinear, Monotonic, MonotonicLine, Concentric, Hilbert, Archimedean
- Separate from InfillPattern which includes patterns unsuitable for solid surfaces (Lightning, Gyroid, etc.)
- Default for all three (top/bottom/solid): Monotonic

### BedType System
- BedType enum with extended variants: CoolPlate, EngineeringPlate, HighTempPlate, TexturedPEI, SmoothPEI, SatinPEI
- Our own snake_case naming (cool_plate, textured_pei, etc.) — import mapper translates from OrcaSlicer strings
- Per-type temperature fields in FilamentPropsConfig (hot_plate_temp, cool_plate_temp, etc.)
- Auto-resolve bed_temperature and first_layer_bed_temperature from per-type temps + selected bed type
- Keep both resolved values AND raw per-type temps in final config
- When filament has no temp for selected bed type: warn and fall back to default bed_temperature

### Claude's Discretion
- Placement of surface pattern fields (top-level vs sub-struct)
- Exact SurfacePattern enum variant names
- Internal bridge support field placement
- Exact min_length acceleration field names and count (check OrcaSlicer source)
- Per-type temperature field naming in FilamentPropsConfig
- Order of fields within sub-structs

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Config audit & field inventory
- `designDocs/CONFIG_PARITY_AUDIT.md` — Complete field-by-field comparison, P0/P1/P2 gap categorization, mapping coverage stats
- `designDocs/01-PRODUCT_REQUIREMENTS.md` §7 — SettingDefinition schema (informs doc comment format for Phase 35 compatibility)

### Current config implementation
- `crates/slicecore-engine/src/config.rs` — PrintConfig and all sub-structs (~1400 lines)
- `crates/slicecore-engine/src/support/config.rs` — SupportConfig (~40 pub fields)
- `crates/slicecore-engine/src/profile_import.rs` — OrcaSlicer/BambuStudio JSON field mapping tables (~1200 lines)
- `crates/slicecore-engine/src/profile_import_ini.rs` — PrusaSlicer INI field mapping

### Prior config decisions
- `.planning/phases/20-expand-printconfig-field-coverage-and-profile-mapping/20-CONTEXT.md` — Phase 20 config expansion decisions (sub-struct organization, Vec arrays, mapping strategy)
- `.planning/phases/30-cli-profile-composition-and-slice-workflow/30-CONTEXT.md` — Profile merge model, G-code template variables, reproduce command
- `.planning/quick/4-config-parity-audit-and-phase-planning-f/4-CONTEXT.md` — Audit task context (parity strategy, gap categorization approach)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `PrintConfig` struct with nested sub-configs (LineWidthConfig, SpeedConfig, CoolingConfig, etc.) — established pattern for adding new sub-structs
- `profile_import.rs` apply_field_mapping + apply_array_field_mapping — pattern for adding new upstream key mappings
- `profile_import_ini.rs` — PrusaSlicer INI import with same field mapping pattern
- `InfillPattern` enum — reference for creating new SurfacePattern enum
- Existing G-code template variable system — extend for new fields
- Config validation in profile merge — extend for new field range checks

### Established Patterns
- Sub-struct organization: group related fields into named config structs (LineWidthConfig, SpeedConfig, etc.)
- Serde derive with `#[serde(default)]` for backward-compatible deserialization
- Vec<f64> for multi-extruder array fields (Phase 20 pattern)
- `passthrough` BTreeMap<String,String> as catch-all for unmapped upstream keys

### Integration Points
- `config.rs` — add new sub-structs and fields
- `profile_import.rs` — add JSON field mappings for OrcaSlicer/BambuStudio
- `profile_import_ini.rs` — add INI field mappings for PrusaSlicer
- G-code generator — emit M-codes and template variables for new fields
- Config validation — add range checks for new fields
- Existing tests — update snapshots and add new field-specific tests

</code_context>

<specifics>
## Specific Ideas

- Chamber temperature needs dual representation: machine capability (max) + filament requirement (desired), validated during merge
- Z offset needs dual representation: global + per-filament (additive)
- DimensionalCompensationConfig groups all compensation fields — migrate elephant_foot_compensation into it (breaking change accepted)
- Profile re-conversion should be a separate plan that can be re-run independently when upstream profiles are updated
- Profile management CLI tooling (import, update, sync) is deferred but important — see deferred ideas

</specifics>

<deferred>
## Deferred Ideas

- **Profile management CLI** — `slicecore profiles` subcommand grouping: import, update, list, search, show, validate. Unifies existing scattered profile commands. Future phase.
- **Upstream sync workflow** — Tooling to pull latest OrcaSlicer/BambuStudio/PrusaSlicer profile JSONs from their repos and re-import. CI-friendly. Future phase.
- **Config migration system** — Versioned config format with automatic migration when structs change (e.g., elephant_foot moving to DimensionalCompensationConfig). Future phase.
- **Engine behavior for P0 fields** — Wiring xy_compensation into geometry, filament_shrink into scaling, etc. Separate phase after config is in place.

</deferred>

---

*Phase: 32-p0-config-gap-closure-critical-missing-fields*
*Context gathered: 2026-03-16*
