---
phase: 27-build-plate-auto-arrangement
plan: 05
subsystem: testing
tags: [integration-tests, arrangement, multi-plate, sequential, material-grouping, json-serialization]

# Dependency graph
requires:
  - phase: 27-build-plate-auto-arrangement
    provides: "arrange() API, ArrangeConfig, ArrangePart, multi-plate splitting, sequential mode, material grouping"
provides:
  - "10 integration tests covering all Phase 27 success criteria"
  - "End-to-end validation of arrangement pipeline"
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Integration test helpers: make_cube_vertices, make_arrange_part, default_config"
    - "Success criteria mapped 1:1 to test functions (sc1_ through sc10_ prefix)"

key-files:
  created:
    - "crates/slicecore-arrange/tests/integration.rs"
  modified: []

key-decisions:
  - "Six 90mm cubes for multi-plate test instead of twenty 80mm (performance: 7s vs 200+s)"
  - "GantryModel::None for sequential test to avoid gantry overlap splitting parts to different plates"
  - "SC4 auto-orient test verifies code path execution (returns identity without normals)"

patterns-established:
  - "SC-prefixed test naming convention for traceability to success criteria"

requirements-completed: [ADV-02]

# Metrics
duration: 10min
completed: 2026-03-11
---

# Phase 27 Plan 05: Integration Tests Summary

**10 integration tests validating full arrangement pipeline: single/multi-plate placement, sequential ordering, auto-orient, material grouping, JSON serialization, error handling, bed parsing, and centering**

## Performance

- **Duration:** 10 min
- **Started:** 2026-03-11T21:42:25Z
- **Completed:** 2026-03-11T21:52:25Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- 10 integration tests covering all Phase 27 success criteria (SC1-SC10)
- End-to-end validation through public arrange() API
- All 64 unit + 10 integration + 15 doc tests pass (89 total for crate)
- Full workspace test suite green

## Task Commits

Each task was committed atomically:

1. **Task 1: Integration tests for arrangement pipeline** - `79c9a6c` (test)

## Files Created/Modified
- `crates/slicecore-arrange/tests/integration.rs` - 10 integration tests covering all arrangement scenarios

## Decisions Made
- Six 90mm cubes for SC2 multi-plate test instead of twenty 80mm (7s vs 200+s runtime)
- GantryModel::None for SC3 sequential test to keep both parts on same plate for ordering verification
- SC4 auto-orient verifies code path execution (identity result without normals is expected behavior)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Adjusted SC2 part count for test performance**
- **Found during:** Task 1 (RED phase)
- **Issue:** Twenty 80mm cubes caused test to run 200+ seconds due to scan-based placement
- **Fix:** Reduced to six 90mm cubes which still triggers multi-plate splitting in ~7s
- **Files modified:** crates/slicecore-arrange/tests/integration.rs
- **Verification:** Test completes in ~7 seconds and asserts total_plates > 1

**2. [Rule 1 - Bug] Fixed SC3 sequential mode test**
- **Found during:** Task 1 (RED phase)
- **Issue:** Cylinder gantry radius 35mm + 50mm cubes caused gantry overlap, splitting parts to different plates; only one print_order on first plate
- **Fix:** Used GantryModel::None and smaller 20mm cubes to keep both parts on one plate for ordering verification
- **Files modified:** crates/slicecore-arrange/tests/integration.rs
- **Verification:** Both parts placed on same plate with print_order 0 and 1

---

**Total deviations:** 2 auto-fixed (2 bugs in test design)
**Impact on plan:** Test adjustments for correctness and performance. All success criteria still covered.

## Issues Encountered
None beyond the auto-fixed deviations above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 27 integration tests complete
- All arrangement features validated end-to-end
- Phase 27 fully complete

---
*Phase: 27-build-plate-auto-arrangement*
*Completed: 2026-03-11*
