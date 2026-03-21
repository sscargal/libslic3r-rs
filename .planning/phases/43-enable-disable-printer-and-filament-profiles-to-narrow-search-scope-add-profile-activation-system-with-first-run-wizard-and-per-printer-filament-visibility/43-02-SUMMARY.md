---
phase: 43-enable-disable-printer-and-filament-profiles
plan: 02
subsystem: cli
tags: [clap, profile-activation, enable-disable, cli-subcommands]

requires:
  - phase: 43-01
    provides: "EnabledProfiles data model with load/save/enable/disable/counts"
provides:
  - "Enable, Disable, Status CLI commands for profile activation"
  - "Activation-aware list filtering with --enabled/--disabled/--all flags"
  - "JSON output support on all new commands"
affects: [43-03-wizard, profile-management]

tech-stack:
  added: []
  patterns: [enabled-profiles-path-helper, activation-filter-defaulting]

key-files:
  created:
    - crates/slicecore-cli/tests/cli_profile_enable.rs
  modified:
    - crates/slicecore-cli/src/profile_command.rs

key-decisions:
  - "Used allow(clippy::too_many_arguments) on cmd_list rather than restructuring the function signature, since the 3 new bool params are a natural extension of the existing API"
  - "Disable command removes from all three sections when --type not specified, since profile IDs may exist in only one section and this is safe (no-op on missing)"
  - "Interactive picker placeholder exits with code 1 and message -- Plan 03 will implement the actual interactive path"

patterns-established:
  - "enabled_profiles_path() helper centralizes config file location logic for --profiles-dir override"
  - "Activation filter defaulting: --enabled when config exists, --all when it does not"

requirements-completed: [API-02]

duration: 4min
completed: 2026-03-21
---

# Phase 43 Plan 02: Enable/Disable/Status CLI Commands Summary

**Enable/disable/status profile activation commands with --json support and activation-aware list filtering**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-21T00:50:18Z
- **Completed:** 2026-03-21T00:54:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Added Enable, Disable, and Status CLI subcommands to `slicecore profile`
- List command now defaults to showing only enabled profiles when enabled-profiles.toml exists
- All new commands support --json flag for programmatic output
- Five integration tests verify end-to-end CLI behavior

## Task Commits

Each task was committed atomically:

1. **Task 1: Add Enable, Disable, Status variants and activation-aware List** - `7a93208` (feat)
2. **Task 2: Add CLI integration tests** - `a452468` (test)

## Files Created/Modified
- `crates/slicecore-cli/src/profile_command.rs` - Added Enable/Disable/Status variants, match arms, implementation functions, and activation filtering to List
- `crates/slicecore-cli/tests/cli_profile_enable.rs` - Integration tests for status, enable, disable commands

## Decisions Made
- Used `allow(clippy::too_many_arguments)` on `cmd_list` rather than restructuring, since the 3 new bool params naturally extend the existing API
- Disable removes from all sections by default for simplicity (safe no-op on missing)
- Interactive picker placeholder exits with code 1 -- Plan 03 implements the real interactive path

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Enable/disable/status commands operational for explicit profile IDs
- Ready for Plan 03 to add interactive picker and first-run wizard
- List filtering integrates with the enabled-profiles.toml from Plan 01

---
*Phase: 43-enable-disable-printer-and-filament-profiles*
*Completed: 2026-03-21*
