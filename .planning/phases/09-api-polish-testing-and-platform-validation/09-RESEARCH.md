# Phase 9: API Polish, Testing, and Platform Validation - Research

**Researched:** 2026-02-17
**Domain:** Production readiness -- API documentation, structured output, cross-platform CI, WASM validation, performance benchmarking, test coverage
**Confidence:** HIGH

## Summary

Phase 9 is the final production-readiness phase. The codebase is functionally complete across 11 crates (~48,800 lines of Rust, 1,111 test functions, 483 public API items), but needs polish in six areas: (1) rustdoc completeness, (2) structured JSON/MessagePack output, (3) cross-platform CI matrix, (4) WASM compilation fixes, (5) benchmark suite and performance validation, and (6) test coverage to >80%.

The codebase is in strong shape. All 1,111 tests pass. All non-module public items already have doc comments. The `wasm32-wasip2` target already compiles successfully. Many key types already derive `Serialize`/`Deserialize`. The primary gaps are: no event/progress system exists yet, no MessagePack support, no benchmark suite, no fuzz targets, no golden file tests, the CI matrix only runs on `ubuntu-latest`, and `wasm32-unknown-unknown` fails due to `getrandom` 0.3 in the `boostvoronoi` dependency chain.

**Primary recommendation:** Work in six parallel streams -- (1) fix `pub mod` doc comments and rustdoc warnings, (2) add Serialize/Deserialize to remaining types + rmp-serde + event system, (3) expand CI matrix to macOS/Windows/ARM, (4) fix WASM `getrandom` issue and build browser demo, (5) add criterion benchmarks and measure vs C++ baseline, (6) add fuzz targets + golden tests + coverage measurement.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `rmp-serde` | 1.3 | MessagePack serialization/deserialization via serde | 63M+ downloads, the standard MessagePack crate for Rust. Serde integration means zero new code for types that already derive Serialize/Deserialize. |
| `criterion` | 0.5 | Statistical micro-benchmarking with regression detection | 132M+ downloads, de facto standard for Rust benchmarks. Version 0.5 is the latest stable that works with stable Rust (0.8 requires nightly features or Rust 1.88+). |
| `cargo-tarpaulin` | 0.31+ | Line coverage measurement | Standard Rust coverage tool. Supports `--engine llvm` for cross-platform measurement. Linux ptrace default on x86_64; llvm engine for other architectures. |
| `cargo-fuzz` | latest | Fuzz testing framework using libFuzzer | Official Rust fuzzing tool. Requires nightly toolchain. Already installed in this environment. |
| `wasm-bindgen` | 0.2 | WASM-to-JS bridge for browser demo | Required if browser demo needs JS interop. Used by virtually all Rust WASM projects. |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `getrandom` | 0.3 (dep) | Random number generation (transitive through boostvoronoi->ahash) | Must configure `getrandom_backend="wasm_js"` in `.cargo/config.toml` for `wasm32-unknown-unknown` target |
| `serde_json` | 1 (already present) | JSON structured output | Already a workspace dependency; needs to be added as a runtime dep (not just dev-dep) in engine crate |
| `cross` | latest | Cross-compilation for ARM targets | Use via `houseabsolute/actions-rust-cross` GitHub Action for Linux ARM and other cross-compilation targets in CI |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `cargo-tarpaulin` | `llvm-cov` (via `cargo-llvm-cov`) | `llvm-cov` uses LLVM instrumentation and works across all platforms; tarpaulin is Linux-centric but simpler to use. Either works; tarpaulin is more established for simple line coverage. |
| `criterion` 0.5 | `criterion` 0.8 | 0.8 requires Rust 1.88+ (we have 1.93, so it would work). However, 0.5 is more battle-tested and has broader ecosystem support. Use 0.5 for stability. |
| `cargo-fuzz` | `proptest` (already used) | proptest is property-based testing, not fuzz testing. Both are valuable but serve different purposes. Fuzz testing is required by TEST-04 specifically for mesh parsers. |
| `rmp-serde` | `bincode` | bincode is faster but not MessagePack format. The requirement (API-04) specifically calls for MessagePack. |

**Installation:**
```bash
# Add to workspace Cargo.toml
rmp-serde = "1"
criterion = { version = "0.5", features = ["html_reports"] }

# Install tools
cargo install cargo-tarpaulin
# cargo-fuzz already installed

# For CI
# cargo install cross (handled by GitHub Action)
```

## Architecture Patterns

### Recommended Project Structure Changes

```
.
├── .cargo/
│   └── config.toml          # Add getrandom_backend for WASM
├── .github/
│   └── workflows/
│       └── ci.yml            # Expand from single ubuntu job to full matrix
├── benches/                  # NEW: workspace-level criterion benchmarks
│   ├── slice_benchmark.rs    # Full pipeline benchmark (5 models)
│   └── geometry_benchmark.rs # Hot-path micro-benchmarks
├── fuzz/                     # NEW: cargo-fuzz targets
│   └── fuzz_targets/
│       ├── fuzz_stl_binary.rs
│       ├── fuzz_stl_ascii.rs
│       └── fuzz_obj.rs
├── tests/                    # NEW: workspace-level integration tests
│   └── golden/               # Golden file test fixtures
│       ├── calibration_cube.gcode.expected
│       └── ...
└── wasm-demo/                # NEW: browser slicing demo
    ├── src/lib.rs            # wasm-bindgen entry point
    ├── index.html
    └── Cargo.toml
```

### Pattern 1: Structured Output via Serde

**What:** Add `Serialize`/`Deserialize` to all public result types and provide JSON/MessagePack output functions.
**When to use:** Any type that crosses the API boundary (SliceResult, ToolpathSegment, FeatureType, etc.)

**Current state analysis:**
- 134 public struct/enum definitions across all crates
- ~71 already have `Serialize` derive
- ~63 need `Serialize` added (many are in toolpath.rs, perimeter.rs, support types)
- `SliceResult` (the main output type) does NOT derive Serialize currently
- `FeatureType`, `ToolpathSegment`, `LayerToolpath` do NOT derive Serialize

**Implementation approach:**
```rust
// In slicecore-engine/src/engine.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceResult {
    // ... existing fields ...
}

// New: structured output module
pub mod output {
    use super::*;

    pub fn to_json(result: &SliceResult) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(result)
    }

    pub fn to_msgpack(result: &SliceResult) -> Result<Vec<u8>, rmp_serde::encode::Error> {
        rmp_serde::to_vec(result)
    }
}
```

### Pattern 2: Event System (API-05)

**What:** A pub/sub event system for progress, warnings, and errors during slicing.
**When to use:** Long-running operations (Engine::slice) that consumers need to monitor.

**Design (from designDocs/03-API-DESIGN.md Section 8):**
```rust
/// All events emitted by the slicing engine.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum SliceEvent {
    StageChanged { stage: SliceStage, progress: f32 },
    LayerComplete { layer: usize, total: usize, z: f64 },
    Warning { message: String, layer: Option<usize> },
    Error { message: String },
    PerformanceMetric { stage: String, duration_ms: u64 },
}

/// Trait for receiving events.
pub trait EventSubscriber: Send + Sync {
    fn on_event(&self, event: &SliceEvent);
}

/// Event bus for dispatching events to subscribers.
pub struct EventBus {
    subscribers: Vec<Box<dyn EventSubscriber>>,
}
```

**Implementation notes:**
- The Engine currently has no progress reporting at all
- Need to thread an `Option<&EventBus>` or `Arc<EventBus>` through the pipeline
- Start simple: emit events at layer boundaries and stage transitions
- Built-in subscribers: `CallbackSubscriber`, `NdjsonSubscriber` (for JSON Lines output)

### Pattern 3: Criterion Benchmark Suite

**What:** A benchmark suite covering the 5 key models required by SC-5.
**When to use:** CI and local performance regression detection.

```rust
// benches/slice_benchmark.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_slice_calibration_cube(c: &mut Criterion) {
    let mesh = load_test_mesh("calibration_cube.stl");
    let config = PrintConfig::default();
    let engine = Engine::new(config);

    c.bench_function("slice_calibration_cube", |b| {
        b.iter(|| engine.slice(&mesh))
    });
}

criterion_group!(benches,
    bench_slice_calibration_cube,
    bench_slice_benchy,
    bench_slice_complex,
    bench_slice_thin_wall,
    bench_slice_overhang,
);
criterion_main!(benches);
```

### Pattern 4: Golden File Tests

**What:** Compare G-code output against known-good reference files.
**When to use:** Regression detection for output correctness.

```rust
#[test]
fn golden_calibration_cube() {
    let mesh = load_test_mesh("calibration_cube.stl");
    let config = PrintConfig::default();
    let engine = Engine::new(config);
    let result = engine.slice(&mesh).unwrap();

    let expected = include_str!("../golden/calibration_cube.gcode.expected");
    // Compare semantically, not byte-for-byte (ignore comments, whitespace)
    assert_gcode_equivalent(&String::from_utf8_lossy(&result.gcode), expected);
}
```

**Important:** Golden file comparison must be semantic, not byte-exact. Compare commands and parameters, not comment lines or whitespace.

### Pattern 5: CI Matrix Expansion

**What:** Multi-platform CI covering macOS (ARM+x86), Linux (ARM+x86), Windows (x86).
**Current state:** CI only runs on `ubuntu-latest` (single platform).

```yaml
# Expanded CI matrix
jobs:
  test:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            use_cross: true
          - os: macos-latest        # ARM (M-series)
            target: aarch64-apple-darwin
          - os: macos-13            # x86_64
            target: x86_64-apple-darwin
          - os: windows-latest
            target: x86_64-pc-windows-msvc
```

**Note on ARM targets:**
- macOS ARM: GitHub Actions `macos-latest` runs on M1/M2 (aarch64-apple-darwin natively)
- Linux ARM: Requires `cross` for aarch64-unknown-linux-gnu (cross-compilation on x86 runner)
- Windows ARM: GitHub Actions does not yet offer ARM Windows runners. Cross-compilation with `--target aarch64-pc-windows-msvc` is possible but has limited toolchain support. **Recommend deferring Windows ARM to a separate validation step.**

### Anti-Patterns to Avoid

- **Byte-for-byte G-code comparison in golden tests:** G-code comments, timestamps, and floating-point formatting can change between builds. Compare commands semantically.
- **Running benchmarks in CI for absolute performance claims:** CI runners have variable performance. Use benchmarks for *regression detection* only, not for claiming "beats C++". Absolute performance comparison requires dedicated hardware.
- **Adding `Serialize` to types with lifetimes or references:** Some internal types borrow data. Only add Serialize to owned types that cross the API boundary. Internal pipeline types don't need it.
- **Forcing all crates to compile for `wasm32-unknown-unknown`:** The `slicecore-plugin` (wasmtime), `slicecore-ai` (reqwest/tokio), and `slicecore-cli` (clap) crates have dependencies that cannot compile to WASM. Exclude them from WASM builds.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| MessagePack serialization | Custom binary serializer | `rmp-serde` | Format spec is complex; edge cases around integers, maps, arrays. 63M downloads prove reliability. |
| Coverage measurement | Manual test counting | `cargo-tarpaulin` or `cargo-llvm-cov` | Line coverage requires instrumentation; manual counts are inaccurate. |
| Benchmark statistics | Manual timing loops | `criterion` | Statistical rigor (warmup, outlier detection, confidence intervals) prevents false performance claims. |
| Fuzz testing harness | Random byte generation | `cargo-fuzz` (libFuzzer) | Coverage-guided fuzzing finds edge cases that random testing cannot. libFuzzer has decades of engineering. |
| Cross-platform CI | Manual platform scripts | GitHub Actions matrix + `cross` | The matrix strategy handles combinatorial explosion of OS/arch/target. |
| WASM-JS bridge | Manual FFI glue | `wasm-bindgen` | Handles memory management, string conversion, error propagation between WASM and JS. |

**Key insight:** Phase 9 is about production readiness, not novel engineering. Every tool listed above is a mature, battle-tested solution. The engineering effort should go into *integration and configuration*, not building custom alternatives.

## Common Pitfalls

### Pitfall 1: getrandom WASM Compilation Failure

**What goes wrong:** `cargo build --target wasm32-unknown-unknown` fails with `cannot find function inner_u64 in module backends` in the `getrandom` crate.
**Why it happens:** `getrandom` 0.3 requires explicit configuration for `wasm32-unknown-unknown` because the target name alone doesn't indicate which randomness source is available. The `boostvoronoi` crate (used for Arachne perimeters) depends on `ahash` which depends on `getrandom`.
**How to avoid:** Add to `.cargo/config.toml`:
```toml
[target.wasm32-unknown-unknown]
rustflags = ["-C", "opt-level=s", "--cfg", "getrandom_backend=\"wasm_js\""]
```
And add `getrandom` as a dependency with the `wasm_js` feature for the WASM target:
```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
```
**Warning signs:** Any dependency that transitively pulls in `getrandom` 0.3 will cause this on `wasm32-unknown-unknown`.

### Pitfall 2: Coverage Measurement Limitations

**What goes wrong:** `cargo-tarpaulin` reports inaccurate coverage on complex pattern matches, async code, or macro-generated code. Also, it only works reliably on Linux x86_64 with the ptrace engine.
**Why it happens:** ptrace-based instrumentation has inherent limitations with certain code patterns. The llvm engine is more accurate but slower.
**How to avoid:** Use `--engine llvm` for authoritative coverage numbers. Accept that some macro-generated code will appear uncovered. Focus on meaningful coverage (algorithms, error paths) rather than chasing 100%.
**Warning signs:** Coverage numbers fluctuate between runs; untested code appears as covered.

### Pitfall 3: Benchmark Suite Model Selection

**What goes wrong:** Benchmarks use only trivial models (tiny cube) or only extreme models (million-triangle organic), making results unrepresentative of real workloads.
**Why it happens:** Test model selection is ad hoc; no systematic representation of real print jobs.
**How to avoid:** SC-5 requires 5 models. Select for diversity:
1. **Calibration cube** (simple, fast, baseline)
2. **Benchy** (moderate complexity, overhangs, bridges -- industry standard benchmark)
3. **Complex organic model** (~50-100K triangles, tests BVH and contour chaining)
4. **Thin-wall model** (tests Arachne variable-width perimeters)
5. **Multi-region model** (tests supports, infill density variation)

**Warning signs:** All benchmarks complete in <100ms (too simple) or all take >60s (too complex for CI).

### Pitfall 4: SliceResult Serialization Breaking the API

**What goes wrong:** Adding `Serialize` to `SliceResult` requires all nested types to also derive `Serialize`. If any nested type contains non-serializable fields (file handles, function pointers, `OnceLock`, etc.), compilation fails.
**Why it happens:** Serde derives are transitive -- every field type must implement Serialize.
**How to avoid:** Audit the type graph from `SliceResult` down before adding derives. The `gcode: Vec<u8>` field is already serializable. The `preview: Option<SlicePreview>` already derives Serialize. The `time_estimate: PrintTimeEstimate` and `filament_usage: FilamentUsage` already derive Serialize. The main concern is if any future fields contain non-serializable types.
**Warning signs:** Cascade of compilation errors when adding `#[derive(Serialize)]` to a top-level type.

### Pitfall 5: Windows and macOS CI Build Failures from Unix-Specific Code

**What goes wrong:** Code compiles on Linux but fails on Windows (path separators, missing `libc` calls, case-insensitive filesystem issues) or macOS (different linker behavior, framework requirements).
**Why it happens:** Development and all testing has been Linux-only until now.
**How to avoid:** Run the CI matrix *before* committing to fixing all issues. Identify platform-specific code with `#[cfg(unix)]` or `#[cfg(windows)]` guards. The codebase appears to have minimal platform-specific code (pure algorithm focus), so issues are likely limited to filesystem paths in tests and the CLI.
**Warning signs:** Tests that create temp files with hardcoded `/tmp/` paths, or use `std::env::consts` assumptions.

## Code Examples

### JSON Structured Output for CLI

```rust
// In slicecore-cli/src/main.rs
#[derive(Subcommand)]
enum Commands {
    Slice {
        input: PathBuf,
        #[arg(short, long)]
        config: Option<PathBuf>,
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Output slicing metadata as JSON
        #[arg(long)]
        json: bool,
        /// Output slicing metadata as MessagePack
        #[arg(long)]
        msgpack: bool,
    },
}

// In the slice handler:
if json {
    let metadata = SliceMetadata {
        layer_count: result.layer_count,
        estimated_time: result.time_estimate.clone(),
        filament: result.filament_usage.clone(),
    };
    println!("{}", serde_json::to_string_pretty(&metadata).unwrap());
}
```

### Fuzz Target for STL Parser

```rust
// fuzz/fuzz_targets/fuzz_stl_binary.rs
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Should never panic, even on malformed input
    let _ = slicecore_fileio::load_mesh(data);
});
```

### Event System Integration

```rust
// In slicecore-engine/src/event.rs
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum SliceEvent {
    StageChanged { stage: String, progress: f32 },
    LayerComplete { layer: usize, total: usize, z: f64 },
    Warning { message: String },
    Complete { layers: usize, time_seconds: f64 },
}

pub trait EventSubscriber: Send + Sync {
    fn on_event(&self, event: &SliceEvent);
}

pub struct EventBus {
    subscribers: Vec<Box<dyn EventSubscriber>>,
}

impl EventBus {
    pub fn new() -> Self { Self { subscribers: Vec::new() } }

    pub fn subscribe(&mut self, sub: Box<dyn EventSubscriber>) {
        self.subscribers.push(sub);
    }

    pub fn emit(&self, event: &SliceEvent) {
        for sub in &self.subscribers {
            sub.on_event(event);
        }
    }
}

// Callback subscriber for simple use cases
pub struct CallbackSubscriber<F: Fn(&SliceEvent) + Send + Sync>(pub F);

impl<F: Fn(&SliceEvent) + Send + Sync> EventSubscriber for CallbackSubscriber<F> {
    fn on_event(&self, event: &SliceEvent) { (self.0)(event); }
}
```

### getrandom WASM Fix

```toml
# .cargo/config.toml
[build]
incremental = true

[target.wasm32-unknown-unknown]
rustflags = ["-C", "opt-level=s", "--cfg", "getrandom_backend=\"wasm_js\""]
```

```toml
# In workspace Cargo.toml or slicecore-engine/Cargo.toml
[target.'cfg(all(target_arch = "wasm32", target_os = "unknown"))'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `getrandom` 0.2 `js` feature | `getrandom` 0.3 `getrandom_backend` cfg flag | 2024-2025 | Breaking change for all WASM projects using ahash/rand. Must use new config mechanism. |
| `cargo-tarpaulin` ptrace only | `cargo-tarpaulin --engine llvm` | 2023+ | llvm engine is more accurate and works on more platforms (macOS, ARM). |
| `criterion` single crate | `criterion` with workspace benchmarks | Ongoing | Workspace-level benchmarks can test cross-crate performance (full pipeline). |
| `wasm32-wasi` target | `wasm32-wasip1` and `wasm32-wasip2` | 2024+ | The `wasm32-wasi` target is being renamed. Use `wasm32-wasip2` for Component Model support. Already configured in this repo. |

**Deprecated/outdated:**
- `wasm32-wasi` target name: Use `wasm32-wasip1` or `wasm32-wasip2` instead
- `getrandom` 0.2 `js` feature: Replaced by `getrandom_backend` cfg in 0.3+
- `cargo-tarpaulin` ptrace engine on non-x86_64: Use `--engine llvm` instead

## Codebase Audit Results

### Rustdoc Status

- **Total rustdoc warnings:** 6 (4 in slicecore-engine, 2 in slicecore-mesh)
  - `Engine::with_plugin_registry` - broken intra-doc link (method doesn't exist with that name)
  - `GcodeWriter` - unresolved link in gcode_gen.rs
  - `PrintConfig` - unresolved link in polyhole.rs
  - Missing escape in engine lib doc
  - `[1]` and `[2]` - unescaped brackets in repair/normals.rs
- **Public items without doc comments:** 0 non-module items are undocumented
- **Public modules without doc comments:** ~40 `pub mod` declarations lack module-level docs

### Serialization Readiness

| Type | Has Serialize | Needs It | Notes |
|------|--------------|----------|-------|
| `SliceResult` | No | Yes | Main output type. All fields are serializable. |
| `FeatureType` | No | Yes | Enum used in toolpath segments. Trivial to add. |
| `ToolpathSegment` | No | Yes | Struct with Point2 (already serializable), f64 fields. |
| `LayerToolpath` | No | Yes | Vec of ToolpathSegment + metadata. |
| `ContourPerimeters` | No | Maybe | Internal type; may not need external serialization. |
| `PrintConfig` | Yes | -- | Already fully serializable (TOML + serde). |
| `PrintTimeEstimate` | Yes | -- | Already serializable. |
| `FilamentUsage` | Yes | -- | Already serializable. |
| `SlicePreview` | Yes | -- | Already serializable (JSON visualization). |
| `SupportResult` | No | Maybe | May want to expose support metadata. |

### WASM Compilation Status

| Target | Status | Blocker |
|--------|--------|---------|
| `wasm32-wasip2` | **Compiles** | None |
| `wasm32-unknown-unknown` | **Fails** | `getrandom` 0.3 in `boostvoronoi` -> `ahash` chain |
| Crates excluded from WASM | `slicecore-plugin`, `slicecore-plugin-api`, `slicecore-cli`, `slicecore-ai` | `wasmtime`, `reqwest`, `clap`, `tokio` are not WASM-compatible |

### Test Coverage Baseline

| Crate | Test Functions | Status |
|-------|---------------|--------|
| `slicecore-engine` | 509 | Heaviest test coverage |
| `slicecore-math` | 128 | Well tested |
| `slicecore-geo` | 107 | Well tested |
| `slicecore-gcode-io` | 97 | Well tested |
| `slicecore-ai` | 76 | Well tested |
| `slicecore-mesh` | 62 | Moderate |
| `slicecore-plugin` | 47 | Moderate |
| `slicecore-fileio` | 46 | Moderate |
| `slicecore-slicer` | 24 | Needs more tests |
| `slicecore-plugin-api` | 15 | Light |
| `slicecore-cli` | 0 | No tests |
| **Total** | **1,111** | |

**Note:** Actual line coverage percentage is unknown until `cargo-tarpaulin` is run. 1,111 test functions across 48,800 lines is a good ratio, but test count does not equal coverage.

### CI Current State

The existing CI (`ci.yml`) has 5 jobs, all on `ubuntu-latest`:
1. `check` - `cargo check --workspace`
2. `test` - `cargo test --workspace`
3. `clippy` - `cargo clippy --workspace`
4. `fmt` - `cargo fmt --all --check`
5. `wasm` - `cargo build --target wasm32-unknown-unknown` (excluding plugin/plugin-api)

**Missing from CI:**
- macOS (ARM and x86) builds/tests
- Windows builds/tests
- Linux ARM builds
- Coverage reporting
- Benchmark regression detection
- WASM test execution
- Fuzz testing (even short runs)
- `cargo doc --no-deps` warning check

## Open Questions

1. **Performance baseline against C++ libslic3r**
   - What we know: SC-5 requires "matches or beats C++ libslic3r on a benchmark suite of 5 models" and "memory usage at or below 80% of C++ libslic3r."
   - What's unclear: We need C++ baseline numbers. The C++ analysis data is in `~/slicer-analysis/analysis/` but we haven't established concrete benchmark numbers to compare against.
   - Recommendation: Create the benchmark suite first, then measure C++ libslic3r separately. If C++ baselines aren't available, document methodology and run against PrusaSlicer CLI as the reference.

2. **Browser-based slicing demo scope**
   - What we know: SC-4 requires "a browser-based slicing demo produces correct G-code."
   - What's unclear: How elaborate does this demo need to be? A minimal HTML page with JS that loads a WASM module and slices a hardcoded STL? Or a full interactive UI?
   - Recommendation: Minimal viable demo -- a single HTML page that loads a pre-embedded STL, calls the WASM slicer, and displays basic output (layer count, G-code size). No file upload UI needed. The point is to prove WASM works, not to build a product.

3. **Test coverage measurement on multi-platform**
   - What we know: `cargo-tarpaulin` works best on Linux x86_64. macOS and Windows support is via `--engine llvm`.
   - What's unclear: Whether the >80% target should be measured on Linux only, or averaged across platforms.
   - Recommendation: Measure coverage on Linux x86_64 with tarpaulin as the authoritative number. Other platforms verify *build and test pass*, not coverage percentage.

4. **Windows ARM availability in CI**
   - What we know: GitHub Actions does not offer Windows ARM runners. SC-3 requires Windows ARM.
   - What's unclear: Whether cross-compilation counts, or if native execution is required.
   - Recommendation: Cross-compile with `--target aarch64-pc-windows-msvc` to verify compilation. If tests need to run, use QEMU or defer to manual validation. Document the limitation.

5. **Test model licensing for CI**
   - What we know: The benchmark suite needs 5 models. Some popular models (Benchy) have specific licenses. Golden tests need reproducible models.
   - What's unclear: Which models are safe to include in the repository.
   - Recommendation: Use programmatically generated test models (cubes, cylinders, spheres) for golden tests. For benchmarks, use models with permissive licenses or generate synthetic models with controlled complexity.

## Sources

### Primary (HIGH confidence)
- Codebase analysis: Direct inspection of all 11 crates, 128 source files, 48,800 lines
- `.github/workflows/ci.yml` - Current CI configuration
- `designDocs/03-API-DESIGN.md` - Event system design (Section 8), structured output (Section 10.3)
- `.planning/REQUIREMENTS.md` - Full requirement traceability
- `Cargo.toml` (workspace + per-crate) - Dependency analysis
- `.cargo/config.toml` - Current WASM configuration
- `cargo doc --no-deps` output - 6 warnings identified
- `cargo test` output - 1,111 tests, all passing
- `cargo build --target wasm32-wasip2` - Successful compilation verified
- `cargo build --target wasm32-unknown-unknown` - Failure analyzed (getrandom)

### Secondary (MEDIUM confidence)
- [rmp-serde crates.io](https://crates.io/crates/rmp-serde) - v1.3.1, 63M downloads
- [criterion crates.io](https://crates.io/crates/criterion) - v0.5, 132M downloads
- [cargo-tarpaulin GitHub](https://github.com/xd009642/tarpaulin) - Coverage tool, Linux-primary
- [getrandom docs](https://docs.rs/getrandom) - WASM backend configuration
- [actions-rust-cross GitHub](https://github.com/houseabsolute/actions-rust-cross) - Cross-compilation CI Action
- [Rust Fuzz Book](https://rust-fuzz.github.io/book/cargo-fuzz.html) - cargo-fuzz guide

### Tertiary (LOW confidence)
- C++ performance baseline: Not yet established. Will need to run PrusaSlicer benchmarks on the same hardware used for Rust benchmarks.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- All recommended tools are mature, well-documented, and widely used in the Rust ecosystem
- Architecture: HIGH -- Patterns are straightforward (serde derives, criterion benchmarks, CI matrix expansion); no novel design needed
- Pitfalls: HIGH -- WASM/getrandom issue verified by direct reproduction; coverage limitations documented by tarpaulin maintainers
- Performance benchmarking: MEDIUM -- Methodology is clear but C++ baseline comparison requires additional setup not yet verified
- Browser demo: MEDIUM -- WASM compilation proven for wasip2, but wasm32-unknown-unknown fix needs validation

**Research date:** 2026-02-17
**Valid until:** 2026-03-17 (stable domain; tools are mature)
