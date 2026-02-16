---
phase: 02-mesh-io-and-repair
plan: 03
subsystem: gcode-io
tags: [gcode, marlin, klipper, reprap, bambu, writer, validator, firmware-dialect]

# Dependency graph
requires:
  - phase: 01-foundation-types
    provides: "slicecore-math crate with core geometric types"
provides:
  - "slicecore-gcode-io crate with structured G-code command types (GcodeCommand enum)"
  - "Dialect-aware GcodeWriter supporting Marlin, Klipper, RepRapFirmware, and Bambu"
  - "G-code validator (validate_gcode) checking syntax, coordinates, feedrate, and temperature"
  - "Start/end G-code sequences for all 4 firmware dialects"
affects: [03-vertical-slice, phase-04, phase-05]

# Tech tracking
tech-stack:
  added: [thiserror, serde]
  patterns: [enum-based-command-types, dialect-dispatch, write-trait-generic-output]

key-files:
  created:
    - crates/slicecore-gcode-io/Cargo.toml
    - crates/slicecore-gcode-io/src/lib.rs
    - crates/slicecore-gcode-io/src/error.rs
    - crates/slicecore-gcode-io/src/commands.rs
    - crates/slicecore-gcode-io/src/dialect.rs
    - crates/slicecore-gcode-io/src/writer.rs
    - crates/slicecore-gcode-io/src/marlin.rs
    - crates/slicecore-gcode-io/src/klipper.rs
    - crates/slicecore-gcode-io/src/reprap.rs
    - crates/slicecore-gcode-io/src/bambu.rs
    - crates/slicecore-gcode-io/src/validate.rs
  modified: []

key-decisions:
  - "M83 (relative extrusion) as default for all dialects -- avoids E-axis overflow, simpler math"
  - "Extrusion values use 5 decimal places (E0.12345) for sub-micron precision in volumetric calculations"
  - "Coordinates 3 decimal places, feedrate 1 decimal place, temperature 0 decimal places"
  - "Extended command support in validator (Klipper uppercase-underscore commands like TURN_OFF_HEATERS)"
  - "GcodeWriter generic over Write trait -- works with Vec<u8>, File, or any writer including WASM streams"

patterns-established:
  - "Dialect dispatch: GcodeDialect enum selects firmware-specific start/end via match in writer"
  - "Structured commands: GcodeCommand enum with Display impl instead of raw string concatenation"
  - "Validator result pattern: ValidationResult with errors (fatal) + warnings (informational)"

# Metrics
duration: 6min
completed: 2026-02-16
---

# Phase 2 Plan 3: G-code I/O Summary

**Structured G-code command types with 17-variant enum, dialect-aware writer supporting Marlin/Klipper/RepRap/Bambu, and syntax/semantic validator**

## Performance

- **Duration:** 6 min
- **Started:** 2026-02-16T21:11:57Z
- **Completed:** 2026-02-16T21:18:37Z
- **Tasks:** 2
- **Files created:** 11

## Accomplishments
- GcodeCommand enum with 17 typed variants covering movement, temperature, fan, retract, and raw pass-through
- GcodeWriter generic over std::io::Write with dialect-specific start/end sequence dispatch
- Four distinct firmware dialect implementations (Marlin, Klipper, RepRapFirmware, Bambu Lab)
- G-code validator checking line syntax, coordinate finiteness, feedrate positivity, and temperature range (0-400)
- 52 unit tests + 1 doctest all passing, WASM compilation verified

## Task Commits

Each task was committed atomically:

1. **Task 1: G-code command types, writer core, and Marlin dialect** - `5107a22` (feat)
2. **Task 2: Klipper, RepRap, Bambu dialect modules and G-code validator** - `06c5af3` (feat)

## Files Created/Modified
- `crates/slicecore-gcode-io/Cargo.toml` - Crate manifest with slicecore-math, serde, thiserror deps
- `crates/slicecore-gcode-io/src/lib.rs` - Module declarations and re-exports
- `crates/slicecore-gcode-io/src/error.rs` - GcodeError enum (IoError, InvalidFeedrate, InvalidTemperature, InvalidCoordinate, FormatError)
- `crates/slicecore-gcode-io/src/commands.rs` - GcodeCommand enum with 17 variants and Display impl
- `crates/slicecore-gcode-io/src/dialect.rs` - GcodeDialect enum, StartConfig, EndConfig structs
- `crates/slicecore-gcode-io/src/writer.rs` - GcodeWriter<W: Write> with dialect dispatch and integration tests
- `crates/slicecore-gcode-io/src/marlin.rs` - Marlin start/end sequences (M83, dual-phase heating, safe shutdown)
- `crates/slicecore-gcode-io/src/klipper.rs` - Klipper with BED_MESH_CALIBRATE and TURN_OFF_HEATERS
- `crates/slicecore-gcode-io/src/reprap.rs` - RepRapFirmware with home-before-heat and M0 H1 halt
- `crates/slicecore-gcode-io/src/bambu.rs` - Bambu Lab with simplified start (built-in calibration)
- `crates/slicecore-gcode-io/src/validate.rs` - validate_gcode() with ValidationResult (errors + warnings)

## Decisions Made
- **M83 relative extrusion for all dialects:** Research recommended relative extrusion to avoid E-axis overflow and simplify per-layer reset. All 4 dialects emit M83.
- **5 decimal places for extrusion values:** Provides sub-micron precision for volumetric extrusion calculations without excessive output size.
- **Extended command support in validator:** Klipper uses uppercase-underscore commands (TURN_OFF_HEATERS, BED_MESH_CALIBRATE) that aren't standard G/M codes. Validator accepts these as valid.
- **Generic writer over Write trait:** Enables in-memory Vec<u8> for testing, File for disk output, and any WASM-compatible writer.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- G-code output foundation complete for Phase 3 vertical slice
- Marlin dialect fully functional as the Phase 3 target
- GcodeCommand types ready for toolpath-to-gcode conversion
- Validator ready for output quality assurance in the pipeline

## Self-Check: PASSED

All 11 created files verified present. Both task commits (5107a22, 06c5af3) verified in git log.
