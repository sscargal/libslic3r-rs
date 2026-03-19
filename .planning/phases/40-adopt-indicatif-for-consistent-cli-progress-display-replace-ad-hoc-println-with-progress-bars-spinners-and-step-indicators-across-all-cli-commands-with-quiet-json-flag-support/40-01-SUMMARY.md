---
phase: 40-adopt-indicatif-for-consistent-cli-progress-display
plan: 01
subsystem: cli
tags: [indicatif, console, progress-bar, spinner, cli-output, tty-detection]

requires:
  - phase: none
    provides: N/A
provides:
  - CliOutput struct with spinner, progress bar, step indicator, warn, error_msg, info methods
  - ColorMode enum for always/never/auto color handling
  - Global --quiet and --color flags on Cli struct
  - Backwards-compatible SliceProgress shim in cli_output.rs
affects: [40-02, 40-03, all-cli-commands]

tech-stack:
  added: [console 0.15]
  patterns: [CliOutput abstraction, global CLI flags via clap global=true, NO_COLOR env var support]

key-files:
  created:
    - crates/slicecore-cli/src/cli_output.rs
  modified:
    - crates/slicecore-cli/src/main.rs
    - crates/slicecore-cli/src/diff_profiles_command.rs
    - crates/slicecore-cli/Cargo.toml

key-decisions:
  - "Kept SliceProgress as temporary backwards compat shim in cli_output.rs until Plan 02 migrates cmd_slice"
  - "Used console crate for color control alongside indicatif (explicit dependency, already transitive)"
  - "Global --quiet uses -q short flag; global --color defaults to auto"

patterns-established:
  - "CliOutput::new(quiet, json, color) as single entry point for all CLI output"
  - "effective_quiet = quiet || json to suppress progress in JSON mode"
  - "Non-TTY fallback: plain eprintln instead of spinners, hidden ProgressBar return"

requirements-completed: [CLI-PROGRESS-01]

duration: 5min
completed: 2026-03-19
---

# Phase 40 Plan 01: CliOutput Abstraction and Global CLI Flags Summary

**CliOutput struct with spinner/progress-bar/step-indicator API, global --quiet/-q and --color flags, console crate integration, and diff-profiles migrated to global flags**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-19T21:58:02Z
- **Completed:** 2026-03-19T22:03:28Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Created CliOutput abstraction with 10 public methods covering spinners, progress bars, step indicators, warnings, errors, and info messages
- Added global --quiet/-q and --color (always/never/auto) flags to the Cli struct with clap global=true propagation
- Migrated diff-profiles per-command --color and --quiet to use global flags
- Removed per-command --quiet from slice subcommand (backwards compatible via global flag)
- Deleted progress.rs and replaced with backwards-compatible shim in cli_output.rs

## Task Commits

Each task was committed atomically:

1. **Task 1: Create cli_output.rs with CliOutput abstraction** - `85af4b8` (feat)
2. **Task 2: Add global --quiet/--color flags and wire into main()** - `3b5175c` (feat)

## Files Created/Modified
- `crates/slicecore-cli/src/cli_output.rs` - New unified CLI output handler with CliOutput struct, ColorMode enum, spinner/progress/message API, SliceProgress compat shim, and unit tests
- `crates/slicecore-cli/src/main.rs` - Global --quiet/--color flags on Cli struct, module swap from progress to cli_output, color_mode parsing in main()
- `crates/slicecore-cli/src/diff_profiles_command.rs` - Removed per-command --color and --quiet fields, updated function signature to accept global values
- `crates/slicecore-cli/Cargo.toml` - Added console = "0.15" dependency

## Decisions Made
- Kept SliceProgress and create_progress as temporary backwards compatibility in cli_output.rs rather than updating all cmd_slice references (Plan 02 will handle migration)
- Used console crate for set_colors_enabled_stderr rather than manual ANSI handling, since indicatif already depends on it
- Added TODO comments for AnalyzeGcode and CompareGcode --no-color flags rather than migrating them (out of scope for this plan)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- CliOutput abstraction ready for Plan 02 to migrate cmd_slice and Plan 03 to migrate remaining commands
- Global --quiet and --color flags propagate to all subcommands automatically
- SliceProgress backwards compatibility shim ensures no breakage during incremental migration

## Self-Check: PASSED

- cli_output.rs: FOUND
- progress.rs: CONFIRMED DELETED
- Commit 85af4b8: FOUND
- Commit 3b5175c: FOUND

---
*Phase: 40-adopt-indicatif-for-consistent-cli-progress-display*
*Completed: 2026-03-19*
