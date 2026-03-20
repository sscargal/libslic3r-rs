---
phase: 41-travel-move-optimization
plan: 03
subsystem: cli
tags: [clap, cli-flags, travel-optimization, debugging]

requires:
  - phase: 41-01
    provides: TravelOptConfig with enabled field in PrintConfig
provides:
  - "--no-travel-opt CLI flag for disabling travel optimization"
affects: [41-04, benchmarking, debugging-workflows]

tech-stack:
  added: []
  patterns: [cli-flag-to-config-override]

key-files:
  created: []
  modified:
    - crates/slicecore-cli/src/main.rs

key-decisions:
  - "Placed override after full config resolution but before Engine::new to ensure it wins over profile/TOML settings"
  - "Made print_config binding mutable to support CLI flag override pattern"

patterns-established:
  - "CLI flag override pattern: add #[arg(long)] bool field, thread through cmd_slice, apply to mutable config before Engine::new"

requirements-completed: [GCODE-05]

duration: 2min
completed: 2026-03-20
---

# Phase 41 Plan 03: CLI Flag for Travel Optimization Bypass Summary

**--no-travel-opt CLI flag on slice command to disable TSP travel optimization for A/B comparison and debugging**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-20T17:06:40Z
- **Completed:** 2026-03-20T17:09:19Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Added `--no-travel-opt` flag to the Slice CLI subcommand
- Flag sets `config.travel_opt.enabled = false` after config resolution, before engine creation
- Enables users to do A/B comparison of travel-optimized vs unoptimized output

## Task Commits

Each task was committed atomically:

1. **Task 1: Add --no-travel-opt flag to slice command** - `9c41503` (feat)

## Files Created/Modified
- `crates/slicecore-cli/src/main.rs` - Added no_travel_opt flag to Slice struct, threaded through destructure and cmd_slice signature, applied override before Engine::new

## Decisions Made
- Placed the override after config resolution (both profile workflow and legacy --config path) but before Engine::new, so the flag always wins regardless of config source
- Changed `print_config` binding from immutable to mutable to support the override

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- CLI flag ready for use in benchmark validation (plan 04)
- Users can now compare optimized vs unoptimized travel distances

---
*Phase: 41-travel-move-optimization*
*Completed: 2026-03-20*
