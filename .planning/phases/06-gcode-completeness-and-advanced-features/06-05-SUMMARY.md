---
phase: 06-gcode-completeness-and-advanced-features
plan: 05
subsystem: engine
tags: [estimation, trapezoid-motion, filament-usage, print-time, cost-tracking]

# Dependency graph
requires:
  - phase: 06-01
    provides: "Acceleration/jerk/pressure advance config fields (print_acceleration, travel_acceleration)"
provides:
  - "Trapezoid motion model for print time estimation (estimation.rs)"
  - "Filament usage estimation: length, weight, cost (filament.rs)"
  - "PrintTimeEstimate and FilamentUsage structs in SliceResult"
  - "filament_density and filament_cost_per_kg in PrintConfig"
affects: [06-09, visualization, cli-output]

# Tech tracking
tech-stack:
  added: []
  patterns: ["trapezoid velocity profile for motion estimation", "E-value summation for filament tracking"]

key-files:
  created:
    - "crates/slicecore-engine/src/estimation.rs"
    - "crates/slicecore-engine/src/filament.rs"
  modified:
    - "crates/slicecore-engine/src/config.rs"
    - "crates/slicecore-engine/src/engine.rs"
    - "crates/slicecore-engine/src/lib.rs"

key-decisions:
  - "Entry speed uses min(current_feedrate, previous_feedrate) as simple lookahead approximation (no full junction speed computation)"
  - "Fixed overhead per retraction (0.5s) and per layer change (0.2s) for non-move time accounting"
  - "Filament density defaults to 1.24 g/cm3 (PLA), cost defaults to 25.0 USD/kg"
  - "estimated_time_seconds kept for backward compatibility, populated from time_estimate.total_seconds"

patterns-established:
  - "GcodeCommand stream analysis: iterate commands post-generation for estimation (same stream used for both time and filament)"
  - "Trapezoid model with triangular fallback for short segments"

# Metrics
duration: 7min
completed: 2026-02-17
---

# Phase 6 Plan 5: Print Time and Filament Estimation Summary

**Trapezoid motion model for print time estimation with filament length/weight/cost tracking via GcodeCommand stream analysis**

## Performance

- **Duration:** 7 min
- **Started:** 2026-02-17T18:22:41Z
- **Completed:** 2026-02-17T18:30:09Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Trapezoid velocity profile model accounting for acceleration/deceleration ramps (produces higher, more realistic estimates than naive distance/feedrate)
- Filament usage estimation from E-value summation with cross-section geometry, density, and cost computation
- Both estimates integrated into SliceResult and computed from the final GcodeCommand stream
- 17 new tests covering edge cases, cross-section math, TOML parsing, and end-to-end SliceResult integration

## Task Commits

Each task was committed atomically:

1. **Task 1: Trapezoid motion model for print time estimation** - `cf59aef` (feat)
2. **Task 2: Filament usage estimation and SliceResult integration** - `4309d26` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/estimation.rs` - Trapezoid motion model: trapezoid_time(), estimate_print_time(), PrintTimeEstimate struct
- `crates/slicecore-engine/src/filament.rs` - Filament usage: estimate_filament_usage(), FilamentUsage struct
- `crates/slicecore-engine/src/config.rs` - Added filament_density (1.24) and filament_cost_per_kg (25.0) to PrintConfig
- `crates/slicecore-engine/src/engine.rs` - SliceResult gains time_estimate and filament_usage fields; engine pipeline computes both from gcode_commands
- `crates/slicecore-engine/src/lib.rs` - Module declarations and re-exports for estimation and filament

## Decisions Made
- Simple lookahead for entry speed: min(current, previous) feedrate -- avoids full junction speed computation complexity while providing reasonable accuracy
- Fixed overhead constants for non-move time: 0.5s per retraction, 0.2s per layer change
- Filament density defaults to PLA (1.24 g/cm3); cost defaults to 25.0 USD/kg -- common reasonable values
- E-value summation excludes negative values (retractions) for filament length computation
- Arc length computed from I/J center offset and endpoint angles for G2/G3 commands
- Backward compatibility maintained: estimated_time_seconds populated from time_estimate.total_seconds

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Print time and filament estimates available in SliceResult for CLI output, visualization, and API consumers
- Trapezoid model parameters (print_acceleration, travel_acceleration) configurable via TOML
- Filament material properties (density, cost_per_kg) configurable via TOML for different materials

---
*Phase: 06-gcode-completeness-and-advanced-features*
*Completed: 2026-02-17*
