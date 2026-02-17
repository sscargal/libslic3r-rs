---
phase: 06-gcode-completeness-and-advanced-features
plan: 07
subsystem: engine
tags: [multi-material, mmu, tool-change, purge-tower, sequential-printing, collision-detection]

# Dependency graph
requires:
  - phase: 06-01
    provides: "G-code command types including ToolChange variant"
  - phase: 06-02
    provides: "Per-feature flow control and ironing infrastructure"
  - phase: 06-06
    provides: "Modifier mesh infrastructure for tool assignment"
provides:
  - "Multi-material tool change sequences (retract-park-change-prime)"
  - "Purge tower generation (dense on tool-change layers, sparse otherwise)"
  - "Sequential printing with collision detection and object ordering"
  - "ToolConfig, MultiMaterialConfig, SequentialConfig in PrintConfig"
affects: [06-09, engine-integration]

# Tech tracking
tech-stack:
  added: []
  patterns: ["clearance envelope collision detection", "tool change state machine"]

key-files:
  created:
    - "crates/slicecore-engine/src/multimaterial.rs"
    - "crates/slicecore-engine/src/sequential.rs"
  modified:
    - "crates/slicecore-engine/src/config.rs"
    - "crates/slicecore-engine/src/lib.rs"

key-decisions:
  - "Purge tower uses simple rectangular geometry with configurable width and position"
  - "XY gap for collision detection measured as bounding box edge distance (not center-to-center)"
  - "Object ordering sorts shortest-first then validates all pairs (not just consecutive)"
  - "Tool change sequence uses per-tool retraction settings from ToolConfig when available"

patterns-established:
  - "Clearance envelope model: radius + height for XY vs full-carriage collision modes"
  - "Dense/sparse tower pattern: same structure, different infill density based on tool-change flag"

# Metrics
duration: 6min
completed: 2026-02-17
---

# Phase 6 Plan 7: Multi-Material and Sequential Printing Summary

**MMU tool change sequences with retract-park-change-prime flow, purge tower generation (dense/sparse), and sequential printing with extruder clearance envelope collision detection**

## Performance

- **Duration:** 6 min
- **Started:** 2026-02-17T18:35:39Z
- **Completed:** 2026-02-17T18:41:51Z
- **Tasks:** 2
- **Files created:** 2
- **Files modified:** 2 (config.rs, lib.rs -- shared with concurrent 06-08)

## Accomplishments
- Multi-material tool change sequences generating correct retract-park-T-code-prime-wipe flow
- Purge tower maintained on every layer (dense infill on tool-change layers, sparse perimeters on non-change layers)
- Sequential printing with collision detection using extruder clearance envelope (radius + height)
- Object ordering algorithm sorting shortest-first with all-pairs validation
- Both features disabled by default via config defaults (backward compatible)
- 22 tests covering all functionality

## Task Commits

Each task was committed atomically:

1. **Task 1: Multi-material support with tool changes and purge tower** - `a16944c` (feat)
2. **Task 2: Sequential printing with collision detection** - `3d3a276` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/multimaterial.rs` - Tool change sequences, purge tower generation, tool assignment
- `crates/slicecore-engine/src/sequential.rs` - Collision detection, object ordering, sequential print planning
- `crates/slicecore-engine/src/config.rs` - ToolConfig, MultiMaterialConfig, SequentialConfig structs
- `crates/slicecore-engine/src/lib.rs` - Module declarations and re-exports

## Decisions Made
- Purge tower uses simple rectangular geometry (configurable position and width) rather than complex shapes
- XY gap between objects measured as minimum edge-to-edge bounding box distance for accurate collision detection
- Object ordering validates all pairs (not just consecutive) since a tall early object can collide with any later object
- Tool change uses per-tool retraction settings from ToolConfig when available, falls back to PrintConfig defaults
- Extrusion cross-section for purge tower uses Slic3r model: (width-height)*height + PI*(height/2)^2
- Safe Z for sequential transitions is clearance_height + 5mm margin

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

Concurrent execution with plan 06-08 resulted in shared config.rs/lib.rs modifications being committed by the other plan first. The MultiMaterialConfig and SequentialConfig types were already present in HEAD when Task 1 committed. This caused no conflicts -- the new module files (multimaterial.rs, sequential.rs) were committed cleanly as untracked additions.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Multi-material and sequential printing modules are ready for integration testing
- Plan 06-09 (integration tests) can validate both features end-to-end
- Both features use existing GcodeCommand types (ToolChange, Retract, etc.) without new dependencies

---
*Phase: 06-gcode-completeness-and-advanced-features*
*Completed: 2026-02-17*
