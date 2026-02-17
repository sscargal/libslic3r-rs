---
phase: 04-perimeter-and-infill-completeness
plan: 06
subsystem: toolpath
tags: [scarf-joint, seam, z-ramp, perimeter, gcode, orcaslicer]

# Dependency graph
requires:
  - phase: 04-02
    provides: "Seam placement with vertex rotation (seam_index at position 0)"
provides:
  - "ScarfJointConfig with 12 OrcaSlicer-compatible parameters"
  - "apply_scarf_joint() algorithm with Z ramp and E adjustment"
  - "Per-segment Z support in G-code generation"
  - "Scarf integration in assemble_layer_toolpath pipeline"
affects: [04-07, 04-08, 04-09, 04-10]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Polygon-level post-processing: collect segments per polygon, transform, then extend main list"
    - "Per-segment Z tracking in gcode_gen with current_z delta detection"

key-files:
  created:
    - "crates/slicecore-engine/src/scarf.rs"
  modified:
    - "crates/slicecore-engine/src/config.rs"
    - "crates/slicecore-engine/src/toolpath.rs"
    - "crates/slicecore-engine/src/gcode_gen.rs"
    - "crates/slicecore-engine/src/lib.rs"

key-decisions:
  - "Scarf joint disabled by default with no impact on existing behavior"
  - "Leading ramp increases Z from seam start, trailing ramp decreases Z before seam close"
  - "E values adjusted proportionally with Z ratio (e * z/layer_z * flow_ratio)"
  - "Per-segment Z emitted in G1 only when Z changes from current_z (delta > 1e-6)"
  - "Effective scarf length capped at half perimeter length to avoid overlap"
  - "Polygon segments collected into local vec, scarf applied, then extended into main segments"

patterns-established:
  - "Per-polygon toolpath post-processing pattern for seam-level modifications"
  - "current_z tracking in gcode_gen for per-segment Z delta emission"

# Metrics
duration: 7min
completed: 2026-02-17
---

# Phase 4 Plan 6: Scarf Joint Seam Summary

**Scarf joint seam with 12 OrcaSlicer-compatible parameters, gradual Z/E ramps at perimeter seam, and per-segment Z G-code output**

## Performance

- **Duration:** 7 min
- **Started:** 2026-02-17T00:52:19Z
- **Completed:** 2026-02-17T00:59:58Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Implemented scarf joint algorithm with leading ramp (Z increases from seam start) and trailing ramp (Z decreases before seam close)
- Added ScarfJointConfig with all 12 parameters matching OrcaSlicer specification
- Integrated scarf into toolpath assembly pipeline with shell-type filtering (outer/inner/holes)
- Updated G-code generator to emit per-segment Z values when they change within a layer
- 19 new tests covering config, algorithm, toolpath integration, and G-code output

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement ScarfJointConfig and scarf algorithm** - `782e6fb` (feat)
2. **Task 2: Integrate scarf joint into toolpath and G-code pipeline** - `855f061` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/scarf.rs` - Scarf joint algorithm: apply_scarf_joint(), leading/trailing ramps, segment splitting
- `crates/slicecore-engine/src/config.rs` - ScarfJointConfig (12 params), ScarfJointType enum, TOML deserialization
- `crates/slicecore-engine/src/toolpath.rs` - Scarf integration in assemble_layer_toolpath per polygon
- `crates/slicecore-engine/src/gcode_gen.rs` - Per-segment Z tracking and emission in G1 moves
- `crates/slicecore-engine/src/lib.rs` - Module registration and re-exports

## Decisions Made
- Scarf joint disabled by default -- no impact on existing behavior or G-code output
- Leading ramp increases Z from `(layer_z - z_drop)` up to `layer_z`; trailing ramp decreases symmetrically
- E values adjusted proportionally: `e * (current_z / layer_z) * scarf_flow_ratio`
- Per-segment Z emitted in G1 only when delta from current_z exceeds 1e-6 (avoids unnecessary G-code bloat)
- Effective scarf length capped at half perimeter length to prevent ramp overlap
- Polygon segments collected into local vec before scarf application, then extended into main segments list

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Scarf joint seam fully implemented and tested
- G-code generator supports per-segment Z for any future features needing intra-layer Z changes
- Pipeline integration pattern established for polygon-level toolpath post-processing

## Self-Check: PASSED

All files verified present, all commits verified in git log.

---
*Phase: 04-perimeter-and-infill-completeness*
*Completed: 2026-02-17*
