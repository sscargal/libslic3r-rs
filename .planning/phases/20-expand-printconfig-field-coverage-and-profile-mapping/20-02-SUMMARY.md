---
phase: 20-expand-printconfig-field-coverage-and-profile-mapping
plan: 02
subsystem: config
tags: [serde, json, profile-import, field-mapping, passthrough, extract-array, multi-extruder]

# Dependency graph
requires:
  - phase: 20-expand-printconfig-field-coverage-and-profile-mapping
    provides: "7 nested sub-config structs, BTreeMap passthrough, Vec<f64> multi-extruder arrays (Plan 01)"
  - phase: 13-json-profile-support
    provides: "JSON profile import with field mapping, extract_string_value helper"
provides:
  - "120+ match arm JSON field mapper covering all upstream process/machine/filament fields"
  - "extract_array_f64 helper for multi-extruder Vec<f64> field extraction"
  - "Passthrough storage in default match arm for unmapped upstream fields"
  - "passthrough_fields tracking on ImportResult"
affects: [20-03, 20-04, 20-05, profile-convert, profile-library, profile-import-ini]

# Tech tracking
tech-stack:
  added: []
  patterns: ["FieldMappingResult enum for tri-state mapping (mapped/passthrough/failed)", "Array field mapping before scalar extraction for Vec<f64> fidelity", "parse_percentage_or_f64 for percentage-or-numeric string parsing"]

key-files:
  created: []
  modified: [crates/slicecore-engine/src/profile_import.rs, crates/slicecore-engine/src/config.rs, crates/slicecore-engine/src/profile_convert.rs, crates/slicecore-engine/src/profile_import_ini.rs, crates/slicecore-engine/src/profile_library.rs, crates/slicecore-engine/tests/integration_profile_convert.rs]

key-decisions:
  - "Array fields (nozzle_diameter, jerk, temperature) handled by separate apply_array_field_mapping with raw JSON value before scalar extraction"
  - "Default match arm stores unmapped fields in passthrough BTreeMap and also tracks in unmapped_fields for backward compat with convert pipeline"
  - "Scalar flat fields (nozzle_temp, jerk_x, etc.) still set alongside Vec/sub-config fields for zero breaking changes"
  - "passthrough_fields added to ImportResult alongside unmapped_fields (not replacing it) for backward compatibility"

patterns-established:
  - "apply_array_field_mapping called first for Vec<f64> fields, then extract_string_value + apply_field_mapping for scalars"
  - "FieldMappingResult::Passthrough used to distinguish passthrough from mapping failure in import tracking"

requirements-completed: [SC4-json-mapper, SC6-x1c-profiles]

# Metrics
duration: 9min
completed: 2026-02-24
---

# Phase 20 Plan 02: Expand JSON Field Mapping Summary

**JSON profile mapper expanded from 43 to 120+ mapped fields with extract_array_f64 helper, passthrough storage, and full sub-config coverage**

## Performance

- **Duration:** 9 min
- **Started:** 2026-02-24T22:56:27Z
- **Completed:** 2026-02-24T23:06:21Z
- **Tasks:** 1
- **Files modified:** 6

## Accomplishments
- Expanded apply_field_mapping from 43 to 120+ match arms covering all sub-config categories: speeds (15), line widths (7), cooling (8), retraction (5), machine (14+ strings/floats), acceleration (7), filament props (9), process misc (9), plus all original fields
- Added extract_array_f64 helper that handles JSON string arrays, number arrays, singles, and nil sentinels for multi-extruder Vec<f64> fields
- Changed default match arm to store unmapped fields in config.passthrough BTreeMap for round-trip fidelity
- Added apply_array_field_mapping for nozzle_diameter, jerk_values, and temperature array fields with backward-compatible scalar field population
- Added passthrough_fields to ImportResult and FieldMappingResult enum
- All 64 profile_import unit tests pass (22 new + 42 existing), full workspace clippy-clean
- Updated integration tests for newly-mapped fields (bridge_speed, gap_infill_speed no longer unmapped)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add extract_array_f64 helper and expand JSON field mapping** - `d152804` (feat)

**Plan metadata:** (pending docs commit)

## Files Created/Modified
- `crates/slicecore-engine/src/profile_import.rs` - Added extract_array_f64, apply_array_field_mapping, FieldMappingResult, parse_percentage_or_f64, expanded apply_field_mapping to 120+ arms, passthrough default arm, 22 new tests (+1100 lines)
- `crates/slicecore-engine/src/config.rs` - Added passthrough_fields to ImportResult construction in from_json_with_details
- `crates/slicecore-engine/src/profile_convert.rs` - Added passthrough_fields to ImportResult constructions, updated test for newly-mapped fields
- `crates/slicecore-engine/src/profile_import_ini.rs` - Added passthrough_fields to ImportResult construction
- `crates/slicecore-engine/src/profile_library.rs` - Added passthrough_fields merging in merge_inheritance
- `crates/slicecore-engine/tests/integration_profile_convert.rs` - Updated test_unmapped_fields_in_output for newly-mapped bridge_speed/gap_infill_speed

## Decisions Made
- Array fields handled by separate apply_array_field_mapping with raw JSON value (not extracted string) to preserve all array elements
- Default match arm stores in passthrough AND tracks in unmapped_fields for backward compat with convert pipeline TOML comments
- Scalar flat fields (nozzle_temp, jerk_x, etc.) still set alongside Vec/sub-config fields so existing engine code works unchanged
- passthrough_fields added to ImportResult alongside unmapped_fields rather than replacing it

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated integration tests for newly-mapped fields**
- **Found during:** Task 1 (field mapping expansion)
- **Issue:** test_unmapped_fields_in_output expected bridge_speed and gap_infill_speed to be unmapped, but they are now typed fields
- **Fix:** Updated tests to use truly unknown fields (ams_drying_temperature, scan_first_layer) instead
- **Files modified:** crates/slicecore-engine/tests/integration_profile_convert.rs, crates/slicecore-engine/src/profile_convert.rs
- **Verification:** All integration tests pass
- **Committed in:** d152804 (part of task commit)

**2. [Rule 3 - Blocking] Fixed json! macro recursion limit for 100+ field test**
- **Found during:** Task 1 (unit test for 100+ fields)
- **Issue:** serde_json::json! macro hit recursion limit with 100+ fields in a single object literal
- **Fix:** Built JSON object programmatically using serde_json::Map instead of json! macro
- **Files modified:** crates/slicecore-engine/src/profile_import.rs
- **Verification:** test_match_arm_count_exceeds_100 passes
- **Committed in:** d152804 (part of task commit)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both auto-fixes necessary for test correctness. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- JSON field mapper is comprehensive (120+ typed fields + passthrough for rest)
- Plan 03 (INI mapper expansion) can follow the same pattern for PrusaSlicer profiles
- Plan 04 (field migration) can proceed with flat-to-sub-config migration
- Plan 05 (profile re-conversion) will benefit from expanded mapping

## Self-Check: PASSED

All 7 files verified present. Commit d152804 verified in git log.

---
*Phase: 20-expand-printconfig-field-coverage-and-profile-mapping*
*Completed: 2026-02-24*
