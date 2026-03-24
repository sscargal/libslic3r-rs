---
phase: 45-global-and-per-object-settings-override-system
plan: 08
subsystem: fileio
tags: [3mf, import, export, per-object-settings, orcaslicer, prusaslicer]

requires:
  - phase: 45-01
    provides: PlateConfig and ObjectConfig types for per-object settings
provides:
  - 3MF import with per-object settings extraction (parse_with_config)
  - 3MF export with per-object overrides (export_plate_to_3mf)
  - Field mapping between PrusaSlicer/OrcaSlicer and SliceCore field names
  - Round-trip preservation of unmapped metadata
affects: [slicecore-cli, slicecore-engine]

tech-stack:
  added: [toml (added to slicecore-fileio)]
  patterns: [dual-namespace metadata (slicecore: + slicer-compat), best-effort field mapping]

key-files:
  created: []
  modified:
    - crates/slicecore-fileio/src/threemf.rs
    - crates/slicecore-fileio/src/export.rs
    - crates/slicecore-fileio/src/lib.rs
    - crates/slicecore-fileio/Cargo.toml

key-decisions:
  - "Store per-object settings in Metadata/model_settings.config matching OrcaSlicer/Bambu format"
  - "Dual-namespace export: slicecore: for native keys plus PrusaSlicer-compat keys for interop"
  - "Best-effort field mapping with unmapped field preservation for round-tripping"

patterns-established:
  - "3MF config round-trip: import maps slicer fields to SliceCore, export writes both namespaces"
  - "Lightweight XML parsing for vendor config files (line-based, no full XML parser needed)"

requirements-completed: [ADV-03]

duration: 5min
completed: 2026-03-24
---

# Phase 45 Plan 08: 3MF Per-Object Settings Import/Export Summary

**3MF interoperability with PrusaSlicer/OrcaSlicer per-object settings via dual-namespace metadata and best-effort field mapping**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-24T16:54:45Z
- **Completed:** 2026-03-24T16:59:48Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- 3MF import extracts per-object settings from OrcaSlicer/Bambu model_settings.config
- 3MF export writes per-object overrides with slicecore: and PrusaSlicer-compat namespaces
- Round-trip import/export preserves override values (wall_count, layer_height, etc.)
- Unmapped vendor-specific fields preserved as pass-through metadata

## Task Commits

Each task was committed atomically:

1. **Task 1: 3MF import with per-object settings extraction** - `cdd8f41` (feat)
2. **Task 2: 3MF export with per-object overrides** - `d7314a2` (feat)

## Files Created/Modified
- `crates/slicecore-fileio/src/threemf.rs` - Added ThreeMfImportResult, parse_with_config, field mapping
- `crates/slicecore-fileio/src/export.rs` - Added export_plate_to_3mf, reverse field mapping, model_settings.config generation
- `crates/slicecore-fileio/src/lib.rs` - Re-exported new public types and functions
- `crates/slicecore-fileio/Cargo.toml` - Added toml dependency

## Decisions Made
- Stored per-object settings in Metadata/model_settings.config (OrcaSlicer/Bambu-compatible XML format) rather than 3MF model-level metadata, for better interop
- Used dual-namespace export (slicecore: prefix for native fields, plus PrusaSlicer-compat field names) to maximize compatibility
- Lightweight line-based XML parsing for config files (no full XML parser dependency needed)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added toml dependency to slicecore-fileio**
- **Found during:** Task 1
- **Issue:** Plan uses toml::map::Map in new structs but toml wasn't a dependency
- **Fix:** Added toml = { workspace = true } to Cargo.toml
- **Files modified:** crates/slicecore-fileio/Cargo.toml
- **Committed in:** cdd8f41

**2. [Rule 1 - Bug] Removed Debug/Clone derives from mesh-containing structs**
- **Found during:** Task 1
- **Issue:** TriangleMesh doesn't implement Debug or Clone, causing derive failures
- **Fix:** Removed Debug/Clone from ThreeMfImportResult, ThreeMfObjectConfig, ThreeMfModifier
- **Files modified:** crates/slicecore-fileio/src/threemf.rs
- **Committed in:** cdd8f41

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both auto-fixes necessary for compilation. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- 3MF per-object settings interop complete
- Ready for CLI integration of parse_with_config in plate workflows
- export_plate_to_3mf available for plate export commands

---
*Phase: 45-global-and-per-object-settings-override-system*
*Completed: 2026-03-24*
