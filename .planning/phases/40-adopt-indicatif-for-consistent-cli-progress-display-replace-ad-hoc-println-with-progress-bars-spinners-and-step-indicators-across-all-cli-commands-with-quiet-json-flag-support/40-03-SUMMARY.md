---
phase: 40-adopt-indicatif-for-consistent-cli-progress-display
plan: 03
subsystem: cli
tags: [indicatif, spinner, progress-bar, cli-output, json-flag]

# Dependency graph
requires:
  - phase: 40-adopt-indicatif-for-consistent-cli-progress-display
    plan: 01
    provides: CliOutput abstraction with spinner/progress/message methods
provides:
  - Spinners on all medium-duration CLI commands (calibrate, csg, convert-profile, analyze-gcode, compare-gcode, ai-suggest, import-profiles)
  - --json flags on convert-profile and import-profiles commands
  - CliOutput routing for all calibrate and CSG subcommands
  - Unified error output through CliOutput in main dispatch
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Spinner wrapping pattern: construct CliOutput, start spinner, run command, finish spinner"
    - "CliOutput passthrough: medium-duration commands accept &CliOutput for internal progress routing"

key-files:
  created: []
  modified:
    - crates/slicecore-cli/src/main.rs
    - crates/slicecore-cli/src/calibrate/mod.rs
    - crates/slicecore-cli/src/calibrate/temp_tower.rs
    - crates/slicecore-cli/src/calibrate/retraction.rs
    - crates/slicecore-cli/src/calibrate/flow.rs
    - crates/slicecore-cli/src/calibrate/first_layer.rs
    - crates/slicecore-cli/src/csg_command.rs

key-decisions:
  - "Spinner wrapping done in main dispatch, CliOutput passed through to subcommands for internal info/warn routing"
  - "calibrate::common.rs dry-run output left as eprintln since it is structured display output, not progress/warning"

patterns-established:
  - "All medium-duration CLI commands wrapped with output_ctx.spinner() in main dispatch"
  - "Subcommand implementations accept &CliOutput for info/warn/error routing"

requirements-completed: [CLI-PROGRESS-03]

# Metrics
duration: 7min
completed: 2026-03-19
---

# Phase 40 Plan 03: Non-slice Command Progress Migration Summary

**Indicatif spinners on all medium-duration CLI commands with --json flags on convert-profile and import-profiles, unified CliOutput error routing in main dispatch**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-19T22:19:04Z
- **Completed:** 2026-03-19T22:26:28Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Added spinners to 7 command categories: convert-profile, import-profiles, analyze-gcode, compare-gcode, ai-suggest, calibrate, csg
- Added --json flag to ConvertProfile and ImportProfiles CLI commands
- Replaced all bare eprintln! error output in main() dispatch with CliOutput.error_msg()
- Routed all calibrate and CSG subcommand progress/info output through CliOutput

## Task Commits

Each task was committed atomically:

1. **Task 1: Add spinners to medium-duration commands in main.rs dispatch** - `8342bb6` (feat)
2. **Task 2: Add spinners to calibrate and CSG subcommand implementations** - `658cbc6` (feat)

## Files Created/Modified
- `crates/slicecore-cli/src/main.rs` - Added --json flags, spinner wrapping, CliOutput error routing in dispatch
- `crates/slicecore-cli/src/calibrate/mod.rs` - Updated run_calibrate to accept &CliOutput, pass through to subcommands
- `crates/slicecore-cli/src/calibrate/temp_tower.rs` - Replaced eprintln! summary with output.info()
- `crates/slicecore-cli/src/calibrate/retraction.rs` - Replaced eprintln! summary with output.info()
- `crates/slicecore-cli/src/calibrate/flow.rs` - Replaced eprintln! summary with output.info()
- `crates/slicecore-cli/src/calibrate/first_layer.rs` - Replaced eprintln! summary with output.info()
- `crates/slicecore-cli/src/csg_command.rs` - Replaced all verbose eprintln! with cli_out.info(), updated all function signatures

## Decisions Made
- Spinner wrapping in main dispatch, CliOutput passed through to subcommands for internal routing
- calibrate::common.rs dry-run output left as eprintln since it is structured display (not progress/warning)
- CSG Info subcommand not modified (already has its own output handling)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All CLI commands now use CliOutput for progress display
- Phase 40 migration complete: slice, calibrate, csg, and all utility commands use indicatif
- --quiet and --json flags suppress progress across all commands

## Self-Check: PASSED

All 7 modified files verified present. Both task commits (8342bb6, 658cbc6) verified in git log.

---
*Phase: 40-adopt-indicatif-for-consistent-cli-progress-display*
*Completed: 2026-03-19*
