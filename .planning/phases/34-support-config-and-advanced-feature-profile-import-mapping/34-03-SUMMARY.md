---
phase: 34-support-config-and-advanced-feature-profile-import-mapping
plan: 03
subsystem: profile-import
tags: [scarf-joint, multi-material, custom-gcode, profile-mapping, orcaslicer, prusaslicer]

requires:
  - phase: 34-01
    provides: "Field inventory of all unmapped upstream keys"
provides:
  - "ScarfJointConfig fully mapped from OrcaSlicer seam_slope_* fields (16 keys)"
  - "MultiMaterialConfig mapped from wipe_tower_*, prime_*, flush_* fields (15+ keys)"
  - "CustomGcodeHooks mapped from before_layer_gcode, toolchange, color_change (5 keys)"
  - "PrusaSlicer INI mappings for multi-material and custom gcode sections"
affects: [34-04, 34-05, 34-06]

tech-stack:
  added: []
  patterns: ["bool-from-string pattern: value == '1' || value.eq_ignore_ascii_case('true')"]

key-files:
  created: []
  modified:
    - crates/slicecore-engine/src/config.rs
    - crates/slicecore-engine/src/profile_import.rs
    - crates/slicecore-engine/src/profile_import_ini.rs
    - crates/slicecore-engine/src/custom_gcode.rs

key-decisions:
  - "New MultiMaterialConfig fields use serde(default) for backward compat"
  - "PrusaSlicer scarf joint section skipped (no upstream equivalent)"
  - "wipe_tower_bridging defaults to 10.0 matching PrusaSlicer default"

patterns-established:
  - "OrcaSlicer-only fields get comment noting PrusaSlicer skip"

requirements-completed: [SCARF-MAP, MULTI-MAP, GCODE-MAP]

duration: 4min
completed: 2026-03-17
---

# Phase 34 Plan 03: Scarf Joint + Multi-Material + Custom G-code Mapping Summary

**16 scarf joint, 15 multi-material, and 5 custom gcode hook upstream keys mapped to typed config fields across both JSON and INI importers**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-17T17:34:25Z
- **Completed:** 2026-03-17T17:38:25Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Mapped all 16 OrcaSlicer seam_slope_* and scarf_joint_* fields to ScarfJointConfig
- Mapped 15+ OrcaSlicer/PrusaSlicer wipe_tower_*, prime_*, flush_* fields to MultiMaterialConfig
- Mapped 5 custom gcode hook fields (before_layer, toolchange, color_change, pause, between_objects)
- Added 4 new ScarfJointConfig fields and 9 new MultiMaterialConfig fields and 3 new CustomGcodeHooks fields
- 40 config.scarf_joint/multi_material/custom_gcode assignments in profile_import.rs

## Task Commits

Each task was committed atomically:

1. **Task 1: Add ScarfJoint and MultiMaterial JSON+INI field mappings** - `f65c1ab` (feat)
2. **Task 2: Add CustomGcodeHooks JSON+INI field mappings** - `a7b4322` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/config.rs` - Added 13 new fields to ScarfJointConfig and MultiMaterialConfig
- `crates/slicecore-engine/src/profile_import.rs` - Added ~36 match arms for scarf, multi-material, and gcode hooks
- `crates/slicecore-engine/src/profile_import_ini.rs` - Added PrusaSlicer INI mappings for multi-material and gcode hooks
- `crates/slicecore-engine/src/custom_gcode.rs` - Added color_change, pause_print, between_objects fields

## Decisions Made
- PrusaSlicer has no scarf joint equivalent; added skip comment in INI mapper
- New MultiMaterialConfig fields all use `#[serde(default)]` for backward compatibility
- purge_in_prime_tower defaults to true (matching OrcaSlicer behavior)
- wipe_tower_bridging defaults to 10.0 (matching PrusaSlicer default)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- ScarfJoint, MultiMaterial, and CustomGcode sections are now at full upstream mapping coverage
- Ready for Plan 04 (PostProcess, P2 niche fields) and Plan 06 (validation sweep)

---
*Phase: 34-support-config-and-advanced-feature-profile-import-mapping*
*Completed: 2026-03-17*
