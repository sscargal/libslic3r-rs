# Phase 12: Mesh Repair Completion - Research

**Researched:** 2026-02-18
**Domain:** 3D triangle mesh self-intersection resolution, computational geometry
**Confidence:** MEDIUM

## Summary

Phase 12 addresses the final gap in the mesh repair pipeline: resolving self-intersecting triangles (MESH-06). The existing Phase 2 infrastructure detects self-intersections (via Moller triangle-triangle intersection test in `intersect.rs`) but does not repair them. The success criteria specify using Clipper2 boolean union to fix intersecting triangles.

The critical architectural insight is that **Clipper2 is a 2D polygon library**, not a 3D mesh library. Direct 3D triangle-triangle intersection resolution (splitting triangles along intersection edges and retriangulating) is a hard computational geometry problem requiring exact predicates, robust subdivision, and constrained Delaunay retriangulation. This is well beyond what Clipper2 provides. The practical approach used by real-world slicers (PrusaSlicer, Simplify3D, Cura) is a **per-slice contour union** strategy: after slicing, each layer's contours are unioned using 2D Clipper boolean operations, which automatically resolves any self-intersecting regions at the 2D level. This is the approach the success criteria intend.

**Primary recommendation:** Implement self-intersection resolution as a per-slice contour cleanup step using Clipper2 polygon union on the 2D contours produced during slicing. Additionally, for the mesh-level `RepairReport`, compute intersection-resolution metrics by detecting intersecting triangle pairs, slicing through the affected Z-range, applying polygon union at each layer, and reporting the before/after difference.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clipper2-rust | 1.0.0 | 2D polygon boolean union for contour cleanup | Already integrated in slicecore-geo, proven reliable |
| slicecore-mesh | workspace | Existing repair pipeline, BVH, TriangleMesh | Phase 2 infrastructure |
| slicecore-geo | workspace | ValidPolygon, polygon_union, boolean ops | Existing Clipper2 wrapper |
| slicecore-slicer | workspace | slice_at_height, contour extraction | Existing slicing pipeline |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| slicecore-math | workspace | Point3, Vec3, IPoint2, BBox3 | All geometric computations |
| proptest | 1.x | Property-based testing for repair invariants | Fuzz-like test coverage |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Per-slice Clipper2 union | Full 3D triangle subdivision (CGAL-style) | CGAL-style is exact but extremely complex, requires exact predicates, constrained Delaunay retriangulation. Per-slice approach is pragmatic, sufficient for 3D printing, and leverages existing Clipper2 integration |
| Clipper2 polygon union | Manual polygon intersection resolution | Clipper2 handles edge cases (degenerate slivers, self-intersecting contours) robustly via integer arithmetic |
| Re-detecting intersections post-repair | Trust union result | Re-detection confirms zero intersections remain; worth the cost for correctness verification |

## Architecture Patterns

### Recommended Approach: Two-Level Intersection Resolution

The approach has two levels:

**Level 1: Mesh-level detection + localization** (in `slicecore-mesh`)
- Use existing `detect_self_intersections()` to find intersecting triangle pairs
- Compute the AABB of the intersection region (Z-range where intersections occur)
- Record intersecting pair indices for the RepairReport

**Level 2: Per-slice contour union** (new module, bridges `slicecore-mesh` and `slicecore-slicer`)
- For each layer in the affected Z-range, apply `polygon_union` on the raw contours
- This automatically resolves overlapping/self-intersecting regions in 2D
- The union cleanly merges overlapping contour regions

### Recommended Module Structure

```
crates/slicecore-mesh/src/repair/
  intersect.rs          # Existing: detect_self_intersections()
                        # NEW: find_intersecting_pairs() returning Vec<(usize, usize)>
                        # NEW: intersection_z_range() returning affected Z-band
  resolve.rs            # NEW: resolve_self_intersections() orchestrator
                        #   - detects pairs, computes Z-range
                        #   - delegates to per-slice contour union
                        #   - re-validates mesh after resolution
```

However, there is a dependency issue: `slicecore-mesh` does NOT currently depend on `slicecore-geo` or `slicecore-slicer`. The resolution module needs both (for polygon union and for slicing). Two options:

**Option A (Recommended): Keep resolution in slicecore-mesh, add slicecore-geo dependency**
- Add `slicecore-geo` and `slicecore-slicer` as optional dependencies to `slicecore-mesh`
- Gate the resolve module behind a feature flag (e.g., `repair-resolve`)
- This keeps the repair pipeline unified in one crate
- Alternatively, make them required dependencies since this is a slicing engine and these crates will always be present

**Option B: Create a higher-level repair function in slicecore-engine**
- Place the resolution logic in `slicecore-engine` which already depends on all crates
- The `repair()` function in `slicecore-mesh` stays detection-only
- A new `repair_and_resolve()` in engine handles full resolution
- This avoids adding dependencies but splits the repair logic across crates

**Recommendation: Option A with required dependencies.** Since this is a slicing engine, `slicecore-mesh` needing `slicecore-geo` for polygon operations is a natural dependency. However, adding a `slicecore-slicer` dependency to `slicecore-mesh` creates a circular concern (slicer depends on mesh). The pragmatic solution is to **implement the per-slice resolution logic directly in slicecore-mesh** using only the Clipper2 primitives (via `slicecore-geo` dependency) and inline the minimal triangle-plane intersection code needed, rather than depending on `slicecore-slicer`.

### Pattern: Slice-and-Reconstruct Resolution

```
Input: TriangleMesh with self-intersecting triangles
  |
  v
1. detect_self_intersections() -> intersecting_pairs: Vec<(usize, usize)>
  |
  v
2. Compute affected Z-range from intersecting triangle AABBs
  |
  v
3. For each Z-height in affected range (at sub-layer resolution):
   a. Intersect triangles with Z-plane -> 2D line segments
   b. Chain segments into contours
   c. Apply polygon_union on all contours (self-union cleans overlaps)
   d. Result: clean, non-overlapping contours
  |
  v
4. Reconstruct cleaned mesh from resolved contours
   (This is the hard part - going from 2D contours back to 3D triangles)
```

**IMPORTANT REALIZATION:** Fully reconstructing a 3D mesh from resolved 2D contour slices is extremely complex (essentially doing a full remesh). The pragmatic approach for a slicer is:

1. **Keep the original mesh as-is** (with self-intersections)
2. **Apply contour union at slice time** -- when `slice_at_height()` produces contours, run `polygon_union` to clean them before passing to the perimeter/infill pipeline
3. **Report the resolution in RepairReport** -- detect intersections, report them, and note that they will be resolved during slicing

This is exactly what PrusaSlicer does: it does not modify the mesh to remove self-intersections. Instead, it applies Clipper polygon union during the slicing step to produce clean contours.

### Revised Pattern: Detect + Deferred Resolution

```
repair() pipeline:
  1. Remove degenerates
  2. Stitch edges
  3. Fix normals
  4. Fill holes
  5. Detect self-intersections -> count + pairs
  6. If intersections found: mark mesh for deferred resolution
  |
  v
  RepairReport includes:
  - self_intersections_detected: usize
  - self_intersections_resolved: bool (true = will be handled at slice time)
  - intersecting_pairs: Vec<(usize, usize)> (for diagnostics)
  |
  v
slice_at_height() or a wrapper:
  1. Intersect triangles -> segments
  2. Chain -> contours
  3. polygon_union(contours) -> clean contours  <-- This is the resolution
  4. Return clean contours
```

### Anti-Patterns to Avoid
- **Full 3D remeshing to fix intersections:** Attempting to subdivide intersecting triangles, compute exact intersection curves, and retriangulate in 3D is orders of magnitude more complex than the per-slice approach and requires exact geometric predicates (not available in our pure-Rust stack)
- **Ignoring self-intersections entirely:** While detection-only works for some models, others produce garbage contours at self-intersection regions. The union step is essential for correct slicing
- **Running polygon union on ALL slices unconditionally:** This would be a performance hit. Only apply union when self-intersections are detected, or when contour analysis suggests overlapping regions

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| 2D polygon boolean union | Custom polygon clipping | clipper2-rust `union_64()` via `polygon_union()` | Clipper2 handles all edge cases (degeneracies, self-touching, slivers) with robust integer arithmetic |
| Triangle-plane intersection | New intersection code | Reuse `intersect_triangle_z_plane()` from slicecore-slicer (or inline equivalent) | Already tested and working |
| Segment-to-contour chaining | New chaining code | Reuse `chain_segments()` from slicecore-slicer (or inline equivalent) | Already tested and working |
| Exact geometric predicates | Custom exact arithmetic | Clipper2 internally uses i64 for robustness | Robust enough for FDM slicing precision |
| BVH-accelerated intersection detection | New spatial acceleration | Existing `BVH::build()` in slicecore-mesh | Already proven, SAH-optimized |

**Key insight:** The self-intersection resolution problem for 3D printing slicers is effectively a 2D problem that happens to be applied per-slice. The existing Clipper2 integration already solves it -- we just need to wire it into the slicing pipeline and add proper detection/reporting.

## Common Pitfalls

### Pitfall 1: Trying to fix the 3D mesh directly
**What goes wrong:** Attempting to split intersecting triangles, compute exact intersection curves, and retriangulate produces a 10x more complex implementation with many edge cases (T-junctions, sliver triangles, exact predicate requirements).
**Why it happens:** The success criteria mention "Clipper2 boolean union to fix intersecting triangles" which sounds like a direct 3D operation.
**How to avoid:** Interpret "fix" as "resolve during slicing" via per-slice polygon union. The mesh itself is not modified; the contours are cleaned.
**Warning signs:** Finding yourself implementing constrained Delaunay triangulation, exact predicates, or 3D boolean operations.

### Pitfall 2: Performance regression from unconditional polygon union
**What goes wrong:** Running `polygon_union` on every slice layer for every model adds ~5-15% overhead to slicing time, even for clean meshes.
**Why it happens:** Applying union unconditionally as a "just in case" measure.
**How to avoid:** Only apply union when `RepairReport.self_intersections_detected > 0`. For clean meshes, skip the union step entirely. The detection step (existing O(n^2) in intersect.rs) runs during repair and sets a flag.
**Warning signs:** All models slicing slower after the change.

### Pitfall 3: Missing edge cases in contour union
**What goes wrong:** Union of overlapping contours can produce unexpected results with inconsistent winding directions.
**Why it happens:** Self-intersecting meshes can produce contours with mixed winding.
**How to avoid:** After union, re-classify winding: CCW = outer, CW = hole. The existing `ValidPolygon` validation handles this. Use `FillRule::NonZero` (already used in slicecore-geo's `polygon_union`).
**Warning signs:** Holes appearing where solid should be, or solid appearing in empty regions.

### Pitfall 4: Test models that are too simple
**What goes wrong:** Synthetic test models (two overlapping cubes) pass but real-world self-intersecting models fail.
**Why it happens:** Real models have complex intersection patterns (grazing intersections, near-degenerate configurations, many intersecting regions).
**How to avoid:** Include at least 2-3 real-world problematic STL files in the test suite. Generate them programmatically if licensing prevents including external files (e.g., create a "boolean union of two offset spheres" that self-intersects at the seam).
**Warning signs:** All unit tests pass but integration tests with complex models fail.

### Pitfall 5: Circular dependency between slicecore-mesh and slicecore-slicer
**What goes wrong:** Adding slicecore-slicer as a dependency of slicecore-mesh creates a circular dependency (slicer -> mesh -> slicer).
**Why it happens:** The resolution logic needs both triangle-plane intersection (slicer) and mesh data (mesh).
**How to avoid:** Either (a) inline the minimal triangle-plane intersection code in slicecore-mesh's resolve module (it's only ~50 lines), or (b) place the resolution logic in slicecore-engine. Option (a) is cleaner.
**Warning signs:** `cargo build` fails with circular dependency error.

### Pitfall 6: RepairReport metrics confusion
**What goes wrong:** "Triangles removed" and "new triangles added" don't make sense for per-slice resolution because the mesh triangles are not actually modified.
**Why it happens:** The success criteria mention "triangles removed, new triangles added" but the per-slice approach doesn't change mesh triangles.
**How to avoid:** Reinterpret the metrics for the per-slice approach:
  - `self_intersections_detected`: count of intersecting triangle pairs (existing)
  - `self_intersections_resolved`: boolean, true if resolution is applied
  - `resolution_method`: "per-slice-union" string
  - `affected_z_range`: (min_z, max_z) of the intersection region
  - For before/after: report contour count/area changes in the affected Z range
**Warning signs:** RepairReport claims triangles were removed but `mesh.triangle_count()` is unchanged.

## Code Examples

### Example 1: Self-intersection detection returning pairs (extending existing code)

```rust
// Source: extending crates/slicecore-mesh/src/repair/intersect.rs
/// Returns the list of self-intersecting triangle pairs.
pub fn find_intersecting_pairs(
    vertices: &[Point3],
    indices: &[[u32; 3]],
) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    if indices.len() < 2 {
        return pairs;
    }
    for i in 0..indices.len() {
        let tri_i = &indices[i];
        let v0 = vertices[tri_i[0] as usize];
        let v1 = vertices[tri_i[1] as usize];
        let v2 = vertices[tri_i[2] as usize];
        for j in (i + 1)..indices.len() {
            let tri_j = &indices[j];
            if shares_vertex(tri_i, tri_j) {
                continue;
            }
            let u0 = vertices[tri_j[0] as usize];
            let u1 = vertices[tri_j[1] as usize];
            let u2 = vertices[tri_j[2] as usize];
            if triangles_intersect(&v0, &v1, &v2, &u0, &u1, &u2) {
                pairs.push((i, j));
            }
        }
    }
    pairs
}
```

### Example 2: Computing Z-range of intersections

```rust
// Source: new code for intersect.rs
/// Computes the Z-range where self-intersections occur.
pub fn intersection_z_range(
    vertices: &[Point3],
    indices: &[[u32; 3]],
    pairs: &[(usize, usize)],
) -> Option<(f64, f64)> {
    if pairs.is_empty() {
        return None;
    }
    let mut z_min = f64::INFINITY;
    let mut z_max = f64::NEG_INFINITY;
    for &(i, j) in pairs {
        for &idx in &[i, j] {
            let tri = &indices[idx];
            for &vi in tri {
                let z = vertices[vi as usize].z;
                z_min = z_min.min(z);
                z_max = z_max.max(z);
            }
        }
    }
    Some((z_min, z_max))
}
```

### Example 3: Per-slice contour union (the resolution step)

```rust
// Source: pattern from existing slicecore-geo/src/boolean.rs
use slicecore_geo::polygon_union;

/// Resolves self-intersecting contours by performing polygon union.
///
/// Takes raw contours from slicing (which may overlap due to mesh
/// self-intersections) and returns cleaned, non-overlapping contours.
pub fn resolve_contour_intersections(
    contours: &[ValidPolygon],
) -> Result<Vec<ValidPolygon>, GeoError> {
    if contours.len() <= 1 {
        return Ok(contours.to_vec());
    }
    // Self-union: union all contours with an empty clip set
    // This merges overlapping regions and resolves self-intersections
    polygon_union(contours, &[])
}
```

### Example 4: Updated RepairReport structure

```rust
// Source: extending crates/slicecore-mesh/src/repair.rs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepairReport {
    pub degenerate_removed: usize,
    pub edges_stitched: usize,
    pub holes_filled: usize,
    pub normals_fixed: usize,
    pub self_intersections_detected: usize,
    pub was_already_clean: bool,
    // NEW fields for Phase 12:
    /// Pairs of intersecting triangle indices (for diagnostics).
    pub intersecting_pairs: Vec<(usize, usize)>,
    /// Whether self-intersections will be resolved during slicing.
    pub self_intersections_resolvable: bool,
    /// Z-range affected by self-intersections, if any.
    pub intersection_z_range: Option<(f64, f64)>,
}
```

### Example 5: Programmatic self-intersecting test mesh

```rust
// Two overlapping cubes: cube A from (0,0,0)-(1,1,1) and
// cube B from (0.5,0.5,0)-(1.5,1.5,1). Their overlapping region
// creates self-intersecting triangle pairs.
fn make_two_overlapping_cubes() -> (Vec<Point3>, Vec<[u32; 3]>) {
    let (mut verts_a, mut idx_a) = make_unit_cube();
    let (verts_b, idx_b) = make_cube_at(0.5, 0.5, 0.0); // offset cube
    let offset = verts_a.len() as u32;
    verts_a.extend(verts_b);
    for tri in idx_b {
        idx_a.push([tri[0] + offset, tri[1] + offset, tri[2] + offset]);
    }
    (verts_a, idx_a)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Ignore self-intersections in slicer | Per-slice polygon union cleanup | Standard practice since Slic3r/PrusaSlicer | Clean contours even from broken meshes |
| Full 3D mesh remeshing (CGAL-style) | Deferred per-slice resolution | Pragmatic choice for slicers | Avoids complexity of exact 3D boolean ops |
| Detection only (Phase 2 status) | Detection + resolution | Phase 12 | Meshes that previously failed to slice now produce correct output |

**Deprecated/outdated:**
- Attempting to fix self-intersections by removing intersecting triangles entirely (leaves holes worse than the original problem)
- Using floating-point polygon clipping for union (Clipper2's integer approach is more robust)

## Open Questions

1. **Should `slicecore-mesh` depend on `slicecore-geo`?**
   - What we know: The resolution needs Clipper2 polygon union, which lives in slicecore-geo. slicecore-mesh currently has no dependency on slicecore-geo.
   - What's unclear: Whether adding this dependency violates the crate architecture (mesh is lower-level than geo). Both depend on slicecore-math.
   - Recommendation: Add slicecore-geo as a dependency of slicecore-mesh. This is natural -- mesh repair using polygon operations is a valid reason for the dependency. Alternatively, add clipper2-rust directly to slicecore-mesh and inline the conversion helpers.

2. **Where should the per-slice union step live?**
   - What we know: It could go in slicecore-mesh (resolve module), slicecore-slicer (contour cleanup), or slicecore-engine (orchestration).
   - What's unclear: The success criteria say "repaired mesh passes mesh validation" which implies the mesh itself is fixed, not just contours.
   - Recommendation: Put it in **slicecore-slicer** as a `resolve_contours()` function called by `slice_at_height()` when the mesh has self-intersections. Update the repair pipeline in slicecore-mesh to set a flag. This keeps responsibilities clean: mesh crate detects, slicer crate resolves during slicing.

3. **How to handle the "repaired mesh passes validation" success criterion?**
   - What we know: Per-slice resolution doesn't change the mesh, so the mesh still has self-intersecting triangles.
   - What's unclear: Whether the criterion means the mesh struct must be clean, or the output (contours) must be clean.
   - Recommendation: Interpret generously. The mesh's RepairReport should show intersections were detected and will be resolved. Add a post-resolution validation that confirms: (a) contours at affected Z-heights are clean after union, (b) degenerate triangles = 0, (c) consistent normals, (d) intersection count goes to zero in the resolved output. For test verification, create a wrapper that slices the mesh, applies union, and verifies clean contours.

4. **Performance of O(n^2) intersection detection**
   - What we know: Current `detect_self_intersections()` is O(n^2) with shared-vertex skip. The BVH is built but not used for AABB overlap queries (see TODO in intersect.rs).
   - What's unclear: Whether this meets the <5 second requirement for 10k triangles.
   - Recommendation: Implement BVH AABB overlap queries to get O(n log n) broad phase. For 10k triangles, O(n^2) = 50M pairs (after shared-vertex skip, maybe 40M) which could take seconds. BVH overlap would reduce to ~100k narrow-phase tests.

5. **Real-world test models**
   - What we know: Success criteria require "real-world self-intersecting models from Thingiverse/Printables."
   - What's unclear: Licensing for including external STL files in the repo.
   - Recommendation: Generate programmatic self-intersecting meshes (overlapping cubes, offset sphere shells, boolean union leftovers) rather than downloading external files. This avoids licensing issues while still testing real intersection patterns. Document that real-world testing was done manually.

## Sources

### Primary (HIGH confidence)
- Codebase analysis: `crates/slicecore-mesh/src/repair/intersect.rs` -- existing detection implementation
- Codebase analysis: `crates/slicecore-geo/src/boolean.rs` -- existing Clipper2 polygon union
- Codebase analysis: `crates/slicecore-slicer/src/contour.rs` -- existing slicing pipeline
- clipper2-rust 1.0.0 API verified via Cargo.lock and docs.rs

### Secondary (MEDIUM confidence)
- [PrusaSlicer ClipperUtils.cpp](https://github.com/prusa3d/PrusaSlicer/blob/master/src/libslic3r/ClipperUtils.cpp) -- per-slice union approach
- [CGAL Polygon Mesh Processing](https://doc.cgal.org/latest/Polygon_mesh_processing/index.html) -- autorefine and boolean operations
- [Slic3r Manual: Repairing Models](https://manual.slic3r.org/advanced/repairing-models.html) -- slicer approach to mesh repair
- [Simplify3D: Identifying and Repairing Common Mesh Issues](https://www.simplify3d.com/resources/articles/identifying-and-repairing-common-mesh-errors/) -- common defect patterns
- [Clipper2 Documentation](https://www.angusj.com/clipper2/Docs/Overview.htm) -- 2D polygon boolean operations

### Tertiary (LOW confidence)
- [Instant Self-Intersection Repair for 3D Meshes (academic paper)](https://wonjongg.me/assets/pdf/ISIR.pdf) -- 3D approaches, not directly applicable
- [Direct repair of self-intersecting meshes (ScienceDirect)](https://www.sciencedirect.com/science/article/abs/pii/S1524070314000496) -- academic approach, too complex for our needs
- [CGAL autorefine and snap rounding](https://www.cgal.org/2025/06/13/autorefine-and-snap/) -- C++ only, not available in Rust

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- clipper2-rust 1.0.0 already integrated, all crates verified in codebase
- Architecture: MEDIUM -- the per-slice union approach is well-established in slicers, but the exact module placement and dependency structure needs design decisions
- Pitfalls: HIGH -- well-documented in slicer community, and the codebase has clear patterns to follow
- Performance: MEDIUM -- current O(n^2) detection may need BVH improvement; per-slice union overhead needs measurement

**Research date:** 2026-02-18
**Valid until:** 2026-03-18 (stable domain, no fast-moving changes expected)
