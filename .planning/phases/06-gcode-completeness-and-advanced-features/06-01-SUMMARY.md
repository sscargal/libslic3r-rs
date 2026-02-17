---
phase: 06-gcode-completeness-and-advanced-features
plan: 01
subsystem: gcode
tags: [gcode, firmware-dialect, acceleration, jerk, pressure-advance, marlin, klipper, reprap, bambu, arc-moves]

# Dependency graph
requires:
  - phase: 02-io-layer
    provides: "GcodeCommand enum, GcodeDialect enum, GcodeWriter, dialect modules"
  - phase: 03-vertical-slice
    provides: "PrintConfig, Engine pipeline, gcode_gen module"
provides:
  - "6 new GcodeCommand variants: ArcMoveCW, ArcMoveCCW, SetAcceleration, SetJerk, SetPressureAdvance, ToolChange"
  - "Dialect-aware formatting functions: format_acceleration, format_pressure_advance, format_jerk"
  - "Configurable gcode_dialect field in PrintConfig (replaces hardcoded Marlin)"
  - "Acceleration/jerk/PA config fields in PrintConfig with backward-compatible defaults"
  - "Acceleration emission at feature transitions in gcode_gen"
  - "Pressure advance emission at print body start"
affects: [06-02, 06-04, 06-05, 06-06, 06-07, 06-08, 06-09]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Dialect-specific formatting via standalone functions returning String for Raw command injection"
    - "Acceleration/jerk/PA commands emitted as GcodeCommand::Raw using dialect formatters"
    - "acceleration_enabled=false default for backward compatibility"

key-files:
  created: []
  modified:
    - crates/slicecore-gcode-io/src/commands.rs
    - crates/slicecore-gcode-io/src/dialect.rs
    - crates/slicecore-gcode-io/src/bambu.rs
    - crates/slicecore-gcode-io/src/lib.rs
    - crates/slicecore-gcode-io/src/validate.rs
    - crates/slicecore-engine/src/config.rs
    - crates/slicecore-engine/src/engine.rs
    - crates/slicecore-engine/src/gcode_gen.rs
    - crates/slicecore-engine/src/custom_gcode.rs

key-decisions:
  - "Display impl uses Marlin format as default; dialect-aware Raw commands for non-Marlin"
  - "acceleration_enabled defaults to false for backward compatibility"
  - "Pressure advance emitted once at print body start (not per-layer)"
  - "Acceleration emitted at every feature transition when enabled"

patterns-established:
  - "Dialect formatting pattern: standalone format_* functions in dialect.rs returning String, injected as GcodeCommand::Raw"
  - "Feature transition hook pattern: detect feature type change in gcode_gen, emit dialect-specific commands"

# Metrics
duration: 9min
completed: 2026-02-17
---

# Phase 6 Plan 1: Dialect-Specific Inline Commands Summary

**Extended GcodeCommand with 6 new variants (arcs, acceleration, jerk, PA, tool change), added dialect-aware formatting for 4 firmware targets, and made engine dialect configurable via PrintConfig**

## Performance

- **Duration:** 9 min
- **Started:** 2026-02-17
- **Completed:** 2026-02-17
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- Extended GcodeCommand enum with ArcMoveCW, ArcMoveCCW, SetAcceleration, SetJerk, SetPressureAdvance, and ToolChange variants with proper Display formatting
- Added 3 dialect-aware formatting functions (format_acceleration, format_pressure_advance, format_jerk) covering Marlin, Klipper, RepRapFirmware, and Bambu dialects
- Made engine's G-code dialect configurable via PrintConfig.gcode_dialect (replacing hardcoded Marlin)
- Added acceleration, jerk, and pressure advance config fields to PrintConfig with backward-compatible defaults
- Engine now emits acceleration commands at feature transitions and pressure advance at print body start when enabled

## Task Commits

Each task was committed atomically:

1. **Task 1: Extend GcodeCommand enum and dialect modules** - `e6778eb` (feat)
2. **Task 2: Make dialect configurable in Engine and emit acceleration commands** - `dfb1416` (feat, shared with 06-03 concurrent commit)

**Plan metadata:** [pending]

## Files Created/Modified
- `crates/slicecore-gcode-io/src/commands.rs` - 6 new GcodeCommand variants with Display impl and 7 new unit tests
- `crates/slicecore-gcode-io/src/dialect.rs` - 3 dialect-aware formatting functions (acceleration, PA, jerk) with 12 unit tests
- `crates/slicecore-gcode-io/src/bambu.rs` - M620/M621 AMS commands in start sequence with test
- `crates/slicecore-gcode-io/src/lib.rs` - Re-exports for format_acceleration, format_pressure_advance, format_jerk
- `crates/slicecore-gcode-io/src/validate.rs` - 6 new validator tests for arc, tool change, Klipper, RepRap, Bambu, and acceleration commands
- `crates/slicecore-engine/src/config.rs` - GcodeDialect import, 8 new config fields, 5 new tests
- `crates/slicecore-engine/src/engine.rs` - Uses config.gcode_dialect instead of hardcoded Marlin
- `crates/slicecore-engine/src/gcode_gen.rs` - Acceleration at feature transitions, PA at print body start
- `crates/slicecore-engine/src/custom_gcode.rs` - Fixed clippy derivable_impls warning

## Decisions Made
- **Display impl uses Marlin format as default:** SetAcceleration/SetJerk/SetPressureAdvance Display trait formats to Marlin syntax (M204/M205/M900). For non-Marlin dialects, the engine uses the dialect formatting functions to produce Raw commands. This avoids coupling Display to runtime dialect selection.
- **acceleration_enabled defaults to false:** New acceleration fields are inactive by default, ensuring all existing configs continue to produce identical output (backward compatible).
- **Pressure advance emitted once at print body start:** PA value is set once after start G-code, not re-emitted per layer. This matches typical slicer behavior.
- **Acceleration emitted at every feature transition:** When acceleration_enabled=true, acceleration commands are emitted whenever the feature type changes (e.g., perimeter to infill, print to travel). Travel uses travel_acceleration; all other features use print_acceleration.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed clippy derivable_impls warning in custom_gcode.rs**
- **Found during:** Task 2 (clippy verification)
- **Issue:** CustomGcodeHooks had a manual Default impl that was identical to what derive(Default) would produce
- **Fix:** Added Default to derive macro, removed manual impl
- **Files modified:** crates/slicecore-engine/src/custom_gcode.rs
- **Verification:** cargo clippy passes with no warnings
- **Committed in:** dfb1416 (part of task commit)

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** Minor cleanup for clippy compliance. No scope creep.

## Issues Encountered
- **Concurrent plan execution overlap:** Plan 06-03 was executing simultaneously and committed the Task 2 engine file changes (config.rs, engine.rs, gcode_gen.rs, custom_gcode.rs) as part of its commit dfb1416. Task 2 changes were included in that commit rather than a separate 06-01 commit. All changes are present and verified.
- **Linter interference with GcodeDialect import:** The linter repeatedly removed the `use slicecore_gcode_io::GcodeDialect;` import from config.rs during editing, which cascaded to removing the struct fields. Required multiple re-edits to stabilize.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- All 4 firmware dialects now have dialect-specific formatting for inline commands
- Engine pipeline is dialect-configurable -- subsequent plans (06-02 custom G-code hooks, 06-04 temperature planning) can use config.gcode_dialect
- Acceleration/jerk/PA infrastructure ready for per-feature tuning in later plans
- Arc move commands (G2/G3) ready for arc fitting implementation

## Self-Check: PASSED

- All 9 modified files verified on disk
- Commit e6778eb (Task 1) verified in git log
- Commit dfb1416 (Task 2 changes) verified in git log

---
*Phase: 06-gcode-completeness-and-advanced-features*
*Completed: 2026-02-17*
