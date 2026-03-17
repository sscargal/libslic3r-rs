---
phase: 34-support-config-and-advanced-feature-profile-import-mapping
plan: 04
subsystem: config
tags: [profile-import, p2-fields, post-process, slicing-tolerance, timelapse, niche-config]

# Dependency graph
requires:
  - phase: 34-01
    provides: "Field inventory with P2 niche fields and straggler lists"
provides:
  - "PostProcessConfig mapped from upstream (scripts, timelapse, gcode_label_objects)"
  - "All ~20 P2 niche fields with typed representation and upstream mapping"
  - "Straggler fields closed (ironing_angle, print_sequence, jerk fields)"
  - "MachineConfig extended with AMS, toolchange, silent_mode fields"
  - "AccelerationConfig extended with 7 jerk fields"
affects: [34-05, 34-06]

# Tech tracking
tech-stack:
  added: []
  patterns: ["P2 niche field pattern: #[serde(default)] for all optional fields"]

key-files:
  created: []
  modified:
    - "crates/slicecore-engine/src/config.rs"
    - "crates/slicecore-engine/src/profile_import.rs"
    - "crates/slicecore-engine/src/profile_import_ini.rs"

key-decisions:
  - "Added SlicingTolerance as new enum (Middle/Nearest/Gauss) rather than string passthrough"
  - "PostProcess scripts split by semicolon or newline matching upstream format"
  - "Jerk fields added to AccelerationConfig rather than MachineConfig to group with other process-level controls"
  - "Machine straggler fields (retract_length_toolchange, min_extruding_rate) added to MachineConfig"

patterns-established:
  - "P2 niche fields use #[serde(default)] for zero-cost defaults on all new optional fields"

requirements-completed: [POST-MAP, P2-FIELDS]

# Metrics
duration: 3min
completed: 2026-03-17
---

# Phase 34 Plan 04: PostProcess + P2 Niche Fields + Straggler Coverage Summary

**PostProcessConfig mapped with scripts/timelapse/gcode_label_objects, 20+ P2 niche fields added with SlicingTolerance enum, jerk/machine straggler fields closed**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-17T17:41:56Z
- **Completed:** 2026-03-17T17:45:50Z
- **Tasks:** 1
- **Files modified:** 3

## Accomplishments
- PostProcessConfig fields mapped from upstream: scripts (Vec<String>), timelapse_type, gcode_label_objects, gcode_comments, gcode_add_line_number, filename_format
- All 20+ P2 niche fields from CONFIG_PARITY_AUDIT.md now have typed representation: slicing_tolerance, thumbnails, silent_mode, nozzle_hrc, emit_machine_limits_to_gcode, bed_custom_texture, bed_custom_model, extruder_offset, cooling_tube_length, cooling_tube_retraction, parking_pos_retraction, extra_loading_move, compatible_printers_condition, inherits_group, max_travel_detour_length, exclude_object, reduce_infill_retraction, reduce_crossing_wall
- MachineConfig extended with 15 new fields (AMS, toolchange, silent mode, bed preview)
- AccelerationConfig extended with 7 jerk fields (default, outer_wall, inner_wall, top_surface, infill, travel, initial_layer)
- Straggler fields closed: ironing_angle mapping, print_sequence -> sequential.enabled, extruder_clearance_height (INI alternate key)
- All mappings added to both JSON (profile_import.rs) and INI (profile_import_ini.rs) importers
- upstream_key_to_config_field and prusaslicer_key_to_config_field updated with all new entries
- All 761 tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Add P2 niche fields to config.rs and map PostProcess fields** - `628db7a` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/config.rs` - SlicingTolerance enum, PostProcessConfig scripts/gcode fields, MachineConfig P2 fields, AccelerationConfig jerk fields, PrintConfig P2 niche fields
- `crates/slicecore-engine/src/profile_import.rs` - JSON field mappings for all new P2/PostProcess/straggler fields + upstream_key_to_config_field entries
- `crates/slicecore-engine/src/profile_import_ini.rs` - INI field mappings for PrusaSlicer equivalents + prusaslicer_key_to_config_field entries

## Decisions Made
- SlicingTolerance implemented as a proper enum rather than string passthrough for type safety
- PostProcess scripts parsed by splitting on semicolon or newline to match both OrcaSlicer and PrusaSlicer conventions
- Jerk fields placed in AccelerationConfig (not MachineConfig) since they are process-level printing parameters
- extruder_offset stored as Vec<[f64; 2]> but complex parsing deferred to passthrough for the JSON importer

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- PostProcessConfig now at full upstream mapping coverage
- All P2 niche fields from CONFIG_PARITY_AUDIT.md have typed representation
- Straggler fields from IroningConfig, SequentialConfig, MachineConfig, AccelerationConfig are closed
- Ready for Plan 05 (straggler sweep + G-code template variable translation) and Plan 06 (validation)

---
*Phase: 34-support-config-and-advanced-feature-profile-import-mapping*
*Completed: 2026-03-17*
