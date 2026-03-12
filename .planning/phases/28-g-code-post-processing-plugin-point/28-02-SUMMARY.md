---
phase: 28-g-code-post-processing-plugin-point
plan: 02
subsystem: engine
tags: [post-processing, gcode, built-in-plugins, pipeline, timelapse, pause-at-layer, fan-override]

requires:
  - phase: 28-g-code-post-processing-plugin-point
    provides: "PostProcessorPluginAdapter trait, run_post_processors pipeline runner, PluginRegistry post-processor support"
  - phase: 06-advanced-gcode
    provides: "GcodeCommand enum, arc fitting, time estimation"
provides:
  - "PostProcessConfig with serde(default) in PrintConfig"
  - "PauseAtLayerPlugin built-in post-processor"
  - "TimelapseCameraPlugin built-in post-processor"
  - "FanSpeedOverridePlugin built-in post-processor"
  - "CustomGcodeInjectionPlugin built-in post-processor"
  - "create_builtin_postprocessors factory function"
  - "Engine step 4d post-processing pipeline integration"
affects: [28-03-PLAN]

tech-stack:
  added: []
  patterns: ["Built-in post-processors implement PostProcessorPluginAdapter with PluginKind::Builtin", "Self-skip pattern: return input unchanged when unconfigured", "Post-processing at step 4d ensures time estimation reflects modifications"]

key-files:
  created:
    - "crates/slicecore-engine/src/postprocess_builtin.rs"
  modified:
    - "crates/slicecore-engine/src/config.rs"
    - "crates/slicecore-engine/src/engine.rs"
    - "crates/slicecore-engine/src/lib.rs"
    - "crates/slicecore-engine/Cargo.toml"

key-decisions:
  - "slicecore-plugin promoted from optional to required dependency (default-features=false) for PostProcessorPluginAdapter access"
  - "SpeedConfig.perimeter used as print_speed in FfiPrintConfigSnapshot (no generic 'print' speed field exists)"
  - "Post-processing runs after arc fitting and purge tower, before time estimation"
  - "Built-in plugins self-skip via empty config check rather than enabled flag per plugin"

patterns-established:
  - "Self-skip pattern: each built-in checks config emptiness and returns input unchanged"
  - "Layer tracking via Comment('Layer N at Z=...') prefix matching"
  - "PostProcessConfig at PrintConfig.post_process with serde(default) for backward compat"

requirements-completed: [ADV-04]

duration: 9min
completed: 2026-03-12
---

# Phase 28 Plan 02: Built-in Post-Processors and Engine Integration Summary

**4 built-in post-processor plugins (pause/timelapse/fan/custom-gcode) with engine pipeline step 4d integration and self-skip behavior**

## Performance

- **Duration:** 9 min
- **Started:** 2026-03-12T17:25:28Z
- **Completed:** 2026-03-12T17:34:10Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- PostProcessConfig with all features disabled by default (backward compatible)
- 4 built-in post-processors: PauseAtLayer, TimelapseCamera, FanSpeedOverride, CustomGcodeInjection
- Engine pipeline step 4d between purge tower and time estimation
- StageChanged("post_processing") event emitted with progress tracking
- 17 unit tests covering self-skip, injection, and serde roundtrip
- All 677 existing engine tests pass unchanged

## Task Commits

Each task was committed atomically:

1. **Task 1: PostProcessConfig and 4 built-in post-processor plugins** - `26dc191` (feat)
2. **Task 2: Engine pipeline integration with progress and cancellation** - `442fd1b` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/postprocess_builtin.rs` - 4 built-in post-processor plugins with factory function and tests
- `crates/slicecore-engine/src/config.rs` - PostProcessConfig, TimelapseConfig, FanOverrideRule, CustomGcodeTrigger, CustomGcodeRule types
- `crates/slicecore-engine/src/engine.rs` - run_post_processing_pipeline method and step 4d integration in both pipelines
- `crates/slicecore-engine/src/lib.rs` - Module declaration and re-export for postprocess_builtin
- `crates/slicecore-engine/Cargo.toml` - slicecore-plugin promoted to required dependency, slicecore-plugin-api added

## Decisions Made
- slicecore-plugin changed from optional (plugins feature) to required with default-features=false -- enables PostProcessorPluginAdapter without pulling in wasmtime/native-plugin loaders
- SpeedConfig has no generic "print" speed; used perimeter speed as FfiPrintConfigSnapshot.print_speed
- Layer detection uses Comment("Layer N at Z=...") prefix matching (matching gcode_gen output format)
- Built-in plugins use from_config constructor pattern for testability

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] slicecore-plugin dependency restructured**
- **Found during:** Task 1 (postprocess_builtin compilation)
- **Issue:** slicecore-plugin was optional (plugins feature only), but PostProcessorPluginAdapter trait needed for built-in plugins
- **Fix:** Made slicecore-plugin required with default-features=false; plugins feature now controls only PluginRegistry usage, not dependency availability
- **Files modified:** crates/slicecore-engine/Cargo.toml
- **Committed in:** 26dc191 (Task 1 commit)

**2. [Rule 1 - Bug] SpeedConfig field name correction**
- **Found during:** Task 2 (FfiPrintConfigSnapshot construction)
- **Issue:** Plan referenced self.config.speeds.print but SpeedConfig has no "print" field
- **Fix:** Used self.config.speeds.perimeter as the print speed equivalent
- **Files modified:** crates/slicecore-engine/src/engine.rs
- **Committed in:** 442fd1b (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both fixes necessary for correct compilation. No scope creep.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- All 4 built-in post-processors ready for integration testing in Plan 03
- Pipeline step 4d active when post_process.enabled = true
- Post-processor plugin adapter pattern ready for future external plugins

---
*Phase: 28-g-code-post-processing-plugin-point*
*Completed: 2026-03-12*
