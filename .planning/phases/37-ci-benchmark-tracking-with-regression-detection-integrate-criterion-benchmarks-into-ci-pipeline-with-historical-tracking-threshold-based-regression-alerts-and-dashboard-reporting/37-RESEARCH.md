# Phase 37: CI Benchmark Tracking with Regression Detection - Research

**Researched:** 2026-03-19
**Domain:** GitHub Actions CI, Criterion benchmarks, performance regression detection
**Confidence:** HIGH

## Summary

This phase integrates the project's 4 existing criterion benchmark suites into CI using two complementary GitHub Actions: `boa-dev/criterion-compare-action` for per-PR base-vs-head comparison comments, and `benchmark-action/github-action-benchmark` for historical tracking on a gh-pages dashboard. The workflow adds a new `bench` job to `.github/workflows/ci.yml` that runs on Ubuntu only, with path filtering to skip when only non-code files change.

The implementation is straightforward because all benchmark infrastructure already exists locally. The core work is YAML workflow configuration, `/usr/bin/time -v` memory wrapper scripting, and developer documentation. No Rust code changes are needed beyond possibly a small shell script for memory capture.

**Primary recommendation:** Add a single `bench` job to ci.yml that runs all 4 benchmark suites, uses `--output-format bencher` for github-action-benchmark compatibility, and wraps runs with `/usr/bin/time -v` for peak RSS tracking. Use `dorny/paths-filter` for skip logic.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Two-tier thresholds: warn at 5% regression, block PR at 15% regression
- Same thresholds apply uniformly across all benchmark suites (no per-suite customization)
- Thresholds apply per individual benchmark (any single benchmark exceeding 15% blocks), with a suite-level summary also shown in the PR comment
- `bench-ok` label on PRs overrides the block -- for expected regressions from large features
- When a "regressed" PR merges via override, the next main push becomes the new baseline automatically
- Both improvements (speedups) and regressions are highlighted in PR comments
- Use criterion defaults for sample size (100 iterations, 5s measurement time)
- Use both GitHub Actions: `boa-dev/criterion-compare-action` (PR base/head comparison) and `benchmark-action/github-action-benchmark` (historical gh-pages tracking)
- PR comments show a formatted table of regressions/improvements
- GitHub check annotations appear in the Files Changed tab alongside relevant code
- Run benchmarks on every PR and on main pushes (for baseline updates)
- Skip benchmark job when only non-code files changed (.md, .planning/, .github/ paths)
- Run on Ubuntu only for consistent, comparable results
- Benchmarks are a separate job in ci.yml (not part of existing test matrix)
- Historical data stored on gh-pages branch as JSON
- Public dashboard at GitHub Pages URL
- Retain last 100 data points per benchmark
- Run all 4 suites: slicecore-engine (slice_benchmark, geometry_benchmark, parallel_benchmark with --features parallel) and slicecore-mesh (csg_bench)
- Explicit registration -- each benchmark is listed in CI YAML; new benchmarks must be added manually
- Use /usr/bin/time -v wrapper to capture peak RSS for each benchmark run
- Memory regression thresholds: same 5%/15% warn/block as timing
- Add documentation section covering local benchmark running, CI interpretation, bench-ok label usage, and adding new benchmarks

### Claude's Discretion
- Exact GitHub Actions workflow YAML structure and job naming
- How to format the PR comment table (markdown layout)
- Whether to use a reusable workflow or inline job
- Cache strategy for benchmark builds
- Exact paths-filter configuration for skip logic

### Deferred Ideas (OUT OF SCOPE)
- Binary size tracking
- Local baseline script
- WASM size tracking
- Benchmark flame graphs
- Compile time tracking
</user_constraints>

## Standard Stack

### Core
| Library/Action | Version | Purpose | Why Standard |
|----------------|---------|---------|--------------|
| criterion | 0.5 (workspace dep) | Rust benchmarking framework | Already in use; statistical rigor |
| benchmark-action/github-action-benchmark | v1 | Historical tracking, gh-pages charts | Most popular GH Actions benchmark tool; 4k+ stars |
| boa-dev/criterion-compare-action | v3 | PR base-vs-head comparison comments | Purpose-built for criterion; runs both branches |
| dorny/paths-filter | v3 | Skip benchmarks on non-code changes | Standard path filtering action; well-maintained |

### Supporting
| Library/Action | Version | Purpose | When to Use |
|----------------|---------|---------|-------------|
| actions/checkout@v6 | v6 | Repo checkout | Already used in CI |
| dtolnay/rust-toolchain@stable | stable | Rust toolchain setup | Already used in CI |
| Swatinem/rust-cache@v2 | v2 | Cargo build caching | Already used in CI |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| dorny/paths-filter | GitHub native `paths` filter on `on:` | Native `paths` only works for triggering, not conditional job skip within a workflow; paths-filter is more flexible for conditional jobs |
| /usr/bin/time -v | Custom Rust memory profiler | /usr/bin/time is simpler, captures peak RSS without code changes |

## Architecture Patterns

### Workflow Structure
```
.github/workflows/ci.yml
  jobs:
    changes:          # Path filter job (fast, <5s)
    bench:            # Benchmark job (depends on changes)
      needs: [changes]
      if: needs.changes.outputs.code == 'true'
      steps:
        - checkout
        - rust-toolchain
        - rust-cache
        - run benchmarks (cargo bench with --output-format bencher)
        - capture memory via /usr/bin/time -v
        - criterion-compare-action (PR only)
        - github-action-benchmark (push to main)
```

### Pattern 1: Criterion Output for github-action-benchmark
**What:** Criterion's `--output-format bencher` flag produces output compatible with `tool: 'cargo'` in github-action-benchmark.
**When to use:** When capturing criterion results for historical tracking.
**Example:**
```yaml
- name: Run benchmarks
  run: |
    cargo bench --bench slice_benchmark \
      --bench geometry_benchmark \
      --bench csg_bench \
      -- --output-format bencher | tee output.txt
    cargo bench --bench parallel_benchmark \
      --features parallel \
      -- --output-format bencher | tee -a output.txt
```

### Pattern 2: Two-Action Split (PR vs Main)
**What:** Use criterion-compare-action on PRs for immediate feedback, github-action-benchmark on main pushes for historical tracking.
**When to use:** This is the decided architecture.
**Example:**
```yaml
# PR comparison (only on pull_request)
- name: Compare benchmarks
  if: github.event_name == 'pull_request'
  uses: boa-dev/criterion-compare-action@v3
  with:
    branchName: ${{ github.base_ref }}
    token: ${{ secrets.GITHUB_TOKEN }}

# Historical tracking (only on push to main)
- name: Store benchmark result
  if: github.event_name == 'push' && github.ref == 'refs/heads/main'
  uses: benchmark-action/github-action-benchmark@v1
  with:
    tool: 'cargo'
    output-file-path: output.txt
    github-token: ${{ secrets.GITHUB_TOKEN }}
    auto-push: true
    alert-threshold: '115%'
    fail-on-alert: false
    comment-on-alert: true
```

### Pattern 3: Path Filtering with dorny/paths-filter
**What:** A dedicated lightweight job that determines if code files changed, gating the benchmark job.
**When to use:** To skip expensive benchmark runs when only docs/planning files change.
**Example:**
```yaml
changes:
  runs-on: ubuntu-latest
  permissions:
    pull-requests: read
  outputs:
    code: ${{ steps.filter.outputs.code }}
  steps:
    - uses: actions/checkout@v6
    - uses: dorny/paths-filter@v3
      id: filter
      with:
        filters: |
          code:
            - 'crates/**'
            - 'Cargo.toml'
            - 'Cargo.lock'
            - '.github/workflows/ci.yml'
```

### Pattern 4: Memory Tracking with /usr/bin/time
**What:** Wrap benchmark runs with `/usr/bin/time -v` to capture peak RSS.
**When to use:** For every benchmark suite in CI.
**Example:**
```bash
#!/bin/bash
# scripts/bench-with-memory.sh
set -euo pipefail

BENCH_NAME="$1"
shift
TIME_OUTPUT=$(mktemp)

/usr/bin/time -v cargo bench --bench "$BENCH_NAME" "$@" \
  -- --output-format bencher 2>"$TIME_OUTPUT" | tee -a output.txt

PEAK_RSS=$(grep "Maximum resident set size" "$TIME_OUTPUT" | awk '{print $NF}')
echo "MEMORY:${BENCH_NAME}:${PEAK_RSS}kB" >> memory-results.txt
rm "$TIME_OUTPUT"
```

### Anti-Patterns to Avoid
- **Running benchmarks as part of the test matrix:** Benchmarks need consistent hardware; multi-OS matrix introduces variance. Always run on a single ubuntu-latest runner.
- **Using `cargo +nightly bench` for criterion:** The github-action-benchmark example uses nightly, but criterion 0.5 works fine with stable Rust. Stick with stable to match the project's existing toolchain.
- **Forgetting `--output-format bencher`:** Without this flag, criterion output is human-readable but not parseable by github-action-benchmark's `tool: 'cargo'`.
- **Using `auto-push: true` on PRs:** Only push to gh-pages from main branch pushes, never from PR branches. PR results are ephemeral.
- **Blocking on historical alert-threshold:** The `github-action-benchmark` alert-threshold compares against historical data (affected by hardware variance). Use it for warnings only. The `criterion-compare-action` does base-vs-head comparison on the same runner, which is immune to hardware variance -- that is the one to use for blocking.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| PR benchmark comparison | Custom script to checkout base, run, diff | boa-dev/criterion-compare-action | Handles checkout, build, run, diff, and PR comment formatting |
| Historical charts | JSON storage + chart.js dashboard | benchmark-action/github-action-benchmark | Provides interactive charts, data retention, alert thresholds |
| Path filtering | Bash script parsing git diff | dorny/paths-filter | Handles PR vs push events, glob patterns, proper change detection |
| Peak RSS capture | Custom Rust allocator tracking | /usr/bin/time -v | OS-level measurement, zero code changes, captures actual peak |

**Key insight:** This phase is primarily CI YAML configuration, not Rust code. Every component has a well-maintained GitHub Action or OS tool. The only custom code needed is a small shell script to wrap benchmark runs with memory tracking and a documentation page.

## Common Pitfalls

### Pitfall 1: criterion-compare-action and --save-baseline
**What goes wrong:** The action internally uses `--save-baseline` which can conflict with projects that have both tests and benchmarks, producing "Unrecognized option: save-baseline" errors.
**Why it happens:** criterion-compare-action passes `--save-baseline` to `cargo bench`, which can be misinterpreted when test targets exist.
**How to avoid:** Use `benchName` parameter to specify individual bench targets, not the full workspace. Run each benchmark suite explicitly rather than `cargo bench --workspace`.
**Warning signs:** CI errors mentioning "Unrecognized option: 'save-baseline'"

### Pitfall 2: github-action-benchmark alert-threshold is a percentage of baseline
**What goes wrong:** Setting `alert-threshold: '105%'` means "alert when result is 105% of baseline" (i.e., 5% slower). This is correct for the 5% warn threshold, but the format is easy to confuse.
**Why it happens:** The threshold is expressed as a ratio, not a delta. 100% = identical to baseline.
**How to avoid:** For 15% block threshold, use `115%`. For 5% warn, use `105%`.
**Warning signs:** Alerts firing too aggressively or not at all.

### Pitfall 3: criterion-compare-action runs both branches from scratch
**What goes wrong:** The action checks out the base branch, builds, benchmarks, then checks out head, builds, benchmarks. This doubles the CI time and requires sufficient disk space.
**Why it happens:** By design -- this is how it achieves hardware-independent comparison.
**How to avoid:** Accept the ~2x time cost as the price for accurate comparison. Use Rust cache to speed up builds. Consider running only specific benchmark targets if time becomes excessive.
**Warning signs:** Benchmark job taking 30+ minutes.

### Pitfall 4: gh-pages branch not initialized
**What goes wrong:** `github-action-benchmark` with `auto-push: true` fails if the gh-pages branch doesn't exist.
**Why it happens:** First-time setup requires creating an empty gh-pages branch and enabling GitHub Pages.
**How to avoid:** Create the gh-pages branch before the first CI run. Include this as a setup step in documentation or as a one-time manual action.
**Warning signs:** "Error: fatal: couldn't find remote ref gh-pages"

### Pitfall 5: bench-ok label check timing
**What goes wrong:** The workflow needs to check for the `bench-ok` label at the right point -- after benchmarks run but before the fail decision.
**Why it happens:** Label state is read from the PR context via `github.event.pull_request.labels`.
**How to avoid:** Check labels using `contains(github.event.pull_request.labels.*.name, 'bench-ok')` in the `if` condition of the fail step.
**Warning signs:** PRs being blocked even when `bench-ok` is applied, or label check being evaluated before the label is added.

### Pitfall 6: Memory tracking via /usr/bin/time captures total process RSS
**What goes wrong:** `/usr/bin/time -v` captures peak RSS of the entire `cargo bench` process including the cargo wrapper, rustc compilations (if any), and the benchmark harness overhead.
**Why it happens:** /usr/bin/time measures the spawned process tree, not just the benchmark function.
**How to avoid:** This is acceptable for regression detection since the overhead is consistent across runs. The absolute values include overhead, but the relative changes are meaningful. Document this clearly so developers don't confuse peak RSS with benchmark-only memory usage.
**Warning signs:** Unexpectedly high RSS values (hundreds of MB) even for small benchmarks.

## Code Examples

### Complete Benchmark Job Structure
```yaml
bench:
  name: Benchmarks
  needs: [changes]
  if: needs.changes.outputs.code == 'true'
  runs-on: ubuntu-latest
  permissions:
    contents: write
    pull-requests: write
  steps:
    - uses: actions/checkout@v6
    - uses: dtolnay/rust-toolchain@stable
    - uses: Swatinem/rust-cache@v2
      with:
        prefix-key: bench

    # Run benchmarks with memory tracking
    - name: Run benchmarks
      run: |
        mkdir -p bench-results
        for bench in slice_benchmark geometry_benchmark; do
          /usr/bin/time -v cargo bench \
            -p slicecore-engine --bench "$bench" \
            -- --output-format bencher 2>bench-results/${bench}-time.txt \
            | tee -a bench-results/output.txt
        done
        /usr/bin/time -v cargo bench \
          -p slicecore-engine --bench parallel_benchmark \
          --features parallel \
          -- --output-format bencher 2>bench-results/parallel_benchmark-time.txt \
          | tee -a bench-results/output.txt
        /usr/bin/time -v cargo bench \
          -p slicecore-mesh --bench csg_bench \
          -- --output-format bencher 2>bench-results/csg_bench-time.txt \
          | tee -a bench-results/output.txt

    # Extract memory results
    - name: Extract peak RSS
      run: |
        for f in bench-results/*-time.txt; do
          bench=$(basename "$f" -time.txt)
          peak=$(grep "Maximum resident set size" "$f" | awk '{print $NF}')
          echo "${bench}: ${peak} kB" >> bench-results/memory.txt
        done
        cat bench-results/memory.txt
```

### criterion-compare-action for PR Comparison
```yaml
    # Per-PR comparison (pull_request only)
    - name: Compare benchmarks (PR)
      if: github.event_name == 'pull_request'
      uses: boa-dev/criterion-compare-action@v3
      with:
        branchName: ${{ github.base_ref }}
        token: ${{ secrets.GITHUB_TOKEN }}
```

### github-action-benchmark for Historical Tracking
```yaml
    # Historical tracking (main push only)
    - name: Store benchmark result
      if: github.event_name == 'push' && github.ref == 'refs/heads/main'
      uses: benchmark-action/github-action-benchmark@v1
      with:
        name: libslic3r-rs Benchmarks
        tool: 'cargo'
        output-file-path: bench-results/output.txt
        github-token: ${{ secrets.GITHUB_TOKEN }}
        gh-pages-branch: gh-pages
        benchmark-data-dir-path: dev/bench
        auto-push: true
        alert-threshold: '115%'
        comment-on-alert: true
        fail-on-alert: false
        max-items-in-chart: 100
```

### Threshold Check with bench-ok Override
```yaml
    - name: Check regression thresholds
      if: github.event_name == 'pull_request'
      run: |
        # Parse criterion comparison output for regressions > 15%
        # This step would parse the criterion-compare-action output
        # and fail if any benchmark regressed > 15%
        # unless the bench-ok label is present
        echo "Checking for regressions..."

    - name: Block on severe regression
      if: |
        github.event_name == 'pull_request' &&
        !contains(github.event.pull_request.labels.*.name, 'bench-ok')
      run: |
        # Custom script to parse results and exit 1 if >15% regression
        echo "Checking thresholds (bench-ok label not present)..."
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| cargo bench (nightly only) | criterion 0.5 (stable Rust) | criterion 0.5, 2023 | No need for nightly toolchain |
| Manual baseline comparison | criterion-compare-action v3 | 2023+ | Automated PR comments with statistical comparison |
| No historical tracking | github-action-benchmark v1 | Ongoing | gh-pages charts with alert thresholds |
| criterion --save-baseline (local) | CI-based baselines | Current | Reproducible, hardware-consistent baselines |

**Deprecated/outdated:**
- `cargo-criterion` binary: Separate binary that was meant to replace criterion's built-in runner. Development stalled; criterion 0.5's built-in `--output-format bencher` is sufficient.
- `matchai/criterion-compare-action`: Original unmaintained fork; `boa-dev/criterion-compare-action` is the actively maintained fork.

## Open Questions

1. **criterion-compare-action workspace support**
   - What we know: The action supports `package` and `benchName` parameters for targeting specific crates.
   - What's unclear: Whether the action can be invoked multiple times in one job for different packages/features, or if it needs separate invocations.
   - Recommendation: Use separate `boa-dev/criterion-compare-action` steps per crate, or use the action once at workspace level if it supports `--workspace`. Test during implementation.

2. **Memory regression detection automation**
   - What we know: /usr/bin/time -v captures peak RSS. Memory thresholds are 5%/15% warn/block.
   - What's unclear: Neither github-action-benchmark nor criterion-compare-action handle memory data natively. A custom script is needed to compare memory results between base and head.
   - Recommendation: Write a small bash script that reads memory.txt from current and base runs, computes deltas, and posts results in the PR comment alongside timing data.

3. **criterion-compare-action + --features parallel**
   - What we know: parallel_benchmark requires `--features parallel`.
   - What's unclear: Whether criterion-compare-action's `features` input applies to all benchmarks or can be scoped per target.
   - Recommendation: Either run criterion-compare-action separately for the parallel benchmark, or use a wrapper that sets features per bench target.

## Validation Architecture

> Nyquist validation is not explicitly disabled in config.json, so including this section.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | GitHub Actions workflow syntax + manual verification |
| Config file | `.github/workflows/ci.yml` |
| Quick run command | `act -j bench` (if act is installed) or push to a test branch |
| Full suite command | Create a PR to trigger the full benchmark workflow |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| N/A-01 | Benchmarks run on PR | smoke | Create test PR, verify bench job runs | N/A (CI workflow) |
| N/A-02 | Benchmarks skip on docs-only changes | smoke | Push .md-only change, verify bench skipped | N/A (CI workflow) |
| N/A-03 | PR comment with comparison table | smoke | Create PR with code change, verify comment | N/A (CI workflow) |
| N/A-04 | gh-pages updated on main push | smoke | Merge PR, verify gh-pages branch updated | N/A (CI workflow) |
| N/A-05 | bench-ok label overrides block | smoke | Apply label, verify PR not blocked | N/A (CI workflow) |
| N/A-06 | Memory results reported | smoke | Check PR comment for memory data | N/A (CI workflow) |

### Sampling Rate
- **Per task commit:** Validate YAML syntax with `yamllint` or visual inspection
- **Per wave merge:** Push to test branch to trigger actual CI run
- **Phase gate:** Full PR cycle (create PR, verify bench runs, verify comment, verify skip on docs-only)

### Wave 0 Gaps
- [ ] gh-pages branch needs to be created (empty branch for benchmark data storage)
- [ ] GitHub Pages needs to be enabled on the repository
- [ ] `bench-ok` label needs to be created in the GitHub repository
- [ ] `scripts/bench-with-memory.sh` helper script (new file)

## Sources

### Primary (HIGH confidence)
- benchmark-action/github-action-benchmark repo and examples - configuration options, tool: 'cargo', alert-threshold format, auto-push behavior
- boa-dev/criterion-compare-action repo and README - inputs (branchName, package, benchName, features), v3 usage
- Existing project CI at `.github/workflows/ci.yml` - current job structure, actions versions, cache setup
- Existing benchmark files in `crates/slicecore-engine/benches/` and `crates/slicecore-mesh/benches/` - benchmark names, features required

### Secondary (MEDIUM confidence)
- dorny/paths-filter v3 documentation - filter syntax, job output pattern
- criterion 0.5 `--output-format bencher` flag - compatibility with github-action-benchmark

### Tertiary (LOW confidence)
- criterion-compare-action --save-baseline issue (#24) - potential compatibility issue with workspace builds, needs validation during implementation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All actions are well-documented, actively maintained, and widely used
- Architecture: HIGH - Two-action pattern (compare + history) is the standard approach in Rust CI
- Pitfalls: MEDIUM - Some edge cases (save-baseline issue, features flag scoping) need validation during implementation

**Research date:** 2026-03-19
**Valid until:** 2026-04-19 (stable domain; GitHub Actions versions rarely break)
