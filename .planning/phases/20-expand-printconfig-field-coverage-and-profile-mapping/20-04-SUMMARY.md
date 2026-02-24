---
phase: 20-expand-printconfig-field-coverage-and-profile-mapping
plan: 04
subsystem: config
tags: [printconfig, sub-config, field-migration, refactor, retraction, speed, cooling, acceleration, filament]

# Dependency graph
requires:
  - phase: 20-01
    provides: Sub-config struct definitions (SpeedConfig, CoolingConfig, RetractionConfig, etc.)
  - phase: 20-02
    provides: JSON profile import mapping to sub-config fields
  - phase: 20-03
    provides: PrusaSlicer INI import mapping to sub-config fields
provides:
  - All flat PrintConfig fields migrated into sub-config structs (27 fields removed)
  - All engine call sites updated to use nested sub-config paths
  - Clean single-source-of-truth for speed, cooling, retraction, machine, acceleration, filament props
affects: [20-05-verify-field-coverage, all-engine-consumers]

# Tech tracking
tech-stack:
  added: []
  patterns: [mut-config-builder-in-tests, sub-config-accessor-pattern]

key-files:
  created: []
  modified:
    - crates/slicecore-engine/src/config.rs
    - crates/slicecore-engine/src/engine.rs
    - crates/slicecore-engine/src/planner.rs
    - crates/slicecore-engine/src/gcode_gen.rs
    - crates/slicecore-engine/src/toolpath.rs
    - crates/slicecore-engine/src/output.rs
    - crates/slicecore-engine/src/multimaterial.rs
    - crates/slicecore-engine/src/arachne.rs
    - crates/slicecore-engine/src/gap_fill.rs
    - crates/slicecore-engine/src/statistics.rs
    - crates/slicecore-engine/src/profile_import.rs
    - crates/slicecore-engine/src/profile_import_ini.rs
    - crates/slicecore-engine/src/profile_convert.rs
    - crates/slicecore-engine/src/profile_library.rs
    - crates/slicecore-engine/tests/integration_profile_import.rs
    - crates/slicecore-engine/tests/integration_profile_convert.rs
    - crates/slicecore-engine/tests/integration_profile_library.rs
    - crates/slicecore-engine/tests/integration_profile_library_ini.rs
    - crates/slicecore-engine/tests/integration_profile_library_bambu.rs
    - crates/slicecore-engine/tests/integration_profile_library_creality.rs
    - crates/slicecore-engine/tests/calibration_cube.rs

key-decisions:
  - "Removed backward-compat flat scalar fields from profile_import.rs and profile_import_ini.rs array mapping sections since flat fields no longer exist"
  - "Test struct literals converted from PrintConfig { field: val, ..Default } to mut config = PrintConfig::default(); config.sub.field = val pattern"
  - "Native JSON config test updated to use nested JSON structure matching new serde layout"
  - "TOML output assertions updated to check for sub-table names ([speeds], [retraction], etc.) instead of flat field names"

patterns-established:
  - "mut-config-builder: Tests use let mut config = PrintConfig::default(); config.sub.field = val; instead of struct literal syntax for nested fields"
  - "sub-config-accessor: Use config.filament.nozzle_temp(), config.machine.nozzle_diameter(), config.machine.jerk_x() for Vec<f64> first-element access"

requirements-completed: ["SC1-process-fields", "SC2-machine-fields", "SC3-filament-fields", "SC7-no-regressions"]

# Metrics
duration: 25min
completed: 2026-02-24
---

# Phase 20 Plan 04: Migrate Flat Fields to Sub-configs Summary

**Removed 27 flat PrintConfig fields, migrated into 6 sub-config structs, and updated ~170 field references across 21 files with all 604 tests passing**

## Performance

- **Duration:** 25 min
- **Started:** 2026-02-24T23:18:44Z
- **Completed:** 2026-02-24T23:43:42Z
- **Tasks:** 2
- **Files modified:** 21

## Accomplishments

- Migrated 27 flat fields from PrintConfig into SpeedConfig, CoolingConfig, RetractionConfig, MachineConfig, AccelerationConfig, and FilamentPropsConfig sub-structs
- Updated ~170 field references across 13 source files and 7 integration test files
- Removed backward-compatibility flat scalar assignments from profile import pipelines (JSON and INI)
- All 604 tests pass, clippy clean, no regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: Migrate flat fields into sub-configs in config.rs** - `eecc3b1` (feat)
2. **Task 2: Update all engine call sites for migrated field paths** - `32a772b` (refactor)

## Files Created/Modified

### config.rs (Task 1)
- `crates/slicecore-engine/src/config.rs` - Removed 27 flat fields, moved into 6 sub-config structs, updated Default impl, updated extrusion_width() to use self.machine.nozzle_diameter(), updated SettingOverrides::merge_into(), updated all config tests

### Engine source files (Task 2)
- `crates/slicecore-engine/src/engine.rs` - Updated speed, filament, acceleration, temperature, bed size field references
- `crates/slicecore-engine/src/planner.rs` - Updated retraction, temperature, fan, nozzle_diameter references + test struct literals
- `crates/slicecore-engine/src/gcode_gen.rs` - Updated retraction, fan, acceleration references + test struct literal
- `crates/slicecore-engine/src/toolpath.rs` - Updated speed, filament references + test struct literals
- `crates/slicecore-engine/src/output.rs` - Updated ConfigSummary builder references + test assertion
- `crates/slicecore-engine/src/multimaterial.rs` - Updated retraction, speed references
- `crates/slicecore-engine/src/arachne.rs` - Updated nozzle_diameter reference + test struct literals
- `crates/slicecore-engine/src/gap_fill.rs` - Updated nozzle_diameter reference
- `crates/slicecore-engine/src/statistics.rs` - Updated filament_diameter, filament_density references

### Profile import/convert files (Task 2)
- `crates/slicecore-engine/src/profile_import.rs` - Updated field mapping code, removed flat scalar backward compat, updated upstream_key_to_config_field string mappings
- `crates/slicecore-engine/src/profile_import_ini.rs` - Updated field mapper code, removed flat scalar backward compat, updated test assertions
- `crates/slicecore-engine/src/profile_convert.rs` - Updated test field references and TOML output assertions
- `crates/slicecore-engine/src/profile_library.rs` - Updated test assertions

### Integration test files (Task 2)
- `crates/slicecore-engine/tests/integration_profile_import.rs` - Updated all field assertions, updated native JSON test to use nested structure
- `crates/slicecore-engine/tests/integration_profile_convert.rs` - Updated all roundtrip assertions
- `crates/slicecore-engine/tests/integration_profile_library.rs` - Updated nozzle_temp assertions
- `crates/slicecore-engine/tests/integration_profile_library_ini.rs` - Updated all field assertions
- `crates/slicecore-engine/tests/integration_profile_library_bambu.rs` - Updated temp and nozzle_diameter assertions
- `crates/slicecore-engine/tests/integration_profile_library_creality.rs` - Updated temp and nozzle_diameter assertions
- `crates/slicecore-engine/tests/calibration_cube.rs` - Updated temp assertions and fan test struct literal

## Decisions Made

- **Removed backward-compat flat scalar fields from import mappers**: Since flat fields no longer exist on PrintConfig, the profile_import.rs and profile_import_ini.rs code that set both Vec fields and flat scalar fields simultaneously was cleaned up to only set Vec/sub-config fields.
- **Test struct literal pattern change**: Tests that used `PrintConfig { field: val, ..Default::default() }` with removed flat fields were converted to `let mut config = PrintConfig::default(); config.sub.field = val;` pattern since nested fields can't be set in struct literals.
- **Native JSON test updated**: The `test_native_json_config` test was updated to use nested JSON structure (`{"speeds": {"perimeter": 60.0}}`) matching the new serde serialization layout.
- **TOML output assertions updated**: The `test_convert_basic_process_profile` test was updated to check for `[speeds]` section with `perimeter = 200.0` instead of flat `perimeter_speed`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Additional files needed call site updates beyond plan list**
- **Found during:** Task 2
- **Issue:** Plan listed 13 files but statistics.rs, profile_library.rs, calibration_cube.rs, and 4 additional integration test files (bambu, creality, library, convert) also had references to migrated fields
- **Fix:** Updated all additional files systematically using cargo check errors as guide
- **Files modified:** statistics.rs, profile_library.rs, calibration_cube.rs, integration_profile_library_bambu.rs, integration_profile_library_creality.rs, integration_profile_library.rs, integration_profile_convert.rs
- **Verification:** cargo check --workspace --tests passes cleanly
- **Committed in:** 32a772b (Task 2 commit)

**2. [Rule 1 - Bug] Two engine.rs filament_diameter references missed in first pass**
- **Found during:** Task 2
- **Issue:** Two references to config.filament_diameter in engine.rs compute_e_value calls were missed during initial replacement
- **Fix:** Updated both to config.filament.diameter
- **Verification:** cargo check passes
- **Committed in:** 32a772b (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both auto-fixes were necessary for compilation. The plan underestimated the number of files with references to migrated fields. No scope creep.

## Issues Encountered

None beyond the deviations documented above.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- All flat PrintConfig fields have been migrated to sub-config structs
- Profile import pipelines (JSON and INI) write directly to sub-config fields
- TOML serialization produces nested [speeds], [retraction], [filament], [machine], [cooling], [accel] sections
- Ready for Plan 05 (verify-field-coverage) to audit completeness

## Self-Check: PASSED

- [x] 20-04-SUMMARY.md exists
- [x] Commit eecc3b1 (Task 1) found in git log
- [x] Commit 32a772b (Task 2) found in git log
- [x] All 21 modified files exist on disk

---
*Phase: 20-expand-printconfig-field-coverage-and-profile-mapping*
*Completed: 2026-02-24*
