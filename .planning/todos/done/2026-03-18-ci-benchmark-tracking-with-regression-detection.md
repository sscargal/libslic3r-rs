---
created: 2026-03-18T21:30:00.000Z
title: CI benchmark tracking with regression detection
area: testing
files:
  - .github/workflows/ci.yml
  - crates/slicecore-engine/benches/slice_benchmark.rs
  - crates/slicecore-engine/benches/geometry_benchmark.rs
  - crates/slicecore-engine/benches/parallel_benchmark.rs
  - crates/slicecore-mesh/benches/csg_bench.rs
---

## Problem

Criterion benchmarks run locally via `cargo bench` and the QA script, but there's no CI integration to catch performance regressions on PRs. Developers must manually check benchmark results, and there's no historical tracking of performance over time.

## Solution

Add CI benchmark tracking using one or both of these GitHub Actions:

1. **[benchmark-action/github-action-benchmark](https://github.com/benchmark-action/github-action-benchmark)** — Full tracking with charts
   - Parses criterion JSON output
   - Stores historical results in `gh-pages` branch as JSON
   - Renders performance charts at a dashboard URL
   - Can fail PRs that exceed a configurable regression threshold (e.g., 10%)
   - Best for: long-term trending, identifying gradual regressions

2. **[boa-dev/criterion-compare-action](https://github.com/boa-dev/criterion-compare-action)** — PR comment diffs
   - Runs benchmarks on both the PR base branch and head
   - Posts a comparison table as a PR comment showing +/- for each benchmark
   - Ephemeral (no persistent history beyond the PR)
   - Best for: quick per-PR regression checking

Recommended approach: Use both — `criterion-compare-action` for immediate PR feedback, `github-action-benchmark` for historical tracking. Add a separate `bench` job in ci.yml that only runs on `ubuntu-latest` (consistent hardware) and uses `--bench` flags to run criterion benchmarks.
