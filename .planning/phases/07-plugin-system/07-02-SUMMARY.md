---
phase: 07-plugin-system
plan: 02
subsystem: plugin
tags: [abi_stable, plugin-registry, native-loader, discovery, toml-manifest, type-conversion, FFI]

# Dependency graph
requires:
  - phase: 07-01
    provides: "FFI-safe InfillPatternPlugin trait, InfillPluginMod RootModule, InfillRequest/InfillResult types, PluginManifest metadata"
  - phase: 01-foundation-types
    provides: "IPoint2, ValidPolygon, COORD_SCALE for type conversion"
provides:
  - "slicecore-plugin crate with PluginRegistry for discover, register, get, list operations"
  - "Native plugin loader via abi_stable with multi-path library resolution"
  - "Plugin discovery via plugin.toml manifest scanning with semver version validation"
  - "Type conversion between ValidPolygon/IPoint2 and FFI-safe InfillRequest/InfillResult"
  - "CI WASM build exclusion for plugin crates"
affects: [07-03, 07-04, 07-05, 07-06, 07-07]

# Tech tracking
tech-stack:
  added: [toml 0.8]
  patterns: [host-side plugin registry, multi-path library resolution, cfg-gated WASM exclusion]

key-files:
  created:
    - crates/slicecore-plugin/Cargo.toml
    - crates/slicecore-plugin/src/lib.rs
    - crates/slicecore-plugin/src/error.rs
    - crates/slicecore-plugin/src/registry.rs
    - crates/slicecore-plugin/src/native.rs
    - crates/slicecore-plugin/src/discovery.rs
    - crates/slicecore-plugin/src/convert.rs
  modified:
    - Cargo.toml
    - .github/workflows/ci.yml

key-decisions:
  - "InfillPluginAdapter host-side trait wraps both native and WASM plugins uniformly"
  - "resolve_library_path searches direct, target/debug, target/release in priority order"
  - "PluginManifest.library_filename is just a filename; loader resolves full path"
  - "CI WASM build uses --exclude flags (not workspace exclude) to keep crate in workspace"
  - "PluginKind enum separate from PluginType (host-side includes Builtin variant)"

patterns-established:
  - "Multi-path library resolution: direct > target/debug > target/release for native plugins"
  - "cfg-gated modules: native.rs behind cfg(not(target_family = wasm)) for WASM compatibility"
  - "Host-side adapter trait pattern: InfillPluginAdapter wraps FFI trait objects with Rust error types"
  - "Plugin discovery by scanning one level deep for plugin.toml manifests"

# Metrics
duration: 5min
completed: 2026-02-17
---

# Phase 7 Plan 2: Plugin Registry and Native Loader Summary

**Host-side plugin registry with abi_stable native loader, manifest discovery, multi-path library resolution, and ValidPolygon-to-FFI type conversion**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-17T20:29:08Z
- **Completed:** 2026-02-17T20:34:47Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- Created slicecore-plugin crate with complete PluginRegistry supporting discover, register, get, list, and has operations
- Native plugin loader via abi_stable with library path resolution searching plugin_dir root, target/debug, and target/release directories
- Plugin discovery scanning directories for plugin.toml manifests with semver version compatibility validation
- Type conversion utilities bridging ValidPolygon/IPoint2 to FFI-safe InfillRequest/InfillResult
- CI WASM build updated to exclude plugin crates that depend on abi_stable
- 26 tests passing across all modules (registry, discovery, native path resolution, type conversion)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create slicecore-plugin crate with registry and native loader** - `658cc4e` (feat)
2. **Task 2: Type conversion utilities and CI WASM exclusion** - `31cec01` (feat)

## Files Created/Modified
- `crates/slicecore-plugin/Cargo.toml` - Crate manifest with abi_stable (cfg-gated), toml, semver, thiserror deps
- `crates/slicecore-plugin/src/lib.rs` - Crate root with module declarations and re-exports
- `crates/slicecore-plugin/src/error.rs` - PluginSystemError enum (LoadFailed, VersionIncompatible, ManifestError, ExecutionFailed, NotFound, Io)
- `crates/slicecore-plugin/src/registry.rs` - PluginRegistry with InfillPluginAdapter trait, PluginKind enum, PluginInfo struct
- `crates/slicecore-plugin/src/native.rs` - NativeInfillPlugin with resolve_library_path and load_native_plugin via abi_stable
- `crates/slicecore-plugin/src/discovery.rs` - discover_plugins scanning for plugin.toml with semver version validation
- `crates/slicecore-plugin/src/convert.rs` - regions_to_request and ffi_result_to_lines conversion functions
- `Cargo.toml` - Added toml to workspace dependencies
- `.github/workflows/ci.yml` - WASM build step excludes slicecore-plugin and slicecore-plugin-api

## Decisions Made
- Used host-side InfillPluginAdapter trait (not FFI-safe) to wrap both native and WASM plugins with uniform Rust error types
- resolve_library_path searches three candidate locations in priority order: direct (installed), target/debug (dev), target/release (prod)
- PluginManifest.library_filename stores just the filename; the loader resolves the full path at runtime
- CI WASM build uses --exclude flags rather than workspace exclude to keep plugin crates available for `cargo check -p`
- PluginKind host-side enum includes Builtin variant not present in PluginType manifest enum
- discover_and_load method is cfg-gated behind not(target_family = "wasm") since it requires the native loader

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed PluginManifest field access pattern**
- **Found during:** Task 1 (native.rs implementation)
- **Issue:** Plan referenced `manifest.plugin_type.library` but actual PluginManifest struct has `library_filename` as a top-level String field and `plugin_type` as a PluginType enum (Native/Wasm), not a struct
- **Fix:** Used `manifest.library_filename` instead of `manifest.plugin_type.library`
- **Files modified:** crates/slicecore-plugin/src/native.rs
- **Verification:** cargo check compiles cleanly
- **Committed in:** 658cc4e (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Necessary correction to match actual API types from plan 07-01. No scope creep.

## Issues Encountered
- Pre-existing WASM build failure: `cargo build --target wasm32-unknown-unknown` was already broken before this plan due to getrandom 0.3.4 not supporting wasm32-unknown-unknown (transitive dep from boostvoronoi in phase 04-09). The WASM build for core crates (math, geo, gcode-io, mesh, fileio) continues to work. The CI update correctly excludes plugin crates.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- PluginRegistry is ready for WASM plugin loading integration (07-03)
- InfillPluginAdapter trait provides the extension point for WASM loader to implement
- Type conversion utilities ready for engine integration (07-04)
- Discovery and manifest parsing ready for example plugin testing (07-06)

## Self-Check: PASSED

- All 7 created files verified on disk
- Both task commits verified: 658cc4e, 31cec01
- 26/26 tests passing
- cargo check, cargo clippy, cargo fmt all clean

---
*Phase: 07-plugin-system*
*Completed: 2026-02-17*
