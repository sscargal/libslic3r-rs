---
phase: 20-expand-printconfig-field-coverage-and-profile-mapping
plan: 01
subsystem: config
tags: [serde, toml, sub-config, btreemap, vec-f64, multi-extruder]

# Dependency graph
requires:
  - phase: 03-vertical-slice
    provides: PrintConfig with serde(default) pattern
  - phase: 13-json-profile-support
    provides: JSON profile import with field mapping
provides:
  - 7 nested sub-config structs (LineWidthConfig, SpeedConfig, CoolingConfig, RetractionConfig, MachineConfig, AccelerationConfig, FilamentPropsConfig)
  - BTreeMap passthrough field for unmapped upstream profile fields
  - 9 new flat process misc fields on PrintConfig
  - Vec<f64> multi-extruder array storage with scalar accessor methods
affects: [20-02, 20-03, 20-04, 20-05, profile-import, profile-convert]

# Tech tracking
tech-stack:
  added: [std::collections::BTreeMap]
  patterns: [nested sub-config with serde(default), Vec<f64> with scalar accessor convenience methods, BTreeMap passthrough for round-trip fidelity]

key-files:
  created: []
  modified: [crates/slicecore-engine/src/config.rs]

key-decisions:
  - "BTreeMap (not HashMap) for passthrough to ensure deterministic TOML serialization order"
  - "Vec<f64> for multi-extruder arrays with first-element accessor methods returning sensible defaults for empty vecs"
  - "New sub-config fields added alongside existing flat fields (no migration yet) for zero breaking changes"
  - "Industry-standard defaults from BambuStudio reference profiles (0.42mm outer wall, 25mm/s bridge speed, etc.)"

patterns-established:
  - "Sub-config struct pattern: #[derive(Debug, Clone, Serialize, Deserialize)] + #[serde(default)] + custom Default impl"
  - "Vec<f64> multi-extruder pattern: Vec with single-element default + convenience accessor returning first().unwrap_or(fallback)"

requirements-completed: [SC1-process-fields, SC2-machine-fields, SC3-filament-fields]

# Metrics
duration: 3min
completed: 2026-02-24
---

# Phase 20 Plan 01: Sub-Config Structs and Passthrough Summary

**7 nested sub-config structs with ~86 new typed fields, BTreeMap passthrough, and Vec<f64> multi-extruder arrays added to PrintConfig with zero breaking changes**

## Performance

- **Duration:** 3 min
- **Started:** 2026-02-24T22:51:00Z
- **Completed:** 2026-02-24T22:54:14Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Added 7 nested sub-config structs covering line widths, speeds, cooling, retraction, machine, acceleration, and filament properties
- Added BTreeMap<String, String> passthrough field for round-trip fidelity of unmapped upstream fields
- Added 9 flat process misc fields (bridge_flow, elefant_foot_compensation, infill_direction, infill_wall_overlap, spiral_mode, only_one_wall_top, resolution, raft_layers, detect_thin_wall)
- MachineConfig stores Vec<f64> arrays for nozzle_diameters and jerk_values with scalar accessor convenience methods
- FilamentPropsConfig stores Vec<f64> arrays for per-extruder temperatures with scalar accessor convenience methods
- All 39 existing config tests pass unchanged -- zero breaking changes to config format
- 5 new tests verify sub-config defaults, TOML parsing, passthrough round-trip, and Vec<f64> array semantics

## Task Commits

Each task was committed atomically:

1. **Task 1: Add sub-config structs and passthrough to PrintConfig** - `fdf6c42` (feat)

**Plan metadata:** (pending docs commit)

## Files Created/Modified
- `crates/slicecore-engine/src/config.rs` - Added 7 sub-config structs, passthrough BTreeMap, 9 flat process misc fields, convenience accessor methods, and 5 new unit tests (+807 lines)

## Decisions Made
- Used BTreeMap (not HashMap) for passthrough to ensure deterministic TOML serialization order
- Vec<f64> for multi-extruder arrays with first-element accessor methods returning sensible defaults for empty vecs (0.4 for nozzle_diameter, 8.0 for jerk_x/y, etc.)
- All new sub-config fields added alongside existing flat fields with no migration (Plan 04 handles migration)
- Industry-standard defaults sourced from BambuStudio reference profiles
- Option<f64> for filament retraction overrides (None = use global setting)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Sub-config data model is ready for Plans 02-03 (mapper expansion)
- Plan 04 (field migration) can migrate existing flat fields into these sub-configs
- All workspace crates compile cleanly with no clippy warnings

---
*Phase: 20-expand-printconfig-field-coverage-and-profile-mapping*
*Completed: 2026-02-24*
