---
phase: 07-plugin-system
plan: 01
subsystem: plugin
tags: [abi_stable, ffi, sabi_trait, plugin-api, StableAbi, RVec, RString]

# Dependency graph
requires:
  - phase: 04-advanced-infill
    provides: "InfillLine, InfillPattern types that the FFI-safe types mirror"
provides:
  - "slicecore-plugin-api crate with FFI-safe types (InfillRequest, InfillResult, FfiInfillLine)"
  - "InfillPatternPlugin sabi_trait with name(), description(), generate() methods"
  - "InfillPluginMod RootModule for native plugin entry points"
  - "PluginMetadata, PluginManifest, PluginCapability serde types for plugin discovery"
  - "PluginError FFI-safe error type with RString message"
affects: [07-02, 07-03, 07-04, 07-05, 07-06, 07-07]

# Tech tracking
tech-stack:
  added: [abi_stable 0.11, semver 1]
  patterns: [three-crate plugin architecture, sabi_trait for FFI-safe traits, StableAbi derive for FFI types, prefix types for version-extensible modules]

key-files:
  created:
    - crates/slicecore-plugin-api/Cargo.toml
    - crates/slicecore-plugin-api/src/lib.rs
    - crates/slicecore-plugin-api/src/types.rs
    - crates/slicecore-plugin-api/src/traits.rs
    - crates/slicecore-plugin-api/src/error.rs
    - crates/slicecore-plugin-api/src/metadata.rs
  modified:
    - Cargo.toml

key-decisions:
  - "abi_stable 0.11 for FFI-safe traits and type layout verification"
  - "RVec<i64> flattened boundary encoding (not RVec<RVec<i64>>) for simplicity and StableAbi compatibility"
  - "Metadata types are plain serde (not FFI-safe) since they are parsed before plugin loading"
  - "non_local_definitions lint suppressed at crate level due to sabi_trait macro expansion limitation"

patterns-established:
  - "Three-crate pattern: slicecore-plugin-api (shared types), plugin crates (implementations), slicecore-plugin (registry/loader)"
  - "FFI-safe types: #[repr(C)] + #[derive(StableAbi)] + RVec/RString instead of Vec/String"
  - "sabi_trait for plugin traits with #[sabi(last_prefix_field)] on the last stable method"
  - "InfillPluginMod prefix type with RootModule impl for native plugin entry points"

# Metrics
duration: 5min
completed: 2026-02-17
---

# Phase 7 Plan 1: Plugin API Summary

**FFI-safe plugin API crate with abi_stable sabi_trait InfillPatternPlugin, StableAbi request/result types, and serde plugin metadata**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-17T20:20:42Z
- **Completed:** 2026-02-17T20:26:18Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Created slicecore-plugin-api crate as the shared interface between host and plugin crates
- Defined FFI-safe InfillRequest, InfillResult, FfiInfillLine types with StableAbi and RVec/RString
- Defined InfillPatternPlugin sabi_trait with name(), description(), generate() methods
- Defined InfillPluginMod RootModule for native plugin entry point discovery
- Defined PluginMetadata, PluginManifest, PluginCapability, ResourceLimits serde types
- 15 tests passing covering serde roundtrips, trait object creation, and generate invocation

## Task Commits

Each task was committed atomically:

1. **Task 1: Create slicecore-plugin-api crate with FFI-safe types** - `46f817e` (feat)
2. **Task 2: Define sabi_trait InfillPatternPlugin and RootModule** - `aeb37b8` (feat)

## Files Created/Modified
- `crates/slicecore-plugin-api/Cargo.toml` - Crate manifest with abi_stable, serde, semver deps
- `crates/slicecore-plugin-api/src/lib.rs` - Crate root with module re-exports and three-crate architecture docs
- `crates/slicecore-plugin-api/src/types.rs` - FFI-safe InfillRequest, InfillResult, FfiInfillLine (StableAbi)
- `crates/slicecore-plugin-api/src/error.rs` - FFI-safe PluginError with RString message
- `crates/slicecore-plugin-api/src/metadata.rs` - PluginMetadata, PluginManifest, PluginCapability, ResourceLimits (serde)
- `crates/slicecore-plugin-api/src/traits.rs` - InfillPatternPlugin sabi_trait, InfillPluginMod RootModule
- `Cargo.toml` - Added abi_stable and semver to workspace dependencies

## Decisions Made
- Used abi_stable 0.11 (latest stable) for FFI-safe trait objects and type layout verification at load time
- Flattened boundary encoding (RVec<i64> pairs + RVec<u32> lengths) rather than nested RVec -- simpler and guaranteed StableAbi compatible
- Metadata types (PluginMetadata, PluginManifest) use plain serde, not FFI-safe types, since they are parsed from TOML before the plugin binary is loaded
- Suppressed non_local_definitions lint at crate level because sabi_trait macro generates impl blocks in const items (known abi_stable 0.11 behavior on newer rustc)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added serde_json as dev-dependency for tests**
- **Found during:** Task 1 (metadata serde roundtrip tests)
- **Issue:** Tests used serde_json for serialization roundtrips but it was not listed as a dev-dependency
- **Fix:** Added `serde_json = { workspace = true }` to `[dev-dependencies]`
- **Files modified:** crates/slicecore-plugin-api/Cargo.toml
- **Verification:** cargo test passes with all 13 metadata tests
- **Committed in:** 46f817e (Task 1 commit)

**2. [Rule 1 - Bug] Fixed abi_stable::prelude import path**
- **Found during:** Task 2 (sabi_trait compilation)
- **Issue:** `abi_stable::prelude` does not exist in abi_stable 0.11; the research doc examples used an incorrect import
- **Fix:** Removed prelude import, used explicit imports (abi_stable::library::RootModule, abi_stable::sabi_types::version::VersionStrings, etc.)
- **Files modified:** crates/slicecore-plugin-api/src/traits.rs
- **Verification:** cargo check compiles cleanly
- **Committed in:** aeb37b8 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both fixes necessary for compilation. No scope creep.

## Issues Encountered
- The sabi_trait macro in abi_stable 0.11 generates non-local impl definitions that trigger the `non_local_definitions` lint on recent Rust compilers. This is a known upstream issue. Resolved by adding `#![allow(non_local_definitions)]` at crate level.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Plugin API crate is ready as the shared dependency for both the plugin registry (07-02) and example plugin crates (07-06)
- InfillPatternPlugin_TO and InfillPluginMod_Ref are the key types the native loader will use
- InfillRequest/InfillResult are the types the WASM host will convert to/from WIT types

---
*Phase: 07-plugin-system*
*Completed: 2026-02-17*
