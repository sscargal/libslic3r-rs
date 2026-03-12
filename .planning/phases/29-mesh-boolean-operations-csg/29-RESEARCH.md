# Phase 29: Mesh Boolean Operations (CSG) - Research

**Researched:** 2026-03-12
**Domain:** 3D mesh boolean operations, constructive solid geometry, computational geometry
**Confidence:** MEDIUM

## Summary

This phase implements true 3D mesh boolean operations (CSG) within the existing `slicecore-mesh` crate. The core challenge is computing intersection curves between triangle meshes, classifying triangles as inside/outside, and re-triangulating along intersection boundaries -- all while handling degenerate cases (coplanar faces, near-coincident edges, exact-on-plane vertices) robustly.

The Rust ecosystem for 3D mesh booleans is immature. The two existing crates (`csgrs` and `boolmesh`) are either BSP-based (csgrs, which depends on nalgebra/Parry/Rapier -- heavyweight and architectural mismatch) or very new and unproven (boolmesh at 0.1.5). Neither fits the project's pure-Rust, minimal-dependency, arena+index `TriangleMesh` architecture. The recommended approach is to implement mesh booleans from scratch using the well-established algorithm pipeline: BVH broad-phase, Moller triangle-triangle intersection (already in codebase), intersection curve computation, triangle classification via ray casting, and constrained re-triangulation.

**Primary recommendation:** Implement mesh booleans from scratch within `slicecore-mesh`, using the `robust` crate (v1.2.0) for adaptive-precision geometric predicates (`orient3d`) and symbolic perturbation for coplanar face handling. The existing BVH, triangle-triangle intersection test, and repair infrastructure provide a strong foundation.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- All four boolean operations: union, difference, intersection, XOR
- Pairwise API for all ops, plus optimized N-ary union
- Output is always a new `TriangleMesh` (no lazy CSG tree)
- Function-based API: `mesh_union()`, `mesh_difference()`, `mesh_intersection()`, `mesh_xor()`, `mesh_union_many()`
- True 3D mesh booleans (compute intersection curves, classify triangles, re-triangulate)
- Correctness first, optimize later
- Symbolic perturbation for coplanar face handling
- Lives in `slicecore-mesh` (new module: `src/csg/`)
- Dedicated `mesh_split_at_plane()` returning both halves, capped by default
- `hollow_mesh(mesh, wall_thickness)` using mesh offset + CSG difference, optional drain hole
- Robust Minkowski sum for mesh offset (grow/shrink)
- Mesh primitives: box, cylinder, sphere, cone, torus, plane (half-space), wedge
- Configurable tessellation resolution (default 32 segments for cylinder)
- Basic lattice generation (1-2 patterns: cubic, gyroid)
- Optional per-triangle attributes (material ID, color) on TriangleMesh
- CSG operations preserve attributes
- Auto-repair inputs before CSG, error if repair fails
- Auto-cleanup results: merge near-duplicate vertices, remove zero-area triangles, consistent winding
- `CsgReport` with Serialize/Deserialize
- Rich context errors with thiserror (`CsgError`)
- `Result<(TriangleMesh, CsgReport), CsgError>`
- CLI: `slicecore csg <operation> <input1> <input2> -o <output>` with `--json` flag
- CancellationToken support, feature-gated rayon parallelism
- Criterion benchmarks for cube pair, 10K, and 100K triangle mesh pairs
- CSG operations exposed through plugin API traits
- Fuzz target for CSG operations

### Claude's Discretion
- Exact arithmetic approach (Shewchuk predicates, `robust` crate, or adaptive)
- Modifier system integration (whether to wire up modifier.rs in this phase)
- Internal algorithm structure (BSP-based, plane-sweep, or other approach)
- Lattice pattern selection (which 1-2 patterns to implement)
- Material ID propagation strategy

### Deferred Ideas (OUT OF SCOPE)
- Auto cut-to-fit build plate
- CSG undo in UI/application layer
- Connector/alignment features (dovetails, pins)
- Advanced lattice generation
- Benchmark comparison with reference tools
- Full material-aware multi-material CSG (if not covered by Claude's discretion)
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `robust` | 1.2.0 | Adaptive-precision geometric predicates (orient3d, insphere) | Pure Rust port of Shewchuk's predicates; no_std compatible; well-tested |
| `slicecore-mesh` (existing) | workspace | TriangleMesh, BVH, repair, transform | Already in codebase; arena+index pattern is the foundation |
| `slicecore-math` (existing) | workspace | Point3, Vec3, BBox3, Matrix4x4 | Already in codebase; all coordinate math |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `serde` | workspace | Serialize/Deserialize for CsgReport | Already in workspace deps |
| `thiserror` | workspace | CsgError type | Already in workspace deps |
| `criterion` | workspace | Benchmarks | Already in workspace deps |
| `rayon` | (existing feature) | Parallel triangle classification | Already feature-gated in project |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `robust` crate | `geometry-predicates` 0.3.0 | geometry-predicates exposes arithmetic primitives for extensions; `robust` is more widely used (1.2.0, more downloads), simpler API |
| Custom implementation | `csgrs` crate | csgrs depends on nalgebra/Parry/Rapier ecosystem (heavyweight); BSP-based approach doesn't integrate with existing arena+index TriangleMesh |
| Custom implementation | `boolmesh` 0.1.5 | Too new (v0.1.5), MPL-2.0 license vs project MIT/Apache-2.0, unknown robustness |
| Minkowski sum for offset | Voxel-based offset | Minkowski is exact for convex offsets; voxel approach loses precision |

**Installation:**
```bash
cargo add robust --package slicecore-mesh
```

## Architecture Patterns

### Recommended Project Structure
```
crates/slicecore-mesh/src/
  csg/
    mod.rs           # Public API: mesh_union, mesh_difference, etc.
    error.rs         # CsgError type
    report.rs        # CsgReport type
    intersect.rs     # Intersection curve computation between two meshes
    classify.rs      # Triangle classification (inside/outside/boundary)
    retriangulate.rs # Constrained Delaunay triangulation of split triangles
    perturb.rs       # Symbolic perturbation for coplanar face handling
    primitives.rs    # Box, cylinder, sphere, cone, torus, plane, wedge
    split.rs         # mesh_split_at_plane() optimized path
    hollow.rs        # hollow_mesh() via offset + difference
    offset.rs        # Mesh offset via vertex-normal displacement
    lattice.rs       # Basic cubic/gyroid lattice generation
    attributes.rs    # Per-triangle attribute tracking through operations
  triangle_mesh.rs   # Extended with optional per-triangle attributes
  benches/
    csg_bench.rs     # Criterion benchmarks
```

### Pattern 1: CSG Pipeline (Main Algorithm)
**What:** The standard mesh boolean pipeline used by virtually all modern implementations.
**When to use:** All four boolean operations.
**Steps:**
```
1. Auto-repair inputs (existing repair::repair)
2. BVH broad-phase: find candidate triangle pairs (existing bvh::query_aabb_overlaps)
3. Narrow-phase: compute exact intersection segments between candidate pairs
4. Build intersection curves from segments (chains of points on mesh surfaces)
5. Split intersected triangles along intersection curves (constrained retriangulation)
6. Classify all triangles as INSIDE or OUTSIDE relative to other mesh
7. Select triangles based on operation:
   - Union: outside_A + outside_B
   - Intersection: inside_A + inside_B
   - Difference: outside_A + inside_B (with flipped normals on B)
   - XOR: (outside_A + inside_B_flipped) + (outside_B + inside_A_flipped)
8. Merge selected triangles into output mesh
9. Cleanup: weld vertices, remove degenerates, fix winding
```

### Pattern 2: Symbolic Perturbation for Coplanar Handling
**What:** Avoid degenerate geometric configurations by infinitesimal perturbation.
**When to use:** When two triangles lie in the same plane or share an edge.
**Approach:**
- Use the Manifold library's approach: perturb mesh A in the surface normal direction for union, opposite for difference/intersection
- This ensures touching cubes merge, equal-height differences produce through-holes, and mesh minus itself produces empty
- Implementation: when orient3d returns exactly 0 (coplanar), apply a tie-breaking rule based on vertex indices (simulation of simplicity)

### Pattern 3: Triangle Classification via Ray Casting
**What:** Determine if a triangle is inside or outside the other mesh.
**When to use:** After splitting, for every non-intersected triangle.
**Approach:**
- Cast a ray from the triangle centroid in an arbitrary direction
- Count intersections with the other mesh using BVH-accelerated ray casting
- Odd count = inside, even count = outside
- When ray hits vertex/edge exactly, perturb and retry (cascaded approach)

### Pattern 4: Constrained Retriangulation
**What:** Split a triangle along intersection curve segments.
**When to use:** For every triangle that is crossed by an intersection curve.
**Approach:**
- Insert intersection points as constraints on triangle edges
- Use ear-clipping or constrained Delaunay triangulation to re-mesh
- Ear-clipping is simpler and sufficient for the convex sub-polygons created by splitting

### Pattern 5: Vertex-Normal Offset for Mesh Offset
**What:** Approximate Minkowski sum by displacing vertices along averaged normals.
**When to use:** For `hollow_mesh()` and mesh offset operations.
**Approach:**
- Compute per-vertex normals (average of adjacent face normals, weighted by angle)
- Displace each vertex by `thickness * vertex_normal`
- For convex regions this is exact; for concave regions it can produce self-intersections
- Follow with self-intersection repair or CSG union with original to clean up
- Note: True Minkowski sum of general polyhedra is extremely complex (deferred to advanced lattice phase)

### Anti-Patterns to Avoid
- **BSP tree approach for mesh booleans:** BSP trees create many unnecessary triangle splits; the intersection-curve approach creates minimal new geometry
- **Floating-point equality checks:** Never use `==` for geometric predicates; use `robust::orient3d` for all orientation tests
- **Eager BVH rebuild:** Don't rebuild BVH after every intermediate step; batch operations and rebuild once
- **Allocating per-intersection:** Use arena-style allocation for intersection points to avoid allocation pressure during the tight intersection loop
- **Ignoring winding order during merge:** The difference operation must flip winding on the "inside B" triangles before adding them to the result

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Geometric predicates (orient3d) | Custom adaptive arithmetic | `robust` crate v1.2.0 | Shewchuk's predicates are proven correct; custom impl will have precision bugs |
| Triangle-triangle intersection test | New intersection code | Existing `repair::intersect::triangles_intersect` | Already tested and working in codebase |
| BVH spatial queries | New spatial index | Existing `bvh::BVH::query_aabb_overlaps` | SAH-based BVH already optimized |
| Mesh repair pipeline | Custom cleanup | Existing `repair::repair()` | Full repair pipeline already handles degenerates, normals, holes |
| Mesh transforms for primitives | Custom positioning | Existing `transform::translate/rotate/scale` | Already handles winding correction |
| Serialization | Custom JSON output | `serde` derive on CsgReport | Standard pattern throughout codebase |

**Key insight:** The existing codebase already has ~60% of the infrastructure needed (BVH, triangle-triangle intersection, repair, transforms). The core new work is intersection curve computation, constrained retriangulation, and triangle classification.

## Common Pitfalls

### Pitfall 1: Floating-Point Degeneracies
**What goes wrong:** Intersection curve computation produces inconsistent results when triangles are nearly coplanar, causing gaps or overlapping triangles in output.
**Why it happens:** Standard floating-point comparisons (epsilon-based) give contradictory answers for different triangle pairs that share edges.
**How to avoid:** Use `robust::orient3d` for ALL point-plane orientation tests. Never use raw floating-point comparisons for geometric decisions.
**Warning signs:** Output mesh has holes, non-manifold edges, or self-intersections that weren't in the input.

### Pitfall 2: Inconsistent Triangle Classification
**What goes wrong:** A triangle is classified as both inside and outside depending on which neighbor was processed first.
**Why it happens:** Ray casting for classification hits exact edges/vertices, giving ambiguous results. Or classification propagation through connected components has bugs.
**How to avoid:** Use robust ray casting with perturbation retry. Propagate classification through connected mesh patches (all triangles in a connected component of non-intersected triangles have the same classification).
**Warning signs:** Random triangles missing from output, or extra unwanted triangles appearing.

### Pitfall 3: T-Junctions from Retriangulation
**What goes wrong:** After splitting triangles along intersection curves, adjacent triangles don't share the exact same edge vertices, creating tiny gaps.
**Why it happens:** Intersection point computed slightly differently for the two triangles sharing an edge.
**How to avoid:** Compute each intersection point ONCE and reference it by index. Use an intersection point registry that maps (triangle_pair, edge) to a canonical point.
**Warning signs:** Slicing the result mesh produces gaps or extra contours at intersection regions.

### Pitfall 4: Winding Order Corruption
**What goes wrong:** Output mesh has inverted normals on some faces, causing inside-out rendering or incorrect slicing.
**Why it happens:** Difference operation forgets to flip winding on the "inside B" triangles, or retriangulation doesn't preserve the parent triangle's winding.
**How to avoid:** Always track which mesh (A or B) each output triangle came from, and flip B's winding for difference. Verify output winding consistency with existing `repair::normals::fix_normal_directions`.
**Warning signs:** Volume computation returns negative for parts of the output.

### Pitfall 5: Mesh Offset Self-Intersections
**What goes wrong:** Vertex-normal offset produces self-intersecting geometry at concave features (sharp internal edges, thin features).
**Why it happens:** At concave edges, adjacent vertex normals diverge and offset surfaces cross each other.
**How to avoid:** Accept that vertex-normal offset is approximate for concave meshes. After offset, run self-intersection detection and either (a) union the offset mesh with itself to resolve, or (b) warn in CsgReport. For hollowing, the inner offset is typically convex-ward so this is less of an issue.
**Warning signs:** Self-intersection count > 0 in CsgReport after offset operation.

### Pitfall 6: N-ary Union Performance
**What goes wrong:** Naive N-ary union (fold over pairwise union) is O(n^2) in triangle count as each intermediate result grows.
**Why it happens:** Each pairwise union produces a mesh with all previous triangles plus new ones.
**How to avoid:** Use a divide-and-conquer approach: split meshes into pairs, union each pair, repeat. Or better: build a single BVH over all meshes and process all intersections simultaneously.
**Warning signs:** N-ary union of 10+ meshes takes orders of magnitude longer than expected.

## Code Examples

### CsgError Type
```rust
// Source: project conventions (thiserror pattern from repair.rs, error.rs)
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CsgError {
    #[error("mesh A repair failed: {0}")]
    RepairFailedA(#[source] crate::error::MeshError),

    #[error("mesh B repair failed: {0}")]
    RepairFailedB(#[source] crate::error::MeshError),

    #[error("empty result: boolean {operation} produced no triangles")]
    EmptyResult { operation: String },

    #[error("intersection computation failed at triangle pair ({tri_a}, {tri_b}): {reason}")]
    IntersectionFailed {
        tri_a: usize,
        tri_b: usize,
        reason: String,
    },

    #[error("result mesh construction failed: {0}")]
    ResultConstruction(#[source] crate::error::MeshError),

    #[error("operation cancelled")]
    Cancelled,
}
```

### Public API Pattern
```rust
// Source: mirrors slicecore-geo::boolean API pattern
use crate::triangle_mesh::TriangleMesh;

pub fn mesh_union(
    a: &TriangleMesh,
    b: &TriangleMesh,
) -> Result<(TriangleMesh, CsgReport), CsgError> {
    mesh_boolean(a, b, BooleanOp::Union)
}

pub fn mesh_difference(
    a: &TriangleMesh,
    b: &TriangleMesh,
) -> Result<(TriangleMesh, CsgReport), CsgError> {
    mesh_boolean(a, b, BooleanOp::Difference)
}

// Internal shared implementation
fn mesh_boolean(
    a: &TriangleMesh,
    b: &TriangleMesh,
    op: BooleanOp,
) -> Result<(TriangleMesh, CsgReport), CsgError> {
    // 1. Auto-repair inputs
    // 2. BVH broad-phase
    // 3. Intersection curves
    // 4. Retriangulate
    // 5. Classify
    // 6. Select + merge
    // 7. Cleanup
    todo!()
}
```

### Using robust::orient3d for Point-Plane Classification
```rust
// Source: robust crate docs (docs.rs/robust)
use robust::orient3d;

/// Classifies a point relative to a plane defined by three points.
/// Returns: positive = above, negative = below, zero = on plane.
fn point_plane_orientation(
    plane_a: [f64; 3],
    plane_b: [f64; 3],
    plane_c: [f64; 3],
    point: [f64; 3],
) -> f64 {
    orient3d(plane_a, plane_b, plane_c, point)
}
```

### Mesh Primitive: Cylinder
```rust
// Source: standard parametric surface tessellation
use std::f64::consts::TAU;
use slicecore_math::Point3;

pub fn cylinder(radius: f64, height: f64, segments: u32) -> TriangleMesh {
    let segments = segments.max(3);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Bottom center (0) and top center (1)
    vertices.push(Point3::new(0.0, 0.0, 0.0));
    vertices.push(Point3::new(0.0, 0.0, height));

    // Ring vertices: bottom ring starts at 2, top ring at 2 + segments
    for i in 0..segments {
        let angle = TAU * f64::from(i) / f64::from(segments);
        let x = radius * angle.cos();
        let y = radius * angle.sin();
        vertices.push(Point3::new(x, y, 0.0));        // bottom
        vertices.push(Point3::new(x, y, height));      // top
    }

    // Bottom cap, top cap, and side quads (as triangle pairs)
    // ... (standard cylinder tessellation)

    TriangleMesh::new(vertices, indices).expect("cylinder primitive should be valid")
}
```

### Per-Triangle Attributes Extension
```rust
// Source: project pattern (backward-compatible Optional field)
// In triangle_mesh.rs, extend TriangleMesh:

/// Optional per-triangle attributes. When `None`, the mesh has no attributes
/// (backward-compatible with all existing code).
attributes: Option<Vec<TriangleAttributes>>,

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TriangleAttributes {
    /// Material identifier (0 = default/unset).
    pub material_id: u32,
    /// RGBA color (optional per-triangle coloring).
    pub color: Option<[u8; 4]>,
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| BSP-tree CSG | Intersection-curve + classification | ~2013 (Bernstein 2013, Cherchi 2022) | BSP creates unnecessary splits; intersection-curve is minimal |
| Exact rational arithmetic | Adaptive precision (Shewchuk) + symbolic perturbation | 1996-2022 | Exact is slow; adaptive is fast in common case, exact only when needed |
| Epsilon-based degeneracy handling | Simulation of simplicity / symbolic perturbation | 2022 (Manifold library) | Epsilon breaks transitivity; symbolic perturbation is formally correct |
| Naive O(n^2) intersection | BVH-accelerated O(n log n) | Standard practice | Orders of magnitude faster for real meshes |

**Deprecated/outdated:**
- BSP-based mesh booleans: Create O(n) unnecessary triangle splits for n input triangles. Modern approaches only split triangles that actually intersect.
- Fixed-epsilon degeneracy handling: Breaks for meshes at different scales. Adaptive predicates handle all scales.

## Open Questions

1. **Minkowski Sum Complexity**
   - What we know: True Minkowski sum of general 3D polyhedra is extremely complex (O(n^3m^3) for non-convex). Vertex-normal offset is a practical approximation.
   - What's unclear: Whether vertex-normal offset is sufficient for all hollowing use cases, or if users will hit concave-feature issues frequently.
   - Recommendation: Implement vertex-normal offset as "mesh_offset". Document limitations for concave geometry. Defer true Minkowski sum.

2. **Constrained Retriangulation Algorithm Choice**
   - What we know: Ear-clipping works for simple polygons created by splitting a triangle. CDT (Constrained Delaunay Triangulation) produces better-shaped triangles.
   - What's unclear: Whether ear-clipping quality is sufficient or if CDT is needed for downstream slicing quality.
   - Recommendation: Start with ear-clipping (simpler, sufficient for convex sub-polygons from triangle splitting). Switch to CDT only if quality issues emerge.

3. **Plugin API Design for CSG**
   - What we know: Current plugin API only supports InfillPatternPlugin via abi_stable sabi_trait.
   - What's unclear: Whether CSG plugin trait should follow the same abi_stable pattern or use a simpler approach since CSG is less frequently extended.
   - Recommendation: Add CSG operation traits to plugin-api following the same abi_stable pattern for consistency.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test + proptest + criterion |
| Config file | Cargo.toml [dev-dependencies] |
| Quick run command | `cargo test -p slicecore-mesh --lib csg` |
| Full suite command | `cargo test --all-features --workspace` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CSG-01 | Union of two cubes produces correct mesh | integration | `cargo test -p slicecore-mesh csg_union` | Wave 0 |
| CSG-02 | Difference of two cubes produces correct mesh | integration | `cargo test -p slicecore-mesh csg_difference` | Wave 0 |
| CSG-03 | Intersection of two cubes produces correct mesh | integration | `cargo test -p slicecore-mesh csg_intersection` | Wave 0 |
| CSG-04 | XOR of two cubes produces correct mesh | integration | `cargo test -p slicecore-mesh csg_xor` | Wave 0 |
| CSG-05 | N-ary union of multiple meshes | integration | `cargo test -p slicecore-mesh csg_union_many` | Wave 0 |
| CSG-06 | Plane split produces two capped halves | integration | `cargo test -p slicecore-mesh csg_split` | Wave 0 |
| CSG-07 | Hollow mesh produces correct wall thickness | integration | `cargo test -p slicecore-mesh csg_hollow` | Wave 0 |
| CSG-08 | All 7 mesh primitives are watertight | unit | `cargo test -p slicecore-mesh csg_primitives` | Wave 0 |
| CSG-09 | Per-triangle attributes preserved through operations | unit | `cargo test -p slicecore-mesh csg_attributes` | Wave 0 |
| CSG-10 | Coplanar faces handled correctly (touching cubes) | integration | `cargo test -p slicecore-mesh csg_coplanar` | Wave 0 |
| CSG-11 | CLI subcommand produces valid output | integration | `cargo test -p slicecore-cli csg_cli` | Wave 0 |
| CSG-12 | CsgReport serializes to JSON | unit | `cargo test -p slicecore-mesh csg_report_serde` | Wave 0 |
| CSG-13 | Cancellation token stops long operations | integration | `cargo test -p slicecore-mesh csg_cancellation` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-mesh --lib`
- **Per wave merge:** `cargo test --all-features --workspace`
- **Phase gate:** Full suite green + clippy + fmt + doc before verify

### Wave 0 Gaps
- [ ] `crates/slicecore-mesh/src/csg/mod.rs` -- CSG module root
- [ ] `crates/slicecore-mesh/src/csg/error.rs` -- CsgError type
- [ ] `crates/slicecore-mesh/src/csg/report.rs` -- CsgReport type
- [ ] `crates/slicecore-mesh/benches/csg_bench.rs` -- Criterion benchmarks
- [ ] `fuzz/fuzz_targets/fuzz_csg.rs` -- Fuzz target
- [ ] Add `robust = "1.2"` to slicecore-mesh Cargo.toml

## Sources

### Primary (HIGH confidence)
- `robust` crate v1.2.0 docs (docs.rs/robust) -- orient3d, insphere predicates API
- `geometry-predicates` crate v0.3.0 docs (docs.rs/geometry-predicates) -- alternative predicate library
- Existing codebase: `slicecore-mesh/src/repair/intersect.rs` -- Moller triangle-triangle intersection
- Existing codebase: `slicecore-mesh/src/bvh.rs` -- SAH-based BVH with AABB overlap queries
- Existing codebase: `slicecore-geo/src/boolean.rs` -- API pattern for boolean operations

### Secondary (MEDIUM confidence)
- [Manifold Library wiki](https://github.com/elalish/manifold/wiki/Manifold-Library) -- Symbolic perturbation approach, float-based robustness strategy
- [Cherchi et al. "Interactive and Robust Mesh Booleans" (2022)](https://ar5iv.labs.arxiv.org/html/2205.14151) -- Intersection-curve pipeline, exact arithmetic, octree acceleration, implicit point representation
- [csgrs crate](https://crates.io/crates/csgrs) -- BSP-based Rust CSG (evaluated and rejected for architectural mismatch)
- [boolmesh crate](https://crates.io/crates/boolmesh) -- Rust mesh booleans (evaluated and rejected for maturity/license)

### Tertiary (LOW confidence)
- Minkowski sum for general 3D polyhedra -- complex problem; vertex-normal offset is the practical approximation. Exact Minkowski sum deferred.
- Gyroid lattice mesh generation -- mathematical formula is well-known (sin(x)cos(y) + sin(y)cos(z) + sin(z)cos(x) = 0), but generating a clean triangle mesh from an implicit surface requires marching cubes or similar. Needs validation during implementation.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- `robust` crate is well-established, existing codebase provides most infrastructure
- Architecture: MEDIUM -- Pipeline approach is standard in literature, but implementation details (retriangulation, attribute tracking) need validation during coding
- Pitfalls: HIGH -- These are well-documented in computational geometry literature
- Mesh offset: LOW -- Vertex-normal offset is approximate; true Minkowski sum is deferred

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (stable domain, algorithms don't change)
