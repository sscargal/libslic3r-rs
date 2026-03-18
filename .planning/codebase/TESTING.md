# Testing Patterns

**Analysis Date:** 2026-03-18

## Test Framework

**Runner:**
- Rust built-in test harness (`cargo test`)
- No separate test runner config files (no `jest.config`, no `vitest.config`)

**Assertion Library:**
- Rust's built-in `assert!`, `assert_eq!`, `assert_ne!`
- Custom message format: `assert!(cond, "message: {}", value)` — always include diagnostics
- Floating-point comparisons use manual epsilon checks, not `approx` crate (despite `approx` being a workspace dependency, it is not observed in tests)

**Benchmarks:**
- Criterion 0.5 with `html_reports` feature
- Config: `workspace.dependencies` in `Cargo.toml`

**Run Commands:**
```bash
cargo test                     # Run all tests
cargo test --package slicecore-engine  # Run tests for one crate
cargo test -- --nocapture      # Show println output
cargo bench                    # Run all benchmarks
cargo bench --package slicecore-engine --bench geometry_benchmark
```

## Test File Organization

**Location:** Separate `tests/` directory at the crate root (integration tests), plus inline `#[cfg(test)] mod tests` blocks within source files (unit tests).

**Naming:**
- Integration test files: `integration.rs`, `integration_tests.rs`, `csg_boolean.rs`, `determinism.rs`, `cli_plugins.rs`
- Phase-tagged tests when tied to a specific milestone: `phase4_integration.rs`, `phase34_integration.rs`, `phase33_p1_integration.rs`
- CLI test files named by subcommand: `cli_slice_profiles.rs`, `cli_calibrate.rs`, `cli_csg.rs`, `cli_output.rs`

**Structure:**
```
crates/
├── slicecore-engine/
│   ├── src/             # Inline unit tests at bottom of each module
│   ├── tests/           # Integration test files (one per concern)
│   └── benches/         # Criterion benchmark files
├── slicecore-mesh/
│   ├── src/
│   └── tests/
├── slicecore-ai/
│   └── tests/
│       ├── integration.rs
│       └── mock_provider.rs   # Shared mock module declared via `mod mock_provider;`
└── slicecore-cli/
    └── tests/           # CLI integration tests using `Command`
```

## Test Structure

**Integration test file pattern:**
```rust
//! Integration tests for [topic].
//!
//! [Description of what is verified]

// ─────────────────────────────────────────────────────────────────────────
// Test 1: [Name]
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn test_thing_does_expected_behavior() {
    // Setup
    let config = PrintConfig { field: value, ..PrintConfig::default() };
    let mesh = build_test_mesh();

    // Exercise
    let result = Engine::new(config).slice(&mesh, None).expect("slice should succeed");

    // Assert with message
    assert!(
        result.gcode.len() > 100,
        "expected non-trivial gcode, got {} bytes",
        result.gcode.len()
    );
}
```

**Unit test block pattern (inline):**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thing_under_test() {
        let p = Point2::new(1.5, 2.5);
        assert_eq!(p.x, 1.5);
    }
}
```

**Success Criteria (SC) tagging:** Integration tests explicitly label which phase SC they verify, both in the module doc comment and in test names:
```rust
//! SC1: geometry features extracted
//! SC3: deterministic output
#[test]
fn sc1_cube_geometry_features() { ... }
```

## Mocking

**Framework:** No external mock framework (mockall, etc.). Mocks are hand-written structs implementing the relevant trait.

**Pattern:**
```rust
// tests/mock_provider.rs
pub struct SmartMockProvider;

#[async_trait::async_trait]
impl AiProvider for SmartMockProvider {
    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse, AiError> {
        // Inspect request content and return deterministic response
        Ok(CompletionResponse { ... })
    }
    fn name(&self) -> &str { "smart-mock" }
}
```

**Shared mock module:** When multiple test files need the same mock, a dedicated module file is placed in `tests/` and declared with `mod mock_provider;` at the top of each test file that uses it. Example: `crates/slicecore-ai/tests/mock_provider.rs`.

**What is mocked:**
- External AI providers (no network calls in tests): `SmartMockProvider` in `crates/slicecore-ai/tests/mock_provider.rs`
- Plugin API types via in-memory struct implementations

**What is NOT mocked:**
- Mesh operations (real geometry computed)
- G-code generation (full pipeline runs)
- File I/O in integration tests (uses `tempfile::tempdir()`)

## Fixtures and Factories

**Test mesh constructors:** Each test file defines private helper functions that build standard test meshes. These are NOT shared across crates — each test file defines its own:

```rust
/// Creates a 20mm x 20mm x 20mm calibration cube mesh.
fn calibration_cube_20mm() -> TriangleMesh {
    let vertices = vec![ Point3::new(...), ... ];
    let indices = vec![ [4, 5, 6], ... ];
    TriangleMesh::new(vertices, indices).expect("calibration cube should be valid")
}
```

**Named mesh fixtures (slicecore-ai tests):**
- `simple_cube()` — 20mm cube, no overhangs
- `overhang_model()` — T-shape with significant overhang surfaces
- `thin_plate()` — 50mm x 50mm x 0.8mm, triggers `has_small_features`

All fixture constructors are documented with `///` explaining geometry intent.

**Config fixtures:** Built inline using struct update syntax:
```rust
let config = PrintConfig {
    layer_height: 0.1,
    infill_density: 0.3,
    ..PrintConfig::default()
};
```

**Temporary files:** `tempfile::tempdir()` for any test that writes to disk. Paths obtained via `.path()`, cleaned up automatically on drop.

## CLI Integration Tests

CLI tests in `crates/slicecore-cli/tests/` spawn the actual compiled binary using `std::process::Command`:

```rust
fn cli_binary() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    if path.ends_with("deps") { path.pop(); }
    path.push("slicecore");
    path
}

#[test]
fn slice_plugin_dir_flag_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(cli_binary())
        .args(["slice", stl_path, "--output", gcode_path])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(output.status.success(), "...: {}", String::from_utf8_lossy(&output.stderr));
    assert!(gcode_path.exists(), "gcode output should be created");
}
```

STL binary files are written programmatically via `write_cube_stl()` helpers in each test file.

## Coverage

**Requirements:** No enforced coverage threshold. No `tarpaulin`, `llvm-cov`, or CI coverage gate detected.

**View Coverage (manual):**
```bash
cargo llvm-cov --all-features --workspace
```

## Test Types

**Unit Tests:**
- Scope: single function, single type
- Location: `#[cfg(test)] mod tests` inline in source file
- Pattern: exhaustive per-method coverage of math types (`Point2`, `Point3`, `BBox`, etc.)
- Files: `crates/slicecore-math/src/point.rs`, `crates/slicecore-plugin/src/status.rs`, etc.

**Integration Tests:**
- Scope: full pipeline or multi-module interaction
- Location: `crates/*/tests/` directory
- Pattern: build real mesh → run pipeline → assert on G-code content, byte counts, layer counts, or validation results
- Files: `crates/slicecore-engine/tests/integration.rs`, `crates/slicecore-mesh/tests/csg_boolean.rs`, etc.

**CLI End-to-End Tests:**
- Scope: full binary execution
- Location: `crates/slicecore-cli/tests/`
- Pattern: spawn binary, check exit code, verify output file exists or stderr contents
- Files: `cli_plugins.rs`, `cli_csg.rs`, `cli_calibrate.rs`, `cli_output.rs`, etc.

**Benchmarks:**
- Framework: Criterion 0.5
- Location: `crates/*/benches/`
- Pattern: helper constructs input, `c.bench_function("name", |b| b.iter(|| ...))` with no black_box wrapping observed
- Files: `crates/slicecore-engine/benches/geometry_benchmark.rs`, `crates/slicecore-mesh/benches/csg_bench.rs`

**Fuzz Tests:**
- Location: `fuzz/` at workspace root
- Framework: cargo-fuzz (libFuzzer)

## Common Patterns

**Determinism testing:**
```rust
let result1 = Engine::new(config.clone()).slice(&mesh, None).expect("...");
let result2 = Engine::new(config).slice(&mesh, None).expect("...");
assert_eq!(result1.gcode, result2.gcode, "identical input must produce identical output");
```

**Error path testing:**
```rust
let result = mesh_intersection(&a, &b);
assert!(result.is_err(), "intersection of non-overlapping boxes should produce EmptyResult");
```

**Range/ratio testing:**
```rust
let ratio = result_full.gcode.len() as f64 / result_zero.gcode.len() as f64;
assert!(ratio >= 1.5, "100% infill ({} bytes) should be >= 1.5x 0% infill ({} bytes), ratio={:.2}",
    result_full.gcode.len(), result_zero.gcode.len(), ratio);
```

**Feature flag-gated tests:**
```rust
#[cfg(feature = "parallel")]
#[test]
fn test_parallel_sequential_determinism() { ... }
```

**Helper assertion functions** (defined per test file, not shared):
```rust
fn assert_manifold(mesh: &TriangleMesh) { ... }
fn assert_positive_volume(mesh: &TriangleMesh) { ... }
fn assert_approx(actual: f64, expected: f64, tolerance: f64, label: &str) { ... }
```

---

*Testing analysis: 2026-03-18*
