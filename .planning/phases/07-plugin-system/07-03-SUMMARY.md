---
phase: 07-plugin-system
plan: 03
subsystem: plugin
tags: [wasmtime, wasm, component-model, wit, sandbox, fuel-limits, wasi]

# Dependency graph
requires:
  - phase: 07-01
    provides: "FFI-safe InfillRequest/InfillResult types, PluginManifest with ResourceLimits"
  - phase: 07-02
    provides: "PluginRegistry with InfillPluginAdapter trait, discovery, native loader"
provides:
  - "WIT interface definition (wit/slicecore-plugin.wit) for WASM infill plugins"
  - "WasmInfillPlugin loader using wasmtime Component Model with sandboxed execution"
  - "SandboxConfig with configurable memory and CPU fuel limits per plugin"
  - "PluginRegistry WASM integration routing discovery to wasmtime loader"
  - "Feature-gated wasm-plugins: wasmtime dependencies optional"
affects: [07-04, 07-06, 07-07]

# Tech tracking
tech-stack:
  added: [wasmtime 41, wasmtime-wasi 41]
  patterns: [WIT Component Model for WASM plugins, fresh Store per generate() call for sandboxing, WasiCtxView for wasmtime-wasi 41 API]

key-files:
  created:
    - crates/slicecore-plugin/wit/slicecore-plugin.wit
    - crates/slicecore-plugin/src/wasm.rs
    - crates/slicecore-plugin/src/sandbox.rs
  modified:
    - crates/slicecore-plugin/Cargo.toml
    - crates/slicecore-plugin/src/lib.rs
    - crates/slicecore-plugin/src/registry.rs

key-decisions:
  - "wasmtime 41 with Component Model and cranelift features for WASM plugin loading"
  - "WasiCtxView struct with ctx + table fields for wasmtime-wasi 41 WasiView trait"
  - "wasmtime_wasi::p2::add_to_linker_sync for WASI preview 2 host function linking"
  - "Fully qualified paths for FFI types to avoid name collision with bindgen-generated types"
  - "Fresh Store per generate() call prevents cross-call resource accumulation"
  - "discover_and_load handles failed plugins gracefully (log and continue, not abort)"

patterns-established:
  - "WIT-to-Rust type conversion: flattened RVec<i64> boundary points to list<point2>, and back"
  - "Feature-gated WASM loading: cfg(feature = wasm-plugins) with clear error when disabled"
  - "Metadata caching: query_metadata uses generous fuel (10M) to call name()/description() once at load time"
  - "SandboxConfig::from_resource_limits converts manifest ResourceLimits to runtime config"

# Metrics
duration: 9min
completed: 2026-02-17
---

# Phase 7 Plan 3: WASM Plugin Loading Summary

**wasmtime 41 Component Model WASM plugin loader with WIT interface, configurable sandbox (memory + CPU fuel), and PluginRegistry integration for mixed native/WASM discovery**

## Performance

- **Duration:** 9 min
- **Started:** 2026-02-17T20:42:50Z
- **Completed:** 2026-02-17T20:52:03Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Created WIT interface (wit/slicecore-plugin.wit) defining typed infill-plugin world with Point2, InfillLine, InfillRequest, InfillResult records
- Implemented WasmInfillPlugin using wasmtime Component Model with per-call sandboxed Store creation
- Added SandboxConfig with configurable memory (default 64 MiB) and CPU fuel (default 10M instructions) limits
- Integrated WASM plugin loading into PluginRegistry discovery alongside native plugins
- Feature-gated wasmtime dependencies behind `wasm-plugins` feature with clean fallback
- 34 tests passing across all modules (sandbox, registry, native, discovery, convert)

## Task Commits

Each task was committed atomically:

1. **Task 1: WIT interface definition and wasmtime bindgen** - `93f77cd` (feat)
2. **Task 2: Sandbox configuration and registry WASM integration** - `d74bf7f` (feat)

## Files Created/Modified
- `crates/slicecore-plugin/wit/slicecore-plugin.wit` - WIT interface defining infill-plugin world with typed records
- `crates/slicecore-plugin/src/wasm.rs` - WasmInfillPlugin loader with wasmtime Component Model, type conversion, InfillPluginAdapter impl
- `crates/slicecore-plugin/src/sandbox.rs` - SandboxConfig with defaults, from_resource_limits, serde support
- `crates/slicecore-plugin/Cargo.toml` - wasmtime/wasmtime-wasi optional deps, wasm-plugins feature, serde_json dev-dep
- `crates/slicecore-plugin/src/lib.rs` - Added sandbox and wasm module declarations and re-exports
- `crates/slicecore-plugin/src/registry.rs` - sandbox_config field, builder method, WASM routing in discover_and_load

## Decisions Made
- Used wasmtime 41 (latest stable) with Component Model + cranelift features
- wasmtime-wasi 41 changed WasiView trait: ctx() returns WasiCtxView<'_> struct (not &mut WasiCtx), and table() is no longer a trait method (bundled into WasiCtxView)
- Used wasmtime_wasi::p2::add_to_linker_sync (not wasmtime_wasi::add_to_linker_sync) for WASI preview 2 linking
- Fully qualified paths (slicecore_plugin_api::InfillRequest) avoid name collision with bindgen!-generated InfillRequest type alias
- InfillPlugin::instantiate() in wasmtime 41 returns InfillPlugin directly (not (InfillPlugin, Instance) tuple)
- Changed discover_and_load from aborting on first WASM plugin to graceful per-plugin error handling

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed wasmtime 41 WasiView trait signature**
- **Found during:** Task 1 (wasm.rs compilation)
- **Issue:** Plan's WasiView impl used `fn ctx(&mut self) -> &mut WasiCtx` and `fn table()` method, but wasmtime-wasi 41 changed the trait to return `WasiCtxView<'_>` struct and removed the `table()` method
- **Fix:** Updated to `fn ctx(&mut self) -> WasiCtxView<'_> { WasiCtxView { ctx: &mut self.wasi_ctx, table: &mut self.table } }`
- **Files modified:** crates/slicecore-plugin/src/wasm.rs
- **Verification:** cargo check compiles cleanly
- **Committed in:** 93f77cd (Task 1 commit)

**2. [Rule 1 - Bug] Fixed wasmtime 41 WASI linker function path**
- **Found during:** Task 1 (wasm.rs compilation)
- **Issue:** Plan used `wasmtime_wasi::add_to_linker_sync` but in wasmtime-wasi 41 it moved to `wasmtime_wasi::p2::add_to_linker_sync`
- **Fix:** Changed all call sites to use `wasmtime_wasi::p2::add_to_linker_sync`
- **Files modified:** crates/slicecore-plugin/src/wasm.rs
- **Verification:** cargo check compiles cleanly
- **Committed in:** 93f77cd (Task 1 commit)

**3. [Rule 1 - Bug] Fixed bindgen type name collision with FFI types**
- **Found during:** Task 1 (wasm.rs compilation)
- **Issue:** bindgen! macro generates `type InfillRequest = slicecore::plugin::types::InfillRequest` at module scope, colliding with `use slicecore_plugin_api::types::InfillRequest`
- **Fix:** Removed FFI type imports from module scope; used fully qualified `slicecore_plugin_api::InfillRequest` paths throughout
- **Files modified:** crates/slicecore-plugin/src/wasm.rs
- **Verification:** cargo check compiles cleanly
- **Committed in:** 93f77cd (Task 1 commit)

**4. [Rule 1 - Bug] Fixed InfillPlugin::instantiate return type**
- **Found during:** Task 1 (wasm.rs compilation)
- **Issue:** Plan used `let (bindings, _instance) = InfillPlugin::instantiate(...)` but wasmtime 41 returns `InfillPlugin` directly (not a tuple)
- **Fix:** Changed to `let bindings = InfillPlugin::instantiate(...)`
- **Files modified:** crates/slicecore-plugin/src/wasm.rs
- **Verification:** cargo check compiles cleanly
- **Committed in:** 93f77cd (Task 1 commit)

**5. [Rule 3 - Blocking] Added serde_json dev-dependency for sandbox tests**
- **Found during:** Task 2 (sandbox serde roundtrip test)
- **Issue:** sandbox_config_serde_roundtrip test uses serde_json but it was not a dev-dependency
- **Fix:** Added `serde_json = { workspace = true }` to [dev-dependencies]
- **Files modified:** crates/slicecore-plugin/Cargo.toml
- **Verification:** cargo test passes with all 34 tests
- **Committed in:** d74bf7f (Task 2 commit)

---

**Total deviations:** 5 auto-fixed (4 bugs, 1 blocking)
**Impact on plan:** All fixes necessary to match wasmtime 41 API (plan was written against older wasmtime API conventions). No scope creep.

## Issues Encountered
- wasmtime 41 API has significant differences from earlier versions used in planning: WasiView trait changed, WASI linker function moved to p2 submodule, bindgen generates type aliases that conflict with external types, instantiate() return type simplified. All resolved by adapting to the actual 41.x API.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- WASM plugin loader ready for end-to-end testing once a WASM component plugin is compiled (07-06)
- PluginRegistry handles both native and WASM plugins through InfillPluginAdapter trait
- Engine integration (07-04) can proceed using the unified registry API
- Feature gating allows builds without wasmtime when WASM support is not needed

## Self-Check: PASSED

- All 6 key files verified on disk
- Both task commits verified: 93f77cd, d74bf7f
- 34/34 tests passing
- cargo check with all features clean
- cargo check with native-plugins only clean

---
*Phase: 07-plugin-system*
*Completed: 2026-02-17*
