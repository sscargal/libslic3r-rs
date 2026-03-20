---
phase: 41-travel-move-optimization
plan: 01
subsystem: engine
tags: [tsp, nearest-neighbor, greedy-edge-insertion, 2-opt, travel-optimization, toolpath-ordering]

# Dependency graph
requires: []
provides:
  - "TspNode, Tour, DistanceMatrix types for TSP representation"
  - "Nearest-neighbor, greedy edge insertion, 2-opt algorithms"
  - "optimize_tour dispatcher with Auto/NN/Greedy/NN-only/Greedy-only modes"
  - "TravelOptConfig, TravelOptAlgorithm, PrintOrder config types"
affects: [41-02, 41-03, 41-04]

# Tech tracking
tech-stack:
  added: []
  patterns: [union-find-cycle-detection, asymmetric-distance-matrix, reversible-node-tsp]

key-files:
  created:
    - crates/slicecore-engine/src/travel_optimizer.rs
  modified:
    - crates/slicecore-engine/src/config.rs
    - crates/slicecore-engine/src/lib.rs

key-decisions:
  - "Stored Euclidean distances (not squared) in matrix for correct 2-opt delta computation"
  - "DistanceMatrix not needed by NN or 2-opt (they compute distances directly from node coordinates); only greedy uses it"
  - "Non-exhaustive TravelOptAlgorithm with fallback wildcard pattern for future extensibility"

patterns-established:
  - "TSP node model: entry/exit points with reversible flag for open vs closed paths"
  - "Tour struct encapsulating order + reversed flags, with to_permutation output"

requirements-completed: []

# Metrics
duration: 13min
completed: 2026-03-20
---

# Phase 41 Plan 01: Core TSP Algorithms Summary

**TSP-based travel optimizer with nearest-neighbor, greedy edge insertion, and 2-opt local search for toolpath ordering with asymmetric distance support**

## Performance

- **Duration:** 13 min
- **Started:** 2026-03-20T16:37:04Z
- **Completed:** 2026-03-20T16:50:06Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- TravelOptConfig, TravelOptAlgorithm (5 non-exhaustive variants), and PrintOrder enums added to config.rs with serde, SettingSchema derives
- travel_optimizer.rs with full TSP algorithm suite: NN construction, greedy edge insertion (union-find), 2-opt improvement
- Auto mode: tries both NN and greedy, picks shorter, applies 2-opt; small vs large problem thresholds at n=30
- Asymmetric distance matrix handling for open paths with distinct entry/exit points
- 10 unit tests covering edge cases (0/1/2 nodes), algorithm correctness, reversible nodes, asymmetric distances

## Task Commits

Each task was committed atomically:

1. **Task 1: TravelOptConfig, TravelOptAlgorithm, PrintOrder** - `1153ea8` (feat)
2. **Task 2: travel_optimizer.rs with TSP algorithms** - `ad06376` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/travel_optimizer.rs` - TSP algorithms: TspNode, Tour, DistanceMatrix, NN, greedy, 2-opt, optimize_tour (943 lines)
- `crates/slicecore-engine/src/config.rs` - TravelOptConfig, TravelOptAlgorithm, PrintOrder types + PrintConfig.travel_opt field
- `crates/slicecore-engine/src/lib.rs` - Module declaration and re-exports

## Decisions Made
- Stored actual Euclidean distances (not squared) in DistanceMatrix for correct 2-opt delta computation without needing sqrt per comparison
- DistanceMatrix only required by greedy_edge_insertion; NN and 2-opt compute distances directly from node coordinates, keeping the API simpler
- Used #[non_exhaustive] on TravelOptAlgorithm with #[allow(unreachable_patterns)] wildcard fallback for forward compatibility

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Removed unnecessary DistanceMatrix parameter from functions**
- **Found during:** Task 2 (clippy verification)
- **Issue:** nearest_neighbor, two_opt_improve, and Tour methods accepted DistanceMatrix but never used it (computed distances directly from Point2 coordinates)
- **Fix:** Removed unused matrix parameters; only greedy_edge_insertion retains it for O(1) lookups during edge generation
- **Files modified:** crates/slicecore-engine/src/travel_optimizer.rs
- **Verification:** cargo clippy -p slicecore-engine -- -D warnings passes clean
- **Committed in:** ad06376 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Clippy-driven cleanup of unused parameters. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Core TSP algorithms ready for integration with layer pipeline (plan 02)
- TravelOptConfig available in PrintConfig for user configuration
- optimize_tour API accepts TspNode slice and returns reordering permutation

---
*Phase: 41-travel-move-optimization*
*Completed: 2026-03-20*
