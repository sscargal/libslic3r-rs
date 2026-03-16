---
phase: 31-cli-utility-commands-calibrate-and-estimate
plan: 03
subsystem: cli
tags: [calibration, gcode, mesh-generation, temperature-tower, retraction]

# Dependency graph
requires:
  - phase: 31-01
    provides: "Calibration types, validate_bed_fit, inject_temp_changes, temp_schedule, common CLI helpers"
provides:
  - "Temperature tower mesh generation and CLI command"
  - "Retraction test mesh generation and CLI command"
  - "Retraction schedule and comment injection functions"
  - "Stacked-box tower mesh builder (reusable)"
affects: [31-04, calibration-commands]

# Tech tracking
tech-stack:
  added: []
  patterns: [stacked-box-mesh-generation, gcode-text-postprocessing, companion-instruction-files]

key-files:
  created:
    - crates/slicecore-cli/src/calibrate/temp_tower.rs
    - crates/slicecore-cli/src/calibrate/retraction.rs
  modified:
    - crates/slicecore-engine/src/calibrate.rs
    - crates/slicecore-cli/src/calibrate/mod.rs

key-decisions:
  - "Built stacked tower mesh directly from vertices/indices instead of CSG boolean unions to avoid coincident-face issues"
  - "Post-process G-code as text lines rather than re-parsing to structured GcodeCommand types"

patterns-established:
  - "Calibration mesh builder: build_stacked_tower() creates geometry by concatenating box primitives"
  - "G-code text post-processing: scan lines for Z moves, inject comments/commands at boundaries"
  - "Companion instruction files: .instructions.md alongside .gcode for user guidance"

requirements-completed: []

# Metrics
duration: 7min
completed: 2026-03-16
---

# Phase 31 Plan 03: Temperature Tower and Retraction Calibration Summary

**Stacked-box mesh generation with engine slicing pipeline, temperature injection at Z boundaries, and retraction comment labelling for manual reprint workflow**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-16T17:05:34Z
- **Completed:** 2026-03-16T17:12:45Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Temperature tower generates stacked-box mesh, slices through Engine, injects M104 temperature changes at correct Z heights
- Retraction test generates tower mesh sliced with profile defaults, adds Z-boundary comments labelling each section's target distance
- Both commands use profile defaults with CLI overrides, validate bed fit, and write companion instruction files
- 12 unit tests covering mesh generation, schedule computation, and comment injection

## Task Commits

Each task was committed atomically:

1. **Task 1: Temperature tower mesh generation and command** - `95a1b26` (feat)
2. **Task 2: Retraction test mesh generation and command** - `05ca130` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/calibrate.rs` - Added generate_temp_tower_mesh, generate_retraction_mesh, retraction_schedule, inject_retraction_comments, build_stacked_tower
- `crates/slicecore-cli/src/calibrate/temp_tower.rs` - Full temp-tower CLI command with config resolve, mesh gen, slice, temp injection, G-code output
- `crates/slicecore-cli/src/calibrate/retraction.rs` - Full retraction CLI command with comment injection and manual reprint instruction generation
- `crates/slicecore-cli/src/calibrate/mod.rs` - Wired TempTower and Retraction variants to new implementations

## Decisions Made
- Built mesh geometry directly from vertices and indices rather than using CSG mesh_union, avoiding potential issues with coincident faces at block boundaries
- Used text-based G-code post-processing (scanning for G0/G1 Z moves) instead of trying to parse raw G-code back into structured GcodeCommand types
- Retraction test injects section-labelling comments only (no M207); actual retraction changes require manual reprint per section as documented in companion instructions

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed inject_retraction_comments initial boundary handling**
- **Found during:** Task 1 (unit testing)
- **Issue:** Initial implementation skipped the first schedule entry because the index tracking started after position 0
- **Fix:** Rewrote to use a simpler next_idx counter starting at 0 with a while loop
- **Files modified:** crates/slicecore-engine/src/calibrate.rs
- **Verification:** test_inject_retraction_comments passes
- **Committed in:** 95a1b26 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Minor logic fix required for correct boundary detection. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Temperature tower and retraction commands fully implemented
- Flow rate and first layer commands remain as stubs for future plans
- Companion instruction file pattern established for reuse

---
*Phase: 31-cli-utility-commands-calibrate-and-estimate*
*Completed: 2026-03-16*
