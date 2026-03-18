# Codebase Concerns

**Analysis Date:** 2026-03-18

## Tech Debt

**Unused helper functions silenced with `#[allow(dead_code)]`:**
- Issue: Four private helper functions in `profile_import.rs` (`extract_f64`, `extract_u32`, `extract_bool_from_string`, `extract_percentage`) are dead code suppressed with `#[allow(dead_code)]`. They appear to be scaffolding from early field-mapping work that was never wired up.
- Files: `crates/slicecore-engine/src/profile_import.rs:245-278`
- Impact: Dead code accumulates; callers may never discover these utilities exist.
- Fix approach: Either use them in the field-mapping table or delete them. If kept, remove the lint suppression so the compiler enforces usage.

**`_interface_speed` and `_infill_angle` computed but never used:**
- Issue: `engine.rs` computes `_interface_speed` (line 161) and `assemble_bridge_toolpath` computes `_infill_angle` (line 271) but neither is used. The bridge angle computation result is dropped immediately.
- Files: `crates/slicecore-engine/src/engine.rs:161`, `crates/slicecore-engine/src/engine.rs:271`
- Impact: Bridge infill is always rectilinear regardless of span direction — the bridge angle is computed but never applied.
- Fix approach: Apply `_infill_angle` to rotate the infill generation, or remove the computation entirely.

**`profile_verification_test.rs` at repository root:**
- Issue: A standalone `fn main()` script sits at `/home/steve/libslic3r-rs/profile_verification_test.rs` — it is not part of any crate, not in a `[[bin]]`, and writes to `/tmp/`. Appears to be a manual verification script from early development.
- Files: `profile_verification_test.rs`
- Impact: Orphaned artifact; misleads contributors about codebase structure.
- Fix approach: Delete the file, or move it to a proper integration test in `slicecore-engine/tests/`.

**`AtomicProgress` utility struct largely suppressed:**
- Issue: `AtomicProgress` in `parallel.rs` has both `dead_code` and `dead_code` allows on struct and methods. Its `percent()` method is labeled "used in tests and future parallel features" but serves no production purpose today.
- Files: `crates/slicecore-engine/src/parallel.rs:86-120`
- Impact: Noise; indicates progress reporting to callers is not fully implemented.
- Fix approach: Surface progress percentage through the `EventBus` / `EventSubscriber` pipeline, then remove the lint suppression.

**`slicecore-slicer` has no integration test directory:**
- Issue: `crates/slicecore-slicer/` has no `tests/` directory — all tests are inline `#[cfg(test)]` modules within source files. The slicer is the geometric core of the pipeline.
- Files: `crates/slicecore-slicer/src/`
- Impact: Cross-module slicing behaviors (adaptive layers, contour stitching interactions) are not tested end-to-end in isolation from the engine.
- Fix approach: Add a `crates/slicecore-slicer/tests/` directory with integration scenarios against known meshes.

**`boostvoronoi` pinned at `0.11.1`, not in workspace.dependencies:**
- Issue: `slicecore-engine/Cargo.toml` specifies `boostvoronoi = "0.11.1"` directly rather than via workspace-level dependency management. This crate is C-backed (despite the Rust wrapper) and tracks upstream Boost.Voronoi.
- Files: `crates/slicecore-engine/Cargo.toml:33`
- Impact: Version upgrades must be tracked per-crate; risk of subtle geometry regressions on version bumps.
- Fix approach: Move to `[workspace.dependencies]` and document the pinning rationale.

**`sha2` and `strsim` not in workspace.dependencies:**
- Issue: `sha2 = "0.10"` and `strsim = "0.11"` are declared only in `slicecore-engine/Cargo.toml`, breaking the workspace's single-version-of-truth pattern.
- Files: `crates/slicecore-engine/Cargo.toml:37-38`
- Impact: If another crate needs the same dependency it may diverge on version.
- Fix approach: Move both to `[workspace.dependencies]`.

---

## Performance Bottlenecks

**`closest_point_on_mesh` uses brute-force O(n) search:**
- Problem: The function in `spatial.rs` iterates all triangles to find the closest point, with an explicit TODO noting BVH acceleration is needed.
- Files: `crates/slicecore-mesh/src/spatial.rs:41-59`
- Cause: Acknowledged deferred work — "acceptable for Phase 1 as closest-point queries are not a hot path."
- Improvement path: Use the existing `BVH` to limit the candidate set; the BVH is already built lazily on first `ray_cast` call.

**371 `.clone()` calls in non-test production code:**
- Problem: `crates/slicecore-engine/src/` alone (engine.rs, profile_import.rs, profile_resolve.rs, profile_compose.rs, toolpath.rs) contains 85+ `.clone()` calls on `PrintConfig` and profile data structures, which are large nested structs.
- Files: Primarily `crates/slicecore-engine/src/engine.rs`, `crates/slicecore-engine/src/profile_import.rs`, `crates/slicecore-engine/src/profile_resolve.rs`
- Cause: `PrintConfig` is passed by value through the pipeline in several places rather than by reference.
- Improvement path: Audit clone sites; pass `&PrintConfig` in read-only pipeline stages; consider wrapping in `Arc<PrintConfig>` for the parallel slicing pass.

**301 unbounded `Vec::new()` / `HashMap::new()` allocations in engine source:**
- Problem: Many per-layer allocations in `engine.rs` use `Vec::new()` without capacity hints. In a 500-layer print each layer iterates the same growth pattern.
- Files: `crates/slicecore-engine/src/engine.rs` (28 occurrences), other engine source files
- Cause: Convenience; no layer-count-aware pre-allocation.
- Improvement path: Use `Vec::with_capacity(layer_count)` for top-level per-layer result collections; the layer count is known before the main processing loop.

---

## Fragile Areas

**`engine.rs` at 4,215 lines is a maintenance risk:**
- Files: `crates/slicecore-engine/src/engine.rs`
- Why fragile: Single file contains the parallel slicing path, bridge assembly, support toolpath assembly, plugin dispatch, post-processing pipeline, and several inline test helpers. Functions are annotated `#[allow(clippy::too_many_arguments)]` (at line 347). The parallel and sequential code paths duplicate logic.
- Safe modification: Always run the full test suite (`cargo test -p slicecore-engine`) after any change. The golden tests in `tests/golden_tests.rs` catch G-code regressions.
- Test coverage: Well-covered by integration tests, but the sheer file size means adjacent code changes risk unintended interactions.

**`config.rs` at 3,987 lines embeds all tests inline:**
- Files: `crates/slicecore-engine/src/config.rs`
- Why fragile: The test section at the end of the file (~2,500 lines of tests within the same file) makes navigation difficult and inflates compile times for the production binary.
- Safe modification: Config struct fields are `#[serde(default)]`-annotated; new fields added without defaults will silently break deserialization. Always add `#[serde(default)]` to new fields.
- Fix approach: Extract tests into `crates/slicecore-engine/tests/config_integration.rs` (a file already exists — some tests can be migrated there).

**`profile_import.rs` + `profile_import_ini.rs` total 6,200 lines of field mapping:**
- Files: `crates/slicecore-engine/src/profile_import.rs`, `crates/slicecore-engine/src/profile_import_ini.rs`
- Why fragile: Manual string-to-field mapping tables. When `PrintConfig` adds or renames a field, the mapping tables in both files must be updated manually. There is no compile-time enforcement.
- Safe modification: After adding any `PrintConfig` field, search both files for similar upstream key names and add corresponding mapping entries.
- Test coverage: `tests/integration_profile_library_bambu.rs`, `tests/integration_profile_library_ini.rs`, and `tests/integration_profile_import.rs` provide good coverage of the mapping paths.

**`slicecore-cli/src/main.rs` at 3,250 lines with 90 `process::exit` calls:**
- Files: `crates/slicecore-cli/src/main.rs`
- Why fragile: Error handling is scattered `process::exit(1)` calls interspersed with business logic. Several subcommands are handled by large functions annotated `#[allow(clippy::too_many_arguments)]` and `#[allow(clippy::too_many_lines)]`.
- Safe modification: When adding new subcommands, follow the existing pattern of delegating to a dedicated module (e.g., `csg_command.rs`, `slice_workflow.rs`). Do not extend `main.rs` further.
- Fix approach: Refactor to use a `Result<(), CliError>` return from each subcommand handler rather than inline `process::exit`.

---

## Security Considerations

**Native plugin loading uses `unsafe impl Send/Sync` on FFI types:**
- Risk: `NativeInfillPlugin` in `native.rs` wraps an `abi_stable` trait object with `unsafe impl Send` and `unsafe impl Sync`. The safety comment says "InfillPatternPlugin requires Send + Sync" but this is asserted by convention, not enforced by the type system.
- Files: `crates/slicecore-plugin/src/native.rs:34-37`
- Current mitigation: `abi_stable` does layout verification on load. The unsafe impls are justified but undocumented at the type level.
- Recommendations: Add `static_assertions::assert_impl_all!(NativeInfillPlugin: Send, Sync);` to make the invariant explicit.

**Plugin directory path is not canonicalized before traversal:**
- Risk: The plugin discovery code in `discovery.rs` reads arbitrary subdirectories from a user-supplied path without canonicalizing the path first. A symlink in the plugin directory could point outside the expected subtree.
- Files: `crates/slicecore-plugin/src/discovery.rs:32-57`
- Current mitigation: WASM plugins run in a sandboxed wasmtime store with fuel and memory limits. Native plugins execute in-process without filesystem restriction.
- Recommendations: Call `std::fs::canonicalize` on `plugin_dir` before traversal; document that native plugin loading is a trusted-context operation.

**AI API keys loaded from plain TOML config files:**
- Risk: Users configure API keys in plain-text TOML files (e.g., `provider = "anthropic"`, `api_key = "sk-ant-..."`). The `AiConfig` struct uses `SecretString` to prevent logging but the file on disk is unprotected.
- Files: `crates/slicecore-ai/src/config.rs`, `crates/slicecore-cli/src/main.rs`
- Current mitigation: `SecretString` prevents accidental debug output. No key is logged or serialized back.
- Recommendations: Document that API key files should have `chmod 600` permissions. Consider supporting environment variable key loading as an alternative to file-based keys.

**`png_encode.rs` uses `unsafe { std::slice::from_raw_parts }` for pixel cast:**
- Risk: Casts a `&[u32]` pixel buffer to `&[u8]` via raw pointer with manual length calculation.
- Files: `crates/slicecore-render/src/png_encode.rs:19`
- Current mitigation: The length multiplication (`pixels.len() * 4`) is mathematically correct for RGBA layout, but there is no assertion on alignment.
- Recommendations: Replace with `bytemuck::cast_slice::<u32, u8>(&pixels)` which performs alignment and size checks at compile time.

---

## Known Bugs / Edge Cases

**Bridge infill angle is computed but not applied:**
- Symptoms: All bridge regions use rectilinear infill regardless of the detected span direction.
- Files: `crates/slicecore-engine/src/engine.rs:271`
- Trigger: Any model with bridging overhangs.
- Workaround: None — the bridge orientation is currently always rectilinear.

**`vertices.len() as u32` truncation for large meshes:**
- Symptoms: Meshes with more than `u32::MAX` (~4 billion) vertices would produce silently wrong index values when cast with `as u32`.
- Files: `crates/slicecore-mesh/src/repair/stitch.rs:76`, `crates/slicecore-mesh/src/csg/split.rs:216`, `crates/slicecore-mesh/src/csg/boolean.rs:334`, `crates/slicecore-engine/src/engine.rs:4026`
- Trigger: Extremely high-polygon meshes (not practical for FDM, but possible for import).
- Workaround: Not an issue in practice; FDM meshes rarely exceed millions of triangles. Add a bounds check (`assert!(vertices.len() <= u32::MAX as usize)`) to `TriangleMesh::new` to surface this early.

**`expect` panics in `config_schema/json_schema.rs` assume valid state:**
- Symptoms: `expect("root must have a properties object")` and `expect("just inserted")` in `json_schema.rs` will panic if the schema generation code produces unexpected intermediate state.
- Files: `crates/slicecore-config-schema/src/json_schema.rs:33,45,93`
- Trigger: Structural changes to `PrintConfig` that produce unexpected serde output (e.g., flattened fields, enums with complex representations).
- Workaround: These are internal schema generation utilities only called at build/test time — not in the slicing hot path.

**`reqwest::Client::builder().build().expect(...)` panics on client construction failure:**
- Symptoms: If TLS initialization fails (rare in practice, possible in hardened environments), `AnthropicProvider::new` and `OllamaProvider::new` panic rather than returning an error.
- Files: `crates/slicecore-ai/src/providers/anthropic.rs:39`, `crates/slicecore-ai/src/providers/ollama.rs:38`
- Trigger: System TLS library unavailable or misconfigured.
- Workaround: Return `Result<Self, AiError>` from `new()` and propagate via `?`.

---

## Missing Critical Features

**No retry / backoff for AI provider HTTP calls:**
- Problem: All three AI providers (`AnthropicProvider`, `OpenAiProvider`, `OllamaProvider`) make a single HTTP request with no retry on failure. Rate-limit responses (HTTP 429) or transient network errors cause immediate `AiError` propagation.
- Blocks: Reliable AI-suggest usage in CI or high-frequency workflows.
- Files: `crates/slicecore-ai/src/providers/anthropic.rs`, `crates/slicecore-ai/src/providers/openai.rs`, `crates/slicecore-ai/src/providers/ollama.rs`

**Seam alignment in parallel slicing passes is approximate:**
- Problem: In the parallel slicing path (engine.rs), `previous_seam = None` is passed for all layers in Pass 1. Pass 2 corrects only layers where the seam point changed — but layers that happened to align by chance may still drift.
- Files: `crates/slicecore-engine/src/engine.rs:1273`
- Blocks: Perfect seam alignment is not guaranteed when `parallel_slicing = true` and `seam_position = Aligned`.

---

## Test Coverage Gaps

**`slicecore-slicer` has no `tests/` directory:**
- What's not tested: Cross-function integration of `slice_mesh`, `slice_mesh_adaptive`, and contour stitching with edge-case meshes (non-manifold inputs, disconnected components).
- Files: `crates/slicecore-slicer/src/`
- Risk: Geometry regressions in the slicing layer may only be caught by downstream engine tests, making root cause diagnosis harder.
- Priority: High

**`slicecore-geo` boolean operations lack fuzz testing:**
- What's not tested: The polygon union/difference operations in `crates/slicecore-geo/src/boolean.rs` (29 `.unwrap()` calls) are not exercised with randomized inputs beyond the existing unit tests.
- Files: `crates/slicecore-geo/src/boolean.rs`
- Risk: Degenerate inputs (zero-area polygons, self-intersecting contours) may panic in the `unwrap()` calls.
- Priority: Medium

**`slicecore-render` has no CI-verifiable image output tests:**
- What's not tested: The PNG thumbnail output from `png_encode.rs` is tested for byte-level correctness only — no visual regression baseline.
- Files: `crates/slicecore-render/src/png_encode.rs`, `crates/slicecore-render/tests/integration.rs`
- Risk: Color or geometry rendering changes go undetected.
- Priority: Low

**`slicecore-plugin` WASM plugin path is not covered in CI:**
- What's not tested: The `wasm-plugins` feature requires wasmtime and a compiled `.wasm` binary. The `integration_tests.rs` in `slicecore-plugin` uses mock adapters, not a real WASM execution.
- Files: `crates/slicecore-plugin/tests/integration_tests.rs`
- Risk: WASM sandboxing, fuel limits, and memory limits are not validated in the CI suite.
- Priority: Medium

---

## Scaling Limits

**`ProfileLibrary` scans full disk index on every lookup:**
- Current capacity: Works for profile libraries up to a few thousand profiles.
- Limit: `profile_library.rs` at 1,374 lines loads and indexes profiles from disk. No in-memory caching between CLI invocations. Each `list-profiles`, `search-profiles`, or `slice` command rescans.
- Scaling path: Add an in-memory cache keyed on directory mtime, or build a binary index file.
- Files: `crates/slicecore-engine/src/profile_library.rs`

---

*Concerns audit: 2026-03-18*
