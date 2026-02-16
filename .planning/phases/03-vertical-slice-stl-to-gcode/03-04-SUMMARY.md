---
phase: 03-vertical-slice-stl-to-gcode
plan: 04
subsystem: engine
tags: [planner, skirt, brim, retraction, temperature, fan, gcode-gen, toolpath]

# Dependency graph
requires:
  - phase: 03-03
    provides: "Toolpath types (LayerToolpath, ToolpathSegment, FeatureType), extrusion math"
  - phase: 02-03
    provides: "GcodeCommand enum, GcodeWriter"
  - phase: 01-02
    provides: "ValidPolygon, offset_polygon, offset_polygons, convex_hull"
provides:
  - "Skirt generation via convex hull + outward offset"
  - "Brim generation with nozzle-width-spaced outward offsets"
  - "Retraction planning (RetractionMove) based on travel distance threshold"
  - "Temperature planning (first-layer wait, transition, no-op for later layers)"
  - "Fan control (disable_fan_first_layers, full fan_speed when enabled)"
  - "generate_layer_gcode: LayerToolpath to Vec<GcodeCommand>"
  - "generate_full_gcode: all layers to complete print body with M83/G92 preamble"
affects: [03-05, 03-06, phase-04, phase-05]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Planner functions return Vec<GcodeCommand> for composability"
    - "Retraction state tracked as &mut bool across layers"
    - "Feature type comments (TYPE:...) in G-code for readability"

key-files:
  created:
    - "crates/slicecore-engine/src/planner.rs"
    - "crates/slicecore-engine/src/gcode_gen.rs"
  modified:
    - "crates/slicecore-engine/src/lib.rs"

key-decisions:
  - "Phase 3 simplification: full fan_speed whenever fan is enabled (no proportional reduction)"
  - "Unretract at travel destination (after G0) rather than before next extrusion"
  - "Feature type comments use TYPE: prefix matching PrusaSlicer convention"
  - "Temperature planning: M190/M109 (wait) for layer 0, M140/M104 (no wait) for layer 1 transition"

patterns-established:
  - "Planner composability: plan_temperatures/plan_fan return Vec<GcodeCommand> consumed by generate_full_gcode"
  - "Retraction state threading: &mut bool passed through generate_layer_gcode across layers"
  - "G-code generation does not include start/end G-code (delegated to GcodeWriter)"

# Metrics
duration: 5min
completed: 2026-02-16
---

# Phase 3 Plan 4: Planner and G-code Generation Summary

**Skirt/brim generation via convex hull offset, retraction/temperature/fan planning, and toolpath-to-GcodeCommand conversion with retraction and Z-hop support**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-16T23:07:29Z
- **Completed:** 2026-02-16T23:12:14Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Complete planner module: skirt (convex hull + offset), brim (nozzle-width loops), retraction (threshold-based), temperature (per-layer), fan control
- G-code generator converting LayerToolpath to GcodeCommand sequences with proper retraction, Z-hop, feature comments
- generate_full_gcode producing complete print body with M83 relative extrusion preamble
- 28 new tests (17 planner + 11 gcode_gen), total engine test count now 77

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement planner module** - `226bb97` (feat)
2. **Task 2: Implement G-code generation from toolpaths** - `6055d7b` (feat)

**Plan metadata:** [pending] (docs: complete plan)

## Files Created/Modified
- `crates/slicecore-engine/src/planner.rs` - Skirt/brim generation, retraction planning, temperature planning, fan control (17 tests)
- `crates/slicecore-engine/src/gcode_gen.rs` - Toolpath-to-GcodeCommand conversion with retraction/Z-hop (11 tests)
- `crates/slicecore-engine/src/lib.rs` - Module declarations, re-exports for planner and gcode_gen

## Decisions Made
- Phase 3 simplification: full fan_speed whenever fan is enabled (proportional speed reduction deferred to Phase 5)
- Unretract occurs at travel destination rather than at start of next extrusion -- matches PrusaSlicer behavior where filament primes before the next extrusion move begins
- Feature type comments use `TYPE:` prefix (e.g., `; TYPE:Outer perimeter`) matching PrusaSlicer G-code convention
- Temperature planning: wait (M190/M109) only on layer 0, no-wait (M140/M104) for layer 1 transition, nothing thereafter

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All pipeline stages now exist: slicer, perimeter, infill, surface, extrusion, toolpath, planner, gcode_gen
- Ready for 03-05 (pipeline integration / end-to-end orchestration)
- G-code output can be written via GcodeWriter from slicecore-gcode-io

---
*Phase: 03-vertical-slice-stl-to-gcode*
*Completed: 2026-02-16*
