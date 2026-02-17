---
phase: 04-perimeter-and-infill-completeness
plan: 02
subsystem: perimeter
tags: [seam-placement, toolpath, perimeter, concave-corner-detection]

# Dependency graph
requires:
  - phase: 01-foundation-types
    provides: "IPoint2, ValidPolygon, COORD_SCALE coordinate system"
  - phase: 03-vertical-slice-stl-to-gcode
    provides: "Toolpath assembly, perimeter generation, engine pipeline"
provides:
  - "SeamPosition enum with 4 placement strategies (Aligned, Random, Rear, NearestCorner)"
  - "select_seam_point() function for vertex selection"
  - "Cross-layer seam tracking in engine pipeline"
  - "seam_position config field in PrintConfig"
affects: [04-perimeter-and-infill-completeness, toolpath-assembly, gcode-output]

# Tech tracking
tech-stack:
  added: []
  patterns: [sequential-edge-cross-product-concavity, knuth-multiplicative-hash, seam-rotation-in-toolpath]

key-files:
  created:
    - "crates/slicecore-engine/src/seam.rs"
  modified:
    - "crates/slicecore-engine/src/config.rs"
    - "crates/slicecore-engine/src/lib.rs"
    - "crates/slicecore-engine/src/toolpath.rs"
    - "crates/slicecore-engine/src/engine.rs"

key-decisions:
  - "Sequential edge cross product for concavity detection (not vertex-based angle comparison)"
  - "Knuth multiplicative hash (2654435761) for deterministic Random seam placement"
  - "assemble_layer_toolpath returns (LayerToolpath, Option<IPoint2>) tuple for cross-layer seam tracking"
  - "5-degree angle deviation threshold for NearestCorner smooth-curve fallback to Aligned"

patterns-established:
  - "Seam rotation: iterate polygon vertices starting from seam_idx with modular arithmetic wrapping"
  - "Cross-layer state passing: engine tracks previous_seam across layer loop iterations"

# Metrics
duration: 10min
completed: 2026-02-17
---

# Phase 04 Plan 02: Seam Placement Strategies Summary

**Four seam placement strategies (Aligned, Random, Rear, NearestCorner) with concave corner detection and cross-layer alignment integrated into toolpath assembly**

## Performance

- **Duration:** 10 min
- **Started:** 2026-02-17T00:37:09Z
- **Completed:** 2026-02-17T00:47:00Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Implemented SeamPosition enum with Aligned, Random, Rear, and NearestCorner strategies
- select_seam_point correctly identifies concave corners using sequential edge cross products
- Toolpath assembly rotates polygon iteration to start at seam-selected vertex
- Cross-layer seam tracking enables Aligned strategy to maintain vertical seam line
- 14 new tests covering all strategies and integration with toolpath assembly

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement seam placement module and config** - `3943284` (feat)
2. **Task 2: Integrate seam placement into toolpath assembly** - `d4664ca` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/seam.rs` - SeamPosition enum, select_seam_point(), corner angle computation, distance helpers
- `crates/slicecore-engine/src/config.rs` - Added seam_position: SeamPosition field to PrintConfig
- `crates/slicecore-engine/src/lib.rs` - Added pub mod seam and re-exports
- `crates/slicecore-engine/src/toolpath.rs` - Modified assemble_layer_toolpath to use seam placement with rotation
- `crates/slicecore-engine/src/engine.rs` - Added cross-layer previous_seam tracking in slice pipeline

## Decisions Made
- **Sequential edge cross product for concavity**: Using cross(edge_in, edge_out) instead of vertex-based angle comparison correctly identifies concave corners in CCW polygons. Negative cross = concave (right turn) in CCW winding.
- **5-degree threshold for smooth detection**: If all vertex angles deviate less than 5 degrees from the mean, the polygon is considered "smooth" (regular polygon / circle approximation) and NearestCorner falls back to Aligned.
- **Tuple return for seam tracking**: assemble_layer_toolpath now returns `(LayerToolpath, Option<IPoint2>)` so the engine can pass the seam point to the next layer without storing mutable state in the config.
- **Seam rotation via modular indexing**: Instead of physically rotating the points array, we iterate `seam_idx..seam_idx+n` with `% n` wrapping, avoiding allocation.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed incomplete 04-01 plan artifacts**
- **Found during:** Task 1 (initial build)
- **Issue:** Previous plan (04-01) left infill.rs conflicting with infill/mod.rs, and adaptive.rs had missing recompute_z_positions function
- **Fix:** Removed stale infill.rs, verified adaptive.rs already had the function from linter
- **Files modified:** crates/slicecore-engine/src/infill.rs (deleted), crates/slicecore-slicer/src/adaptive.rs
- **Verification:** cargo build -p slicecore-engine succeeds
- **Committed in:** 3943284 (Task 1 commit)

**2. [Rule 1 - Bug] Fixed concavity detection using wrong angle convention**
- **Found during:** Task 1 (L-shape test failing)
- **Issue:** compute_corner_angle with vectors from curr-to-prev/next gives wrong concavity sign. Positive cross from vertex-based vectors does not mean concave for CCW polygons.
- **Fix:** Switched to sequential edge cross product (edge_in x edge_out) for concavity determination, keeping angle computation separate for scoring
- **Files modified:** crates/slicecore-engine/src/seam.rs
- **Verification:** nearest_corner_selects_concave_corner_on_l_shape test passes
- **Committed in:** 3943284 (Task 1 commit)

**3. [Rule 3 - Blocking] Fixed clippy errors in slicer adaptive.rs**
- **Found during:** Task 2 (clippy verification)
- **Issue:** needless_range_loop and implicit_saturating_sub clippy warnings from previous plan's code
- **Fix:** Used iterator enumerate with saturating_sub
- **Files modified:** crates/slicecore-slicer/src/adaptive.rs
- **Verification:** cargo clippy -p slicecore-engine -- -D warnings passes clean
- **Committed in:** d4664ca (Task 2 commit)

---

**Total deviations:** 3 auto-fixed (1 bug, 2 blocking)
**Impact on plan:** All fixes were necessary for correctness and clean compilation. No scope creep.

## Issues Encountered
- The previous plan (04-01) left the codebase in a partially inconsistent state with both infill.rs and infill/mod.rs existing. This required cleanup before the seam module could compile.
- The linter was reverting changes to config.rs and lib.rs between edits, requiring careful re-reading and re-application of changes.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Seam placement is fully integrated and ready for use
- All 4 strategies work correctly with the engine pipeline
- Cross-layer seam tracking enables consistent aligned seams
- Ready for subsequent perimeter/infill plans in Phase 04

## Self-Check: PASSED

- [x] crates/slicecore-engine/src/seam.rs exists
- [x] .planning/phases/04-perimeter-and-infill-completeness/04-02-SUMMARY.md exists
- [x] Commit 3943284 (Task 1) found
- [x] Commit d4664ca (Task 2) found
- [x] 107 unit + 14 integration tests pass
- [x] cargo clippy -p slicecore-engine -- -D warnings clean

---
*Phase: 04-perimeter-and-infill-completeness*
*Completed: 2026-02-17*
