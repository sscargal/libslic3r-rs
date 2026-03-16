---
phase: 31-cli-utility-commands-calibrate-and-estimate
plan: 05
subsystem: cli
tags: [calibration, gcode-analysis, multi-config, dry-run, save-model, output-format]

requires:
  - phase: 31-02
    provides: analyze-gcode command with cost estimation
  - phase: 31-03
    provides: Temperature tower and retraction calibration commands
  - phase: 31-04
    provides: Flow rate and first layer calibration commands
provides:
  - Multi-config filament comparison in analyze-gcode (--compare-filament)
  - Common dry-run display with model dimensions and bed fit status
  - Common save_calibration_model function for all calibrate commands
  - OutputFormat enum and determine_output_format helper
  - ComparisonRow struct for side-by-side filament cost/time display
affects: [cli-docs, calibration-guide, user-facing-commands]

tech-stack:
  added: []
  patterns: [multi-config comparison with delta display, common dry-run display with bed fit checking]

key-files:
  created: []
  modified:
    - crates/slicecore-cli/src/analysis_display.rs
    - crates/slicecore-cli/src/main.rs
    - crates/slicecore-cli/src/calibrate/common.rs
    - crates/slicecore-cli/src/calibrate/mod.rs
    - crates/slicecore-cli/src/calibrate/temp_tower.rs
    - crates/slicecore-cli/src/calibrate/retraction.rs
    - crates/slicecore-cli/src/calibrate/flow.rs
    - crates/slicecore-cli/src/calibrate/first_layer.rs

key-decisions:
  - "Multi-config comparison reuses same GcodeAnalysis (parse once), re-estimates weight/cost per filament profile density and cost_per_kg"
  - "Common display_dry_run shows dimensions, bed fit status (OK/WARNING/ERROR), and parameter table via comfy-table"
  - "OutputFormat enum with JSON > CSV > Markdown > Table priority for flag disambiguation"

patterns-established:
  - "Common calibration utilities: display_dry_run and save_calibration_model reduce duplication across all 4 calibrate commands"
  - "Multi-config comparison pattern: parse once, re-estimate per profile variant"

requirements-completed: []

duration: 5min
completed: 2026-03-16
---

# Phase 31 Plan 05: Multi-Config Comparison, Dry-Run, and Save-Model Summary

**Multi-config filament comparison in analyze-gcode with side-by-side cost/time deltas, and common dry-run/save-model utilities across all calibrate commands**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-16T17:22:09Z
- **Completed:** 2026-03-16T17:27:28Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- analyze-gcode --compare-filament shows side-by-side comparison table with time, weight, filament cost, and total cost deltas from baseline
- All 4 calibrate commands use common display_dry_run showing model dimensions, bed fit status, and parameter table
- All 4 calibrate commands use common save_calibration_model for --save-model flag
- OutputFormat enum centralizes format selection logic (table/json/csv/markdown)
- DryRunInfo struct enables JSON dry-run output with structured data

## Task Commits

Each task was committed atomically:

1. **Task 1: Multi-config comparison for analyze-gcode** - `30d077e` (feat)
2. **Task 2: Calibrate dry-run, save-model, and output format consistency** - `0249044` (feat)

## Files Created/Modified
- `crates/slicecore-cli/src/analysis_display.rs` - Added OutputFormat enum, ComparisonRow struct, display_config_comparison with table/json/csv/markdown
- `crates/slicecore-cli/src/main.rs` - Added --compare-filament and --profiles-dir to AnalyzeGcode, multi-config comparison logic
- `crates/slicecore-cli/src/calibrate/common.rs` - Added display_dry_run, save_calibration_model, DryRunInfo
- `crates/slicecore-cli/src/calibrate/mod.rs` - Removed dead_code allow on common module
- `crates/slicecore-cli/src/calibrate/temp_tower.rs` - Refactored to use common dry-run and save-model
- `crates/slicecore-cli/src/calibrate/retraction.rs` - Refactored to use common dry-run and save-model
- `crates/slicecore-cli/src/calibrate/flow.rs` - Refactored to use common dry-run and save-model
- `crates/slicecore-cli/src/calibrate/first_layer.rs` - Refactored to use common dry-run and save-model

## Decisions Made
- Multi-config comparison reuses the same parsed GcodeAnalysis (G-code parsed once), then re-estimates filament weight and cost for each comparison filament profile based on its density and cost_per_kg
- Dry-run display shows bed fit status with three levels: OK, WARNING (>90% of bed), and ERROR (exceeds bed)
- OutputFormat enum resolves flag conflicts with JSON > CSV > Markdown > Table priority

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All analyze-gcode and calibrate cross-cutting features are complete
- Ready for Plan 06 (integration tests and final validation)

---
*Phase: 31-cli-utility-commands-calibrate-and-estimate*
*Completed: 2026-03-16*
