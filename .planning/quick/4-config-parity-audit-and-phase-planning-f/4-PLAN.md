---
phase: quick-4
plan: 1
type: execute
wave: 1
depends_on: []
files_modified:
  - designDocs/CONFIG_PARITY_AUDIT.md
autonomous: true
requirements: []

must_haves:
  truths:
    - "Document catalogs every PrintConfig field and its OrcaSlicer/PrusaSlicer equivalent"
    - "Missing fields are categorized by priority (P0/P1/P2)"
    - "Phase recommendations exist for systematic gap closure and ConfigSchema"
  artifacts:
    - path: "designDocs/CONFIG_PARITY_AUDIT.md"
      provides: "Complete config parity audit with gap analysis and phase recommendations"
      min_lines: 300
  key_links: []
---

<objective>
Produce a comprehensive CONFIG_PARITY_AUDIT.md that compares libslic3r-rs PrintConfig against OrcaSlicer/BambuStudio/PrusaSlicer, identifies all missing fields, and recommends new phases for gap closure and ConfigSchema implementation.

Purpose: Enable systematic planning for feature parity with upstream slicers before innovating with unique capabilities.
Output: designDocs/CONFIG_PARITY_AUDIT.md
</objective>

<execution_context>
@./.claude/get-shit-done/workflows/execute-plan.md
@./.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@.planning/ROADMAP.md
@designDocs/01-PRODUCT_REQUIREMENTS.md (Section 7 — SettingDefinition schema)
@crates/slicecore-engine/src/config.rs (~1400 lines — PrintConfig + all sub-structs)
@crates/slicecore-engine/src/support/config.rs (~40 pub fields — SupportConfig)
@crates/slicecore-engine/src/profile_import.rs (~1200 lines — field mapping tables)
</context>

<tasks>

<task type="auto">
  <name>Task 1: Audit current PrintConfig fields and upstream mapping coverage</name>
  <files>designDocs/CONFIG_PARITY_AUDIT.md</files>
  <action>
Create a comprehensive audit document at designDocs/CONFIG_PARITY_AUDIT.md with the following structure:

**Section 1: Executive Summary**
- Total libslic3r-rs fields (~255 across all sub-structs)
- Total upstream mapped fields (217 from profile_import.rs)
- Estimated gap percentage vs OrcaSlicer full config

**Section 2: Current Field Inventory**
Enumerate every field in PrintConfig and its sub-structs (LineWidthConfig, SpeedConfig, CoolingConfig, RetractionConfig, MachineConfig, AccelerationConfig, FilamentPropsConfig, SupportConfig, IroningConfig, ScarfJointConfig, MultiMaterialConfig, SequentialConfig, PostProcessConfig, PerFeatureFlow, CustomGcodeHooks), grouped by sub-struct. For each field, note:
- Field name and type
- Whether it has an upstream JSON mapping (from profile_import.rs apply_field_mapping + apply_array_field_mapping)
- OrcaSlicer equivalent field name (from upstream_key_to_config_field)

**Section 3: Known Missing Fields (Gap Analysis)**
Cross-reference against OrcaSlicer's known config domains to identify fields we do NOT have. Derive from:
1. Fields that go to passthrough (the `_ =>` catch-all in apply_field_mapping)
2. Known OrcaSlicer features with no PrintConfig representation

Organize gaps into categories:

**P0 — Critical for print quality parity:**
- Chamber temperature (chamber_temperature) — needed for ABS/ASA/PC enclosed printing
- XY hole/contour compensation (xy_hole_compensation, xy_contour_compensation) — dimensional accuracy
- Minimum wall length for acceleration (min_length_for_acceleration fields)
- Extra perimeters over overhangs (extra_perimeters_on_overhangs)
- Top/bottom fill pattern selection (top_surface_pattern, bottom_surface_pattern) — separate from infill
- Internal bridge settings (internal_bridge_speed, internal_bridge_support_enabled)
- Filament shrinkage compensation (filament_shrink)
- Z offset (z_offset) — global and per-filament
- Bed type selection (curr_bed_type) for temperature lookup

**P1 — Important for profile fidelity:**
- Input shaping / vibration compensation (accel_to_decel_enable, accel_to_decel_factor)
- Fan speed curves by layer (additional_cooling_fan_speed, auxiliary_fan)
- Precise wall (precise_outer_wall)
- Complete acceleration set (internal_solid_infill_acceleration, support_acceleration, support_interface_acceleration)
- Long retraction when cut (retraction_distances_when_cut, long_retractions_when_cut)
- Filament-specific cooling overrides (fan_max_speed per filament, etc.)
- Skirt height (skirt_height)
- Brim type and ears (brim_type, brim_ears, brim_ears_max_angle)
- Minimum extrusion length (min_bead_width, min_feature_size)
- Draft shield
- Ooze prevention
- Complete interface layer control (support_bottom_interface_layers, support_interface_filament)
- Fuzzy skin (fuzzy_skin, fuzzy_skin_thickness, fuzzy_skin_point_dist)

**P2 — Nice-to-have / Niche:**
- Timelapse type selection (timelapse_type — beyond basic park)
- Printable area as polygon (vs simple bed_x/bed_y rectangle — already partially via bed_shape)
- Thumbnail sizes array (thumbnails)
- Emit machine limits to gcode
- Max travel detour length
- External perimeter extrusion role
- Infill anchor max length
- Independent support extruder selection
- Slicing tolerance (gauss/nearest)
- Post-process scripts path
- Silent mode / stealth chop
- Nozzle HRC (hardness rating)
- Compatible condition expressions

**Section 4: Mapping Coverage Statistics**
- Table showing: sub-struct | our fields | mapped from upstream | unmapped upstream fields known
- Total coverage percentage per category

**Section 5: Recommended Phases**

**Phase 31 (or next available): Config Gap Closure — P0 Fields**
- Add ~15-20 P0 fields to PrintConfig sub-structs
- Add profile_import.rs mappings for each
- Add profile_import_ini.rs mappings for PrusaSlicer equivalents
- Estimated: 3-4 plans, 1 wave
- Fields: chamber_temp, xy_hole_compensation, xy_contour_compensation, extra_perimeters_on_overhangs, top/bottom_surface_pattern, internal_bridge settings, filament_shrink, z_offset, bed_type, min_length acceleration fields

**Phase 32 (or next): Config Gap Closure — P1 Fields**
- Add ~25-30 P1 fields
- Estimated: 3-5 plans, 1-2 waves
- Fields: input shaping, advanced fan curves, precise wall, complete acceleration set, brim improvements, fuzzy skin, draft shield, ooze prevention, complete support interface control

**Phase 33 (or next): ConfigSchema System**
- Build SettingDefinition metadata system from PRD Section 7
- Per-field metadata: display_name, description, tier (0-4), category, value_type, default, constraints, affects/affected_by, units, tags
- Derive from: compile-time macro or build script that reads config struct definitions
- Outputs: JSON Schema, auto-generated docs, validation layer, UI form generation data
- Estimated: 4-5 plans, 2-3 waves
- Phase 1: Core schema types and derive macro
- Phase 2: Apply to all PrintConfig fields
- Phase 3: JSON Schema generation and validation integration

**Section 6: ConfigSchema System Design Notes**
Reference PRD Section 7 SettingDefinition schema. Outline:
- proc-macro approach: `#[setting(tier = 2, category = "speed", units = "mm/s")]`
- Runtime registry: HashMap<SettingKey, SettingDefinition>
- Outputs: JSON Schema for validation, UI form metadata, auto-docs, setting search
- Progressive disclosure tiers (0=AI auto, 1=simple ~15, 2=intermediate ~60, 3=advanced ~200, 4=developer all)

**Section 7: Priority Matrix**
Table: Phase | Fields | Effort | Impact | Depends On
  </action>
  <verify>
    <automated>test -f designDocs/CONFIG_PARITY_AUDIT.md && wc -l designDocs/CONFIG_PARITY_AUDIT.md | awk '{if ($1 >= 300) print "PASS"; else print "FAIL: only " $1 " lines"}'</automated>
  </verify>
  <done>CONFIG_PARITY_AUDIT.md exists with 300+ lines covering: complete field inventory, P0/P1/P2 gap categorization, phase recommendations for gap closure and ConfigSchema, and priority matrix</done>
</task>

</tasks>

<verification>
- designDocs/CONFIG_PARITY_AUDIT.md exists and is comprehensive
- All current PrintConfig sub-structs are inventoried
- Gaps are categorized with clear P0/P1/P2 priorities
- Phase recommendations include scope estimates
- ConfigSchema system design is outlined
</verification>

<success_criteria>
- Document covers field-by-field comparison across all config sub-structs
- At least 40 missing fields identified and categorized
- 3+ phase recommendations with estimated plan counts
- ConfigSchema system recommendation references PRD Section 7
</success_criteria>

<output>
After completion, create `.planning/quick/4-config-parity-audit-and-phase-planning-f/4-SUMMARY.md`
</output>
