# Phase 37: CI Benchmark Tracking with Regression Detection - Context

**Gathered:** 2026-03-19
**Status:** Ready for planning

<domain>
## Phase Boundary

Integrate existing criterion benchmarks into CI with two-tier regression detection (warn/block), historical tracking on a public gh-pages dashboard, per-PR comparison comments, and peak memory tracking. Includes documentation for developers on running and interpreting benchmarks.

</domain>

<decisions>
## Implementation Decisions

### Regression Threshold Policy
- Two-tier thresholds: **warn at 5%** regression, **block PR at 15%** regression
- Same thresholds apply uniformly across all benchmark suites (no per-suite customization)
- Thresholds apply **per individual benchmark** (any single benchmark exceeding 15% blocks), with a **suite-level summary** also shown in the PR comment
- `bench-ok` label on PRs overrides the block — for expected regressions from large features
- When a "regressed" PR merges via override, the next main push becomes the new baseline automatically
- Both improvements (speedups) and regressions are highlighted in PR comments
- Use **criterion defaults** for sample size (100 iterations, 5s measurement time)

### Comparison Strategy
- Use **both** GitHub Actions:
  - `boa-dev/criterion-compare-action` — runs benchmarks on PR base AND head for accurate per-PR comparison (immune to hardware variance)
  - `benchmark-action/github-action-benchmark` — stores historical results on gh-pages for long-term trending
- PR comments show a formatted table of regressions/improvements
- GitHub **check annotations** appear in the Files Changed tab alongside relevant code

### CI Trigger Scope
- Run benchmarks on **every PR** and on **main pushes** (for baseline updates)
- **Skip** benchmark job when only non-code files changed (`.md`, `.planning/`, `.github/` paths)
- Run on **Ubuntu only** for consistent, comparable results
- Benchmarks are a **separate job** in ci.yml (not part of the existing test matrix)

### Dashboard and History
- Historical data stored on **gh-pages branch** as JSON
- Public dashboard at GitHub Pages URL — anyone can view performance trends
- Retain **last 100 data points** per benchmark (prevents unbounded growth)

### Benchmark Suite Selection
- Run **all 4 suites** in CI:
  - `slicecore-engine`: `slice_benchmark`, `geometry_benchmark`, `parallel_benchmark` (with `--features parallel`)
  - `slicecore-mesh`: `csg_bench`
- **Explicit registration** — each benchmark is listed in the CI YAML; new benchmarks must be added manually

### Memory Tracking
- Use `/usr/bin/time -v` wrapper to capture **peak RSS** for each benchmark run
- Report memory alongside timing data in PR comments
- Memory regression thresholds: same 5%/15% warn/block as timing

### Documentation
- Add a section to README or CONTRIBUTING.md covering:
  - How to run benchmarks locally (`cargo bench`)
  - How to interpret CI benchmark results
  - How to use the `bench-ok` label for expected regressions
  - How to add new benchmarks to CI

### Claude's Discretion
- Exact GitHub Actions workflow YAML structure and job naming
- How to format the PR comment table (markdown layout)
- Whether to use a reusable workflow or inline job
- Cache strategy for benchmark builds
- Exact paths-filter configuration for skip logic

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing benchmarks
- `crates/slicecore-engine/benches/slice_benchmark.rs` — Full-pipeline slice benchmarks for 5 synthetic models
- `crates/slicecore-engine/benches/geometry_benchmark.rs` — Geometry hot-path micro-benchmarks (booleans, offsetting, BVH)
- `crates/slicecore-engine/benches/parallel_benchmark.rs` — Parallel vs sequential slicing comparison (requires `parallel` feature)
- `crates/slicecore-mesh/benches/csg_bench.rs` — CSG operation benchmarks (union, intersection, difference, hollow)

### CI configuration
- `.github/workflows/ci.yml` — Current CI workflow (fmt, clippy, test, WASM, doc jobs — no benchmark job yet)

### GitHub Actions to integrate
- `benchmark-action/github-action-benchmark` — Historical tracking with gh-pages charts
- `boa-dev/criterion-compare-action` — PR base-vs-head comparison comments

### Project constraints
- `.planning/PROJECT.md` — Performance target (>=1.5x C++ libslic3r), memory target (<=80% of C++)

### TODO source
- `.planning/todos/pending/2026-03-18-ci-benchmark-tracking-with-regression-detection.md` — Original TODO with recommended approach

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- 4 criterion benchmark suites already written and working locally
- CI workflow at `.github/workflows/ci.yml` — well-structured with matrix builds, can add a new job following same patterns
- `Swatinem/rust-cache@v2` already used in CI for build caching

### Established Patterns
- CI uses `dtolnay/rust-toolchain@stable` for Rust setup
- Matrix strategy with `fail-fast: false` for multi-OS testing
- `actions/checkout@v6` for repo checkout
- `CARGO_TERM_COLOR: always` and `RUSTFLAGS: "-D warnings"` as env defaults

### Integration Points
- New `bench` job added to `.github/workflows/ci.yml`
- gh-pages branch created for benchmark data storage
- GitHub Pages enabled on the repository for public dashboard
- `bench-ok` label created in the GitHub repository

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches for the GitHub Actions integration.

</specifics>

<deferred>
## Deferred Ideas

- **Binary size tracking** — Track compiled binary size over time (different tooling, not criterion-based)
- **Local baseline script** — Convenience script for devs to save/compare local baselines before pushing
- **WASM size tracking** — Track WASM binary size per crate (needs wasm-opt/twiggy tooling)
- **Benchmark flame graphs** — Auto-generate flamegraphs on regression to diagnose where slowdowns occur
- **Compile time tracking** — Track `cargo build` times to catch dependency bloat or excessive generics

</deferred>

---

*Phase: 37-ci-benchmark-tracking*
*Context gathered: 2026-03-19*
