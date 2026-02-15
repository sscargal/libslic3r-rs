---
phase: 01-foundation-types-and-geometry-core
plan: 02
subsystem: geometry
tags: [rust, polygon, boolean-ops, clipper2-rust, offsetting, convex-hull, point-in-polygon, simplification, two-tier-validation]

# Dependency graph
requires:
  - phase: 01-01
    provides: "Coord (i64), IPoint2, COORD_SCALE, mm_to_coord/coord_to_mm, Cargo workspace"
provides:
  - "Polygon/ValidPolygon two-tier type system with geometric invariant enforcement"
  - "polygon_union, polygon_intersection, polygon_difference, polygon_xor via clipper2-rust"
  - "offset_polygon/offset_polygons for inward/outward polygon inflation/deflation"
  - "signed_area_i64/signed_area_f64 shoelace formula with i128 overflow protection"
  - "point_in_polygon winding number test with OnBoundary detection"
  - "simplify (Ramer-Douglas-Peucker) and convex_hull (Graham scan)"
  - "GeoError enum for geometry validation and operation failures"
  - "Polyline type for open paths"
  - "JoinType enum (Round, Square, Miter) for offset corner treatment"
affects: [01-03-PLAN, 01-04-PLAN, slicecore-mesh, slicing-pipeline]

# Tech tracking
tech-stack:
  added: [clipper2-rust 1.0.0, thiserror 2.x]
  patterns: [two-tier-polygon-validation, clipper2-path64-conversion, signed-area-for-winding, i128-intermediate-arithmetic]

key-files:
  created:
    - crates/slicecore-geo/Cargo.toml
    - crates/slicecore-geo/src/lib.rs
    - crates/slicecore-geo/src/error.rs
    - crates/slicecore-geo/src/area.rs
    - crates/slicecore-geo/src/polygon.rs
    - crates/slicecore-geo/src/polyline.rs
    - crates/slicecore-geo/src/point_in_poly.rs
    - crates/slicecore-geo/src/simplify.rs
    - crates/slicecore-geo/src/convex_hull.rs
    - crates/slicecore-geo/src/boolean.rs
    - crates/slicecore-geo/src/offset.rs
  modified:
    - Cargo.lock

key-decisions:
  - "clipper2-rust v1.0.0 selected for boolean ops and offsetting: pure Rust, i64 coords, WASM-compatible"
  - "ValidPolygon stores cached signed area and winding direction, computed once at validation"
  - "ValidPolygon.from_raw_parts is pub(crate) for internal use by boolean/offset post-processing"
  - "signed_area_2x avoids integer division for maximum precision; i128 intermediates prevent overflow"
  - "Boolean ops use NonZero fill rule (standard for slicing operations)"
  - "Polygon offset uses inflate_paths_64 with EndType::Polygon for closed path offsetting"
  - "Degenerate results (zero-area slivers, <3 points) silently skipped in boolean/offset output"
  - "Net area calculation uses signed areas to correctly handle polygon-with-hole results"

patterns-established:
  - "Two-tier validation: Polygon (unvalidated, public fields) -> ValidPolygon (invariants enforced, private fields)"
  - "i128 intermediate arithmetic in cross products and area calculations to prevent i64 overflow"
  - "Conversion boundary: ValidPolygon <-> clipper2 Path64 with IPoint2 <-> Point64 point-by-point mapping"
  - "Offset collapse returns empty Vec (not error) when inward offset exceeds polygon half-width"
  - "Winding convention: CCW = outer boundary (positive area), CW = hole (negative area)"

# Metrics
duration: 9min
completed: 2026-02-15
---

# Phase 1 Plan 2: slicecore-geo Polygon Types, Boolean Operations, and Offsetting Summary

**Polygon/ValidPolygon two-tier types with clipper2-rust boolean ops (union/intersection/difference/XOR), polygon offsetting, and 107 tests including 35 degenerate geometry edge cases**

## Performance

- **Duration:** 9 min
- **Started:** 2026-02-15T04:12:37Z
- **Completed:** 2026-02-15T04:21:51Z
- **Tasks:** 2
- **Files modified:** 12

## Accomplishments
- Complete slicecore-geo crate with Polygon/ValidPolygon two-tier validation enforcing geometric invariants (at least 3 non-collinear points, non-zero area, known winding)
- Boolean operations (union, intersection, difference, XOR) via clipper2-rust v1.0.0 with proper Path64 conversion and NonZero fill rule
- Polygon offsetting (inflate/deflate) with Round, Square, and Miter join types, proper collapse handling for inward offsets
- Hand-rolled algorithms: shoelace area (i128 safe), winding number point-in-polygon, Ramer-Douglas-Peucker simplification, Graham scan convex hull
- 107 total tests: 72 core types/algorithms + 35 boolean/offset tests including 12 degenerate geometry edge cases
- All tests pass, clippy clean with -D warnings

## Task Commits

Each task was committed atomically:

1. **Task 1: Polygon types, validation, and geometry utilities** - `81a6d84` (feat)
2. **Task 2: Polygon boolean operations, offsetting via clipper2-rust, and 35 degenerate geometry tests** - `909f676` (feat)

## Files Created/Modified
- `crates/slicecore-geo/Cargo.toml` - Crate manifest with slicecore-math, clipper2-rust, serde, thiserror dependencies
- `crates/slicecore-geo/src/lib.rs` - Module declarations and re-exports for all public types and functions
- `crates/slicecore-geo/src/error.rs` - GeoError enum with 6 variants (TooFewPoints, ZeroArea, AllCollinear, SelfIntersecting, BooleanOpFailed, OffsetFailed)
- `crates/slicecore-geo/src/area.rs` - signed_area_2x/i64/f64, winding_direction, cross_product_i128, perpendicular_distance helpers
- `crates/slicecore-geo/src/polygon.rs` - Polygon (unvalidated), ValidPolygon (invariants enforced), Winding enum, validate() method
- `crates/slicecore-geo/src/polyline.rs` - Polyline type for open paths with length computation
- `crates/slicecore-geo/src/point_in_poly.rs` - Winding number point-in-polygon test with OnBoundary detection
- `crates/slicecore-geo/src/simplify.rs` - Ramer-Douglas-Peucker polyline simplification
- `crates/slicecore-geo/src/convex_hull.rs` - Graham scan convex hull returning CCW-ordered points
- `crates/slicecore-geo/src/boolean.rs` - polygon_union/intersection/difference/xor wrapping clipper2-rust
- `crates/slicecore-geo/src/offset.rs` - offset_polygon/offset_polygons with JoinType (Round, Square, Miter)
- `Cargo.lock` - Updated with clipper2-rust and thiserror dependencies

## Decisions Made
- **clipper2-rust v1.0.0 selected:** Pure Rust port of Clipper2 with i64 coordinates, WASM-compatible, includes boolean ops AND offsetting. Confirmed available on crates.io. Matches the C++ Clipper2 behavior that PrusaSlicer/OrcaSlicer rely on.
- **Degenerate result paths silently skipped:** Boolean operations can produce zero-area slivers or sub-3-point paths; these are filtered out rather than returning errors, matching standard Clipper2 usage patterns.
- **Net area uses signed values:** For polygon-with-hole results (outer CCW + inner CW), summing signed areas gives correct net area. Using absolute values would double-count the hole region.
- **JoinType is our own enum:** Wraps clipper2_rust::JoinType to avoid leaking the dependency into our public API.
- **ValidPolygon::from_raw_parts is pub(crate):** Boolean and offset operations produce validated geometry from clipper2, so full re-validation is unnecessary. This internal constructor trusts the library output.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed net area calculation for polygon-with-hole results**
- **Found during:** Task 2 (boolean operation tests)
- **Issue:** `total_area_mm2` helper summed absolute areas, giving 500 mm^2 instead of 300 mm^2 for difference operations that produce outer boundary + hole
- **Fix:** Changed to sum signed areas (positive for CCW, negative for CW) then take absolute value
- **Files modified:** crates/slicecore-geo/src/boolean.rs (test helper)
- **Verification:** degenerate_polygon_with_hole and difference_partial tests pass
- **Committed in:** 909f676 (Task 2 commit)

**2. [Rule 3 - Blocking] Added serde_json dev-dependency**
- **Found during:** Task 1 (polygon serde test)
- **Issue:** Winding enum serde round-trip test needed serde_json, which wasn't in dev-dependencies
- **Fix:** Added `serde_json = { workspace = true }` to dev-dependencies
- **Files modified:** crates/slicecore-geo/Cargo.toml
- **Verification:** winding_enum_serde test passes
- **Committed in:** 81a6d84 (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 bug in test helper, 1 missing dev-dependency)
**Impact on plan:** Both fixes necessary for correct test execution. No scope creep.

## Issues Encountered
- Clippy flagged `needless_range_loop` in the RDP simplification algorithm. Refactored to use `iter().enumerate().skip().take()` pattern.
- Two boolean test assertions (degenerate_polygon_with_hole, difference_partial) initially failed because area summation used absolute values instead of signed values for polygon-with-hole results. Fixed by using signed area summation.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- slicecore-geo crate is complete and ready for downstream use by slicecore-mesh (01-03) and slicing pipeline
- All polygon types, boolean operations, and offsetting are validated with 107 tests
- clipper2-rust v1.0.0 integration verified with extensive degenerate geometry test cases
- Two-tier Polygon/ValidPolygon pattern established for all future geometry code

## Self-Check: PASSED

- All 11 source files verified present
- Both task commits verified (81a6d84, 909f676)
- Test count verified: 107 tests (target: 40+)

---
*Phase: 01-foundation-types-and-geometry-core*
*Completed: 2026-02-15*
