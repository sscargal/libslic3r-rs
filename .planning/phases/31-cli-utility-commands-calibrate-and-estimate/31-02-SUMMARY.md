---
phase: 31-cli-utility-commands-calibrate-and-estimate
plan: 02
subsystem: cli
tags: [cost-estimation, gcode-analysis, cli, volume-estimate]

# Dependency graph
requires:
  - phase: 31-01
    provides: "CostInputs/CostEstimate/VolumeEstimate types and compute_cost/volume_estimate functions"
provides:
  - "analyze-gcode --filament-price/--printer-watts/--electricity-rate cost flags"
  - "analyze-gcode --model flag for rough volume-based estimation from STL/3MF"
  - "Cost display in table/JSON/CSV/markdown formats"
  - "7 integration tests covering cost estimation and calibrate CLI"
affects: [31-03, 31-04]

# Tech tracking
tech-stack:
  added: []
  patterns: [progressive-disclosure cost display in CLI, volume-based mesh estimation]

key-files:
  created:
    - crates/slicecore-cli/tests/cli_calibrate.rs
  modified:
    - crates/slicecore-cli/src/main.rs
    - crates/slicecore-cli/src/analysis_display.rs

key-decisions:
  - "Cost table only shown when at least one cost flag is provided (no noise for basic analysis)"
  - "Model estimation path short-circuits before G-code parsing, loads mesh directly via slicecore_fileio"
  - "JSON output combines analysis + cost_estimate in single object for easy downstream consumption"

patterns-established:
  - "CLI cost flag pattern: optional cost flags on analyze-gcode, piped to CostInputs"
  - "Multi-format cost display: table/json/csv/markdown all available via display functions"

requirements-completed: []

# Metrics
duration: 3min
completed: 2026-03-16
---

# Phase 31 Plan 02: CLI Cost Estimation Integration Summary

**analyze-gcode extended with cost estimation flags (filament/electricity/depreciation/labor) and --model for volume-based rough estimation from mesh files**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-16T16:59:17Z
- **Completed:** 2026-03-16T17:02:29Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- analyze-gcode accepts 7 new cost flags and displays 4-component cost breakdown table
- --model flag loads STL/3MF mesh and computes rough volume-based estimates with disclaimer
- Cost output available in table, JSON, CSV, and markdown formats
- 7 integration tests all passing covering cost flags, JSON, CSV, markdown, model estimation, and calibrate list/help

## Task Commits

Each task was committed atomically:

1. **Task 1: Extend analyze-gcode with cost flags and model estimation** - `f40edd0` (feat)
2. **Task 2: Integration tests for cost estimation in analyze-gcode** - `10b603c` (test)

## Files Created/Modified
- `crates/slicecore-cli/src/main.rs` - Added cost flags to AnalyzeGcode, model path, cost computation and display wiring
- `crates/slicecore-cli/src/analysis_display.rs` - Added display_cost_table, display_cost_csv, display_cost_markdown, display_volume_estimate and variants
- `crates/slicecore-cli/tests/cli_calibrate.rs` - 7 integration tests for cost estimation and calibrate commands

## Decisions Made
- Cost table only displayed when at least one cost flag is provided, keeping default output clean
- Model estimation path loads mesh directly and short-circuits before any G-code parsing
- JSON combines analysis and cost_estimate into single object for downstream tooling

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Cost estimation fully wired into CLI, ready for further enhancements
- Calibrate subcommands still skeleton (not-yet-implemented), ready for Plan 03/04 implementation
- All 7 integration tests provide regression safety net

---
*Phase: 31-cli-utility-commands-calibrate-and-estimate*
*Completed: 2026-03-16*
