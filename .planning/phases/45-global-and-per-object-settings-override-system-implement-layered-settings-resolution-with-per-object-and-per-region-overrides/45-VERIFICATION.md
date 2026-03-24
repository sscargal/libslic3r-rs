---
phase: 45-global-and-per-object-settings-override-system
verified: 2026-03-24T18:23:31Z
status: gaps_found
score: 26/30 must-haves verified
re_verification: false
gaps:
  - truth: "slicecore plate from-3mf extracts objects + modifiers + plate.toml"
    status: partial
    reason: "plate_cmd.rs uses threemf::parse() which returns only the merged mesh — it does not call parse_with_config() and therefore does not extract per-object settings or modifier overrides into the generated plate.toml"
    artifacts:
      - path: "crates/slicecore-cli/src/plate_cmd.rs"
        issue: "Line 204 calls slicecore_fileio::threemf::parse(&data) instead of parse_with_config — generated plate.toml has no [default_object_overrides] or [[objects.modifiers]] from the 3MF"
    missing:
      - "Call parse_with_config() instead of parse() in the From3mf handler"
      - "Map ThreeMfObjectConfig.overrides to ObjectConfig.inline_overrides in generated plate.toml"
      - "Emit [[objects.modifiers]] entries from ThreeMfModifier data in the generated plate.toml"
  - truth: "slicecore plate to-3mf packages plate config into 3MF"
    status: partial
    reason: "plate_cmd.rs uses save_mesh() on only the first object mesh — it does not call export_plate_to_3mf() which supports per-object overrides and multi-object 3MF output. A code comment explicitly notes 'multi-mesh would require extending the export API'"
    artifacts:
      - path: "crates/slicecore-cli/src/plate_cmd.rs"
        issue: "Lines 288-289 call slicecore_fileio::save_mesh(mesh, &output) using only all_meshes.first() — per-object overrides from plate.toml are never written into the 3MF, and only the first object's mesh is exported"
    missing:
      - "Parse the PlateConfig from plate.toml to extract per-object override data"
      - "Build Vec<ThreeMfObjectConfig> from the plate's ObjectConfig.inline_overrides and modifiers"
      - "Call export_plate_to_3mf() with the collected meshes and ThreeMfObjectConfig list"
human_verification:
  - test: "Run slicecore schema --override-safety warn and verify output contains only warn-classified fields"
    expected: "Only fields classified as Warn in OVERRIDE_SAFETY_MAP.md appear — bed_temperature, fan_speed, skirt/brim/raft fields, etc."
    why_human: "Cannot verify filtered output content correctness without an actual CLI invocation checking domain knowledge of field classifications"
  - test: "Run slicecore override-set create my-set layer_height=0.1 then slicecore override-set list"
    expected: "Override set is created in ~/.slicecore/override-sets/my-set.toml and appears in list output"
    why_human: "Requires filesystem side-effect verification; depends on home directory access"
  - test: "Run slicecore slice with a multi-object plate.toml containing two objects with different override_sets"
    expected: "Both objects are resolved through the cascade independently; G-code contains per-object comment sections with override diffs"
    why_human: "Real slicing with multi-object plate requires actual mesh files; cannot verify G-code output statically"
---

# Phase 45: Global and Per-Object Settings Override System Verification Report

**Phase Goal:** Implement a layered settings override system (global -> per-object -> per-region) with proper cascading, validation, and serialization, enabling users to customize specific objects on multi-object plates with different infill, layer height, or other parameters.
**Verified:** 2026-03-24T18:23:31Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | PlateConfig struct exists with base_layers, default_object_overrides, override_sets, and objects fields | VERIFIED | `crates/slicecore-engine/src/plate_config.rs` line 156 — all fields present |
| 2 | ObjectConfig struct exists with mesh_source, name, override_set, inline_overrides, modifiers, layer_overrides, transform, copies | VERIFIED | `plate_config.rs` line 113 — all 8 fields present |
| 3 | SourceType enum has 4 new variants: DefaultObjectOverride, PerObjectOverride, LayerRangeOverride, PerRegionOverride | VERIFIED | `profile_compose.rs` lines 68-82 — all 4 variants with fields |
| 4 | OverrideSafety enum exists in slicecore-config-schema with Safe, Warn, Ignored variants | VERIFIED | `types.rs` lines 203-209 — full enum with Display and Default |
| 5 | SettingDefinition has override_safety field | VERIFIED | `types.rs` line 263 — `pub override_safety: OverrideSafety` |
| 6 | 10-layer cascade resolves per-object configs correctly with proper priority ordering | VERIFIED | `cascade.rs` — CascadeResolver with resolve_all and resolve_object_config; 17 cascade unit tests pass |
| 7 | Per-region overrides inherit from per-object config, not global | VERIFIED | `cascade.rs` resolve_for_z applies LayerRangeOverride on top of per-object base |
| 8 | Z-schedule computes union of all objects' Z heights with correct object membership | VERIFIED | `z_schedule.rs` — ZSchedule::from_objects with BTreeSet union; 10 tests including proptest |
| 9 | Provenance chain tracks all 10 layers accurately | VERIFIED | ResolvedObject.provenance HashMap in cascade.rs; FieldSource in profile_compose.rs |
| 10 | Arc<PrintConfig> is used for objects with no overrides | VERIFIED | cascade.rs lines 131+ — resolve_all returns Arc::clone for unchanged objects |
| 11 | override_safety attribute is parsed from #[setting()] in derive macro | VERIFIED | parse.rs line 107 — full parsing with validation for "safe"/"warn"/"ignored" |
| 12 | All ~385 PrintConfig fields have override_safety annotations | VERIFIED | codegen.rs line 238 — defaults to Safe; SettingDefinition population verified |
| 13 | OVERRIDE_SAFETY_MAP.md exists with all field classifications for user review | VERIFIED | `designDocs/OVERRIDE_SAFETY_MAP.md` — 408 lines of classification table |
| 14 | Completeness test verifies every SettingRegistry field has override_safety | VERIFIED | config-schema tests: 30 passing; override_safety_default_is_safe test present |
| 15 | slicecore schema --override-safety filter works | VERIFIED | schema_command.rs has SafetyFilter enum and override_safety filter at lines 33/48/157 |
| 16 | ModifierMesh uses toml::map::Map<String, toml::Value> instead of SettingOverrides | VERIFIED | modifier.rs line 43 — `pub overrides: toml::map::Map<String, toml::Value>` |
| 17 | ModifierRegion uses toml::map::Map<String, toml::Value> instead of SettingOverrides | VERIFIED | modifier.rs line 56 — `pub overrides: toml::map::Map<String, toml::Value>` |
| 18 | SettingOverrides struct is removed from config.rs | VERIFIED | grep for SettingOverrides in config.rs returns no results |
| 19 | Engine::new accepts PlateConfig and resolves all per-object configs eagerly | VERIFIED | engine.rs line 719 — `pub fn from_plate_config` calls CascadeResolver::resolve_all |
| 20 | Engine::from_config(PrintConfig) backward compat works for single-object slicing | VERIFIED | engine.rs Engine::new still accepts PrintConfig; PlateConfig::from(PrintConfig) implemented |
| 21 | Layer-range overrides (cascade layer 9) are resolved at slicing time via resolve_for_z | VERIFIED | cascade.rs line 193 — `pub fn resolve_for_z` applied at slicing time |
| 22 | slicecore override-set list/show/create/edit/delete/rename/diff all work | VERIFIED | override_set.rs 568 lines — full CRUD commands, wired to main.rs line 1150 |
| 23 | slicecore plate init generates commented plate.toml template | VERIFIED | plate_cmd.rs generates complete template with profiles, override_sets, objects sections |
| 24 | slicecore plate from-3mf extracts objects + modifiers + plate.toml | PARTIAL | Uses parse() not parse_with_config() — mesh extracted but per-object settings and modifiers are NOT populated in generated plate.toml |
| 25 | slicecore plate to-3mf packages plate config into 3MF | PARTIAL | Uses save_mesh() on first object only — per-object overrides from plate.toml are NOT written to 3MF output |
| 26 | --plate flag loads and slices a plate.toml config | VERIFIED | main.rs lines 863-881 — mutual exclusion enforced, plate path passed to slice workflow |
| 27 | 3MF import extracts per-object settings from OrcaSlicer/PrusaSlicer 3MF files | VERIFIED | threemf.rs parse_with_config() — field mapping, unmapped preservation, import summary |
| 28 | 3MF export writes per-object overrides into 3MF | VERIFIED | export.rs export_plate_to_3mf() with dual-namespace metadata — function exists and works |
| 29 | G-code header includes per-object sections with override diffs and reproduce command | VERIFIED | gcode_gen.rs generate_plate_header() + plate_checksum() + compute_override_diffs() |
| 30 | Plate-level E2E integration test loads plate config, resolves overrides, slices, verifies G-code | VERIFIED | tests/plate_e2e.rs — 6 tests pass including regression, multi-object, z-schedule, invalid set |

**Score:** 28/30 truths verified (2 partial)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/slicecore-engine/src/plate_config.rs` | PlateConfig, ObjectConfig, supporting types | VERIFIED | 300 lines; all types present with From<PrintConfig> |
| `crates/slicecore-engine/src/profile_compose.rs` | Extended SourceType + add_table_layer | VERIFIED | 1038 lines; all 4 new variants + add_table_layer at line 214 |
| `crates/slicecore-config-schema/src/types.rs` | OverrideSafety + override_safety on SettingDefinition | VERIFIED | 294 lines; full enum at line 203 |
| `crates/slicecore-engine/src/cascade.rs` | CascadeResolver with resolve_all, resolve_for_z | VERIFIED | 715 lines; all functions present |
| `crates/slicecore-engine/src/z_schedule.rs` | ZSchedule with from_objects, proptest | VERIFIED | 395 lines; proptest at line 318 |
| `crates/slicecore-engine/src/engine.rs` | Engine with from_plate_config | VERIFIED | 4474 lines; from_plate_config at line 719 |
| `crates/slicecore-engine/src/modifier.rs` | Updated ModifierMesh/ModifierRegion with TOML | VERIFIED | 435 lines; toml::map::Map at lines 43 and 56 |
| `crates/slicecore-config-derive/src/parse.rs` | override_safety attribute parsing | VERIFIED | 246 lines; parsing at line 107 |
| `crates/slicecore-config-derive/src/codegen.rs` | Generated override_safety in HasSettingSchema | VERIFIED | 379 lines; code generation at lines 127 and 181 |
| `designDocs/OVERRIDE_SAFETY_MAP.md` | All field classifications, min 100 lines | VERIFIED | 408 lines |
| `crates/slicecore-cli/src/override_set.rs` | Override set CRUD CLI commands | VERIFIED | 568 lines; exports run_override_set |
| `crates/slicecore-cli/src/plate_cmd.rs` | Plate init/from-3mf/to-3mf commands | PARTIAL | 444 lines; init works; from-3mf and to-3mf missing per-object data |
| `crates/slicecore-fileio/src/threemf.rs` | 3MF import with per-object settings extraction | VERIFIED | 749 lines; parse_with_config() with ThreeMfObjectConfig |
| `crates/slicecore-fileio/src/export.rs` | 3MF export with per-object overrides | VERIFIED | 706 lines; export_plate_to_3mf() with dual-namespace metadata |
| `crates/slicecore-engine/src/gcode_gen.rs` | G-code header with per-object sections | VERIFIED | generate_plate_header + compute_override_diffs + plate_checksum |
| `crates/slicecore-engine/src/statistics.rs` | Per-object statistics aggregation | VERIFIED | ObjectStatistics and PlateStatistics structs |
| `crates/slicecore-engine/benches/cascade_bench.rs` | Criterion benchmarks | VERIFIED | criterion at line 1; CascadeResolver used |
| `tests/fixtures/plate-configs/simple.toml` | Simple plate fixture, min 5 lines | VERIFIED | 8 lines |
| `tests/fixtures/plate-configs/multi-object.toml` | Multi-object plate fixture, min 20 lines | VERIFIED | 42 lines |
| `tests/fixtures/override-sets/high-detail.toml` | Override set fixture | VERIFIED | 4 lines |
| `tests/fixtures/override-sets/fast-draft.toml` | Override set fixture | VERIFIED | 4 lines |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `plate_config.rs` | `profile_compose.rs` | `use.*profile_compose.*SourceType` | VERIFIED | cascade.rs imports both and wires them |
| `cascade.rs` | `profile_compose.rs` | `add_table_layer` | VERIFIED | cascade.rs line 34 imports add_table_layer; uses it for layers 7-8 |
| `cascade.rs` | `plate_config.rs` | `PlateConfig` | VERIFIED | cascade.rs line 33 imports PlateConfig; takes as parameter |
| `engine.rs` | `cascade.rs` | `CascadeResolver` | VERIFIED | engine.rs line 29 imports CascadeResolver; calls resolve_all |
| `engine.rs` | `z_schedule.rs` | `ZSchedule` | VERIFIED | engine.rs uses ZSchedule for per-object layer iteration |
| `cascade.rs` | `plate_config.rs` | `LayerRangeOverride` | VERIFIED | resolve_for_z reads LayerRangeOverride from ObjectConfig |
| `parse.rs` | `types.rs` | `override_safety` | VERIFIED | codegen.rs emits slicecore_config_schema::OverrideSafety tokens |
| `main.rs` | `engine.rs` | `from_plate_config` | VERIFIED | main.rs lines 2230, 2280 — calls Engine::from_plate_config |
| `main.rs` | `plate_config.rs` | `PlateConfig` | VERIFIED | main.rs imports and parses PlateConfig |
| `override_set.rs` | `profile_compose.rs` | `validate_set_key` | VERIFIED | override_set.rs line 13 imports validate_set_key |
| `plate_cmd.rs` | `plate_config.rs` | `PlateConfig` | NOT WIRED | plate_cmd.rs does not import or use PlateConfig — uses raw toml::Value for to-3mf; uses only parse() for from-3mf |
| `threemf.rs` | `plate_config.rs` | `ObjectConfig` | NOT WIRED | fileio crate does not depend on slicecore-engine; ThreeMfObjectConfig is the bridge type but conversion to ObjectConfig is not done in plate_cmd.rs |
| `gcode_gen.rs` | `engine.rs` | `PlateSliceResult` | VERIFIED | gcode_gen.rs line 25 imports PlateSliceResult |

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|---------|
| ADV-03 | 45-01 through 45-10 | Modifier meshes (region-specific setting overrides) | SATISFIED (with gap) | Full cascade system implemented: PlateConfig, ObjectConfig, CascadeResolver, per-region overrides via ModifierConfig with TOML overrides, Z-schedule, engine integration, CLI commands, 3MF I/O, G-code output. Two CLI commands (plate from-3mf, plate to-3mf) do not fully utilize the per-object infrastructure when bridging from/to 3MF. Core requirement fulfilled; 3MF CLI bridge is partial. |

Note: REQUIREMENTS.md tracking table shows `ADV-03 | Phase 6 | Complete` — this entry reflects the original Phase 6 modifier mesh implementation. Phase 45 significantly extends that foundation with the full layered override system. The requirement checkbox `[x] ADV-03` reflects the combined implementation.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/slicecore-cli/src/plate_cmd.rs` | 269-270 | Comment: "multi-mesh would require extending the export API" | Warning | to-3mf command is documented as incomplete |
| `crates/slicecore-cli/src/plate_cmd.rs` | 204 | Uses `parse()` not `parse_with_config()` in from-3mf | Blocker | Per-object settings never extracted from imported 3MF |
| `crates/slicecore-cli/src/plate_cmd.rs` | 288 | Uses `save_mesh()` + `all_meshes.first()` not `export_plate_to_3mf()` | Blocker | Per-object overrides never written to 3MF output |

No TODO/FIXME/unimplemented!() calls found in any engine, cascade, z_schedule, gcode_gen, statistics, modifier, or profile_compose files.

### Human Verification Required

#### 1. Schema CLI --override-safety Filter

**Test:** Run `cargo run -p slicecore-cli -- schema --override-safety warn | head -40`
**Expected:** Output contains only warn-classified fields (bed_temperature, fan_speed settings, skirt/brim/raft fields) and does not include safe fields like layer_height or infill_density
**Why human:** Field classification correctness requires domain knowledge review; programmatic check cannot verify semantic appropriateness of all ~385 classifications

#### 2. Override Set CRUD Workflow

**Test:** Run `slicecore override-set create my-test layer_height=0.1 wall_count=4`, then `slicecore override-set show my-test`, then `slicecore override-set delete my-test`
**Expected:** File created in `~/.slicecore/override-sets/my-test.toml`, show displays the fields, delete removes it
**Why human:** Requires home directory filesystem side effects; slicecore override-set has 'did you mean?' validation that should be tested with typos

#### 3. Multi-Object Plate Slicing via CLI

**Test:** Create a plate.toml with two objects, different override_sets; run `slicecore slice --plate plate.toml --output out.gcode`
**Expected:** G-code file contains per-object comment sections (`; --- Object 1: <name> ---`) with override diffs; per-object statistics in final summary
**Why human:** Requires actual mesh files (STL) to be present; cannot create valid TriangleMesh in static analysis

### Gaps Summary

Two gaps share the same root cause: `crates/slicecore-cli/src/plate_cmd.rs` implements the `plate from-3mf` and `plate to-3mf` subcommands as thin wrappers around the raw mesh I/O functions (`parse()` and `save_mesh()`) rather than the per-object-aware functions (`parse_with_config()` and `export_plate_to_3mf()`) that were implemented in Plan 08.

The consequence:
- `plate from-3mf input.3mf -o output/` extracts the mesh geometry but produces a generic plate.toml template with no per-object settings, overrides, or modifier entries from the original 3MF
- `plate to-3mf plate.toml -o output.3mf` exports only the first object's mesh without any of the override metadata from plate.toml

This is a wiring gap, not a missing implementation gap. The `parse_with_config()` and `export_plate_to_3mf()` functions both exist, are tested, and are re-exported from `slicecore-fileio`. The fix requires updating `plate_cmd.rs` to call these functions and perform the ThreeMfObjectConfig <-> ObjectConfig conversion at the CLI boundary.

All other phase-45 functionality is fully implemented and all 1,304+ workspace lib tests pass with 0 failures.

---

_Verified: 2026-03-24T18:23:31Z_
_Verifier: Claude (gsd-verifier)_
