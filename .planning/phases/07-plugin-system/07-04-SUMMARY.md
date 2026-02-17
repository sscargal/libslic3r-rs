---
phase: 07-plugin-system
plan: 04
subsystem: engine
tags: [plugin-integration, infill-dispatch, engine-pipeline, feature-gating, ffi-conversion]

# Dependency graph
requires:
  - phase: 07-01
    provides: "FFI-safe InfillRequest/InfillResult types for plugin communication"
  - phase: 07-02
    provides: "PluginRegistry with InfillPluginAdapter trait, regions_to_request/ffi_result_to_lines conversion"
  - phase: 07-03
    provides: "WASM plugin loading integrated into PluginRegistry"
  - phase: 04-01
    provides: "InfillPattern enum and generate_infill dispatch function"
provides:
  - "InfillPattern::Plugin(String) variant for selecting plugin-provided infill patterns"
  - "Engine with optional PluginRegistry for plugin infill dispatch"
  - "Engine::generate_infill_for_layer routing Plugin to registry, built-in to generate_infill"
  - "EngineError::Plugin variant with plugin name and error message"
  - "generate_infill takes &InfillPattern by reference (breaking Copy removal cascade)"
  - "plugins feature flag for optional slicecore-plugin dependency"
  - "plugin_dir config field for plugin discovery directory"
affects: [07-05, 07-06, 07-07]

# Tech tracking
tech-stack:
  added: []
  patterns: [cfg-gated plugin integration, by-reference enum dispatch, feature-gated optional dependency]

key-files:
  created: []
  modified:
    - crates/slicecore-engine/src/infill/mod.rs
    - crates/slicecore-engine/src/engine.rs
    - crates/slicecore-engine/src/error.rs
    - crates/slicecore-engine/src/config.rs
    - crates/slicecore-engine/src/lib.rs
    - crates/slicecore-engine/Cargo.toml
    - crates/slicecore-engine/src/support/traditional.rs
    - crates/slicecore-engine/tests/phase4_integration.rs

key-decisions:
  - "Changed InfillPattern from Copy+Clone to Clone-only (Plugin(String) is not Copy)"
  - "Changed generate_infill signature from pattern: InfillPattern to pattern: &InfillPattern by reference to avoid clone cascade"
  - "Plugin dispatch handled by Engine helper method, not by generate_infill directly (avoids threading registry through infill module)"
  - "generate_infill returns empty Vec for Plugin variant as fallback when engine doesn't intercept"
  - "Engine::generate_plugin_infill uses cfg(feature = plugins) with clear error for missing feature/registry"
  - "Solid infill (Rectilinear) bypasses plugin dispatch entirely (always built-in)"

patterns-established:
  - "By-reference enum dispatch: &InfillPattern avoids Copy requirement for String-containing variants"
  - "Feature-gated plugin integration: cfg(feature = plugins) isolates slicecore-plugin dependency"
  - "Engine helper method pattern: generate_infill_for_layer wraps dispatch logic, returning Result"
  - "Graceful plugin fallback: missing registry or feature returns EngineError::Plugin, not panic"

# Metrics
duration: 10min
completed: 2026-02-17
---

# Phase 7 Plan 4: Engine Plugin Integration Summary

**InfillPattern::Plugin(String) variant with Engine-side PluginRegistry dispatch, FFI type conversion, and cfg-gated optional slicecore-plugin dependency**

## Performance

- **Duration:** 10 min
- **Started:** 2026-02-17T20:55:21Z
- **Completed:** 2026-02-17T21:05:22Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- Extended InfillPattern enum with Plugin(String) variant for plugin-provided infill patterns
- Wired PluginRegistry into Engine with full plugin dispatch pipeline (regions_to_request -> plugin.generate -> ffi_result_to_lines -> InfillLine)
- All 509 existing tests pass with zero regression (3 new tests added)
- Engine compiles cleanly both with and without the plugins feature flag
- Plugin errors propagate as EngineError::Plugin with plugin name and descriptive message

## Task Commits

Each task was committed atomically:

1. **Task 1: Add InfillPattern::Plugin variant and generate_infill dispatch** - `7046df9` (feat)
2. **Task 2: Wire PluginRegistry into Engine pipeline** - `88ffac9` (feat)

## Files Created/Modified
- `crates/slicecore-engine/Cargo.toml` - Added plugins feature and optional slicecore-plugin dependency
- `crates/slicecore-engine/src/infill/mod.rs` - Plugin(String) variant, &InfillPattern reference parameter, Plugin fallback arm
- `crates/slicecore-engine/src/error.rs` - EngineError::Plugin variant with plugin name and message
- `crates/slicecore-engine/src/engine.rs` - Engine with optional PluginRegistry, generate_infill_for_layer helper, generate_plugin_infill dispatch, with_plugin_registry builder, 3 new tests
- `crates/slicecore-engine/src/config.rs` - plugin_dir field, SettingOverrides clone fix for non-Copy InfillPattern
- `crates/slicecore-engine/src/lib.rs` - Re-export PluginRegistry/PluginInfo/PluginKind when plugins feature enabled
- `crates/slicecore-engine/src/support/traditional.rs` - Updated generate_infill calls to pass &InfillPattern
- `crates/slicecore-engine/tests/phase4_integration.rs` - Fixed Copy->Clone for InfillPattern in test patterns

## Decisions Made
- Changed `generate_infill(pattern: InfillPattern, ...)` to `generate_infill(pattern: &InfillPattern, ...)` -- this avoids a cascading Copy removal that would require `.clone()` at every call site. By-reference is cleaner and cheaper.
- Plugin dispatch is handled by `Engine::generate_infill_for_layer`, not by modifying `generate_infill` itself. This keeps the infill module independent of the plugin system.
- Solid infill always uses Rectilinear and bypasses the plugin dispatch entirely. Only sparse infill and fallback inner_contour infill route through `generate_infill_for_layer`.
- The `generate_plugin_infill` method uses `cfg(feature = "plugins")` gating so the engine compiles without any plugin system dependency when the feature is disabled.
- Plugin(String) variant uses `#[serde(rename = "plugin")]` for clean TOML syntax: `infill_pattern = { plugin = "name" }`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed InfillPattern Copy removal cascade in test files**
- **Found during:** Task 1 (after adding Plugin(String) variant)
- **Issue:** Removing Copy from InfillPattern caused compilation errors in phase4_integration.rs where `*pattern` dereferences were used in for loops
- **Fix:** Changed `*pattern` to `pattern.clone()` and converted `for pattern in [...]` to `for pattern in &patterns` with `.clone()` where ownership needed
- **Files modified:** crates/slicecore-engine/tests/phase4_integration.rs
- **Verification:** All 17 phase4_integration tests pass
- **Committed in:** 7046df9 (Task 1 commit)

**2. [Rule 1 - Bug] Fixed SettingOverrides::merge_into for non-Copy InfillPattern**
- **Found during:** Task 1 (checking all InfillPattern usage sites)
- **Issue:** `if let Some(v) = self.infill_pattern` tried to move out of `&self` since InfillPattern is no longer Copy
- **Fix:** Changed to `if let Some(ref v) = self.infill_pattern { config.infill_pattern = v.clone(); }`
- **Files modified:** crates/slicecore-engine/src/config.rs
- **Verification:** cargo check passes, config tests pass
- **Committed in:** 7046df9 (Task 1 commit)

**3. [Rule 1 - Bug] Fixed plugin error test using too-small mesh**
- **Found during:** Task 2 (unit test for Plugin pattern error)
- **Issue:** unit_cube (1mm) had no infill regions after perimeter generation, so Plugin dispatch was never reached
- **Fix:** Changed test to use calibration_cube_20mm (20mm) which has sufficient infill regions
- **Files modified:** crates/slicecore-engine/src/engine.rs
- **Verification:** Test correctly triggers EngineError::Plugin
- **Committed in:** 88ffac9 (Task 2 commit)

---

**Total deviations:** 3 auto-fixed (3 bugs)
**Impact on plan:** All fixes necessary for correctness. No scope creep.

## Issues Encountered
- Pre-existing WASM build failure for slicecore-engine (getrandom 0.3.4 from boostvoronoi dependency not supporting wasm32-unknown-unknown). This was already documented in 07-02-SUMMARY and is not caused by this plan. Core crates (math, geo, gcode-io) continue to build for WASM.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Engine integration complete: plugins can now provide custom infill patterns through PluginRegistry
- Example plugin (07-05) can be tested end-to-end with the engine once built
- Plugin testing plan (07-06) has all infrastructure needed for integration tests
- Documentation plan (07-07) can reference the complete plugin pipeline

## Self-Check: PASSED

- All 8 modified files verified on disk
- Both task commits verified: 7046df9, 88ffac9
- 509/509 tests passing (467 lib + 5 calibration + 5 determinism + 4 integration + 17 phase4 + 11 phase5)
- cargo check with and without plugins feature: clean
- cargo check --features plugins: clean

---
*Phase: 07-plugin-system*
*Completed: 2026-02-17*
