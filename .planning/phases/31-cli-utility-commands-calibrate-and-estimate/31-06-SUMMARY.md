---
phase: 31-cli-utility-commands-calibrate-and-estimate
plan: 06
subsystem: testing
tags: [calibration, cost-estimation, integration-tests, cli, e2e]

# Dependency graph
requires:
  - phase: 31-02
    provides: "analyze-gcode cost estimation CLI flags and display functions"
  - phase: 31-03
    provides: "Temperature tower and retraction calibration commands"
  - phase: 31-04
    provides: "Flow rate and first layer calibration commands"
provides:
  - "14 engine-level integration tests for calibration mesh generation, temp injection, cost model, bed validation"
  - "12 new CLI E2E tests for all calibrate subcommands, dry-run, save-model, output formats, error handling"
  - "Full regression test coverage for Phase 31 calibrate and estimate features"
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns: [engine-level integration test pattern for calibration, CLI binary E2E test pattern with tempdir]

key-files:
  created:
    - crates/slicecore-engine/tests/calibration_tests.rs
  modified:
    - crates/slicecore-cli/tests/cli_calibrate.rs

key-decisions:
  - "Used 40mm bed for small-bed test (not 50mm) since 30mm model fits exactly on 50mm bed with 10mm margins"
  - "CLI E2E tests verify process does not panic rather than requiring specific error codes for edge cases"

patterns-established:
  - "Calibration integration test pattern: test mesh dimensions, bed fit validation, temperature injection correctness"
  - "CLI E2E error handling pattern: verify no panic on bad input rather than specific exit codes"

requirements-completed: []

# Metrics
duration: 3min
completed: 2026-03-16
---

# Phase 31 Plan 06: Integration Tests for Calibrate and Estimate Summary

**33 integration tests covering all calibrate subcommands (temp-tower, retraction, flow, first-layer), cost estimation, dry-run, save-model, output formats, and error handling**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-16T17:29:26Z
- **Completed:** 2026-03-16T17:32:42Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- 14 engine-level tests: temp tower mesh dimensions (9 blocks, ~73mm height), bed fit validation (pass/fail), temp injection at correct Z heights, cost model (full/partial/zero-hours), volume estimate, flow mesh, first layer mesh, retraction mesh
- 12 new CLI E2E tests: temp-tower generates gcode with M104, dry-run shows info without creating files, save-model produces valid STL, instructions file created, retraction/flow/first-layer generate valid gcode, list shows all 4 names, all output formats (JSON/CSV/markdown), model rough estimate with disclaimer, bad temp range and invalid output dir error handling
- Total 19 CLI tests (7 existing + 12 new) and 14 engine tests all passing

## Task Commits

Each task was committed atomically:

1. **Task 1: Engine-level calibration tests** - `b0fbc6e` (test)
2. **Task 2: CLI E2E integration tests** - `f54d1fd` (test)

## Files Created/Modified
- `crates/slicecore-engine/tests/calibration_tests.rs` - 14 integration tests for mesh generation, temp injection, cost model, bed validation
- `crates/slicecore-cli/tests/cli_calibrate.rs` - Extended with 12 new E2E tests for calibrate commands, cost formats, error handling

## Decisions Made
- Used 40mm bed (not 50mm) for the "fails small bed" test because 30mm model fits exactly on a 50mm bed with 10mm per-side margins (50 - 20 = 30)
- CLI E2E error tests verify no panic rather than requiring specific error codes, since graceful degradation is more important than specific exit codes

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed small-bed test bed size**
- **Found during:** Task 1 (engine-level calibration tests)
- **Issue:** Plan specified 50x50 bed, but 30mm model fits exactly (50 - 2*10mm margin = 30mm)
- **Fix:** Changed test to use 40x40 bed where 30mm model exceeds 20mm usable space
- **Files modified:** crates/slicecore-engine/tests/calibration_tests.rs
- **Verification:** test_temp_tower_mesh_fails_small_bed passes
- **Committed in:** b0fbc6e (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Minor test parameter adjustment. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 31 is now fully complete with all 6 plans executed
- All calibrate commands, cost estimation, multi-config comparison, dry-run, save-model, and output formats are implemented and tested
- 33 total integration tests provide comprehensive regression coverage

---
*Phase: 31-cli-utility-commands-calibrate-and-estimate*
*Completed: 2026-03-16*
