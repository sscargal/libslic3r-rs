---
phase: 37-ci-benchmark-tracking
plan: 01
subsystem: infra
tags: [ci, benchmarks, criterion, github-actions, regression-detection, memory-tracking]

# Dependency graph
requires:
  - phase: 04-benchmark-infrastructure
    provides: criterion benchmark suites (slice, geometry, parallel, csg)
provides:
  - CI benchmark job with path filtering and base-branch comparison
  - Memory-tracking wrapper script (bench-with-memory.sh)
  - Two-tier regression enforcement for timing and memory (check-bench-regressions.sh)
  - Historical benchmark tracking via github-action-benchmark on gh-pages
affects: [ci-pipeline, benchmark-infrastructure]

# Tech tracking
tech-stack:
  added: [dorny/paths-filter@v3, boa-dev/criterion-compare-action@v3, benchmark-action/github-action-benchmark@v1]
  patterns: [base-branch-benchmarking-for-pr-comparison, two-tier-threshold-enforcement, memory-rss-tracking]

key-files:
  created:
    - scripts/bench-with-memory.sh
    - scripts/check-bench-regressions.sh
  modified:
    - .github/workflows/ci.yml

key-decisions:
  - "Base-branch benchmarks run on same CI hardware as head for accurate comparison"
  - "criterion-compare-action is informational only; check-bench-regressions.sh is the enforcement gate"
  - "Integer arithmetic in bash sufficient for 5%/15% threshold comparison"
  - "bench-ok label skips enforcement step entirely via workflow conditional"

patterns-established:
  - "Two-tier threshold pattern: 5% warn (::warning::), 15% block (exit 1) for both timing and memory"
  - "Memory tracking via /usr/bin/time -v wrapper around cargo bench"

requirements-completed: [BENCH-CI, BENCH-COMPARE, BENCH-HISTORY, BENCH-MEMORY, BENCH-SKIP]

# Metrics
duration: 2min
completed: 2026-03-19
---

# Phase 37 Plan 01: CI Benchmark Tracking Summary

**CI benchmark pipeline with path filtering, base-branch comparison, two-tier timing+memory regression enforcement (5% warn / 15% block), and historical tracking via gh-pages**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-19T03:05:04Z
- **Completed:** 2026-03-19T03:07:18Z
- **Tasks:** 4 (1 manual checkpoint + 3 automated)
- **Files modified:** 3

## Accomplishments
- Memory-tracking wrapper script captures peak RSS for each benchmark suite
- Regression enforcement script enforces both timing and memory thresholds independently
- CI workflow runs base-branch benchmarks on PRs for accurate same-hardware comparison
- Historical tracking pushes results to gh-pages on main merges with 5% alert threshold

## Task Commits

Each task was committed atomically:

1. **Task 0: One-time GitHub repository setup** - N/A (manual checkpoint)
2. **Task 1: Create memory-tracking benchmark wrapper script** - `f0f41e3` (feat)
3. **Task 2: Create regression threshold enforcement script** - `31e9f3e` (feat)
4. **Task 3: Add changes filter and bench job to CI workflow** - `5f89bc0` (feat)

## Files Created/Modified
- `scripts/bench-with-memory.sh` - Wraps cargo bench with /usr/bin/time -v for peak RSS capture
- `scripts/check-bench-regressions.sh` - Parses bencher-format and memory output, enforces 5%/15% thresholds
- `.github/workflows/ci.yml` - Added changes filter job and bench job with full pipeline

## Decisions Made
- Base-branch benchmarks run in the same CI job as head benchmarks for accurate hardware-consistent comparison
- criterion-compare-action serves as informational PR comment only; enforcement is via custom script
- Integer arithmetic in bash is sufficient for percentage threshold calculations
- bench-ok label skips the enforcement step entirely via GitHub Actions conditional

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
Task 0 required one-time manual setup (gh-pages branch, GitHub Pages enabled, bench-ok label created). This was completed before task execution began.

## Next Phase Readiness
- CI benchmark infrastructure is complete and ready for use on PRs and main pushes
- First main push with code changes will establish the historical baseline on gh-pages
- First PR with code changes will run the full comparison pipeline

---
*Phase: 37-ci-benchmark-tracking*
*Completed: 2026-03-19*
