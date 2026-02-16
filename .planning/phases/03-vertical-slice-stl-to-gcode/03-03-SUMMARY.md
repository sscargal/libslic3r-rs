---
phase: 03-vertical-slice-stl-to-gcode
plan: 03
subsystem: engine
tags: [surface-classification, extrusion-math, toolpath, e-axis, solid-infill, nearest-neighbor]

# Dependency graph
requires:
  - phase: 01-foundation-types
    provides: "ValidPolygon, IPoint2, Point2, coord_to_mm coordinate system"
  - phase: 01-foundation-types
    provides: "polygon_difference for boolean subtraction"
  - phase: 03-vertical-slice-stl-to-gcode
    provides: "PrintConfig with filament_diameter, extrusion_multiplier, speeds"
  - phase: 03-vertical-slice-stl-to-gcode
    provides: "generate_perimeters() producing ContourPerimeters with shells"
  - phase: 03-vertical-slice-stl-to-gcode
    provides: "generate_rectilinear_infill() producing InfillLine/LayerInfill"
provides:
  - "classify_surfaces(): top/bottom solid layer detection from SliceLayer stack"
  - "SurfaceClassification with solid_regions and sparse_regions"
  - "extrusion_cross_section(): Slic3r cross-section model (rectangle + semicircular ends)"
  - "compute_e_value(): E-axis filament feed from move length and extrusion geometry"
  - "move_length(): Euclidean distance between 2D points"
  - "FeatureType enum: OuterPerimeter, InnerPerimeter, SolidInfill, SparseInfill, Skirt, Brim, Travel"
  - "ToolpathSegment: linear move with feature type, E-value, feedrate, Z"
  - "LayerToolpath: ordered segments for one layer with estimated_time_seconds()"
  - "assemble_layer_toolpath(): converts perimeters + infill into ordered toolpath segments"
affects: [03-04-gcode-planning, 03-05-gcode-pipeline, 03-06-integration]

# Tech tracking
tech-stack:
  added: []
  patterns: [slic3r-cross-section-model, nearest-neighbor-toolpath-ordering, polygon-difference-surface-detection]

key-files:
  created:
    - crates/slicecore-engine/src/surface.rs
    - crates/slicecore-engine/src/extrusion.rs
    - crates/slicecore-engine/src/toolpath.rs
  modified:
    - crates/slicecore-engine/src/lib.rs

key-decisions:
  - "Simplified surface classification for Phase 3: first N bottom layers and last N top layers fully solid"
  - "Interior surface detection via polygon_difference with adjacent layers (1-layer lookahead)"
  - "E-axis math uses Slic3r cross-section model: rectangle with semicircular ends"
  - "Nearest-neighbor heuristic for infill line ordering minimizes travel moves"
  - "Toolpath speeds stored in mm/min (config stores mm/s, converted at assembly)"
  - "Travel moves inserted between disconnected paths with 0.001mm threshold"

patterns-established:
  - "Surface classification: index-based solid for top/bottom N layers, polygon boolean for interior"
  - "Extrusion computation: cross_section * move_length / filament_area * multiplier"
  - "Toolpath assembly: perimeters first, then infill, with travel insertions"
  - "Nearest-neighbor ordering: greedy closest-endpoint selection with optional line reversal"
  - "Layer time estimation: sum of segment_length / feedrate_mm_per_sec"

# Metrics
duration: 5min
completed: 2026-02-16
---

# Phase 3 Plan 03: Surface Classification, Extrusion Math, and Toolpath Assembly Summary

**Top/bottom solid surface detection via polygon difference, Slic3r cross-section E-axis computation, and toolpath assembly with nearest-neighbor infill ordering and per-feature speed control**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-16T22:59:42Z
- **Completed:** 2026-02-16T23:05:00Z
- **Tasks:** 2
- **Files created:** 3
- **Files modified:** 1

## Accomplishments
- Surface classification correctly identifies top/bottom N layers as solid and interior layers with different adjacent geometry as having solid regions
- E-axis values match the Slic3r cross-section model (rectangle + semicircular ends) with linear scaling and multiplier support
- Toolpath assembly converts perimeters and infill into ordered segments with travel moves, per-feature speeds, and first-layer speed override
- Nearest-neighbor infill line ordering reduces travel distance by picking closest next-line start
- Layer time estimation sums segment_length / feedrate for all segments
- 24 new tests (5 surface + 9 extrusion + 10 toolpath), all 49 engine tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement surface classification and extrusion math** - `cf11509` (feat)
2. **Task 2: Define toolpath segment types and layer toolpath assembly** - `d7c0bca` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/surface.rs` - Top/bottom solid layer classification: classify_surfaces(), SurfaceClassification
- `crates/slicecore-engine/src/extrusion.rs` - E-axis math: extrusion_cross_section(), compute_e_value(), move_length()
- `crates/slicecore-engine/src/toolpath.rs` - Toolpath types and assembly: FeatureType, ToolpathSegment, LayerToolpath, assemble_layer_toolpath()
- `crates/slicecore-engine/src/lib.rs` - Added surface, extrusion, toolpath module declarations and re-exports

## Decisions Made
- Simplified surface classification for Phase 3: first N bottom layers and last N top layers are entirely solid; interior layers use polygon_difference with 1-layer lookahead for exposed surface detection
- E-axis math uses the Slic3r cross-section model (rectangle with semicircular ends), validated against known values
- Nearest-neighbor heuristic for infill line ordering: greedily picks closest line start/end, reversing line direction when the end is closer
- Toolpath assembly order: perimeters first (in wall_order per config), then infill; travel moves inserted between disconnected paths
- Speeds stored in mm/min internally (config values in mm/s multiplied by 60 at assembly time)
- First layer uses first_layer_speed for all extrusion features; subsequent layers use per-feature speeds

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed E-value expected test value**
- **Found during:** Task 1 (extrusion.rs test)
- **Issue:** Plan suggested E value of ~0.0295 for 10mm move with 0.44mm width, but correct Slic3r cross-section math gives ~0.330 (plan's comment had an arithmetic error in the filament area calculation)
- **Fix:** Corrected the expected value in the test to match the actual Slic3r formula output
- **Files modified:** crates/slicecore-engine/src/extrusion.rs
- **Committed in:** cf11509 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 test value correction)
**Impact on plan:** Trivial test fix correcting an arithmetic error in the plan's expected value. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `classify_surfaces()` identifies solid vs sparse regions for infill density selection in plan 03-04/05
- `compute_e_value()` ready for G-code emission: takes move length and config, returns filament feed
- `assemble_layer_toolpath()` produces LayerToolpath segments ready for G-code conversion
- `estimated_time_seconds()` enables layer-time-based fan control in plan 03-04
- All types re-exported at slicecore-engine crate root for ergonomic imports

---
*Phase: 03-vertical-slice-stl-to-gcode*
*Plan: 03*
*Completed: 2026-02-16*

## Self-Check: PASSED

All 4 created/modified files verified present. Both task commits (cf11509, d7c0bca) verified in git log.
