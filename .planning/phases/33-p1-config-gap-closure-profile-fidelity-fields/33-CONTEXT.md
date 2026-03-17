# Phase 33: P1 Config Gap Closure — Profile Fidelity Fields - Context

**Gathered:** 2026-03-17
**Status:** Ready for planning

<domain>
## Phase Boundary

Add ~30 config fields that OrcaSlicer/BambuStudio/PrusaSlicer profiles commonly set and that affect imported profile accuracy. These are P1 priority fields from the config parity audit — important for profile fidelity but not critical for basic printing (P0 was Phase 32). Config + mapping only — no engine behavior changes in this phase.

</domain>

<decisions>
## Implementation Decisions

### Field Grouping Strategy
- Create new sub-structs for natural clusters: FuzzySkinConfig (3 fields), BrimSkirtConfig (4+ fields), InputShapingConfig (2 fields), ToolChangeRetractionConfig (2 fields initially)
- Extend existing sub-structs where fields belong naturally: AccelerationConfig, CoolingConfig, SpeedConfig, FilamentPropsConfig, MultiMaterialConfig
- Follows Phase 32 pattern of DimensionalCompensationConfig for new sub-structs

### New Sub-structs

**FuzzySkinConfig** (3 fields):
- fuzzy_skin (bool — enable/disable)
- fuzzy_skin_thickness (f64 — amplitude in mm)
- fuzzy_skin_point_dist (f64 — point distance in mm)

**BrimSkirtConfig** (4+ fields):
- brim_type: BrimType enum (None, Outer, Inner, Both — Rust-idiomatic names, import mapper translates from OrcaSlicer strings)
- brim_ears (bool)
- brim_ears_max_angle (f64)
- skirt_height (u32 — layers)
- Skirt and brim grouped together as closely related features (mutually exclusive in most slicers)

**InputShapingConfig** (2 fields):
- accel_to_decel_enable (bool)
- accel_to_decel_factor (f64 — ratio)

**ToolChangeRetractionConfig** (2 fields, room for P2 expansion):
- retraction_distance_when_cut (f64)
- long_retraction_when_cut (bool)
- Vendor-neutral naming — applies to any multi-filament system (Bambu AMS, Prusa MMU, Anycubic ACE, etc.)
- P2 fields (cooling_tube_length, cooling_tube_retraction, parking_pos_retraction, extra_loading_move) deferred but this struct is designed to hold them later
- Placement (nested in MultiMaterialConfig vs top-level): Claude's discretion

### Extended Existing Sub-structs

**AccelerationConfig** (+3 fields):
- internal_solid_infill_acceleration (f64)
- support_acceleration (f64)
- support_interface_acceleration (f64)

**CoolingConfig** (+2 fields):
- additional_cooling_fan_speed (f64 — auxiliary fan speed)
- auxiliary_fan (bool — enable auxiliary fan)

**SpeedConfig** (+1 field):
- enable_overhang_speed (bool — master switch for overhang speed adjustments)

**FilamentPropsConfig** (+1 field):
- filament_colour (String — hex color for preview)

**MultiMaterialConfig** (+4 fields):
- wall_filament (Option<usize> — 0-based, None = use default)
- solid_infill_filament (Option<usize>)
- support_filament (Option<usize>)
- support_interface_filament (Option<usize>)
- Import mapper translates OrcaSlicer's 1-based indices to 0-based

### Top-level PrintConfig Fields
- precise_outer_wall (bool)
- draft_shield (bool or enum)
- ooze_prevention (bool)
- infill_combination (u32 — combine infill every N layers)
- infill_anchor_max (f64 — mm)

### Arachne Wall Generation Fields
- min_bead_width (f64)
- min_feature_size (f64)
- Placement: Claude's discretion (PrintConfig top-level or new ArachneConfig)

### Support Fields
- support_bottom_interface_layers (u32)
- (support_filament and support_interface_filament covered under MultiMaterialConfig)

### Patterns Carried Forward from Phase 32
- Config + mapping only — fields stored, serialized, round-tripped, NOT wired into engine
- Migrate from passthrough → typed, remove from passthrough once typed
- OrcaSlicer defaults as baseline
- Both OrcaSlicer JSON AND PrusaSlicer INI mappings added together
- Full Rust doc comments on every field (units, range, description — Phase 35 prep)
- TOML inline comments for self-documenting configs
- G-code template variables for all new fields
- G-code comments emitting new field values
- Basic range validation per field (warn on out-of-range)
- Full re-conversion of ~21k profiles after adding mappings

### Claude's Discretion
- ToolChangeRetractionConfig placement (nested in MultiMaterialConfig vs top-level on PrintConfig)
- Arachne fields placement (top-level vs new ArachneConfig sub-struct)
- draft_shield type (bool vs DraftShieldMode enum)
- Exact field ordering within sub-structs
- G-code template variable naming for new fields
- Which existing brim/skirt fields to migrate into BrimSkirtConfig

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Config audit & field inventory
- `designDocs/CONFIG_PARITY_AUDIT.md` §P1 (line 388-424) — Complete P1 field list with OrcaSlicer keys, descriptions, categories
- `designDocs/01-PRODUCT_REQUIREMENTS.md` §7 — SettingDefinition schema (informs doc comment format for Phase 35 compatibility)

### Current config implementation
- `crates/slicecore-engine/src/config.rs` — PrintConfig and all sub-structs (~1400 lines)
- `crates/slicecore-engine/src/support/config.rs` — SupportConfig (~40 pub fields)
- `crates/slicecore-engine/src/profile_import.rs` — OrcaSlicer/BambuStudio JSON field mapping tables
- `crates/slicecore-engine/src/profile_import_ini.rs` — PrusaSlicer INI field mapping

### Prior config decisions
- `.planning/phases/32-p0-config-gap-closure-critical-missing-fields/32-CONTEXT.md` — Phase 32 P0 patterns (sub-struct organization, migration, validation, G-code emission)
- `.planning/phases/20-expand-printconfig-field-coverage-and-profile-mapping/20-CONTEXT.md` — Phase 20 config expansion patterns (Vec arrays, mapping strategy)
- `.planning/phases/30-cli-profile-composition-and-slice-workflow/30-CONTEXT.md` — Profile merge model, G-code template variables, reproduce command

### Config parity audit context
- `.planning/quick/4-config-parity-audit-and-phase-planning-f/4-CONTEXT.md` — Audit task context (parity strategy, gap categorization)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `PrintConfig` with nested sub-configs — established pattern for new sub-structs (FuzzySkinConfig, BrimSkirtConfig, InputShapingConfig, ToolChangeRetractionConfig)
- `DimensionalCompensationConfig` (Phase 32) — recent example of new sub-struct creation
- `SurfacePattern` enum (Phase 32) — reference for creating BrimType enum
- `BedType` enum (Phase 32) — reference for enum with serde rename mapping
- `profile_import.rs` apply_field_mapping — pattern for upstream key → typed field mapping
- `profile_import_ini.rs` — PrusaSlicer INI mapping pattern
- G-code template variable system — extend for new fields
- Config validation in profile merge — extend for range checks

### Established Patterns
- Sub-struct organization with `#[serde(default)]` for backward compatibility
- Vec<f64> for multi-extruder array fields
- `passthrough` BTreeMap<String,String> catch-all for unmapped keys
- Enum variants with `#[serde(rename = "...")]` for upstream string mapping

### Integration Points
- `config.rs` — add new sub-structs, enums, and fields
- `profile_import.rs` — add JSON field mappings for all 30 fields
- `profile_import_ini.rs` — add INI field mappings
- G-code generator — emit template variables and comments
- Config validation — add range checks
- Existing tests — update snapshots and add field-specific tests

</code_context>

<specifics>
## Specific Ideas

- Multi-filament retraction is vendor-neutral, not "Bambu-specific" — Prusa MMU, Anycubic ACE, and others use similar concepts
- BrimType enum uses Rust-idiomatic short names (None, Outer, Inner, Both) with import mapper handling OrcaSlicer string translation
- Filament index fields use 0-based Option<usize> (None = default extruder), with import mapper translating from OrcaSlicer's 1-based convention
- ToolChangeRetractionConfig designed to be extended with P2 fields later (cooling_tube_length, parking_pos_retraction, etc.)

</specifics>

<deferred>
## Deferred Ideas

- **P2 tool-change retraction fields** — cooling_tube_length, cooling_tube_retraction, parking_pos_retraction, extra_loading_move. ToolChangeRetractionConfig designed to hold these.
- **Profile migration tooling** — Versioned config format with automatic migration when structs change
- **Vendor-specific config extensions** — Extensible config sections for vendor-unique features beyond standard fields
- **Advanced fan curves** — Per-layer fan speed curves, ramping profiles beyond simple min/max
- **Engine behavior for P1 fields** — Wiring fuzzy skin into surface generation, brim ears into brim generator, input shaping into motion planning, etc. Separate phase.

</deferred>

---

*Phase: 33-p1-config-gap-closure-profile-fidelity-fields*
*Context gathered: 2026-03-17*
