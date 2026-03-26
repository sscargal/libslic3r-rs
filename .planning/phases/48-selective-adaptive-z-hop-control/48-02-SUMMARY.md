---
phase: 48-selective-adaptive-z-hop-control
plan: 02
subsystem: gcode
tags: [z-hop, gcode-generation, surface-gating, motion-planning, retraction]

requires:
  - phase: 48-01
    provides: ZHopConfig, ZHopType, ZHopHeightMode, SurfaceEnforce enums, TopSolidInfill variant
provides:
  - plan_z_hop() function with 6-stage gating (enabled, surface, distance, Z-range, height, Auto resolution)
  - ZHopDecision struct with computed height, resolved type, speed, travel_angle
  - emit_z_hop_up/emit_slope_segments/emit_spiral_segments G-code emission helpers
  - Surface-gated z-hop in gcode_gen.rs Travel arm using departure feature context
affects: [48-03, gcode-generation, retraction-pipeline]

tech-stack:
  added: []
  patterns: [surface-gated z-hop planning, multi-motion-type G0 emission, departure-feature tracking]

key-files:
  created: []
  modified:
    - crates/slicecore-engine/src/planner.rs
    - crates/slicecore-engine/src/gcode_gen.rs

key-decisions:
  - "Z-hop planning separated from retraction: RetractionMove.z_hop removed, plan_z_hop() independent function"
  - "Auto type resolves to Spiral on TopSolidInfill/Ironing, Normal elsewhere"
  - "Z-hop descent is always Normal (symmetric Slope/Spiral down deferred)"
  - "Spiral radius capped at 1mm via min(1.0, z_hop * 2.0)"

patterns-established:
  - "Departure feature tracking: last_extrusion_feature variable in generate_layer_gcode()"
  - "Motion type dispatch: emit_z_hop_up() delegates to Normal/Slope/Spiral/Auto handlers"

requirements-completed: [GCODE-03]

duration: 4min
completed: 2026-03-25
---

# Phase 48 Plan 02: Z-Hop Planning and G-Code Emission Summary

**plan_z_hop() with 6-stage gating (surface/distance/Z-range/height/Auto) plus Normal/Slope/Spiral G-code emission via emit_z_hop_up()**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-25T23:05:19Z
- **Completed:** 2026-03-25T23:09:30Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Implemented plan_z_hop() with all 6 gating checks: enabled, surface enforcement, distance gate, Z-range filters, proportional height computation with min/max clamping, and Auto type resolution
- Refactored gcode_gen.rs Travel arm to use plan_z_hop() with departure feature context instead of direct config.z_hop.height
- Added 3 motion type emitters: Normal (1 G0 Z), Slope (6 diagonal G0 segments), Spiral (6 helical G0 segments)
- Removed z_hop field from RetractionMove, fully separating z-hop from retraction logic
- 33 z-hop-specific tests passing, 927 total engine tests passing

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement plan_z_hop() with ZHopDecision and gating logic** - `9192200` (feat)
2. **Task 2: Refactor gcode_gen.rs z-hop emission with motion types** - `c78e81f` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/planner.rs` - Added ZHopDecision struct, plan_z_hop() function, removed z_hop from RetractionMove, 21 z-hop tests
- `crates/slicecore-engine/src/gcode_gen.rs` - Refactored Travel arm with plan_z_hop(), added emit_z_hop_up/emit_slope_segments/emit_spiral_segments, last_extrusion_feature tracking, 6 new tests

## Decisions Made
- Z-hop planning fully separated from retraction (RetractionMove no longer carries z_hop)
- Auto type resolves to Spiral on TopSolidInfill/Ironing, Normal on all other surfaces
- Z-hop descent always uses Normal motion (symmetric Slope/Spiral descent deferred to future work)
- Spiral radius formula: min(1.0, z_hop * 2.0) -- small enough to avoid nozzle marks, capped at 1mm

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Z-hop planning and emission complete, ready for Plan 03 (VLH pipeline integration / end-to-end tests)
- All motion types implemented and tested at unit level

---
*Phase: 48-selective-adaptive-z-hop-control*
*Completed: 2026-03-25*
