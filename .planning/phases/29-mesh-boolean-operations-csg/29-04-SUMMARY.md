---
phase: 29-mesh-boolean-operations-csg
plan: 04
subsystem: mesh
tags: [csg, split, offset, hollow, plane-split, drain-hole, ear-clipping]

# Dependency graph
requires:
  - phase: 29-mesh-boolean-operations-csg/03
    provides: "Boolean operations API (mesh_difference)"
provides:
  - "mesh_split_at_plane with capped watertight halves"
  - "SplitPlane with xy/xz/yz constructors and arbitrary normal+offset"
  - "mesh_offset with angle-weighted vertex normals"
  - "hollow_mesh with configurable wall thickness and optional drain hole"
  - "11 integration tests for split, offset, and hollow operations"
affects: [29-05, 29-06, 29-07]

# Tech tracking
tech-stack:
  added: []
  patterns: [topology-based-boundary-detection, ear-clipping-triangulation, vertex-normal-offset]

key-files:
  created:
    - crates/slicecore-mesh/src/csg/split.rs
    - crates/slicecore-mesh/src/csg/offset.rs
    - crates/slicecore-mesh/src/csg/hollow.rs
    - crates/slicecore-mesh/tests/csg_split_hollow.rs
  modified:
    - crates/slicecore-mesh/src/csg/mod.rs

key-decisions:
  - "Topology-based boundary detection for cap generation instead of tracking boundary edges during split"
  - "Ear-clipping triangulation for cap polygons rather than simple fan triangulation"
  - "Hollow mesh uses direct boolean difference of original minus inward-offset solid"

patterns-established:
  - "Boundary-from-topology: find boundary edges by looking for count==1 edges in the mesh, filter to plane"
  - "Position-hash deduplication for intersection points during plane split"
  - "Angle-weighted vertex normals for mesh offset"

requirements-completed: [CSG-06, CSG-07]

# Metrics
duration: 11min
completed: 2026-03-13
---

# Phase 29 Plan 04: Split, Offset, Hollow Summary

**Plane splitting with watertight capping, vertex-normal mesh offset, and mesh hollowing via boolean difference with optional drain hole**

## Performance

- **Duration:** 11 min
- **Started:** 2026-03-13T00:22:31Z
- **Completed:** 2026-03-13T00:34:24Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Plane split with direct vertex classification, triangle splitting, and ear-clipped cap polygon for watertight halves
- Angle-weighted vertex-normal mesh offset for grow/shrink operations
- Mesh hollowing that creates a shell via inward offset + boolean difference, with optional cylindrical/tapered drain hole
- 11 integration tests covering split (capped, uncapped, sphere, diagonal, no-intersection), hollow (box, sphere, drain hole, thick wall warning), and offset (positive, negative)

## Task Commits

Each task was committed atomically:

1. **Task 1: Plane split and mesh offset** - `2149697` (feat)
2. **Task 2: Hollow mesh and integration tests** - `33c7f42` (feat)

## Files Created/Modified
- `crates/slicecore-mesh/src/csg/split.rs` - Plane split with SplitPlane, SplitOptions, ear-clipping cap, boundary detection
- `crates/slicecore-mesh/src/csg/offset.rs` - Vertex-normal mesh offset with angle-weighted normals
- `crates/slicecore-mesh/src/csg/hollow.rs` - Mesh hollowing via offset+difference, DrainHole, HollowOptions
- `crates/slicecore-mesh/src/csg/mod.rs` - Module declarations and re-exports for split, offset, hollow
- `crates/slicecore-mesh/tests/csg_split_hollow.rs` - 11 integration tests

## Decisions Made
- Used topology-based boundary detection (find edges with count==1, filter to plane) rather than tracking boundary edges during triangle splitting. This is more robust because the split triangle emitter doesn't need to maintain consistent edge directions.
- Ear-clipping triangulation for cap polygons on the cut plane, supporting non-convex cross-sections from arbitrary angle splits.
- Hollow mesh subtracts the inward-offset solid directly from the original (no winding flip needed) since the offset mesh maintains correct outward normals.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed borrow conflict with closure-based vertex helpers**
- **Found during:** Task 1 (split.rs compilation)
- **Issue:** Closures capturing above_map/below_map mutably conflicted with passing them to split_triangle
- **Fix:** Replaced closures with standalone add_vert_to function
- **Files modified:** crates/slicecore-mesh/src/csg/split.rs
- **Verification:** Compiles and unit tests pass
- **Committed in:** 2149697

**2. [Rule 1 - Bug] Fixed non-watertight capped halves from plane split**
- **Found during:** Task 2 (integration tests)
- **Issue:** Original cap approach tracked boundary edges during splitting but edge directions were inconsistent, producing caps that didn't connect to boundary
- **Fix:** Rewrote capping to discover boundary from mesh topology (edges with count==1 on the plane), then ear-clip triangulate
- **Files modified:** crates/slicecore-mesh/src/csg/split.rs
- **Verification:** All watertightness assertions pass in integration tests
- **Committed in:** 33c7f42

**3. [Rule 1 - Bug] Fixed hollow mesh producing larger volume than original**
- **Found during:** Task 2 (hollow unit test)
- **Issue:** Flipping inner shell winding before boolean difference confused the classification, producing union-like result
- **Fix:** Use inward-offset mesh directly without winding flip, since it maintains correct outward normals
- **Files modified:** crates/slicecore-mesh/src/csg/hollow.rs
- **Verification:** Hollow volume correctly less than original
- **Committed in:** 33c7f42

---

**Total deviations:** 3 auto-fixed (3 bugs)
**Impact on plan:** Bug fixes during implementation. No scope creep.

## Issues Encountered
None beyond the auto-fixed bugs above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Split, offset, and hollow operations fully functional and tested
- Plans 05-07 can build on these operations for advanced CSG workflows
- All public types re-exported from csg module root

---
*Phase: 29-mesh-boolean-operations-csg*
*Completed: 2026-03-13*
