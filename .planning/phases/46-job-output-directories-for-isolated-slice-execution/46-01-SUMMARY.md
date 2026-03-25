---
phase: 46-job-output-directories-for-isolated-slice-execution
plan: 01
subsystem: cli
tags: [job-dir, manifest, locking, uuid, chrono, sha2]

# Dependency graph
requires: []
provides:
  - "JobDir struct with create, create_auto, resolve_base, and artifact path methods"
  - "Manifest struct with running/success/failed lifecycle and JSON serialization"
  - "PID-based file locking with stale lock detection and Drop cleanup"
  - "ArtifactChecksums, PrintStats, ProfileSource, InputModelMeta, EnvironmentInfo types"
affects: [46-02-PLAN]

# Tech tracking
tech-stack:
  added: [uuid, chrono, sha2 (workspace)]
  patterns: [pid-lock-file, manifest-lifecycle, tdd-red-green]

key-files:
  created:
    - crates/slicecore-cli/src/job_dir.rs
  modified:
    - crates/slicecore-cli/Cargo.toml
    - crates/slicecore-engine/Cargo.toml
    - Cargo.toml
    - crates/slicecore-cli/src/main.rs

key-decisions:
  - "Moved sha2 to workspace dependency so both slicecore-engine and slicecore-cli share it"
  - "Used kill -0 via std::process::Command for cross-platform process existence check instead of libc"
  - "Added #[allow(dead_code)] on mod declaration since module is not yet wired into cmd_slice (Plan 02)"

patterns-established:
  - "PID-based lock file pattern: acquire lock before emptiness check (TOCTOU safety)"
  - "Manifest lifecycle: new_running -> into_success/into_failed with immutable transitions"

requirements-completed: []

# Metrics
duration: 5min
completed: 2026-03-24
---

# Phase 46 Plan 01: Job Directory Module Summary

**JobDir module with PID-based locking, Manifest JSON lifecycle, and 17 unit tests using TDD**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-24T21:53:35Z
- **Completed:** 2026-03-24T21:58:41Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Created job_dir.rs module with JobDir, Manifest, and all supporting types
- Implemented PID-based file locking with stale lock detection and Drop-based cleanup
- Added uuid, chrono, and sha2 dependencies; promoted sha2 to workspace dep
- 17 unit tests covering creation, locking, emptiness, auto mode, base resolution, manifest serialization

## Task Commits

Each task was committed atomically:

1. **Task 1: Add uuid and chrono dependencies** - `e40a095` (chore)
2. **Task 2 RED: Failing tests** - `116d8d7` (test)
3. **Task 2 GREEN: Implementation** - `760e567` (feat)

## Files Created/Modified
- `crates/slicecore-cli/src/job_dir.rs` - JobDir struct, Manifest, locking, artifact paths, unit tests
- `crates/slicecore-cli/Cargo.toml` - Added uuid, chrono, sha2, thiserror dependencies
- `crates/slicecore-engine/Cargo.toml` - Changed sha2 to workspace reference
- `Cargo.toml` - Added sha2 to workspace dependencies
- `crates/slicecore-cli/src/main.rs` - Added `mod job_dir;` declaration

## Decisions Made
- Promoted sha2 from a direct dependency in slicecore-engine to a workspace dependency so both engine and CLI crates can share it
- Used `std::process::Command::new("kill").args(["-0", &pid])` for process existence checking instead of adding a libc dependency
- Added `#[allow(dead_code)]` on the module declaration since Plan 02 will wire it into cmd_slice

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Promoted sha2 to workspace dependency**
- **Found during:** Task 1
- **Issue:** sha2 was a direct dependency in slicecore-engine but not a workspace dep; plan specified `sha2 = { workspace = true }` for slicecore-cli
- **Fix:** Added `sha2 = "0.10"` to workspace Cargo.toml, updated slicecore-engine to use `sha2 = { workspace = true }`
- **Files modified:** Cargo.toml, crates/slicecore-engine/Cargo.toml
- **Verification:** `cargo check -p slicecore-cli` passes
- **Committed in:** e40a095

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary to enable workspace-shared sha2 dependency. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- job_dir module is ready for Plan 02 to wire into cmd_slice and cmd_slice_plate
- All exported types (JobDir, Manifest, JobStatus, JobDirError) available for integration
- Module marked with `#[allow(dead_code)]` pending Plan 02 integration

---
*Phase: 46-job-output-directories-for-isolated-slice-execution*
*Completed: 2026-03-24*
