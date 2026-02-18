---
phase: 11-config-integration
plan: 03
subsystem: engine
tags: [multi-material, purge-tower, mmu, pipeline, validation]

# Dependency graph
requires:
  - phase: 11-02
    provides: "Sequential printing pipeline integration pattern in Engine"
provides:
  - "Multi-material config validation in Engine pipeline (tool_count/tools mismatch detection)"
  - "Purge tower G-code generation when multi_material.enabled and tool_count > 1"
  - "Warnings for single-tool and no-tool-assignment multi-material scenarios"
affects: [11-04, config-integration]

# Tech tracking
tech-stack:
  added: []
  patterns: ["conditional pipeline step pattern (step 0b validation, step 4c post-processing)"]

key-files:
  created: []
  modified:
    - "crates/slicecore-engine/src/engine.rs"

key-decisions:
  - "Purge tower G-code appended after all model G-code (V1 simplification, per-layer interleaving deferred)"
  - "All V1 purge tower layers are sparse (no actual tool changes without modifier mesh API)"
  - "tool_count vs tools.len() mismatch returns hard ConfigError (not a warning)"

patterns-established:
  - "Multi-material validation as pipeline step 0b (after sequential check, before slicing)"
  - "Purge tower generation as pipeline step 4c (after arc fitting, before time estimation)"

# Metrics
duration: 2min
completed: 2026-02-18
---

# Phase 11 Plan 03: Multi-Material Pipeline Summary

**Multi-material config validation and sparse purge tower G-code generation wired into Engine pipeline**

## Performance

- **Duration:** 2 min
- **Started:** 2026-02-18T18:39:47Z
- **Completed:** 2026-02-18T18:42:14Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Multi-material config validation catches tool_count vs tools.len() mismatch with ConfigError
- Warnings emitted for single-tool multi-material (tool_count <= 1) and no-tool-assignment scenarios
- Sparse purge tower layers generated for each layer when multi_material.enabled and tool_count > 1
- All 479 existing unit tests pass (multi_material defaults to disabled, no behavioral change)

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire multi-material validation and purge tower into Engine pipeline** - `2b24355` (feat)

**Plan metadata:** (pending final commit)

## Files Created/Modified
- `crates/slicecore-engine/src/engine.rs` - Added step 0b (multi-material validation) and step 4c (purge tower G-code generation)

## Decisions Made
- Purge tower G-code appended after all model G-code in V1 (per-layer interleaving deferred to full multi-material implementation)
- All V1 purge tower layers are sparse (has_tool_change=false) since single-mesh API has no modifier mesh tool assignments
- tool_count vs tools.len() mismatch returns a hard ConfigError rather than a warning, since this is a definitive config error

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Multi-material pipeline integration complete
- Ready for plan 11-04 (remaining config integration tasks)
- Full multi-material with modifier mesh tool assignments available via assign_tools_per_region()

## Self-Check: PASSED

- FOUND: crates/slicecore-engine/src/engine.rs
- FOUND: .planning/phases/11-config-integration/11-03-SUMMARY.md
- FOUND: commit 2b24355

---
*Phase: 11-config-integration*
*Completed: 2026-02-18*
