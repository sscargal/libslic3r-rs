---
phase: 28-g-code-post-processing-plugin-point
plan: 01
subsystem: plugin
tags: [abi_stable, ffi, gcode, post-processing, sabi_trait, plugin-api]

requires:
  - phase: 07-plugin-system
    provides: "InfillPatternPlugin sabi_trait, PluginRegistry, InfillPluginAdapter pattern"
  - phase: 02-io-formats
    provides: "GcodeCommand enum with 23 variants"
provides:
  - "FfiGcodeCommand FFI-safe enum with all 23 GcodeCommand variants + RawGcode"
  - "GcodePostProcessorPlugin sabi_trait with process_all, process_layer, processing_mode"
  - "PostProcessorPluginMod root module for native plugin loading"
  - "Bidirectional lossless GcodeCommand <-> FfiGcodeCommand conversion"
  - "PostProcessorPluginAdapter host-side trait for uniform plugin interface"
  - "run_post_processors pipeline runner with priority ordering"
  - "PluginRegistry post-processor registration and lookup"
  - "PluginCapability::GcodePostProcessor variant"
affects: [28-02-PLAN, 28-03-PLAN]

tech-stack:
  added: []
  patterns: ["PostProcessorPluginAdapter trait mirrors InfillPluginAdapter for post-processors", "Priority-ordered pipeline runner with name tie-break"]

key-files:
  created:
    - "crates/slicecore-plugin-api/src/postprocess_types.rs"
    - "crates/slicecore-plugin-api/src/postprocess_traits.rs"
    - "crates/slicecore-plugin/src/postprocess_convert.rs"
    - "crates/slicecore-plugin/src/postprocess.rs"
  modified:
    - "crates/slicecore-plugin-api/src/metadata.rs"
    - "crates/slicecore-plugin-api/src/lib.rs"
    - "crates/slicecore-plugin/src/registry.rs"
    - "crates/slicecore-plugin/src/lib.rs"
    - "crates/slicecore-plugin/Cargo.toml"

key-decisions:
  - "FfiGcodeCommand RawGcode variant separate from Raw for plugin-generated arbitrary codes"
  - "PostProcessorPluginAdapter includes priority() for pipeline ordering"
  - "GcodePostProcessor capability manifests logged but not loaded (built-ins are v1 mechanism)"

patterns-established:
  - "PostProcessorPluginAdapter: host-side trait mirroring InfillPluginAdapter for post-processors"
  - "Pipeline runner: sort by (priority, name), pipe output to next plugin"

requirements-completed: [PLUGIN-01, PLUGIN-02]

duration: 7min
completed: 2026-03-12
---

# Phase 28 Plan 01: Post-Processor Plugin Foundation Summary

**FFI-safe FfiGcodeCommand enum, GcodePostProcessorPlugin sabi_trait, bidirectional conversion, and priority-ordered pipeline runner**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-12T17:14:03Z
- **Completed:** 2026-03-12T17:21:42Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- FfiGcodeCommand with all 23 GcodeCommand variants + RawGcode, all StableAbi-safe
- GcodePostProcessorPlugin sabi_trait with process_all, process_layer, processing_mode methods
- Lossless round-trip conversion between GcodeCommand and FfiGcodeCommand (all 23 variants verified)
- PostProcessorPluginAdapter trait and run_post_processors pipeline runner with priority ordering
- PluginRegistry extended with postprocessor registration, lookup, listing

## Task Commits

Each task was committed atomically:

1. **Task 1: FFI-safe post-processor types and trait** - `0f55ad3` (feat)
2. **Task 2: Host-side conversion, adapter, pipeline, registry** - `977b291` (feat)

## Files Created/Modified
- `crates/slicecore-plugin-api/src/postprocess_types.rs` - FfiGcodeCommand, FfiPrintConfigSnapshot, PostProcessRequest, LayerPostProcessRequest, PostProcessResult, ProcessingMode, FfiConfigParam
- `crates/slicecore-plugin-api/src/postprocess_traits.rs` - GcodePostProcessorPlugin sabi_trait, PostProcessorPluginMod root module
- `crates/slicecore-plugin-api/src/metadata.rs` - Added GcodePostProcessor variant to PluginCapability
- `crates/slicecore-plugin-api/src/lib.rs` - Module declarations and re-exports for post-processor types
- `crates/slicecore-plugin/src/postprocess_convert.rs` - gcode_to_ffi, ffi_to_gcode, commands_to_ffi, commands_from_ffi
- `crates/slicecore-plugin/src/postprocess.rs` - PostProcessorPluginAdapter trait, run_post_processors pipeline
- `crates/slicecore-plugin/src/registry.rs` - Post-processor HashMap, register/get/names/infos methods
- `crates/slicecore-plugin/src/lib.rs` - Module declarations and re-exports
- `crates/slicecore-plugin/Cargo.toml` - Added slicecore-gcode-io dependency

## Decisions Made
- FfiGcodeCommand has both Raw (maps to GcodeCommand::Raw) and RawGcode (plugin-generated arbitrary codes) variants
- PostProcessorPluginAdapter includes priority() method for pipeline ordering (lower number = earlier execution)
- GcodePostProcessor capability in manifests is logged but not loaded via discover_and_load (built-in registration is the v1 mechanism)
- Pipeline runner uses stable sort by (priority, name) for deterministic execution order

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- All foundational types and traits in place for Plan 02 (built-in post-processors)
- PluginRegistry ready to accept PostProcessorPluginAdapter registrations
- Pipeline runner ready to execute post-processors in priority order

---
*Phase: 28-g-code-post-processing-plugin-point*
*Completed: 2026-03-12*
