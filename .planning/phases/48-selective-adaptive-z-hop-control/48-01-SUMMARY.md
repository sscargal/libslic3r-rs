---
phase: 48-selective-adaptive-z-hop-control
plan: 01
subsystem: config
tags: [z-hop, config, feature-type, serde, backward-compat]

# Dependency graph
requires:
  - phase: 20-expand-printconfig-field-coverage-and-profile-mapping
    provides: RetractionConfig with z_hop field, profile import mapping
provides:
  - ZHopConfig struct with 12 fields (height, hop_type, height_mode, etc.)
  - ZHopType, ZHopHeightMode, SurfaceEnforce enums
  - TopSolidInfill variant in FeatureType
  - is_top field on LayerInfill for surface gating
affects: [48-02-z-hop-planning-gcode, 48-03-profile-import-mapping]

# Tech tracking
tech-stack:
  added: []
  patterns: [serde-alias-migration, feature-type-exhaustive-match-propagation]

key-files:
  created: []
  modified:
    - crates/slicecore-engine/src/config.rs
    - crates/slicecore-engine/src/toolpath.rs
    - crates/slicecore-engine/src/infill/mod.rs
    - crates/slicecore-engine/src/gcode_gen.rs
    - crates/slicecore-engine/src/statistics.rs
    - crates/slicecore-engine/src/flow_control.rs
    - crates/slicecore-engine/src/preview.rs
    - crates/slicecore-engine/src/engine.rs
    - crates/slicecore-engine/src/planner.rs
    - crates/slicecore-engine/src/profile_import.rs
    - crates/slicecore-engine/src/profile_import_ini.rs

key-decisions:
  - "ZHopConfig placed on PrintConfig (not RetractionConfig) with serde alias for backward compat"
  - "TopSolidInfill feature type gates on LayerInfill.is_top computed from top_solid_layers threshold"

patterns-established:
  - "Config migration pattern: remove field from old struct, add serde(alias) on new struct for backward compat"
  - "FeatureType propagation: new variant requires updating 6+ exhaustive match sites across the engine"

requirements-completed: [GCODE-03]

# Metrics
duration: 8min
completed: 2026-03-25
---

# Phase 48 Plan 01: Z-Hop Config Types Summary

**ZHopConfig with 12 fields, 3 enums, TopSolidInfill feature type, and is_top infill propagation for surface-gated z-hop**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-25T22:54:16Z
- **Completed:** 2026-03-25T23:02:16Z
- **Tasks:** 2
- **Files modified:** 15

## Accomplishments
- Defined ZHopConfig struct with 12 fields and SettingSchema derive, plus ZHopType, ZHopHeightMode, SurfaceEnforce enums
- Migrated z_hop from RetractionConfig to standalone ZHopConfig on PrintConfig with serde alias backward compatibility
- Added TopSolidInfill variant to FeatureType with is_top propagation through LayerInfill and engine layer processing
- Updated all exhaustive FeatureType match arms across 6 source files (gcode_gen, statistics, flow_control, preview x2, toolpath)

## Task Commits

Each task was committed atomically:

1. **Task 1: Define ZHopConfig types** - `5269008` (test: TDD RED), `c91b71f` (feat: TDD GREEN)
2. **Task 2: Add TopSolidInfill and propagate is_top** - `5037063` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/config.rs` - Added ZHopConfig struct, 3 enums, removed z_hop from RetractionConfig
- `crates/slicecore-engine/src/toolpath.rs` - Added TopSolidInfill variant, updated infill_feature selection
- `crates/slicecore-engine/src/infill/mod.rs` - Added is_top field to LayerInfill
- `crates/slicecore-engine/src/engine.rs` - Propagated is_top_layer to LayerInfill at 3 construction sites
- `crates/slicecore-engine/src/gcode_gen.rs` - Added TopSolidInfill match arm, updated z_hop reference
- `crates/slicecore-engine/src/statistics.rs` - Added TopSolidInfill to display name and feature order
- `crates/slicecore-engine/src/flow_control.rs` - Added TopSolidInfill multiplier (same as SolidInfill)
- `crates/slicecore-engine/src/preview.rs` - Added TopSolidInfill to both match sites
- `crates/slicecore-engine/src/planner.rs` - Updated z_hop reference from retraction to config.z_hop.height
- `crates/slicecore-engine/src/profile_import.rs` - Updated z_hop mapping to z_hop.height
- `crates/slicecore-engine/src/profile_import_ini.rs` - Updated retract_lift mapping to z_hop.height
- `crates/slicecore-engine/tests/integration_phase20.rs` - Updated z_hop assertions
- `crates/slicecore-engine/tests/integration_profile_convert.rs` - Updated z_hop assertions
- `crates/slicecore-engine/tests/integration_profile_library_ini.rs` - Updated z_hop assertions

## Decisions Made
- ZHopConfig placed directly on PrintConfig (not nested under RetractionConfig) to cleanly separate z-hop configuration from retraction mechanics
- Used `#[serde(alias = "z_hop")]` on ZHopConfig.height for backward compatibility with old config formats
- TopSolidInfill uses same flow multiplier and speed as SolidInfill (differentiation is only for z-hop gating)
- is_top computed using top_solid_layers threshold (same logic as ironing top-layer detection)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Updated feature count assertions in statistics and engine tests**
- **Found during:** Task 2 (TopSolidInfill propagation)
- **Issue:** Existing tests asserted 14 real features / 17 total; adding TopSolidInfill made it 15/18
- **Fix:** Updated count assertions in statistics.rs and engine.rs tests
- **Files modified:** crates/slicecore-engine/src/statistics.rs, crates/slicecore-engine/src/engine.rs
- **Verification:** All 900 lib tests pass
- **Committed in:** 5037063 (Task 2 commit)

**2. [Rule 1 - Bug] Updated integration test z_hop references**
- **Found during:** Task 1 (ZHopConfig migration)
- **Issue:** Integration tests referenced config.retraction.z_hop which no longer exists
- **Fix:** Updated to config.z_hop.height in 3 integration test files
- **Files modified:** integration_phase20.rs, integration_profile_convert.rs, integration_profile_library_ini.rs
- **Verification:** Compilation succeeds
- **Committed in:** c91b71f (Task 1 commit)

**3. [Rule 1 - Bug] Added TopSolidInfill to preview.rs feature_type_label match**
- **Found during:** Task 2 (compilation check)
- **Issue:** preview.rs had a second exhaustive match on FeatureType not listed in plan
- **Fix:** Added TopSolidInfill => "top_solid_infill" arm
- **Files modified:** crates/slicecore-engine/src/preview.rs
- **Verification:** Build succeeds with no non-exhaustive warnings
- **Committed in:** 5037063 (Task 2 commit)

---

**Total deviations:** 3 auto-fixed (3 Rule 1 bugs)
**Impact on plan:** All auto-fixes necessary for correctness. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- ZHopConfig types are ready for Plan 02 (z-hop planning + gcode emission)
- TopSolidInfill feature type is ready for z-hop surface gating logic
- Profile import paths updated and ready for Plan 03 (profile import mapping)

---
*Phase: 48-selective-adaptive-z-hop-control*
*Completed: 2026-03-25*
