---
phase: 23-progress-cancellation-api
plan: 02
subsystem: api
tags: [cancellation, progress, rolling-eta, wasm-safe-timing, cooperative-cancellation, event-system]

# Dependency graph
requires:
  - phase: 23-progress-cancellation-api
    provides: CancellationToken type, EngineError::Cancelled, SliceEvent::Progress, updated Engine method signatures
provides:
  - Active cancellation checking in per-layer loop (returns Err(EngineError::Cancelled))
  - SliceEvent::Progress emission with rolling-average ETA after each layer
  - WASM-safe timing via start_timer() helper (returns None on wasm32)
  - Cancellation in slice_with_preview (both slice call and preview loop)
  - Cancellation in slice_with_modifiers per-layer loop
  - 8 integration tests for progress and cancellation
affects: [25-parallel-slicing-pipeline-rayon]

# Tech tracking
tech-stack:
  added: []
  patterns: [wasm-safe-timing-via-optional-instant, rolling-average-eta-window-20-layers, per-layer-cooperative-cancellation]

key-files:
  created:
    - crates/slicecore-engine/tests/progress_cancellation.rs
  modified:
    - crates/slicecore-engine/src/engine.rs

key-decisions:
  - "WASM-safe timing uses cfg-gated start_timer() returning Option<Instant> -- None on wasm32 yields 0.0 elapsed and None ETA"
  - "Rolling average ETA uses last 20 layer durations for stability, returns None until 3 layers processed"
  - "Overall percent maps 10-90% for layer processing stage (0-10% mesh slicing, 90-100% gcode generation)"
  - "slice_with_preview clones cancel token for slice call, retains original for preview loop"
  - "Cancellation checked at very start of each layer iteration (before empty-contours check)"

patterns-established:
  - "WASM-safe timing pattern: start_timer() -> Option<Instant>, .map_or(0.0, |s| s.elapsed()) for elapsed"
  - "Rolling average ETA: Vec<f64> layer durations, sliding window of last N layers for average"

requirements-completed: [API-05]

# Metrics
duration: 4min
completed: 2026-02-27
---

# Phase 23 Plan 02: Progress Emission and Cancellation Checking Summary

**Active cancellation checking per-layer with rolling-average ETA progress events and WASM-safe timing, verified by 8 integration tests**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-27T19:37:56Z
- **Completed:** 2026-02-27T19:42:05Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Implemented active cancellation token checking at the start of every layer in slice_to_writer_with_events, slice_with_preview, and slice_with_modifiers
- Added SliceEvent::Progress emission after each layer with accurate overall_percent (10-90%), stage_percent (0-100%), rolling ETA over last 20 layers, elapsed_seconds, and layers_per_second
- Made timing WASM-safe via cfg-gated start_timer() helper that returns None on wasm32 (elapsed=0.0, eta=None, lps=0.0)
- Created 8 integration tests covering pre-flight cancellation, mid-flight cancellation, normal operation, progress field correctness, ETA phasing, token clone sharing, preview cancellation, and error Display

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement progress emission and cancellation checking in engine pipeline** - `2d9f1cf` (feat)
2. **Task 2: Integration tests for progress events and cancellation** - `8b71e0f` (test)

## Files Created/Modified
- `crates/slicecore-engine/src/engine.rs` - WASM-safe start_timer(), cancellation checks in 3 methods, Progress event emission with rolling ETA
- `crates/slicecore-engine/tests/progress_cancellation.rs` - 8 integration tests for progress and cancellation

## Decisions Made
- WASM-safe timing uses cfg-gated start_timer() returning Option<Instant> -- keeps WASM compilation clean without conditional logic at every timing call site
- Rolling average ETA uses last 20 layer durations (const ETA_WINDOW: usize = 20) for stable estimates
- ETA returns None for first 3 layers (layers_done < 3) to avoid noisy estimates from startup
- Overall percent maps 10-90% for layer processing (0-10% mesh slicing, 90-100% gcode gen), matching the StageChanged event boundaries
- slice_with_preview uses cancel.clone() for the slice call so the original cancel token remains available for the preview layer loop
- Cancellation checked at the very start of each layer iteration, before the empty-contours check, ensuring immediate response

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed cancel token consumption in slice_with_preview**
- **Found during:** Task 1 (implementing cancellation in slice_with_preview)
- **Issue:** Plan 01 passed `cancel` directly to `self.slice(mesh, cancel)` which would move the Option, making it unavailable for the preview loop. The plan specified using `cancel.clone()` but the existing code did not.
- **Fix:** Changed to `self.slice(mesh, cancel.clone())` so the original cancel token remains for the preview loop cancellation check
- **Files modified:** crates/slicecore-engine/src/engine.rs
- **Verification:** Both pre-flight and mid-flight cancellation tests pass for slice_with_preview
- **Committed in:** 2d9f1cf (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Essential fix -- without clone, the cancel token would be consumed by the first slice call and unavailable for the preview loop check.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Progress/Cancellation API is now fully functional: types defined (Plan 01) and logic implemented (Plan 02)
- Phase 25 (Parallel Slicing Pipeline with rayon) can use CancellationToken for cross-thread cancellation
- All existing behavior preserved when passing None for cancel parameter

## Self-Check: PASSED

---
*Phase: 23-progress-cancellation-api*
*Completed: 2026-02-27*
