---
phase: 45-global-and-per-object-settings-override-system
plan: 03
subsystem: engine
tags: [cascade, z-schedule, per-object-config, override-resolution, proptest, ordered-float]

requires:
  - phase: 45-01
    provides: PlateConfig, ObjectConfig, ProfileComposer with SourceType variants for layers 7-10

provides:
  - CascadeResolver with resolve_object_config and resolve_all for layers 7-8
  - ResolvedObject with Arc<PrintConfig> sharing and provenance
  - ZSchedule with from_objects, z_heights, object_membership, is_uniform
  - Fuzzy "did you mean?" error messages for unknown override sets

affects: [45-04, 45-05, 45-06, slicing-integration]

tech-stack:
  added: [ordered-float 4, proptest 1]
  patterns: [cascade-resolution, z-schedule-union, arc-sharing-for-unmodified-objects]

key-files:
  created:
    - crates/slicecore-engine/src/cascade.rs
    - crates/slicecore-engine/src/z_schedule.rs
  modified:
    - crates/slicecore-engine/src/lib.rs
    - crates/slicecore-engine/Cargo.toml

key-decisions:
  - "Layers 9-10 (layer-range and per-region) deferred to slicing integration, not resolved in cascade"
  - "Objects with no overrides share Arc<PrintConfig> for memory efficiency"
  - "Inline overrides applied after named override set so inline wins on conflict"

patterns-established:
  - "Cascade resolution: serialize base config to TOML table, add layers via ProfileComposer"
  - "Z-schedule: BTreeSet<OrderedFloat<f64>> for deterministic Z height union"

requirements-completed: [ADV-03]

duration: 4min
completed: 2026-03-24
---

# Phase 45 Plan 03: Cascade Resolution and Z-Schedule Summary

**10-layer cascade resolver for per-object config (layers 7-8) with Arc sharing, and Z-schedule union computation with proptest coverage**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-24T16:09:26Z
- **Completed:** 2026-03-24T16:13:40Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- CascadeResolver resolves layers 7-8 (default object overrides + per-object overrides) with full provenance tracking
- Objects without overrides share Arc<PrintConfig> for zero-copy memory efficiency
- ZSchedule computes union of per-object Z heights with membership tracking and explosion warnings
- 21 total tests (11 cascade unit + 8 z-schedule unit + 2 proptest property-based)

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement 10-layer cascade resolution with provenance** - `de812e3` (feat)
2. **Task 2: Implement per-object Z-schedule computation with proptest** - `2ed4a7b` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/cascade.rs` - CascadeResolver with resolve_object_config, resolve_all, ResolvedObject
- `crates/slicecore-engine/src/z_schedule.rs` - ZSchedule with from_objects, is_uniform, ObjectZParams
- `crates/slicecore-engine/src/lib.rs` - Added cascade and z_schedule module declarations
- `crates/slicecore-engine/Cargo.toml` - Added ordered-float and proptest dependencies

## Decisions Made
- Layers 9-10 (layer-range and per-region overrides) are not resolved in the cascade -- they are deferred to slicing time when Z heights and modifier regions are known
- Objects with no overrides (no default_object_overrides, no override_set, no inline_overrides) share Arc<PrintConfig> rather than cloning
- Inline overrides are applied after named override sets, so inline values win on conflict (consistent with specificity principle)
- Used strsim::jaro_winkler for fuzzy matching in "did you mean?" error messages (strsim already a dependency)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Cascade resolver ready for integration with slicing pipeline (Plan 05)
- Z-schedule ready for multi-object plate processing
- Layers 9-10 resolution to be added during slicing integration

---
*Phase: 45-global-and-per-object-settings-override-system*
*Completed: 2026-03-24*
