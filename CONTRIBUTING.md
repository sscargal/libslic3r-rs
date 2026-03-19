# Contributing to libslic3r-rs

Thank you for your interest in contributing to libslic3r-rs, a Rust-based 3D printer slicing engine. This document covers guidelines and workflows for contributors.

## Benchmarks

### Running Benchmarks Locally

Run all benchmarks:

```bash
cargo bench --workspace --features parallel
```

Run a specific benchmark suite:

```bash
cargo bench -p slicecore-engine --bench slice_benchmark
cargo bench -p slicecore-engine --bench geometry_benchmark
cargo bench -p slicecore-engine --bench parallel_benchmark --features parallel
cargo bench -p slicecore-mesh --bench csg_bench
```

Criterion saves results in `target/criterion/`. To compare against a baseline:

```bash
cargo bench --bench slice_benchmark -- --save-baseline before
# ... make changes ...
cargo bench --bench slice_benchmark -- --baseline before
```

### CI Benchmark Results

Every PR with code changes triggers the benchmark CI job, which:

1. **Runs all 4 benchmark suites** on Ubuntu with stable Rust
2. **Compares against the PR base branch** using criterion-compare-action (runs both branches on the same hardware for accurate comparison)
3. **Posts a PR comment** with a table of timing regressions and improvements
4. **Adds check annotations** in the Files Changed tab for significant regressions
5. **Captures peak memory (RSS)** for each suite via `/usr/bin/time -v`

**Threshold policy:**
- **5% regression:** Warning annotation (does not block merge)
- **15% regression:** Blocks the PR (any single benchmark exceeding 15% blocks the entire PR)
- Both speedups and regressions are shown in the PR comment

On pushes to `main`, benchmark results are stored on the `gh-pages` branch and displayed on the [performance dashboard](https://github.com/USER/libslic3r-rs/pages) (last 100 data points per benchmark).

**Note on memory values:** Peak RSS includes cargo and benchmark harness overhead, not just the benchmarked code. Absolute values are inflated, but relative changes between runs are meaningful for regression detection.

### Using the `bench-ok` Label

When a PR intentionally regresses benchmarks (e.g., a large feature that adds complexity), apply the `bench-ok` label to the PR before merging. This overrides the 15% regression block.

When a PR with `bench-ok` merges to main, the next main push automatically updates the baseline. Future PRs compare against the new (slower) baseline.

Use sparingly -- every `bench-ok` merge permanently shifts the performance baseline.

### Adding New Benchmarks to CI

Benchmarks are **explicitly registered** in `.github/workflows/ci.yml`. To add a new benchmark:

1. Create the benchmark file in the appropriate crate's `benches/` directory
2. Register it in `Cargo.toml` as a `[[bench]]` target
3. Add a line to the bench job in `.github/workflows/ci.yml`:
   ```yaml
   ./scripts/bench-with-memory.sh <package-name> <bench-name> [--features <feature>]
   ```
4. If using criterion-compare-action per-bench, add the bench name to the action's configuration

New benchmarks are NOT auto-discovered. This is intentional to keep CI predictable.
