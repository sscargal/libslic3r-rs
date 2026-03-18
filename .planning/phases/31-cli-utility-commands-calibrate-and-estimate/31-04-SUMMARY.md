---
phase: 31-cli-utility-commands-calibrate-and-estimate
plan: 04
subsystem: cli
tags: [calibration, gcode, flow-rate, first-layer, m221, z-offset]

requires:
  - phase: 31-01
    provides: calibrate CLI framework, common utilities, CalibrateCommand enum
provides:
  - Flow rate calibration command with M221 flow overrides per tower section
  - First layer calibration command with bed-coverage solid plate
  - generate_flow_mesh and generate_first_layer_mesh engine functions
  - inject_flow_changes_text for G-code post-processing
  - Complete set of 4 calibration types (temp, retraction, flow, first-layer)
affects: [cli-help-docs, calibration-guide]

tech-stack:
  added: []
  patterns: [M221 flow override injection, flat plate mesh generation for single-layer tests]

key-files:
  created:
    - crates/slicecore-cli/src/calibrate/flow.rs
    - crates/slicecore-cli/src/calibrate/first_layer.rs
  modified:
    - crates/slicecore-engine/src/calibrate.rs
    - crates/slicecore-cli/src/calibrate/mod.rs

key-decisions:
  - "Flow tower uses M221 S{percent} injection at Z boundaries for per-section flow rate control"
  - "First layer test generates a 0.3mm flat plate at 80% bed coverage with 100% solid infill"

patterns-established:
  - "Calibration text post-processing: inject_*_text functions modify raw G-code strings at Z boundaries"

requirements-completed: []

duration: 4min
completed: 2026-03-16
---

# Phase 31 Plan 04: Flow Rate and First Layer Calibration Summary

**Flow rate M221 stepped tower and first layer bed-coverage solid plate calibration commands with measurement instructions**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-16T17:14:44Z
- **Completed:** 2026-03-16T17:19:19Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Flow rate calibration generates stacked tower with M221 flow overrides (96%-104% default range)
- First layer calibration generates flat plate covering 80% of bed for Z-offset tuning
- Both commands include companion instruction files with measurement and adjustment guidance
- Complete set of 4 calibration types now available (temp tower, retraction, flow, first layer)

## Task Commits

Each task was committed atomically:

1. **Task 1: Flow rate calibration command** - `6a189ce` (feat)
2. **Task 2: First layer calibration command** - `4e9e42c` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/calibrate.rs` - Added generate_flow_mesh, flow_schedule, inject_flow_changes_text, generate_first_layer_mesh, extract_z_from_gcode_line
- `crates/slicecore-cli/src/calibrate/flow.rs` - Flow rate calibration CLI command with M221 injection
- `crates/slicecore-cli/src/calibrate/first_layer.rs` - First layer calibration CLI command with solid infill override
- `crates/slicecore-cli/src/calibrate/mod.rs` - Wired flow and first_layer modules, replaced stubs

## Decisions Made
- Flow tower uses M221 (set flow rate percentage) G-code command injected at Z boundaries rather than modifying extrusion amounts directly -- simpler and more firmware-compatible
- First layer plate is a simple flat box at first_layer_height (0.3mm) with config overrides for 100% infill and minimal layers -- lets the slicer's infill pattern handle the test pattern
- Kept 5mm block height for flow sections (vs 10mm for temp tower) since flow variations are subtle and more sections in less height is beneficial

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All 4 calibration commands complete (temp-tower, retraction, flow, first-layer)
- Calibration subsystem is feature-complete for phase 31

---
*Phase: 31-cli-utility-commands-calibrate-and-estimate*
*Completed: 2026-03-16*
