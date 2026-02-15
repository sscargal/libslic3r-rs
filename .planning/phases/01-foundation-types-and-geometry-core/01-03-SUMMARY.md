---
phase: 01-foundation-types-and-geometry-core
plan: 03
subsystem: mesh
tags: [rust, triangle-mesh, bvh, sah, spatial-queries, ray-tracing, moller-trumbore, mesh-stats, transforms]

# Dependency graph
requires:
  - phase: 01-01
    provides: "Point3, Vec3, BBox3, Matrix4x4 math types"
provides:
  - "TriangleMesh with arena+index pattern (Vec<Point3> + Vec<[u32; 3]>)"
  - "SAH-based BVH with lazy construction via OnceLock"
  - "Plane intersection query (query_plane) for slicing"
  - "Ray intersection query (intersect_ray) with Moller-Trumbore algorithm"
  - "Closest-point-on-mesh brute-force query"
  - "MeshStats: volume, surface area, manifold/watertight checks"
  - "Transform functions: translate, scale, rotate, mirror, center_on_origin, place_on_bed"
  - "MeshError enum with thiserror derives"
affects: [01-04-PLAN, slicecore-slicer, slicecore-support]

# Tech tracking
tech-stack:
  added: []
  patterns: [arena-index-mesh-storage, lazy-bvh-via-oncelock, sah-partitioning, moller-trumbore-intersection, immutable-transform-pattern]

key-files:
  created:
    - crates/slicecore-mesh/Cargo.toml
    - crates/slicecore-mesh/src/lib.rs
    - crates/slicecore-mesh/src/triangle_mesh.rs
    - crates/slicecore-mesh/src/bvh.rs
    - crates/slicecore-mesh/src/spatial.rs
    - crates/slicecore-mesh/src/stats.rs
    - crates/slicecore-mesh/src/transform.rs
    - crates/slicecore-mesh/src/error.rs
  modified: []

key-decisions:
  - "OnceLock for lazy BVH: thread-safe lazy init, TriangleMesh automatically Send+Sync"
  - "SAH with 12 buckets and max 4 triangles per leaf for BVH construction"
  - "Degenerate triangles stored with Vec3::zero() normals, filtered from BVH"
  - "All transforms return new meshes (immutable pattern), original unchanged"
  - "Negative-determinant transforms auto-reverse winding for consistent normals"
  - "Closest-point-on-mesh uses brute-force (acceptable for Phase 1, not a hot path)"

patterns-established:
  - "Immutable mesh transforms: all transform functions return new TriangleMesh instances"
  - "Lazy spatial index: BVH built on first query via OnceLock::get_or_init"
  - "pub(crate) test module pattern: unit_cube() test fixture shared across modules"
  - "Winding correction on reflection: odd number of axis negations flips triangle winding"

# Metrics
duration: 6min
completed: 2026-02-15
---

# Phase 1 Plan 3: slicecore-mesh with TriangleMesh and SAH-based BVH Summary

**TriangleMesh with arena+index pattern, SAH-based BVH with lazy OnceLock construction, plane/ray spatial queries via Moller-Trumbore, mesh statistics (volume/area/manifold), and immutable transform functions -- 35 tests passing**

## Performance

- **Duration:** 6 min
- **Started:** 2026-02-15T04:24:16Z
- **Completed:** 2026-02-15T04:30:28Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- TriangleMesh with arena+index storage pattern (Vec<Point3> + Vec<[u32; 3]>), automatically Send+Sync
- SAH-based BVH built lazily via OnceLock for thread-safe spatial indexing
- Plane intersection query for slicing (finds all triangles spanning a Z height)
- Ray intersection with Moller-Trumbore algorithm and slab-method AABB test
- Closest-point-on-mesh using Ericson's projection method
- MeshStats: volume (divergence theorem), surface area, manifold/watertight/winding checks
- Transform functions: translate, scale, rotate (Rodrigues), mirror, center_on_origin, place_on_bed
- 35 tests passing, clippy clean with -D warnings, rustdoc clean

## Task Commits

Each task was committed atomically:

1. **Task 1: TriangleMesh struct and SAH-based BVH implementation** - `811b206` (feat)
2. **Task 2: Mesh statistics, transforms, and additional test coverage** - `64f7967` (feat)

## Files Created/Modified
- `crates/slicecore-mesh/Cargo.toml` - Crate manifest depending on slicecore-math, serde, thiserror
- `crates/slicecore-mesh/src/lib.rs` - Module declarations and re-exports
- `crates/slicecore-mesh/src/error.rs` - MeshError enum with 5 variants
- `crates/slicecore-mesh/src/triangle_mesh.rs` - TriangleMesh struct with OnceLock<BVH>, 7 tests
- `crates/slicecore-mesh/src/bvh.rs` - SAH-based BVH with plane/ray queries, 8 tests
- `crates/slicecore-mesh/src/spatial.rs` - Convenience spatial query functions, 3 tests
- `crates/slicecore-mesh/src/stats.rs` - MeshStats computation (volume, area, manifold), 8 tests
- `crates/slicecore-mesh/src/transform.rs` - 7 transform functions with winding correction, 8 tests

## Decisions Made
- **OnceLock for lazy BVH:** Uses `std::sync::OnceLock` (Rust 1.75+) for thread-safe lazy BVH initialization. TriangleMesh is automatically Send+Sync without unsafe impls.
- **SAH parameters:** 12 evaluation buckets, max 4 triangles per leaf, traversal/intersection cost ratio 1:1. Standard PBRT-style values.
- **Degenerate triangle handling:** Degenerate triangles (zero-area) get Vec3::zero() normals and are filtered from BVH construction but remain in the mesh for index stability.
- **Immutable transforms:** All transform functions return new meshes. Original unchanged. Follows research recommendation of immutable-after-construction.
- **Winding auto-correction:** Transforms with negative determinant (mirrors, negative scales) automatically reverse triangle winding to maintain consistent outward normals.
- **Brute-force closest point:** closest_point_on_mesh iterates all triangles. Marked with TODO for BVH acceleration. Acceptable for Phase 1 since it's not a hot path.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Clippy flagged `type_complexity` on the edge map HashMap type in stats.rs. Resolved with a local type alias.
- Clippy flagged `for_kv_map` when iterating edge_map with unused key. Fixed by using `.values()` iterator.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- slicecore-mesh crate ready for use by slicecore-slicer (mesh-to-slice pipeline)
- Plane query (query_triangles_at_z) provides the critical slice-layer triangle lookup
- Ray query provides foundation for support generation
- MeshStats enables mesh validation before slicing
- Transform functions allow mesh positioning (center_on_origin, place_on_bed)

## Self-Check: PASSED

All 8 created files verified on disk. Both task commits (811b206, 64f7967) verified in git history.

---
*Phase: 01-foundation-types-and-geometry-core*
*Completed: 2026-02-15*
