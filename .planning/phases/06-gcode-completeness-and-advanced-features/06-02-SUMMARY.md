---
phase: 06-gcode-completeness-and-advanced-features
plan: 02
subsystem: engine
tags: [flow-control, custom-gcode, ironing, toolpath, feature-type]

# Dependency graph
requires:
  - phase: 03-vertical-slice
    provides: "ToolpathSegment, FeatureType, PrintConfig, gcode_gen pipeline"
  - phase: 04-quality-features
    provides: "InfillPattern dispatch, surface classification, generate_rectilinear_infill"
provides:
  - "PerFeatureFlow struct for per-feature extrusion multipliers"
  - "CustomGcodeHooks for layer transition and per-Z G-code injection"
  - "substitute_placeholders() for G-code template expansion"
  - "IroningConfig and generate_ironing_passes() for top surface smoothing"
  - "FeatureType::Ironing and FeatureType::PurgeTower variants"
affects: [06-gcode-completeness, 07-plugin-system, multi-material]

# Tech tracking
tech-stack:
  added: []
  patterns: [per-feature-flow-multiplier, gcode-placeholder-substitution, ironing-pass-over-top-surfaces]

key-files:
  created:
    - crates/slicecore-engine/src/flow_control.rs
    - crates/slicecore-engine/src/custom_gcode.rs
    - crates/slicecore-engine/src/ironing.rs
  modified:
    - crates/slicecore-engine/src/toolpath.rs
    - crates/slicecore-engine/src/config.rs
    - crates/slicecore-engine/src/gcode_gen.rs
    - crates/slicecore-engine/src/engine.rs
    - crates/slicecore-engine/src/lib.rs

key-decisions:
  - "Ironing reuses rectilinear infill with tight spacing (0.1mm) at 100% density and 10% flow"
  - "Ironing integrated after bridge toolpaths as final feature on top-surface layers"
  - "PerFeatureFlow uses named struct fields (not HashMap) for compile-time safety and TOML ergonomics"
  - "Custom G-code per-Z matching uses 0.001mm tolerance for floating-point Z height comparison"

patterns-established:
  - "Per-feature config: named struct with Default + serde(default) for each feature subsystem"
  - "Engine pipeline extension: new features appended after existing steps with config.X.enabled guard"

# Metrics
duration: 4min
completed: 2026-02-17
---

# Phase 6 Plan 2: Per-Feature Flow, Custom G-code, and Ironing Summary

**Per-feature flow multipliers for 13 feature types, custom G-code injection at layer transitions with placeholder substitution, and ironing pass generation for smooth top surfaces at 10% flow**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-17T18:15:15Z
- **Completed:** 2026-02-17T18:19:04Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- Per-feature flow multiplier system with 13 named fields mapping each FeatureType to an independently configurable extrusion multiplier
- Custom G-code injection hooks at before/after layer change, tool change, and specific Z heights with {layer_num}, {layer_z}, {total_layers} placeholder substitution
- Ironing pass generator that reuses rectilinear infill at tight 0.1mm spacing with 10% flow rate for smooth top surfaces
- Two new FeatureType variants (Ironing, PurgeTower) extending the toolpath type system
- Full engine pipeline integration: ironing runs after all other features on layers with top surfaces

## Task Commits

Each task was committed atomically:

1. **Task 1: Per-feature flow control and custom G-code injection** - `9af6fe5` (feat) -- committed as part of 06-03 plan due to cross-agent work merging
2. **Task 2: Ironing pass generation for top surfaces** - `535f9c5` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/flow_control.rs` - PerFeatureFlow struct with 13 feature multipliers and get_multiplier() dispatch
- `crates/slicecore-engine/src/custom_gcode.rs` - CustomGcodeHooks struct with injection points and substitute_placeholders() function
- `crates/slicecore-engine/src/ironing.rs` - IroningConfig and generate_ironing_passes() using rectilinear infill internally
- `crates/slicecore-engine/src/toolpath.rs` - Added FeatureType::Ironing and FeatureType::PurgeTower variants
- `crates/slicecore-engine/src/config.rs` - Added ironing, per_feature_flow, custom_gcode fields to PrintConfig
- `crates/slicecore-engine/src/gcode_gen.rs` - Per-feature flow multiplier application and custom G-code injection in layer generation
- `crates/slicecore-engine/src/engine.rs` - Ironing integration after bridge toolpaths on top-surface layers
- `crates/slicecore-engine/src/lib.rs` - Module declarations and re-exports for flow_control, custom_gcode, ironing

## Decisions Made
- Ironing reuses the existing rectilinear infill generator with tight spacing (0.1mm) rather than implementing a separate line generator -- reduces code duplication and leverages battle-tested polygon clipping
- PerFeatureFlow uses named struct fields rather than HashMap<String, f64> for compile-time type safety and ergonomic TOML deserialization
- Custom G-code per-Z matching uses 0.001mm tolerance (same as travel move threshold) for floating-point Z height comparison
- Ironing runs as the final toolpath step (after perimeters, infill, support, and bridges) so it smooths the complete top surface
- IroningConfig defaults to disabled (enabled=false) for backward compatibility

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Task 1 committed in 06-03 agent**
- **Found during:** Task 1 continuation
- **Issue:** Task 1 work (flow_control.rs, custom_gcode.rs, toolpath changes, config changes, gcode_gen changes) was completed in a previous session but not committed. A parallel 06-03 agent included these files in its commit to fix compilation blockers.
- **Fix:** Accepted the existing commit (9af6fe5) which correctly includes all Task 1 deliverables. Task 2 committed separately.
- **Files modified:** All Task 1 files (committed in 9af6fe5)
- **Verification:** All 379 unit tests pass, cargo clippy clean
- **Committed in:** 9af6fe5

---

**Total deviations:** 1 auto-fixed (1 blocking -- cross-agent commit merge)
**Impact on plan:** No functional impact. All deliverables present and tested. Commit attribution split across two commits rather than one per task.

## Issues Encountered
- Previous session's Task 1 work was not committed before context limit. The 06-03 plan agent incorporated the uncommitted files to resolve compilation blockers. This is a workflow timing issue, not a code issue.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Per-feature flow multipliers ready for use by multi-material and advanced feature plans
- Custom G-code hooks ready for dialect-specific macro injection
- Ironing feature complete and configurable via TOML [ironing] section
- PurgeTower FeatureType variant ready for Plan 07 multi-material support

## Self-Check: PASSED

All 8 created/modified files verified on disk. Both commits (9af6fe5, 535f9c5) verified in git log. 379 unit tests pass, cargo clippy clean.

---
*Phase: 06-gcode-completeness-and-advanced-features*
*Completed: 2026-02-17*
