---
phase: 06-gcode-completeness-and-advanced-features
plan: 08
subsystem: calibration
tags: [pressure-advance, calibration, gcode-generation, firmware-dialect]

# Dependency graph
requires:
  - phase: 06-01
    provides: "Dialect-specific pressure advance formatting (format_pressure_advance)"
provides:
  - "PA calibration pattern generator (generate_pa_calibration)"
  - "PaCalibrationConfig struct with sensible defaults"
  - "Dialect-aware PA command emission (Marlin, Klipper, RepRap, Bambu)"
affects: [calibration-tools, user-workflow]

# Tech tracking
tech-stack:
  added: []
  patterns: ["Standalone G-code generator pattern (not part of slicing pipeline)"]

key-files:
  created:
    - crates/slicecore-engine/src/calibration.rs
  modified:
    - crates/slicecore-engine/src/config.rs
    - crates/slicecore-engine/src/lib.rs

key-decisions:
  - "Proportional scaling of slow/fast sections when pattern_width != 80mm"
  - "PA value clamped to pa_end to prevent floating-point overshoot"
  - "E-values use Slic3r cross-section model matching rest of pipeline"

patterns-established:
  - "Standalone calibration generator pattern: config struct + generate function returning Vec<u8>/String"

# Metrics
duration: 3min
completed: 2026-02-17
---

# Phase 6 Plan 8: Pressure Advance Calibration Summary

**Standalone PA calibration pattern generator with dialect-specific commands, alternating slow/fast sections, and configurable PA range/step**

## Performance

- **Duration:** 3 min
- **Started:** 2026-02-17T18:35:46Z
- **Completed:** 2026-02-17T18:38:46Z
- **Tasks:** 1
- **Files modified:** 3

## Accomplishments
- PA calibration pattern generates complete G-code with varying PA values from pa_start to pa_end
- Alternating slow/fast extrusion sections (20mm/40mm/20mm scaled to pattern_width) reveal PA artifacts
- Dialect-specific PA commands: M900 K (Marlin/Bambu), SET_PRESSURE_ADVANCE (Klipper), M572 D0 S (RepRap)
- 12 tests covering config defaults, all dialects, step counts, E-value correctness, speed alternation

## Task Commits

Each task was committed atomically:

1. **Task 1: Pressure advance calibration pattern generator** - `32bc99b` (feat)

**Plan metadata:** (pending)

## Files Created/Modified
- `crates/slicecore-engine/src/calibration.rs` - PA calibration pattern generator with generate_pa_calibration and string wrapper
- `crates/slicecore-engine/src/config.rs` - Added PaCalibrationConfig struct with defaults
- `crates/slicecore-engine/src/lib.rs` - Added calibration module and re-exports

## Decisions Made
- Proportional scaling of slow/fast sections (20/40/20mm) when pattern_width differs from 80mm default
- PA value clamped to pa_end to prevent floating-point overshoot past the configured end value
- E-values computed via standard Slic3r cross-section model (same as rest of pipeline) for consistency
- Raw PA commands via format_pressure_advance (dialect.rs) rather than GcodeCommand::SetPressureAdvance to ensure dialect-specific formatting

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- PA calibration generator complete and ready for CLI integration or UI exposure
- Pattern can be extended with additional calibration types (flow rate, temperature tower, etc.)

---
*Phase: 06-gcode-completeness-and-advanced-features*
*Completed: 2026-02-17*
