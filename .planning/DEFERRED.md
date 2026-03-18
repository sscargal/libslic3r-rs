# Deferred Items

Items deferred from Phase 9 for future implementation.

## Phase 9 Deferrals

**Deferred:** 2026-02-17 during Phase 9 planning
**Reason:** Infrastructure limitations, scope management
**Priority:** Should revisit in near future

### 1. Browser-Based WASM Slicing Demo

**Requirement:** FOUND-03, ROADMAP SC-4
**Status:** WASM compilation verified (both wasm32-unknown-unknown and wasm32-wasip2), but browser demo not implemented
**What's missing:**
- Minimal HTML page with JavaScript
- wasm-bindgen bindings for slice() function
- Demo that loads a hardcoded STL, slices it, displays layer count and G-code size
**Why deferred:** WASM compilation works and is CI-verified. Full browser demo is a separate integration effort beyond core library readiness.
**Revisit:** Post-v1.0 — create examples/wasm-browser-demo/ with minimal proof-of-concept

### 2. C++ Performance Baseline Comparison

**Requirement:** FOUND-06 (performance >= C++ libslic3r), FOUND-07 (memory <= 80% of C++)
**Status:** Benchmark suite implemented, but no C++ baseline numbers established
**What's missing:**
- Run PrusaSlicer CLI on the same 5 benchmark models
- Measure timing and peak RSS memory
- Commit baseline numbers as reference data
- Add comparison logic to benchmark suite
**Why deferred:** Requires dedicated hardware setup with both PrusaSlicer and our slicer on same machine. Benchmark infrastructure is in place; comparison is manual validation step.
**Revisit:** Before v1.0 release announcement — run comparison on reference hardware, document results

### 3. Windows ARM (aarch64-pc-windows-msvc) CI Support

**Requirement:** FOUND-02 (multi-platform support for Windows ARM)
**Status:** CI covers Windows x86_64, but not ARM64
**What's missing:**
- Cross-compilation step in CI: `cargo build --target aarch64-pc-windows-msvc`
- Or native test execution (GitHub Actions doesn't offer Windows ARM runners yet)
**Why deferred:** No CI runner available. Cross-compilation is feasible but adds complexity without test execution.
**Revisit:** When GitHub Actions adds Windows ARM runners, or if user reports demand Windows ARM support — add cross-compile-only CI job

## Resolution Plan

When revisiting these items:

1. **Browser demo**: Create a new phase "WASM Examples and Demos" or add to documentation phase
2. **C++ baseline**: Schedule a "Performance Validation" milestone task before v1.0 release
3. **Windows ARM**: Monitor GitHub Actions runner availability; add cross-compile when feasible

## Phase 29 Deferrals

**Deferred:** 2026-03-13 during Phase 29-07 execution
**Reason:** Pre-existing clippy and doc lint failures across workspace from newer Rust toolchain lints

### 4. Workspace-wide Clippy Lint Cleanup

**Status:** 155 clippy errors across workspace from lints newly enforced in Rust 1.93+
**What's missing:**
- `clippy::cargo_common_metadata` -- All crates missing `package.repository`, `readme`, `keywords`, `categories` in Cargo.toml
- `clippy::float_cmp` -- `assert_eq!` on f64 in slicecore-arrange and other test files
- `clippy::type_complexity` -- Complex type parameters in slicecore-slicer, slicecore-fileio tests
- `clippy::cloned_ref_to_slice_refs` -- New lint in slicecore-slicer tests
- `rustdoc::redundant_explicit_link_target` -- Doc link warnings in slicecore-plugin-api, slicecore-render
- `rustdoc::broken_intra_doc_links` -- Unresolved cross-crate doc links
**Why deferred:** Pre-existing across entire workspace, not caused by Phase 29 changes. All Phase 29 crates (slicecore-mesh) pass clippy cleanly.
**Revisit:** Create a dedicated "Workspace Lint Cleanup" task to fix all crates at once

---

*Last updated: 2026-03-13*
