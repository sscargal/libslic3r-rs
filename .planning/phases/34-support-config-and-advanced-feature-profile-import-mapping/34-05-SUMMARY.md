---
phase: 34-support-config-and-advanced-feature-profile-import-mapping
plan: 05
subsystem: config
tags: [gcode-template, variable-translation, profile-import, dual-storage]

# Dependency graph
requires:
  - phase: 34-02
    provides: Support config typed fields and import mappings
  - phase: 34-03
    provides: Scarf joint and multi-material import mappings
  - phase: 34-04
    provides: PostProcess, P2 niche fields, and straggler field coverage
provides:
  - G-code variable translation tables for OrcaSlicer (29 entries) and PrusaSlicer (34 entries)
  - Dual storage (original + translated) for all G-code hook and machine G-code fields
  - Template variable registration for 15 Phase 34 config fields
  - Range validation for 7 Phase 34 fields
affects: [34-06, profile-import, gcode-generation]

# Tech tracking
tech-stack:
  added: []
  patterns: [data-driven-translation-table, dual-gcode-storage, longest-first-replacement]

key-files:
  created:
    - crates/slicecore-engine/src/gcode_template.rs
  modified:
    - crates/slicecore-engine/src/lib.rs
    - crates/slicecore-engine/src/custom_gcode.rs
    - crates/slicecore-engine/src/config.rs
    - crates/slicecore-engine/src/config_validate.rs
    - crates/slicecore-engine/src/profile_import.rs
    - crates/slicecore-engine/src/profile_import_ini.rs

key-decisions:
  - "Sequential replacement with longest-first sort for variable translation (simple, correct for non-overlapping variable names)"
  - "Dual storage with _original suffix fields alongside translated fields for round-trip fidelity"
  - "Translation wired into both importers at field-assignment time, not as a post-processing step"

patterns-established:
  - "gcode_template module: data-driven translation tables with build_*_translation_table() + translate_gcode_template()"
  - "Dual G-code storage: _original fields preserve verbatim upstream G-code"

requirements-completed: [GCODE-TRANSLATE]

# Metrics
duration: 5min
completed: 2026-03-17
---

# Phase 34 Plan 05: G-code Template Variable Translation Summary

**Data-driven G-code variable translation with 63 total mappings, dual storage for 9 G-code fields, 15 new template variable registrations, and 7 range validators**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-17T17:48:15Z
- **Completed:** 2026-03-17T17:53:32Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Created gcode_template.rs module with OrcaSlicer (29 entries) and PrusaSlicer (34 entries) translation tables
- Added dual storage (_original fields) for all 9 G-code template fields across CustomGcodeHooks and MachineConfig
- Wired translation into both profile_import.rs (OrcaSlicer) and profile_import_ini.rs (PrusaSlicer)
- Registered 15 Phase 34 template variables in resolve_variable (support, bridge, scarf, multi-material, P2)
- Added range validation for 7 critical Phase 34 fields

## Task Commits

Each task was committed atomically:

1. **Task 1: Create G-code variable translation table and translate function** - `60568da` (feat)
2. **Task 2: Add dual G-code storage fields and wire translation into import** - `a479652` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/gcode_template.rs` - New module: translation tables and translate function with 12 tests
- `crates/slicecore-engine/src/lib.rs` - Register gcode_template module
- `crates/slicecore-engine/src/custom_gcode.rs` - Add 6 _original fields for dual G-code storage
- `crates/slicecore-engine/src/config.rs` - Add 3 _original fields to MachineConfig
- `crates/slicecore-engine/src/config_validate.rs` - Register 15 template variables, add 7 range validators
- `crates/slicecore-engine/src/profile_import.rs` - Wire OrcaSlicer translation for 8 G-code fields
- `crates/slicecore-engine/src/profile_import_ini.rs` - Wire PrusaSlicer translation for 8 G-code fields

## Decisions Made
- Used sequential replacement with longest-first sort order rather than a more complex single-pass approach. This is correct for non-overlapping variable names and simpler to maintain.
- Placed _original fields directly adjacent to their translated counterparts in the struct definitions for clarity.
- Translation happens at import-time (in the field mapping functions) rather than as a separate post-processing step, so translated values are always available without extra function calls.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Updated PrusaSlicer test for layer_gcode translation**
- **Found during:** Task 2
- **Issue:** Existing test `test_machine_gcode_string_fields` expected raw `[layer_num]` but translation now converts to `{layer_num}`
- **Fix:** Updated assertion to expect translated value and added assertion verifying original is preserved
- **Files modified:** crates/slicecore-engine/src/profile_import_ini.rs
- **Verification:** All 773 tests pass
- **Committed in:** a479652 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** Test update was a direct consequence of the planned changes. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- G-code translation system complete and integrated into both import paths
- Plan 06 (re-conversion sweep and validation) can proceed to verify end-to-end coverage
- All Phase 34 fields now have template variable registration and range validation

---
*Phase: 34-support-config-and-advanced-feature-profile-import-mapping*
*Completed: 2026-03-17*
