---
phase: 21-g-code-analysis-and-comparison-tool
plan: 01
subsystem: analysis
tags: [gcode, parser, slicer-detection, metrics, state-machine]

# Dependency graph
requires:
  - phase: 19-slicing-summary-and-print-statistics
    provides: "Statistics computation patterns and filament weight formula"
provides:
  - "gcode_analysis module with line-by-line G-code parser state machine"
  - "Slicer auto-detection for BambuStudio, OrcaSlicer, PrusaSlicer, Slicecore"
  - "Per-layer and per-feature metric accumulation with speed stats"
  - "Header metadata extraction from BambuStudio and PrusaSlicer G-code formats"
  - "Filament weight and volume computation helpers"
affects: [21-02, 21-03, gcode-analysis-cli]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Line-by-line G-code state machine parser with BufRead streaming"
    - "Incremental weighted mean speed stats accumulation"
    - "Slicer-adaptive comment parsing (FEATURE vs TYPE prefix)"

key-files:
  created:
    - crates/slicecore-engine/src/gcode_analysis/mod.rs
    - crates/slicecore-engine/src/gcode_analysis/parser.rs
    - crates/slicecore-engine/src/gcode_analysis/slicer_detect.rs
    - crates/slicecore-engine/src/gcode_analysis/metrics.rs
  modified:
    - crates/slicecore-engine/src/lib.rs
    - crates/slicecore-engine/src/statistics.rs

key-decisions:
  - "GcodeParserState tracks absolute/relative extrusion mode separately from positioning mode"
  - "Z-hop detection uses retraction-state tracking with zhop_z sentinel"
  - "Header parsing scans both first 200 and last 100 lines for PrusaSlicer tail metadata"
  - "Arc moves (G2/G3) use chord distance approximation for metrics (acceptable for analysis)"
  - "FeatureMetrics and LayerMetrics re-exported at crate root without aliasing (no name conflict)"

patterns-established:
  - "SpeedStats incremental weighted mean: update(speed, distance) with O(1) memory"
  - "Slicer detection priority order: BambuStudio > OrcaSlicer > PrusaSlicer > Slicecore > Unknown"
  - "Feature format dispatch: BambuFeature for ; FEATURE:, PrusaType for ;TYPE:, Both for unknown"

requirements-completed: [SC-1, SC-2, SC-3, SC-4]

# Metrics
duration: 6min
completed: 2026-02-25
---

# Phase 21 Plan 01: G-code Parser Core Summary

**Line-by-line G-code state machine parser with slicer auto-detection, header metadata extraction, and per-layer/per-feature metric accumulation across BambuStudio/OrcaSlicer/PrusaSlicer/Slicecore formats**

## Performance

- **Duration:** 6 min
- **Started:** 2026-02-25T00:56:22Z
- **Completed:** 2026-02-25T01:03:07Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Built complete G-code parser state machine handling G0/G1/G2/G3/G28/G90/G91/G92/M82/M83 commands with position, feedrate, and extrusion mode tracking
- Slicer auto-detection identifies BambuStudio, OrcaSlicer, PrusaSlicer, and Slicecore from header comment patterns
- Feature annotations parsed for both `;TYPE:` (PrusaSlicer) and `; FEATURE:` (BambuStudio) formats
- Header metadata extraction from both BambuStudio and PrusaSlicer G-code comment blocks including estimated time, filament usage, layer count
- Per-layer and per-feature metrics with incremental weighted-mean speed statistics
- Retraction count/distance and Z-hop count/distance tracking
- 43 new unit tests covering parser, slicer detection, metrics, and header parsing with zero regressions (647 total tests pass)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create gcode_analysis module with parser state machine and slicer detection** - `93056c7` (feat)
2. **Task 2: Wire gcode_analysis into engine lib.rs and make filament helper public** - `fe47714` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/gcode_analysis/mod.rs` - Module root with re-exports of all gcode_analysis types
- `crates/slicecore-engine/src/gcode_analysis/parser.rs` - Line-by-line G-code parser state machine with move tracking (543 lines)
- `crates/slicecore-engine/src/gcode_analysis/slicer_detect.rs` - Slicer auto-detection from header comment patterns (165 lines)
- `crates/slicecore-engine/src/gcode_analysis/metrics.rs` - Metric accumulation structs and per-layer/per-feature aggregation (285 lines)
- `crates/slicecore-engine/src/lib.rs` - Added pub mod gcode_analysis and re-exports at crate root
- `crates/slicecore-engine/src/statistics.rs` - Made filament_mm_to_grams pub for cross-module reference

## Decisions Made
- GcodeParserState uses separate `absolute_extrusion` (M82/M83) and `absolute_positioning` (G90/G91) flags for correct E-axis interpretation
- Z-hop detection uses `in_retraction` and `zhop_z: Option<f64>` sentinel pattern to track retraction-then-Z-up sequences
- Header parsing scans both first 200 lines and last 100 lines since PrusaSlicer places some metadata at file end
- Arc moves (G2/G3) use chord distance approximation -- acceptable for analysis metrics without full arc path computation
- BambuStudio combined time line ("model printing time: 9m 48s; total estimated time: 18m 1s") handled by scanning for "total estimated time:" substring anywhere in line
- Common M-codes (M104/M109/M140/M190/M106/M107 etc.) recognized but not counted as unknown commands

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- gcode_analysis module is fully wired in and accessible from crate root
- All public types (GcodeAnalysis, SlicerType, HeaderMetadata, etc.) are importable from `slicecore_engine::`
- Ready for Plan 02 (comparison logic) and Plan 03 (CLI subcommands) to build on this foundation

---
*Phase: 21-g-code-analysis-and-comparison-tool*
*Completed: 2026-02-25*
