---
phase: 41-travel-move-optimization
plan: 02
subsystem: engine
tags: [tsp, travel-optimization, toolpath-ordering, rayon, parallel, statistics]

# Dependency graph
requires:
  - phase: 41-01
    provides: "TSP algorithms (optimize_tour, TspNode) and TravelOptConfig"
provides:
  - "TSP optimizer wired into assemble_layer_toolpath for perimeters, gap fills, and infill"
  - "TravelOptStats struct with baseline/optimized/reduction travel distances"
  - "Per-layer travel stat accumulation compatible with rayon parallelism"
  - "Fallback to original ordering when travel_opt.enabled=false"
affects: [41-03, 41-04]

# Tech tracking
tech-stack:
  added: []
  patterns: [parallel-safe-accumulation, type-alias-for-complex-tuples]

key-files:
  created: []
  modified:
    - crates/slicecore-engine/src/toolpath.rs
    - crates/slicecore-engine/src/engine.rs
    - crates/slicecore-engine/src/statistics.rs
    - crates/slicecore-engine/src/lib.rs
    - crates/slicecore-engine/src/output.rs

key-decisions:
  - "Contour-level TSP reordering for perimeters: optimizer decides contour visit order, wall order within each contour preserved"
  - "LayerResult type alias to address clippy type_complexity warnings from expanded 4-tuple return"
  - "Travel stats accumulated per-layer then summed after parallel collect, avoiding shared mutable state"

patterns-established:
  - "LayerResult type alias: (LayerToolpath, Option<IPoint2>, f64, f64) for toolpath + seam + travel stats"
  - "Parallel-safe stat accumulation: each layer returns independent values, summed sequentially after collect"

requirements-completed: [GCODE-05]

# Metrics
duration: 11min
completed: 2026-03-20
---

# Phase 41 Plan 02: Pipeline Integration Summary

**TSP optimizer wired into toolpath assembly for perimeter, gap fill, and infill reordering with parallel-safe travel distance tracking in TravelOptStats**

## Performance

- **Duration:** 11 min
- **Started:** 2026-03-20T16:53:00Z
- **Completed:** 2026-03-20T17:04:25Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- TravelOptStats struct added to statistics.rs with baseline/optimized/reduction fields, wired into SliceResult
- assemble_layer_toolpath now uses optimize_tour to reorder perimeters (contour-level), gap fills (reversible), and infill lines (replacing nearest_neighbor_order)
- Per-layer travel stats returned from assemble_layer_toolpath and accumulated in engine.rs using parallel-safe pattern (summed after maybe_par_iter collect)
- Feature group ordering preserved (perimeters -> gap fill -> infill); wall order within contours preserved
- Backward compatibility maintained: nearest_neighbor_order used as fallback when travel_opt.enabled=false

## Task Commits

Each task was committed atomically:

1. **Task 1: Add TravelOptStats and wire into SliceResult** - `62600e3` (feat)
2. **Task 2: Integrate optimizer into assemble_layer_toolpath with rayon-compatible stats** - `3a41f9e` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/statistics.rs` - Added TravelOptStats struct
- `crates/slicecore-engine/src/engine.rs` - travel_opt_stats field in SliceResult, LayerResult type alias, parallel-safe travel stat accumulation
- `crates/slicecore-engine/src/toolpath.rs` - TSP optimizer integration for perimeters/gap fills/infill, expanded return type
- `crates/slicecore-engine/src/lib.rs` - Re-export TravelOptStats
- `crates/slicecore-engine/src/output.rs` - Added travel_opt_stats: None in test helper

## Decisions Made
- Contour-level TSP reordering for perimeters: the optimizer decides which contour to visit next, but wall order (inner/outer shells) within each contour is preserved unchanged
- Introduced LayerResult type alias to address clippy type_complexity warnings from the expanded 4-tuple return type
- Travel stat accumulation uses sequential summation after parallel collect -- each layer independently computes (baseline, optimized) inside the map closure, no shared mutable state during parallel execution

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added travel_opt_stats field to output.rs test helper**
- **Found during:** Task 2 (cargo test)
- **Issue:** output.rs test helper constructs SliceResult without the new travel_opt_stats field
- **Fix:** Added travel_opt_stats: None to the test helper in output.rs
- **Files modified:** crates/slicecore-engine/src/output.rs
- **Verification:** cargo test passes
- **Committed in:** 3a41f9e (Task 2 commit)

**2. [Rule 1 - Bug] Fixed clippy needless_range_loop in gap fill iteration**
- **Found during:** Task 2 (cargo clippy)
- **Issue:** `for i in 1..points.len()` when only indexing points -- clippy prefers iterator
- **Fix:** Changed to `for point in &points[1..]`
- **Files modified:** crates/slicecore-engine/src/toolpath.rs
- **Verification:** cargo clippy -p slicecore-engine -- -D warnings passes clean
- **Committed in:** 3a41f9e (Task 2 commit)

**3. [Rule 1 - Bug] Fixed clippy type_complexity for parallel result types**
- **Found during:** Task 2 (cargo clippy)
- **Issue:** `Result<Vec<(LayerToolpath, Option<IPoint2>, f64, f64)>, EngineError>` too complex
- **Fix:** Added `type LayerResult` alias, used throughout engine.rs
- **Files modified:** crates/slicecore-engine/src/engine.rs
- **Verification:** cargo clippy passes clean
- **Committed in:** 3a41f9e (Task 2 commit)

---

**Total deviations:** 3 auto-fixed (2 bugs, 1 blocking)
**Impact on plan:** All fixes necessary for compilation and lint compliance. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- TSP optimizer integrated into pipeline, ready for benchmark validation (plan 03)
- TravelOptStats available in SliceResult for benchmark comparison
- Per-layer parallelism works with parallel feature flag via existing maybe_par_iter! infrastructure

---
*Phase: 41-travel-move-optimization*
*Completed: 2026-03-20*
