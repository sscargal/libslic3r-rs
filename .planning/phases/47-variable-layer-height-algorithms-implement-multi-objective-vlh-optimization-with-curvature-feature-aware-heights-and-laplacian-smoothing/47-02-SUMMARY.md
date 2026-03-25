---
phase: 47-variable-layer-height-algorithms
plan: 02
subsystem: slicer
tags: [vlh, feature-detection, laplacian-smoothing, overhang, ratio-clamping]

requires:
  - phase: 47-01
    provides: VlhConfig, FeatureType, FeatureDetection types

provides:
  - FeatureMap pre-pass with overhang detection from mesh normals
  - query_stress_factor and query_feature_demanded_height binary-search lookups
  - Laplacian smoothing with pinned anchor preservation
  - Ratio clamping safety net (max 50% adjacent change)
  - Combined smooth_vlh_heights pipeline

affects: [47-03, 47-04]

tech-stack:
  added: []
  patterns: [binary-search-z-lookup, most-demanding-wins, laplacian-smoothing-with-anchors]

key-files:
  created:
    - crates/slicecore-slicer/src/vlh/features.rs
    - crates/slicecore-slicer/src/vlh/smooth.rs
  modified:
    - crates/slicecore-slicer/src/vlh/mod.rs

key-decisions:
  - "Overhang detection from mesh normals directly (no contour dependency); hole/bridge/thin-wall deferred to Plan 04"
  - "Binary search on z_min-sorted detections for efficient Z lookup"
  - "Stress factor computed per feature type with angle-based scaling for overhangs"

patterns-established:
  - "FeatureMap pattern: pre-pass build + binary-search query for per-Z feature data"
  - "Laplacian smoothing with pinned array for anchor preservation"
  - "Forward-backward ratio clamping as safety net post-smoothing"

requirements-completed: [SLICE-05]

duration: 6min
completed: 2026-03-25
---

# Phase 47 Plan 02: Feature Map and Laplacian Smoothing Summary

**Feature map pre-pass detecting overhangs from mesh normals with binary-search Z lookup, plus Laplacian smoothing with anchor preservation and ratio clamping safety net**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-25T04:13:12Z
- **Completed:** 2026-03-25T04:19:02Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Feature map pre-pass detects overhangs from triangle normals with configurable angle sensitivity range
- Feature margin extension configurable via `feature_margin_layers * min_height` above/below detection zones
- Binary-search query functions return stress factors and demanded heights with most-demanding-wins semantics
- Laplacian smoothing preserves pinned anchor points and boundary elements across configurable iterations
- Ratio clamping enforces max 50% adjacent height change as safety net
- Combined pipeline (Laplacian + ratio clamp + Z recompute) produces smooth, valid height transitions
- All 18 new tests pass; all 35 VLH tests pass total

## Task Commits

Each task was committed atomically (TDD: test then feat):

1. **Task 1: Feature map pre-pass** - `6086aef` (test) + `01ab438` (feat)
2. **Task 2: Laplacian smoothing and ratio clamping** - `48168ed` (test) + `6ae508f` (feat)

## Files Created/Modified
- `crates/slicecore-slicer/src/vlh/features.rs` - Feature map pre-pass: FeatureMap, build_feature_map, query_stress_factor, query_feature_demanded_height
- `crates/slicecore-slicer/src/vlh/smooth.rs` - Laplacian smoothing: laplacian_smooth, ratio_clamp, smooth_vlh_heights
- `crates/slicecore-slicer/src/vlh/mod.rs` - Added pub mod features and pub mod smooth

## Decisions Made
- Used mesh triangle normals directly for overhang detection instead of sliced contour comparison; simpler and avoids dependency on contour pipeline
- Deferred hole, bridge, and thin-wall detection to Plan 04 where sliced contour data will be available
- Stress factor per feature type uses angle-based scaling for overhangs rather than a uniform formula
- Overhang mesh test fixture adjusted to produce normals at ~50 degrees from horizontal to fall within default [40, 60] detection range

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Adjusted test mesh geometry for correct overhang angles**
- **Found during:** Task 1 (Feature map pre-pass)
- **Issue:** Test mesh `overhang_mesh()` had side face normals at ~63.4 degrees, outside the [40, 60] detection range
- **Fix:** Changed `top_half` from 1.0 to 1.34 to produce ~50 degree angles within range
- **Files modified:** crates/slicecore-slicer/src/vlh/features.rs
- **Verification:** All 9 feature map tests pass
- **Committed in:** 01ab438 (Task 1 feat commit)

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** Test fixture correction only, no scope change.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Feature map and smoothing modules ready for Plan 03 (optimizer integration)
- Plan 04 will wire hole/bridge/thin-wall detection using sliced contour data
- All VLH types, objectives, features, and smoothing now available for optimizer

---
*Phase: 47-variable-layer-height-algorithms*
*Completed: 2026-03-25*

## Self-Check: PASSED

All files exist, all commits verified, all acceptance criteria met (12/12).
