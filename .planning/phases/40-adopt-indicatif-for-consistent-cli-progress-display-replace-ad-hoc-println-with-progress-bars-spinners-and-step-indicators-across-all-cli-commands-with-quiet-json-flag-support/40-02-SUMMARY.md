---
phase: 40-adopt-indicatif-for-consistent-cli-progress-display
plan: 02
subsystem: cli
tags: [indicatif, progress-bars, cli-output, step-indicators, slice-command]

requires:
  - phase: 40-adopt-indicatif-for-consistent-cli-progress-display
    plan: 01
    provides: CliOutput abstraction with start_step, finish_step, warn, info, error_msg, add_progress_bar

provides:
  - cmd_slice fully migrated to CliOutput with numbered step indicators
  - slice_workflow.rs warnings/errors routed through CliOutput
  - SliceProgress/create_progress backwards-compat removed from cli_output.rs
  - CliOutput non-TTY reliability fix (eprintln fallback for warn/info/error_msg)

affects: [40-03, cli-commands]

tech-stack:
  added: []
  patterns:
    - "CliOutput passed to workflow functions for unified output routing"
    - "Step indicators: output.start_step(N, total, msg) / output.finish_step(&pb, msg)"
    - "Non-TTY fallback: eprintln! for warn/info/error_msg when MultiProgress has no active bars"

key-files:
  created: []
  modified:
    - crates/slicecore-cli/src/main.rs
    - crates/slicecore-cli/src/cli_output.rs
    - crates/slicecore-cli/src/slice_workflow.rs

key-decisions:
  - "CliOutput constructed inside cmd_slice with color_mode passed from main"
  - "Dynamic step count: 5 steps for profile workflow, 4 for legacy --config"
  - "Non-TTY fix: warn/info/error_msg use eprintln! directly instead of multi.println to avoid silent drops when no bars are active"
  - "Summary line printed to both stdout (println) and stderr (output.info) to maintain backward compat for tests checking either stream"

patterns-established:
  - "Thread CliOutput through workflow functions as &CliOutput parameter"
  - "Step indicators bracket each phase: start_step -> work -> finish_step"

requirements-completed: [CLI-PROGRESS-02]

duration: 10min
completed: 2026-03-19
---

# Phase 40 Plan 02: Slice Command CliOutput Migration Summary

**cmd_slice migrated to CliOutput with 5-step workflow indicators, slice_workflow.rs warnings routed through CliOutput, SliceProgress removed**

## Performance

- **Duration:** 10 min
- **Started:** 2026-03-19T22:06:22Z
- **Completed:** 2026-03-19T22:16:14Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- cmd_slice uses numbered step indicators [1/5] through [5/5] for profile workflow (or [1/4] through [4/4] for legacy --config)
- All eprintln! in cmd_slice replaced with output.start_step, output.finish_step, output.warn, output.info, output.error_msg
- slice_workflow.rs run_slice_workflow and all helper functions accept &CliOutput and route all messages through it
- Deprecated SliceProgress struct and create_progress function removed from cli_output.rs
- Fixed CliOutput to use eprintln! directly in non-TTY mode for reliable output
- All 126 tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Migrate cmd_slice to CliOutput step-based workflow** - `5e1c259` (feat)
2. **Task 2: Route slice_workflow.rs warnings through CliOutput** - `9c30aa1` (feat)

## Files Created/Modified
- `crates/slicecore-cli/src/main.rs` - cmd_slice uses CliOutput for all progress/warning/info/error output with step indicators
- `crates/slicecore-cli/src/cli_output.rs` - Removed SliceProgress/create_progress; fixed warn/info/error_msg non-TTY behavior
- `crates/slicecore-cli/src/slice_workflow.rs` - All functions accept &CliOutput, zero bare eprintln! remaining

## Decisions Made
- CliOutput constructed inside cmd_slice rather than passed as parameter -- keeps the function signature changes minimal
- color_mode added as parameter to cmd_slice from main() where global --color flag is parsed
- Non-TTY output uses eprintln! directly instead of MultiProgress::println to avoid silently dropped messages when no progress bars are active
- Summary line printed to both stdout and stderr to satisfy tests checking either stream

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed MultiProgress::println silent drop in non-TTY**
- **Found during:** Task 1 (test failures)
- **Issue:** MultiProgress::println silently drops messages when no progress bars are active in non-TTY environments
- **Fix:** Changed warn(), info(), error_msg() to use eprintln! directly in non-TTY mode
- **Files modified:** crates/slicecore-cli/src/cli_output.rs
- **Verification:** All CLI integration tests pass
- **Committed in:** 5e1c259

**2. [Rule 1 - Bug] Fixed summary line output stream for backward compatibility**
- **Found during:** Task 1 (test failures)
- **Issue:** Moving Output: line to stderr only broke test checking stdout
- **Fix:** Print summary to both stdout and stderr in non-json non-quiet mode
- **Files modified:** crates/slicecore-cli/src/main.rs
- **Verification:** Both cli_output and cli_slice_profiles tests pass
- **Committed in:** 5e1c259

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both fixes necessary for test compatibility. No scope creep.

## Issues Encountered
- Pre-existing linker crash in phase12_integration test (LLVM bug) -- not related to changes, out of scope

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- cmd_slice fully migrated, ready for Plan 03 to migrate remaining CLI commands
- CliOutput threading pattern established for workflow functions

---
*Phase: 40-adopt-indicatif-for-consistent-cli-progress-display*
*Completed: 2026-03-19*
