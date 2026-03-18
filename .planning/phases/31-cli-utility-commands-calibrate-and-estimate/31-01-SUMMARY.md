---
phase: 31-cli-utility-commands-calibrate-and-estimate
plan: 01
subsystem: cli
tags: [calibration, cost-estimation, gcode, cli]

# Dependency graph
requires: []
provides:
  - "CostInputs/CostEstimate types with progressive disclosure cost model"
  - "volume_estimate() for rough filament/time from mesh volume"
  - "MachineConfig.watts field for electricity cost"
  - "CalibrateCommand enum with 5 subcommands (temp-tower, retraction, flow, first-layer, list)"
  - "Core calibration types: TempTowerParams, RetractionParams, FlowParams, FirstLayerParams"
  - "validate_bed_fit() and inject_temp_changes() utilities"
  - "Calibration CLI common utilities (profile resolution, instructions, header formatting)"
affects: [31-02, 31-03, 31-04]

# Tech tracking
tech-stack:
  added: [comfy-table (existing)]
  patterns: [progressive-disclosure cost model, calibration parameter derivation from config]

key-files:
  created:
    - crates/slicecore-engine/src/cost_model.rs
    - crates/slicecore-engine/src/calibrate.rs
    - crates/slicecore-cli/src/calibrate/mod.rs
    - crates/slicecore-cli/src/calibrate/common.rs
  modified:
    - crates/slicecore-engine/src/config.rs
    - crates/slicecore-engine/src/lib.rs
    - crates/slicecore-cli/src/main.rs

key-decisions:
  - "Progressive disclosure: each cost component computed independently; None + hints for missing inputs"
  - "Volume estimate uses 0.50 combined infill+shell factor and 40mm/s avg extrusion rate"
  - "Calibration params derive from filament/config with sensible defaults; all overridable via CLI flags"

patterns-established:
  - "Progressive disclosure pattern: optional cost components with missing_hints guidance"
  - "Calibration subcommand pattern: profile flags + test-specific params + common dispatch"

requirements-completed: []

# Metrics
duration: 6min
completed: 2026-03-16
---

# Phase 31 Plan 01: Calibrate/Estimate Foundation Summary

**Cost estimation with 4-component progressive disclosure and calibrate CLI skeleton with temp-tower/retraction/flow/first-layer subcommands**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-16T16:49:52Z
- **Completed:** 2026-03-16T16:56:17Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- CostEstimate computes filament, electricity, depreciation, and labor costs independently with progressive disclosure
- MachineConfig.watts field (default 0.0) enables electricity cost estimation
- volume_estimate() produces rough filament length/weight/time from mesh volume
- Calibrate CLI group with 5 subcommands parses all arguments; list prints formatted table
- Core calibration types with bed validation and temperature injection work correctly
- 15 unit tests covering all cost model and calibration behaviors

## Task Commits

Each task was committed atomically:

1. **Task 1: Cost model module and MachineConfig watts field** - `353662c` (feat)
2. **Task 2: Calibrate CLI skeleton and core calibration types** - `e69130c` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/cost_model.rs` - CostInputs, CostEstimate, compute_cost(), volume_estimate()
- `crates/slicecore-engine/src/calibrate.rs` - TempTowerParams, RetractionParams, FlowParams, validate_bed_fit, inject_temp_changes, temp_schedule
- `crates/slicecore-engine/src/config.rs` - Added watts field to MachineConfig
- `crates/slicecore-engine/src/lib.rs` - Added cost_model and calibrate modules
- `crates/slicecore-cli/src/calibrate/mod.rs` - CalibrateCommand enum with subcommand dispatch
- `crates/slicecore-cli/src/calibrate/common.rs` - Shared calibration utilities
- `crates/slicecore-cli/src/main.rs` - Wired Calibrate variant into Commands enum

## Decisions Made
- Progressive disclosure: each cost component computed independently; None + hints for missing inputs rather than requiring all data upfront
- Volume estimate uses 0.50 combined infill+shell factor (rough but useful for quick estimates)
- Calibrate subcommands follow exact same pattern as existing Csg subcommand group

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Cost model ready for CLI integration (estimate subcommand in plan 31-02+)
- Calibrate skeleton ready for G-code generation implementations
- All core types and utilities in place for subsequent plans

---
*Phase: 31-cli-utility-commands-calibrate-and-estimate*
*Completed: 2026-03-16*
