---
phase: 02-mesh-io-and-repair
plan: 02
subsystem: mesh
tags: [repair, degenerate, normals, stitching, holes, self-intersection, bfs, spatial-hashing, moller]

# Dependency graph
requires:
  - phase: 01-foundation-types
    provides: "Point3, Vec3, BBox3, TriangleMesh, BVH, MeshError"
  - phase: 02-mesh-io-and-repair (plan 01)
    provides: "slicecore-mesh crate structure, TriangleMesh with arena+index pattern"
provides:
  - "repair() pipeline function taking raw vecs and returning TriangleMesh + RepairReport"
  - "Degenerate triangle removal (zero-area, duplicate indices, collinear)"
  - "Normal direction correction via BFS flood-fill"
  - "Edge stitching via spatial hashing for near-duplicate vertex merging"
  - "Hole detection and fan triangulation filling"
  - "Self-intersection detection via Moller triangle-triangle test"
  - "RepairReport struct with Serialize/Deserialize"
affects: [03-vertical-slice, 02-mesh-io-and-repair]

# Tech tracking
tech-stack:
  added: []
  patterns: [admesh-inspired-repair-pipeline, spatial-hashing, bfs-flood-fill, moller-triangle-intersection]

key-files:
  created:
    - crates/slicecore-mesh/src/repair.rs
    - crates/slicecore-mesh/src/repair/degenerate.rs
    - crates/slicecore-mesh/src/repair/normals.rs
    - crates/slicecore-mesh/src/repair/stitch.rs
    - crates/slicecore-mesh/src/repair/holes.rs
    - crates/slicecore-mesh/src/repair/intersect.rs
  modified:
    - crates/slicecore-mesh/src/lib.rs

key-decisions:
  - "Pipeline order: degenerate -> stitch -> normals -> holes -> intersect (normals before holes to avoid false boundaries)"
  - "Stitch tolerance 1e-4 (0.1 micron), well below FDM print resolution"
  - "Self-intersection detection is O(n^2) brute-force with shared-vertex skip (acceptable for 3D printing meshes)"
  - "repair() takes owned Vec<Point3> and Vec<[u32;3]>, returns new TriangleMesh (immutable-after-construction)"

patterns-established:
  - "Repair submodule pattern: each repair step in its own file with tests"
  - "BFS flood-fill for winding consistency propagation"
  - "Spatial hashing grid for O(n) neighbor lookup within tolerance"

# Metrics
duration: 8min
completed: 2026-02-16
---

# Phase 2 Plan 2: Mesh Repair Pipeline Summary

**Full mesh repair pipeline with degenerate removal, edge stitching, BFS normal correction, hole filling, and Moller self-intersection detection**

## Performance

- **Duration:** 8 min
- **Started:** 2026-02-16T21:01:18Z
- **Completed:** 2026-02-16T21:09:45Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Complete repair pipeline following admesh order: removes degenerates, stitches edges, fixes normals, fills holes, detects self-intersections
- RepairReport struct documents every repair action with counts and was_already_clean flag
- 20 new unit tests covering each repair submodule plus integration tests
- WASM-compatible (no std-only dependencies), clippy-clean

## Task Commits

Each task was committed atomically:

1. **Task 1: Degenerate triangle removal and normal direction fix** - `78f34e2` (feat)
2. **Task 2: Edge stitching, hole filling, and self-intersection detection** - `7855d84` (feat)

## Files Created/Modified
- `crates/slicecore-mesh/src/repair.rs` - Pipeline coordinator, RepairReport struct, repair() function
- `crates/slicecore-mesh/src/repair/degenerate.rs` - Removes zero-area and duplicate-index triangles
- `crates/slicecore-mesh/src/repair/normals.rs` - BFS flood-fill winding fix + normal recomputation
- `crates/slicecore-mesh/src/repair/stitch.rs` - Spatial-hash vertex merging for boundary edges
- `crates/slicecore-mesh/src/repair/holes.rs` - Boundary loop detection + fan triangulation
- `crates/slicecore-mesh/src/repair/intersect.rs` - Moller triangle-triangle intersection detection
- `crates/slicecore-mesh/src/lib.rs` - Added repair module and re-exports

## Decisions Made
- Pipeline order changed from plan's "stitch -> holes -> normals" to "stitch -> normals -> holes" -- inconsistent winding creates false boundary edges that confuse hole detection
- Stitch tolerance at 1e-4 mm (0.1 micron) matches plan specification
- Self-intersection detection uses O(n^2) brute-force rather than BVH acceleration -- BVH only supports plane/ray queries, not AABB-overlap queries. For typical 3D printing meshes (<100K triangles) this is acceptable
- repair() signature uses `Vec<Point3>` (owned) not `&mut Vec<Point3>` -- clippy preferred immutable reference and stitch doesn't mutate vertex positions

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed repair pipeline order: normals before holes**
- **Found during:** Task 2 (integration testing)
- **Issue:** Plan specified order degenerate -> stitch -> holes -> normals, but inconsistent winding creates false boundary edges that hole detection interprets as real holes, adding spurious fill triangles
- **Fix:** Moved fix_normal_directions before fill_holes in the pipeline
- **Files modified:** crates/slicecore-mesh/src/repair.rs
- **Verification:** Tetrahedron with one flipped face now correctly reports 0 holes filled after normal fix
- **Committed in:** 7855d84 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** Essential for correctness -- without this fix, every mesh with inconsistent normals would get spurious hole-fill triangles.

## Issues Encountered
None beyond the pipeline order issue documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Repair pipeline ready for integration with STL/OBJ import (plan 02-03/04)
- repair() produces valid TriangleMesh from defective input, ready for Phase 3 vertical slice
- RepairReport provides user-facing diagnostics for mesh quality assessment

## Self-Check: PASSED

All 7 created/modified files verified present. Both task commits (78f34e2, 7855d84) verified in git log.

---
*Phase: 02-mesh-io-and-repair*
*Completed: 2026-02-16*
