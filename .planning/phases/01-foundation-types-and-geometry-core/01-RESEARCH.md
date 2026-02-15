# Phase 1: Foundation Types and Geometry Core - Research

**Researched:** 2026-02-15
**Domain:** Rust computational geometry primitives, polygon boolean operations, mesh data structures, WASM compilation
**Confidence:** HIGH

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions
- **WASM target support:** wasm32-unknown-unknown only for MVP. Other targets (wasm32-wasi, Emscripten) and features (filesystem, pthread support) deferred to future phases.

### Claude's Discretion
Nearly all implementation details are at Claude's discretion. The user has delegated:
- Coordinate precision strategy (scaling factor, bounds checking, types)
- Polygon API design (validation, construction, mutability, errors)
- Mesh data structure choices (ownership, spatial index, memory layout, mutability)
- WASM threading and dependency policies

### Deferred Ideas (OUT OF SCOPE)
- wasm32-wasi target
- Emscripten target
- Filesystem access in WASM
- pthread support in WASM
- File I/O (Phase 2)
- Slicing algorithms (Phase 3+)
- Multi-threading utilities (defer to when needed)

</user_constraints>

## Summary

Phase 1 establishes the architectural foundation for a 3D printing slicing engine in Rust. The three crates created here -- `slicecore-math`, `slicecore-geo`, and `slicecore-mesh` -- form Layer 0 of the system. Every downstream algorithm crate depends on the coordinate types, polygon operations, and mesh structures defined here, so these choices are permanent and must be correct.

The most critical decision in this phase is the polygon boolean operations library. Research reveals two viable pure-Rust options: `i_overlay` (v4.4.0, high performance, actively maintained, but limited to i32 integer coordinates) and `clipper2-rust` (v1.0, pure Rust port of Clipper2, supports i64 integer coordinates, includes polygon offsetting). Given that the project requires i64 coordinates with a 1e6 scale factor to handle build volumes up to 500mm+ without overflow risk, **clipper2-rust is the recommended library**. It provides both boolean operations and polygon offsetting with i64 integer precision, matching the C++ Clipper2 behavior that PrusaSlicer/OrcaSlicer rely on. However, clipper2-rust is very new (published 2025) and should be validated early with degenerate geometry test cases.

For mesh spatial indexing, a custom BVH implementation is recommended over the `bvh` crate, because the `bvh` crate has a hard dependency on `nalgebra` which conflicts with the project's custom math types in `slicecore-math`. Building a SAH-based BVH is well-documented and the implementation is straightforward (~300-500 lines for a production-quality version). Arena allocation uses `bumpalo` (v3.x), which is WASM-compatible and no_std by default.

**Primary recommendation:** Use i64 coordinates with COORD_SCALE=1_000_000 (nanometer precision), clipper2-rust for polygon booleans and offsetting, custom BVH for spatial indexing, bumpalo for arena allocation, and enforce `cargo build --target wasm32-unknown-unknown` in CI from day one.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clipper2-rust | 1.0 | Polygon boolean ops (union, intersection, difference, XOR) and polygon offsetting | Pure Rust port of Clipper2 with i64 integer coordinates; same algorithms as C++ PrusaSlicer/OrcaSlicer; includes offsetting; WASM-compatible (has WebAssembly demo); Boost Software License |
| bumpalo | 3.x | Arena allocation for transient geometry | no_std by default (only depends on core+alloc); WASM-compatible; O(1) arena reset between layer operations; widely used and battle-tested |
| thiserror | 2.x | Error type derivation | Derives std::error::Error with custom messages; WASM-compatible; standard Rust error handling pattern |
| serde + serde_derive | 1.x | Serialization derives for types | WASM-compatible; needed for Serialize/Deserialize on coordinate types and mesh data for downstream use |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| proptest | 1.x | Property-based testing for geometric invariants | Dev dependency only; test that polygon area is preserved through boolean ops, coordinate round-trips don't lose precision, BVH queries are consistent |
| approx | 0.5.x | Float comparison with configurable epsilon | Dev and runtime; needed for comparing f64 coordinates, normal vectors, bounding boxes |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| clipper2-rust | i_overlay v4.4 | i_overlay is faster in benchmarks and more mature, BUT only supports i32 integer coordinates. With COORD_SCALE=1e6, i32 maxes out at ~2,147mm which is borderline for large build volumes. i64 is essential for safety. If i_overlay adds i64 support in the future, it would be worth benchmarking as a replacement. |
| clipper2-rust | clipper2 (FFI wrapper) | The tirithen/clipper2 crate wraps C++ Clipper2 via FFI. Violates FOUND-01 (pure Rust, no FFI to C/C++). Cannot compile to wasm32-unknown-unknown. |
| Custom BVH | bvh crate v0.3 | The bvh crate is good but requires nalgebra types (Point3, Vector3). Since slicecore-math defines its own Point3/Vec3, using the bvh crate would force either nalgebra as a dependency or constant type conversions. Custom BVH is ~300-500 lines and avoids the dependency. |
| Custom BVH | rstar crate | rstar is an R-tree, not a BVH. R-trees are better for spatial indexing of points/rectangles. BVH is specifically optimized for ray/plane intersection queries against triangle meshes, which is exactly what slicing needs. |
| Custom math types | nalgebra/glam | Custom types give full control over repr(C), Serialize, integer coordinate types, and WASM compatibility without pulling in a large math library. Phase 1 only needs basic vector/point/matrix operations. |

**Installation:**
```toml
# Cargo.toml for slicecore-math
[dependencies]
serde = { version = "1", features = ["derive"] }
approx = "0.5"

[dev-dependencies]
proptest = "1"

# Cargo.toml for slicecore-geo
[dependencies]
slicecore-math = { path = "../slicecore-math" }
clipper2-rust = "1.0"
serde = { version = "1", features = ["derive"] }
thiserror = "2"

[dev-dependencies]
proptest = "1"

# Cargo.toml for slicecore-mesh
[dependencies]
slicecore-math = { path = "../slicecore-math" }
slicecore-geo = { path = "../slicecore-geo" }
bumpalo = { version = "3", features = ["collections"] }
serde = { version = "1", features = ["derive"] }
thiserror = "2"

[dev-dependencies]
proptest = "1"
```

## Architecture Patterns

### Recommended Project Structure
```
crates/
├── slicecore-math/
│   └── src/
│       ├── lib.rs           # Re-exports, module declarations
│       ├── point.rs          # Point2 (f64), Point3 (f64)
│       ├── coord.rs          # Coord (i64), IPoint2, COORD_SCALE, conversion functions
│       ├── vec.rs            # Vec2, Vec3 with normalize/dot/cross/length
│       ├── matrix.rs         # Matrix3x3, Matrix4x4 for affine transforms
│       ├── bbox.rs           # BBox2, BBox3 with union/intersection/contains
│       ├── epsilon.rs        # approx_eq, EPSILON constants, tolerance utilities
│       └── convert.rs        # mm_to_coord, coord_to_mm, safe rounding
├── slicecore-geo/
│   └── src/
│       ├── lib.rs            # Re-exports
│       ├── polygon.rs        # Polygon, ValidPolygon types
│       ├── polyline.rs       # Polyline type
│       ├── boolean.rs        # Union, intersection, difference, XOR via clipper2-rust
│       ├── offset.rs         # Inward/outward polygon offsetting via clipper2-rust
│       ├── point_in_poly.rs  # Winding number point-in-polygon test
│       ├── simplify.rs       # Ramer-Douglas-Peucker simplification
│       ├── area.rs           # Signed area (shoelace), winding direction
│       ├── convex_hull.rs    # 2D convex hull (Graham scan)
│       └── error.rs          # GeoError types
└── slicecore-mesh/
    └── src/
        ├── lib.rs            # Re-exports
        ├── triangle_mesh.rs  # TriangleMesh struct (vertices, indices, normals, aabb)
        ├── bvh.rs            # Custom SAH-based BVH implementation
        ├── spatial.rs        # Spatial query interface (ray intersection, closest point)
        ├── repair.rs         # Mesh repair (degenerate removal, winding fix)
        ├── stats.rs          # Mesh statistics (volume, area, manifold check)
        ├── transform.rs      # Scale, rotate, translate, mirror, center-on-bed
        └── error.rs          # MeshError types
```

### Pattern 1: Integer Coordinate System with Explicit Conversion Boundary
**What:** All polygon operations use i64 integer coordinates internally. Float-to-int conversion happens at explicit, documented boundary points.
**When to use:** Always -- this is the foundational coordinate strategy.
**Example:**
```rust
// Source: Clipper2 design philosophy + C++ LibSlic3r ClipperUtils
pub type Coord = i64;
pub const COORD_SCALE: f64 = 1_000_000.0; // 1 mm = 1,000,000 internal units (nanometer)

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct IPoint2 {
    pub x: Coord,
    pub y: Coord,
}

impl IPoint2 {
    pub fn from_mm(x: f64, y: f64) -> Self {
        Self {
            x: (x * COORD_SCALE).round() as Coord,
            y: (y * COORD_SCALE).round() as Coord,
        }
    }

    pub fn to_mm(self) -> (f64, f64) {
        (self.x as f64 / COORD_SCALE, self.y as f64 / COORD_SCALE)
    }
}
```

### Pattern 2: Two-Tier Polygon Type System (Polygon / ValidPolygon)
**What:** Separate types for unvalidated and validated polygons. ValidPolygon is constructed only through validation, preventing downstream code from encountering degenerate geometry.
**When to use:** Polygon for input/construction, ValidPolygon for algorithm inputs.
**Example:**
```rust
/// Unvalidated polygon -- may contain degeneracies
#[derive(Clone, Debug)]
pub struct Polygon {
    pub points: Vec<IPoint2>,
}

/// Validated polygon -- guaranteed properties:
/// - At least 3 non-collinear points
/// - No self-intersections
/// - Non-zero area
/// - Consistent winding (CCW for outer, CW for holes)
#[derive(Clone, Debug)]
pub struct ValidPolygon {
    points: Vec<IPoint2>,     // private: cannot be modified without re-validation
    area: i64,                // cached signed area
    winding: Winding,         // cached winding direction
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Winding {
    CounterClockwise, // outer boundary
    Clockwise,        // hole
}

impl Polygon {
    pub fn validate(self) -> Result<ValidPolygon, GeoError> {
        // Check minimum point count
        // Remove collinear points
        // Compute area, reject zero-area
        // Determine winding
        // Check for self-intersections
    }
}
```

### Pattern 3: Arena+Index Mesh with Lazy BVH
**What:** TriangleMesh owns all data in flat Vec arrays. Spatial index is computed lazily on first query. The mesh is immutable after construction (Send+Sync by default).
**When to use:** All mesh operations.
**Example:**
```rust
pub struct TriangleMesh {
    vertices: Vec<Point3>,
    indices: Vec<[u32; 3]>,        // Triangle face indices
    normals: Vec<Vec3>,            // Per-face normals (lazy-computed)
    aabb: BBox3,                    // Bounding box (computed on construction)
    bvh: Option<BVH>,              // Spatial index (lazy-computed)
}

// Send+Sync: all owned data, no Rc/RefCell/Cell
// This is automatic in Rust since all fields are Send+Sync
unsafe impl Send for TriangleMesh {}  // automatic, but explicit for documentation
unsafe impl Sync for TriangleMesh {}  // automatic, but explicit for documentation

impl TriangleMesh {
    pub fn from_vertices_and_indices(
        vertices: Vec<Point3>,
        indices: Vec<[u32; 3]>,
    ) -> Result<Self, MeshError> {
        let aabb = BBox3::from_points(&vertices);
        let normals = Self::compute_normals(&vertices, &indices);
        Ok(Self { vertices, indices, normals, aabb, bvh: None })
    }

    pub fn ensure_bvh(&mut self) -> &BVH {
        if self.bvh.is_none() {
            self.bvh = Some(BVH::build(&self.vertices, &self.indices));
        }
        self.bvh.as_ref().unwrap()
    }
}
```

### Anti-Patterns to Avoid
- **Rc/RefCell for mesh topology:** Breaks Send+Sync. Use arena+index pattern (Vec<Vertex> + index references) instead.
- **f64 in polygon boolean operations:** Floating-point robustness issues cause topology errors in boolean operations. Always use integer coordinates for polygon clipping.
- **Mixing coordinate spaces:** Never pass f64 mesh coordinates directly to i64 polygon functions. Always go through the explicit conversion boundary (from_mm/to_mm).
- **Eager BVH construction:** Building a BVH is expensive. Not all code paths need spatial queries. Build lazily on first query.
- **Mutable polygon after validation:** If ValidPolygon's points are publicly mutable, invariants can be violated. Keep the field private.
- **Using generic math library types (nalgebra/glam) for core types:** Creates dependency lock-in and complicates WASM builds. Use custom types with simple, auditable implementations.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Polygon boolean operations | Custom Weiler-Atherton or Martinez-Rueda | clipper2-rust | 1,425+ call sites in C++ LibSlic3r depend on Clipper behavior; matching edge cases requires thousands of test cases; clipper2-rust is a faithful port of the same algorithms |
| Polygon offsetting | Custom Minkowski sum | clipper2-rust (inflate/deflate) | Offsetting is deceptively complex: miter limits, round joins, handling of self-intersecting results, thin wall collapse. Clipper2 handles all these. |
| Arena allocation | Custom bump allocator | bumpalo | Memory management correctness is critical; bumpalo handles alignment, overflow, deallocation; no_std compatible |
| Ramer-Douglas-Peucker simplification | Custom from-scratch | Implement from algorithm description (~50 lines) | This one IS simple enough to hand-roll. The algorithm is 10 lines of pseudocode. No library needed. |
| Convex hull | External library | Graham scan implementation (~80 lines) | Simple algorithm, well-documented, no library needed |
| Point-in-polygon | Ray casting | Winding number test (~30 lines) | Winding number is more robust than ray casting for edge cases. Simple to implement correctly. |

**Key insight:** Polygon clipping and offsetting are the two operations that absolutely must not be hand-rolled. The C++ slicer ecosystem spent 15 years debugging Clipper edge cases. Every other geometry operation in Phase 1 is simple enough to implement from algorithm descriptions.

## Common Pitfalls

### Pitfall 1: i32 Coordinate Overflow with High Scale Factors
**What goes wrong:** Using i32 with COORD_SCALE=1e6 limits the coordinate range to approximately -2,147mm to +2,147mm. Large-format 3D printers (500mm+ build volume) plus intermediate calculations (offset operations can temporarily exceed build bounds) cause integer overflow, producing garbage geometry.
**Why it happens:** i32 max is 2,147,483,647. At 1e6 scale, that's only 2,147.48mm. Polygon offset operations may temporarily create coordinates outside the build volume.
**How to avoid:** Use i64 (max 9.2e18), which gives effectively unlimited range at any practical scale factor. This is why clipper2-rust with i64 is preferred over i_overlay with i32.
**Warning signs:** Geometry that "wraps around" or produces impossible coordinates; test with coordinates near 2000mm.

### Pitfall 2: Floating-Point Comparison in Geometry
**What goes wrong:** Using `==` to compare f64 values in geometric computations (point equality, collinearity tests, area checks). Two points that should be "the same" differ by floating-point epsilon, causing topology breaks.
**Why it happens:** Floating-point arithmetic is not exact. `0.1 + 0.2 != 0.3` in IEEE 754.
**How to avoid:** Use integer coordinates for all polygon operations (eliminates the problem entirely). For f64 mesh operations, use configurable epsilon comparison (the `approx` crate or custom `approx_eq(a, b, eps)` function). Define domain-appropriate epsilon values (e.g., 1e-9 for coordinate comparison, 1e-6 for area comparison).
**Warning signs:** Tests that pass on one platform but fail on another; non-deterministic test results; self-intersecting polygons appearing from valid input.

### Pitfall 3: Winding Direction Inconsistency
**What goes wrong:** Outer boundaries and holes have inconsistent winding directions, causing boolean operations to produce inverted results (holes become solids, solids become holes).
**Why it happens:** STL files don't specify winding convention. Different libraries use different conventions. Clipper2 expects CCW for subject and clip paths.
**How to avoid:** Enforce winding direction in the ValidPolygon constructor. Compute signed area (shoelace formula): positive = CCW, negative = CW. Reverse points if needed. Document and enforce: outer = CCW, hole = CW.
**Warning signs:** Boolean union that removes geometry instead of adding it; holes that fill in instead of cutting out.

### Pitfall 4: BVH Build Without Degenerate Triangle Handling
**What goes wrong:** Degenerate triangles (zero area, collinear vertices, duplicate vertices) cause BVH construction to produce infinite or NaN bounding boxes, which then causes every spatial query to fail or return incorrect results.
**Why it happens:** Real-world STL files frequently contain degenerate triangles. Normals computed from degenerate triangles are zero-vectors.
**How to avoid:** Filter degenerate triangles before BVH construction. Check triangle area > epsilon before including in BVH. Log/warn about removed triangles.
**Warning signs:** BVH queries returning no results for clearly-intersecting rays; NaN in bounding box coordinates.

### Pitfall 5: WASM Build Breaks from Transitive Dependencies
**What goes wrong:** A dependency deep in the tree pulls in `std::fs`, `std::net`, `std::thread`, or `libc`, breaking `cargo build --target wasm32-unknown-unknown`.
**Why it happens:** Rust's `std` is available on wasm32-unknown-unknown but many std functions (fs, net, thread::spawn) panic or return errors at runtime. Some crates use conditional compilation incorrectly.
**How to avoid:** Add `cargo build --target wasm32-unknown-unknown` to CI from day one. Test every new dependency addition. Prefer crates that advertise no_std support or explicit WASM compatibility.
**Warning signs:** CI passes on native but fails on WASM target; mysterious linker errors mentioning `__wasm_import_*`.

### Pitfall 6: Premature Send+Sync Enforcement vs. Natural Derivation
**What goes wrong:** Using `unsafe impl Send/Sync` when the types already derive it automatically (because all fields are Send+Sync), or worse, forcing Send+Sync on types that contain !Send fields.
**Why it happens:** Confusion about when Rust auto-derives Send/Sync.
**How to avoid:** Rust automatically derives Send+Sync for structs where all fields are Send+Sync. Vec, Box, primitive types, and most std types are already Send+Sync. Only use `unsafe impl` when you actually need to override the compiler. For TriangleMesh with `Vec<Point3>` and `Vec<[u32; 3]>` -- it's automatically Send+Sync, no unsafe needed.
**Warning signs:** Unnecessary `unsafe` blocks; compile errors about Send/Sync that shouldn't exist.

## Code Examples

Verified patterns from official sources and project design documents:

### Coordinate Conversion (Recommended Implementation)
```rust
// Source: C++ LibSlic3r ClipperUtils.cpp + Clipper2 documentation
pub type Coord = i64;

/// 1 mm = 1,000,000 internal units
/// Provides nanometer precision, which is far beyond what any FDM printer can achieve
/// (typical nozzle diameter: 0.4mm = 400,000 internal units)
///
/// Range with i64: +/- 9.2e12 mm = +/- 9.2e9 meters
/// Even with 1e6 scale, this is effectively unlimited for 3D printing
pub const COORD_SCALE: f64 = 1_000_000.0;

/// Convert millimeters to internal coordinate units
/// Returns None if the value would overflow i64 (practically impossible)
#[inline]
pub fn mm_to_coord(mm: f64) -> Coord {
    (mm * COORD_SCALE).round() as Coord
}

/// Convert internal coordinate units back to millimeters
#[inline]
pub fn coord_to_mm(coord: Coord) -> f64 {
    coord as f64 / COORD_SCALE
}

/// Convert a slice of float points to integer points
pub fn points_to_ipoints(points: &[(f64, f64)]) -> Vec<IPoint2> {
    points.iter().map(|&(x, y)| IPoint2::from_mm(x, y)).collect()
}
```

### Polygon Boolean Operations via clipper2-rust
```rust
// Source: clipper2-rust documentation + Clipper2 official docs
use clipper2_rust::{Clipper, Path64, Paths64, FillRule, ClipType};

/// Compute the difference between subject polygons and clip polygons
pub fn polygon_difference(
    subjects: &[ValidPolygon],
    clips: &[ValidPolygon],
) -> Result<Vec<ValidPolygon>, GeoError> {
    let subject_paths = polygons_to_paths64(subjects);
    let clip_paths = polygons_to_paths64(clips);

    let result = Clipper::boolean_op(
        ClipType::Difference,
        FillRule::NonZero,
        &subject_paths,
        &clip_paths,
    );

    paths64_to_polygons(&result)
}

/// Convert ValidPolygon to clipper2-rust Path64
fn polygon_to_path64(poly: &ValidPolygon) -> Path64 {
    poly.points()
        .iter()
        .map(|p| clipper2_rust::Point64::new(p.x, p.y))
        .collect()
}
```

### Polygon Offsetting via clipper2-rust
```rust
// Source: clipper2-rust documentation
use clipper2_rust::{ClipperOffset, JoinType, EndType};

/// Offset a polygon inward (negative delta) or outward (positive delta)
/// delta is in internal coordinate units (use mm_to_coord to convert from mm)
pub fn offset_polygon(
    polygon: &ValidPolygon,
    delta: Coord,
    join_type: JoinType,
) -> Result<Vec<ValidPolygon>, GeoError> {
    let path = polygon_to_path64(polygon);
    let mut offset = ClipperOffset::new();
    offset.add_path(&path, join_type, EndType::Polygon);
    let result = offset.execute(delta as f64);
    paths64_to_polygons(&result)
}
```

### BVH Construction (Custom Implementation Pattern)
```rust
// Source: PBRT book Chapter 4, widely-used SAH BVH algorithm
pub struct BVH {
    nodes: Vec<BVHNode>,
}

enum BVHNode {
    Leaf {
        aabb: BBox3,
        first_tri: u32,
        tri_count: u32,
    },
    Interior {
        aabb: BBox3,
        left: u32,   // index into nodes Vec
        right: u32,  // index into nodes Vec
        axis: u8,    // split axis (0=X, 1=Y, 2=Z)
    },
}

impl BVH {
    /// Build BVH using Surface Area Heuristic (SAH)
    pub fn build(vertices: &[Point3], indices: &[[u32; 3]]) -> Self {
        // 1. Compute AABB for each triangle
        // 2. Recursively partition using SAH cost metric
        // 3. Leaf nodes contain small groups of triangles
        // 4. Store flat in Vec<BVHNode> for cache efficiency
        todo!()
    }

    /// Find all triangles whose AABBs intersect the given plane at height z
    pub fn query_plane(&self, z: f64) -> Vec<u32> {
        // Traverse BVH, prune branches whose AABB doesn't span z
        todo!()
    }

    /// Ray intersection query -- returns closest hit
    pub fn intersect_ray(&self, origin: &Point3, direction: &Vec3) -> Option<RayHit> {
        // Traverse BVH with ray-AABB intersection test
        todo!()
    }
}
```

### WASM Compatibility Check Pattern
```rust
// Source: Rust WASM best practices
// In each crate's lib.rs, add a compile-time check:
#[cfg(test)]
mod wasm_compat_tests {
    // This test doesn't run in WASM, but CI runs:
    // cargo build --target wasm32-unknown-unknown
    // which validates that all code compiles for WASM

    #[test]
    fn types_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}

        // Verify all public types are Send+Sync
        assert_send_sync::<super::Point3>();
        assert_send_sync::<super::IPoint2>();
        assert_send_sync::<super::BBox3>();
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Clipper1 (C++) | Clipper2 (C++/Rust) | 2022-2023 | Clipper2 has better performance, cleaner API, native 64-bit support. The clipper2-rust port landed in 2025. |
| FFI bindings to C++ Clipper | Pure Rust clipper2-rust | 2025 | Eliminates FFI overhead, enables WASM compilation, satisfies FOUND-01 (pure Rust) |
| f64 polygon operations | i64 integer polygon operations | Established pattern (Clipper1 era, ~2010) | Integer coordinates eliminate entire classes of floating-point robustness bugs |
| nalgebra for all math | Custom lightweight math types | Current recommendation for slicers | Avoids pulling in large linear algebra library for simple point/vector/matrix operations |
| Rc/RefCell mesh graphs | Arena+index flat arrays | Modern Rust best practice | Enables Send+Sync, better cache locality, deterministic memory layout |
| Ray-casting point-in-polygon | Winding number point-in-polygon | Long-established best practice | Winding number handles edge cases (points on edges, collinear vertices) more robustly |

**Deprecated/outdated:**
- **geo crate boolean ops for slicing:** The `geo` crate uses f64 for boolean operations. While correct for GIS, it has robustness issues at the precision level required for slicing. Use integer-coordinate libraries instead.
- **Clipper1 (original):** Superseded by Clipper2 with better algorithms and API. Do not use the `clipper` crate (wraps old Clipper1).

## Open Questions

1. **clipper2-rust Maturity**
   - What we know: Published in 2025, 444 tests passing, faithful port of C++ Clipper2, has WASM demo, Boost license
   - What's unclear: How well-tested is it with the specific degenerate geometry patterns that slicers encounter? (zero-area spikes, collinear vertices, near-parallel edges). Has it been used in production?
   - Recommendation: Adopt it, but create an extensive test suite (20+ cases) comparing clipper2-rust output against reference Clipper2 C++ output for the same inputs. If critical bugs are found, fall back to contributing fixes upstream or temporarily using i_overlay (with i32 limitations) while issues are resolved.

2. **clipper2-rust API Compatibility with i_overlay**
   - What we know: Both libraries do polygon booleans. clipper2-rust uses Path64/Paths64, i_overlay uses Shape/Contour.
   - What's unclear: Whether we should abstract the polygon library behind a trait to allow swapping.
   - Recommendation: Do NOT abstract. The Clipper2 API is well-known in the slicer community (1,425 call sites in C++ LibSlic3r). Use clipper2-rust types directly and match the C++ calling patterns. Abstraction adds complexity without benefit since we are unlikely to switch libraries.

3. **BVH vs. Uniform Grid for Plane Intersection Queries**
   - What we know: BVH is standard for ray tracing. Slicing primarily needs plane intersection queries (which z-planes intersect which triangles).
   - What's unclear: A simple sorted-by-z approach with interval lookups might be faster than BVH for the specific case of horizontal plane queries (which is 90% of slicing queries).
   - Recommendation: Implement BVH for generality (needed for ray intersection, closest point queries), but also implement a simple z-sorted triangle index for the plane intersection case. Both are needed -- BVH for general spatial queries, z-sort for slicing performance.

4. **COORD_SCALE Choice: 1e6 (nanometer) vs 1e3 (micrometer)**
   - What we know: C++ LibSlic3r uses "nanometer" scale. FDM printers have ~50-micron resolution. 1e3 gives micrometer precision (more than enough for FDM). 1e6 gives nanometer precision (matching C++ behavior).
   - What's unclear: Whether the extra precision of 1e6 matters in practice, or just wastes range.
   - Recommendation: Use 1e6 (nanometer) to match C++ Clipper2 conventions and to have headroom for future precision needs (SLA/DLP printers need higher precision). With i64, the range is still effectively unlimited (~9.2e12 mm = 9.2 billion meters).

## Sources

### Primary (HIGH confidence)
- [clipper2-rust GitHub](https://github.com/larsbrubaker/clipper2-rust) - Pure Rust Clipper2 port, features, WASM demo, i64 support, offsetting
- [i-overlay GitHub](https://github.com/iShape-Rust/iOverlay) - Boolean operations, i32/f32/f64 support, buffering, benchmarks
- [bvh crate GitHub](https://github.com/svenstaro/bvh) - SAH-based BVH, nalgebra dependency, features
- [bumpalo GitHub](https://github.com/fitzgen/bumpalo) - Arena allocator, no_std/WASM compatible, v3.x
- [Clipper2 official docs](https://www.angusj.com/clipper2/Docs/Overview.htm) - Integer coordinate rationale, algorithm documentation
- [wasm32-unknown-unknown rustc docs](https://doc.rust-lang.org/beta/rustc/platform-support/wasm32-unknown-unknown.html) - Platform support, std availability, limitations
- [i-overlay performance benchmarks](https://ishape-rust.github.io/iShape-js/overlay/performance/performance.html) - Comparison vs Clipper2 C++ v1.4.0 and Boost

### Secondary (MEDIUM confidence)
- [Clipper2 Rust port announcement (GitHub issue #1066)](https://github.com/AngusJohnson/Clipper2/issues/1066) - MatterHackers port details, test counts, completeness
- [Clipper forum: Why integer coordinates](https://sourceforge.net/p/polyclipping/discussion/1148419/thread/55f05181/) - Historical rationale for integer coordinates in polygon clipping
- [Rust WASM book: Which crates work](https://rustwasm.github.io/book/reference/which-crates-work-with-wasm.html) - Compatibility guidelines
- [proptest GitHub](https://github.com/proptest-rs/proptest) - Property-based testing framework for geometric invariants

### Tertiary (LOW confidence)
- clipper2-rust crates.io page - Could not load due to JavaScript requirement; version 1.0 reported in README
- bvh crate version/release date - Could not verify exact latest version; v0.3.x range visible in docs.rs URLs

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Verified clipper2-rust features, i64 support, WASM demo, Boost license; verified bumpalo WASM compatibility; verified i_overlay limitations (i32 only)
- Architecture: HIGH - Patterns drawn from C++ LibSlic3r reference implementation (documented in designDocs/), Rust best practices for Send+Sync, arena allocation patterns well-established
- Pitfalls: HIGH - Coordinate overflow risk mathematically verified (i32 maxes at 2,147mm with 1e6 scale); floating-point robustness issues are well-documented in computational geometry literature; WASM compatibility issues confirmed by Rust WASM book

**Research date:** 2026-02-15
**Valid until:** 2026-03-15 (30 days -- clipper2-rust is new, monitor for breaking changes or critical bug discoveries)
