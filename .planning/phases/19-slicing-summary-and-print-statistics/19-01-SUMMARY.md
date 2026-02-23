---
phase: 19-slicing-summary-and-print-statistics
plan: 01
subsystem: engine
tags: [statistics, toolpath, gcode-metrics, per-feature, serde]

# Dependency graph
requires:
  - phase: 06-advanced-gcode-generation
    provides: "estimation.rs trapezoid model, filament.rs usage computation"
  - phase: 03-vertical-slice-stl-to-gcode
    provides: "toolpath.rs FeatureType/LayerToolpath, engine.rs SliceResult"
provides:
  - "PrintStatistics type with per-feature breakdown"
  - "GcodeMetrics extraction from command stream"
  - "compute_statistics() function integrated into Engine pipeline"
  - "Time scaling to match trapezoid motion model total"
  - "Support subtotals (model vs support time/filament)"
affects: [19-02, output-formats, cli-summary]

# Tech tracking
tech-stack:
  added: []
  patterns: ["per-feature accumulation with HashMap keying", "time scaling via naive-to-trapezoid ratio", "virtual features from G-code metrics"]

key-files:
  created:
    - crates/slicecore-engine/src/statistics.rs
  modified:
    - crates/slicecore-engine/src/engine.rs
    - crates/slicecore-engine/src/lib.rs
    - crates/slicecore-engine/src/toolpath.rs
    - crates/slicecore-engine/src/output.rs

key-decisions:
  - "Hash derive added to FeatureType in Task 1 (needed for HashMap keying in compute_toolpath_statistics)"
  - "Statistics field is Option<PrintStatistics> on SliceResult for backward compatibility"
  - "Virtual features (Retract, Unretract, Wipe) added from G-code metrics with time = retraction_count * 0.5s"
  - "StageChanged event emitted before statistics computation in event-based pipeline"

patterns-established:
  - "Per-feature accumulation: HashMap<FeatureType, Accumulator> pattern for toolpath analysis"
  - "Time scaling: naive per-segment time scaled by trapezoid_total/naive_total ratio"
  - "Dual percentage: both pct_total (all time) and pct_print (extrusion-only time)"

requirements-completed: []

# Metrics
duration: 7min
completed: 2026-02-23
---

# Phase 19 Plan 01: Print Statistics Summary

**Per-feature print statistics with time scaling to trapezoid model, G-code metrics extraction, and dual percentage breakdowns integrated into Engine pipeline**

## Performance

- **Duration:** 7 min
- **Started:** 2026-02-23T19:01:44Z
- **Completed:** 2026-02-23T19:08:35Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Created comprehensive PrintStatistics type with StatisticsSummary, FeatureStatistics, GcodeMetrics
- Implemented compute_statistics() with per-feature time scaling to match trapezoid motion model total
- Extracted G-code metrics (retraction/unretraction/z-hop counts and distances) from command stream
- Integrated statistics computation into both Engine slice pipeline paths (writer and modifier)
- All 14 FeatureType variants appear in output even when unused (zero values)
- Model/support subtotals computed for time and filament

## Task Commits

Each task was committed atomically:

1. **Task 1: PrintStatistics types and per-feature computation from toolpaths** - `cd2211f` (feat)
2. **Task 2: Integrate statistics computation into Engine slice pipeline** - `9c51a5c` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/statistics.rs` - PrintStatistics types, compute_statistics(), extract_gcode_metrics(), 11 unit tests
- `crates/slicecore-engine/src/engine.rs` - SliceResult.statistics field, pipeline integration, integration test
- `crates/slicecore-engine/src/lib.rs` - pub mod statistics and re-exports
- `crates/slicecore-engine/src/toolpath.rs` - Hash derive on FeatureType
- `crates/slicecore-engine/src/output.rs` - Updated test fixture with statistics: None

## Decisions Made
- Added Hash derive to FeatureType in Task 1 rather than Task 2 because compute_toolpath_statistics() needs HashMap keying immediately
- Statistics field is Option<PrintStatistics> so existing test code can use statistics: None
- Virtual features (Retract, Unretract, Wipe) use time from estimation.rs overhead constants (0.5s per retraction)
- Wipe metrics infrastructure ready but always 0 until wipe-on-retraction is implemented

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Moved Hash derive to Task 1**
- **Found during:** Task 1 (statistics.rs creation)
- **Issue:** compute_toolpath_statistics() uses HashMap<FeatureType, ...> which requires Hash on FeatureType, but plan assigned Hash derive to Task 2
- **Fix:** Added Hash derive in Task 1 since the code would not compile without it
- **Files modified:** crates/slicecore-engine/src/toolpath.rs
- **Verification:** cargo test -p slicecore-engine passes
- **Committed in:** cd2211f (Task 1 commit)

**2. [Rule 3 - Blocking] Updated output.rs test fixture**
- **Found during:** Task 2 (SliceResult field addition)
- **Issue:** output.rs test helper constructs SliceResult without the new statistics field
- **Fix:** Added statistics: None to the sample_result() function
- **Files modified:** crates/slicecore-engine/src/output.rs
- **Verification:** cargo test -p slicecore-engine passes
- **Committed in:** 9c51a5c (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both auto-fixes necessary for compilation. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- PrintStatistics type ready for Plan 02 (ASCII table, CSV, JSON output formats)
- compute_statistics() available at crate root for CLI integration
- GcodeMetrics provides retraction/z-hop data for summary display

## Self-Check: PASSED

- statistics.rs: FOUND
- 19-01-SUMMARY.md: FOUND
- Commit cd2211f: FOUND
- Commit 9c51a5c: FOUND

---
*Phase: 19-slicing-summary-and-print-statistics*
*Completed: 2026-02-23*
