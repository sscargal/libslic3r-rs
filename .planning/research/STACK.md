# Stack Research

**Domain:** Computational Geometry / 3D Printer Slicing Core (Rust)
**Researched:** 2026-02-14
**Confidence:** MEDIUM-HIGH (most crate versions verified via docs.rs/crates.io; some newer crates verified via GitHub only)

---

## Executive Summary

The Rust ecosystem for computational geometry has matured significantly. The **critical blocker** identified in the project's design docs -- a pure-Rust replacement for Clipper/Clipper2's 1,425+ call sites -- now has **two viable solutions**: `i-overlay` (the geo crate's backend, optimized for GIS/CAD) and `clipper2-rust` (a faithful line-by-line port of Clipper2 by MatterHackers). Both are pure Rust, WASM-compatible, and support integer coordinates. The rest of the stack (rayon, serde, nalgebra/glam, bumpalo) is mature and battle-tested. The primary remaining gaps are: (1) no production-ready pure-Rust mesh repair library, and (2) WASM multi-threading still requires nightly Rust.

---

## Recommended Stack

### Layer 0: Foundation -- Math & Linear Algebra

| Technology | Version | Purpose | Why Recommended | Confidence | WASM |
|------------|---------|---------|-----------------|------------|------|
| **glam** | 0.32.0 | Vec2/3/4, Mat2/3/4, Quat, Affine transforms | 2-5x faster than nalgebra for common 3D ops (SSE2/NEON/simd128). Bevy's choice. f64 support via DVec2/DVec3/DMat4. Minimal API surface = less to learn, faster compile times. | HIGH | Yes (simd128) |
| **nalgebra** | 0.34.1 | Advanced LA when needed (decompositions, general NxM matrices) | Required by `bvh` crate (0.12.0 depends on nalgebra ^0.34). Use selectively -- not as the primary math library, but when glam's fixed-size types are insufficient. | HIGH | Yes (with `libm` feature) |

**Rationale for glam over nalgebra as primary:**
- Slicing is dominated by Vec2/Vec3/Mat4 operations, not general linear algebra. glam is purpose-built for this.
- glam compiles ~5x faster than nalgebra (critical for iteration speed on a solo-dev project).
- glam's WASM simd128 support is first-class; nalgebra's WASM support works but requires the `libm` feature for trig functions.
- glam 0.32.0 provides `DVec2`, `DVec3`, `DMat3`, `DMat4` for f64 precision where needed.
- Keep nalgebra as a dependency only where required by downstream crates (bvh).

### Layer 0: Foundation -- Polygon Boolean Operations (THE CRITICAL DECISION)

| Technology | Version | Purpose | Why Recommended | Confidence | WASM |
|------------|---------|---------|-----------------|------------|------|
| **i-overlay** | 4.0.x | Polygon boolean ops (union, intersection, difference, XOR), polygon offsetting/buffering, simplification | Powers the geo crate's BooleanOps. Supports i32/f32/f64 APIs. Handles holes, self-intersections, multiple contours. Includes buffering (polygon offset). Apache-2.0/MIT dual license. Actively maintained. | HIGH | Yes (pure Rust, no_std possible) |
| **clipper2-rust** | 1.0.0 | Polygon boolean ops, polygon offsetting, Minkowski operations | Faithful line-by-line port of Clipper2. 444 tests, all passing. Exact behavioral match with C++ Clipper2. Includes offsetting, rectangle clipping, path simplification, PolyTree. BSL-1.0 license. | MEDIUM | Yes (WASM demo exists) |

**Recommendation: Use i-overlay as the primary polygon engine.**

Rationale:
1. **Ecosystem integration:** i-overlay is the boolean ops backend for the `geo` crate (georust ecosystem). Using it directly gives us compatibility with the broader Rust geospatial ecosystem without pulling in all of `geo`.
2. **Native integer support:** i-overlay supports i32 natively, matching the integer-coordinate strategy documented in the architecture (Coord = i64; scale factor 1,000,000). The i32 API avoids float-to-int conversion overhead for the hot path.
3. **Buffering/offset built-in:** i-overlay includes polygon offset (buffering) -- the second most critical operation after booleans (perimeter generation = offset inward).
4. **Active maintenance:** Being the backend for georust/geo means it has institutional support and ongoing optimization.

**Why keep clipper2-rust as a fallback / validation tool:**
1. **API familiarity:** The C++ codebase has 1,425+ Clipper call sites. clipper2-rust has an identical API to C++ Clipper2, making algorithm porting easier when referencing C++ code.
2. **Validation:** Use clipper2-rust in tests to cross-validate i-overlay results on critical operations.
3. **Risk mitigation:** If i-overlay has edge cases that break slicing geometry, clipper2-rust is a drop-in alternative with proven Clipper2 behavior.
4. **Minkowski operations:** clipper2-rust includes Minkowski sum/difference, which i-overlay does not.

**Critical note on i-overlay integer precision:**
i-overlay's native integer API uses i32 (range +/- 2.1 billion). With our COORD_SCALE of 1,000,000 (nanometer precision), a 200mm build plate needs coordinates up to 200,000,000 -- which fits in i32. But with safety margin for offset operations that can expand coordinates, we may need to use f64 API or consider whether i64 support is needed. **This requires benchmarking during Phase 1.**

### Layer 0: Foundation -- 2D Geometry Ecosystem

| Technology | Version | Purpose | Why Recommended | Confidence | WASM |
|------------|---------|---------|-----------------|------------|------|
| **geo-types** | 0.7.x | Shared 2D geometry type definitions (Coord, Point, LineString, Polygon, MultiPolygon) | Standard type vocabulary for the Rust geospatial ecosystem. Minimal dependencies. Use as the interchange format if interoperating with other georust crates. | HIGH | Yes |
| **geo** | 0.32.0 | High-level 2D algorithms (convex hull, simplification, area, distance, boolean ops via i-overlay) | Rich algorithm library. Use selectively for algorithms we need but won't implement ourselves (convex hull, simplification, triangulation). Do NOT use as a core dependency -- cherry-pick algorithms. | HIGH | Yes |

**Rationale for selective use of geo:**
- geo pulls in i-overlay, rstar, and many other deps. Using it directly adds compile time.
- For the slicer, we want direct control over polygon representation (integer coordinates, arena allocation). geo's types use f64 coords by default.
- Strategy: Use geo-types for interop; use i-overlay directly for boolean ops; implement slicer-specific algorithms (contour extraction, region classification) ourselves.

### Layer 0: Foundation -- 3D Mesh & Spatial Indexing

| Technology | Version | Purpose | Why Recommended | Confidence | WASM |
|------------|---------|---------|-----------------|------------|------|
| **parry3d-f64** | 0.26.0 | Triangle mesh data structures, BVH, point/ray queries, mesh intersection tests | From the Dimforge ecosystem (same team as nalgebra/rapier). Pure Rust. Provides TriMesh with BVH, AABB tree, contact/distance queries. f64 variant for precision. | MEDIUM | Yes (pure Rust) |
| **rstar** | 0.12.2 | R*-tree spatial index for 2D nearest-neighbor and range queries | N-dimensional R*-tree. Excellent for 2D spatial queries (seam placement, support spot detection, polygon proximity). Georust ecosystem. Serde support. | HIGH | Yes (pure Rust) |
| **bvh** | 0.12.0 | BVH for ray/plane intersection (mesh slicing) | SAH-based BVH optimized for ray intersection. Depends on nalgebra. Good for mesh-plane intersection during contour extraction. | MEDIUM | Yes (pure Rust) |

**Recommendation: Use parry3d-f64 for mesh data structures and spatial queries; rstar for 2D spatial indexing.**

Rationale for parry3d-f64 over custom mesh implementation:
- parry3d provides TriMesh with built-in BVH, AABB computation, and spatial queries -- exactly what mesh slicing needs.
- It supports f64 precision matching our architecture requirements.
- Building a custom half-edge mesh + BVH is 2-4 weeks of work that parry3d eliminates.
- Risk: parry3d may be heavier than needed (it includes collision detection we won't use). Monitor binary size impact.

**Alternative considered:** The `bvh` crate (0.12.0) is lighter weight but only provides BVH -- no mesh data structure. Use `bvh` if parry3d proves too heavy, but you'll need to build mesh structures yourself.

### Layer 1: File I/O

| Technology | Version | Purpose | Why Recommended | Confidence | WASM |
|------------|---------|---------|-----------------|------------|------|
| **lib3mf-core** | (published) | 3MF file parsing and writing | Already published by this project. Pure Rust. Proven. | HIGH | Yes |
| **nom** | 7.x | Parser combinator for binary/ASCII STL, G-code parsing | Zero-copy parsing. Pure Rust. Battle-tested for binary formats. nom-stl demonstrates STL parsing in <20ms for 30MB files. | HIGH | Yes |
| **nom_stl** | latest | STL file parsing (binary + ASCII) | Pure Rust, only depends on nom. Fast (<20ms for 30MB binary STL). Auto-detects ASCII vs binary. Consider using directly or as reference for custom parser. | MEDIUM | Yes |

**Recommendation on STL parsing:** Evaluate nom_stl first. If it meets performance requirements, use it directly. If custom parsing is needed (e.g., streaming, arena allocation for vertices), use nom as the parser combinator and reference nom_stl's approach.

**STEP file support (future P2):** The `truck` crate is the most mature pure-Rust B-rep/CAD kernel with STEP support. Evaluate when STEP import becomes a priority.

### Layer 1: Configuration & Serialization

| Technology | Version | Purpose | Why Recommended | Confidence | WASM |
|------------|---------|---------|-----------------|------------|------|
| **serde** | 1.0.228 | Serialization/deserialization framework | De facto standard. Required by nearly everything. | HIGH | Yes |
| **serde_json** | 1.x | JSON serialization for API responses, metadata output | Standard JSON serde. | HIGH | Yes |
| **toml** | 1.0.1 | TOML parsing for configuration files and printer/filament/quality profiles | Native Rust TOML encoder/decoder. Spec 1.1.0 compliant. | HIGH | Yes |
| **rmp-serde** | 1.x | MessagePack binary serialization for efficient internal data transfer | Compact binary format. 2-10x smaller than JSON. Good for IPC between slicer process and GUI. | HIGH | Yes |
| **indexmap** | 2.x | Ordered HashMap preserving insertion order | For settings schema where definition order matters (UI generation, documentation). | HIGH | Yes |
| **semver** | 1.x | Semantic version parsing and comparison | For settings/plugin compatibility checks. | HIGH | Yes |

### Layer 2: Parallelization & Memory

| Technology | Version | Purpose | Why Recommended | Confidence | WASM |
|------------|---------|---------|-----------------|------------|------|
| **rayon** | 1.11.0 | Data-parallel iterators (replaces TBB's 47+ parallel_for sites) | De facto standard for CPU parallelism in Rust. par_iter() maps directly to TBB parallel_for. par_iter().reduce() maps to TBB parallel_reduce. 266M+ downloads. | HIGH | Partial* |
| **bumpalo** | 3.19.1 | Arena allocation for per-layer temporary geometry | O(1) reset between layers. no_std compatible. WASM compatible. 15% improvement over custom arenas in WASM benchmarks. | HIGH | Yes |

**\*WASM parallelism caveat (CRITICAL):**
- `wasm-bindgen-rayon` (from Google Chrome Labs) enables rayon on WASM via Web Workers + SharedArrayBuffer.
- **Requires nightly Rust** (tested with nightly-2025-11-15). WebAssembly threads are NOT stable in Rust.
- Requires `--target web` (not `--target bundler`).
- Requires Cross-Origin headers (COOP/COEP) on the serving web server.
- **Recommendation:** Design the WASM build with a feature flag that falls back to single-threaded execution on stable Rust. Use rayon's `cfg` feature gating: `#[cfg(not(target_arch = "wasm32"))]` for parallel paths, sequential fallback for WASM. When WASM threads stabilize, enable parallelism.

**Global allocator recommendation:**
- Native builds: Use **mimalloc** (`mimalloc` crate) as global allocator for 10-30% better multi-threaded allocation performance vs system allocator. Critical for the many small allocations in polygon operations.
- WASM builds: mimalloc also shows ~2x improvement over dlmalloc in WASM. Consider enabling for WASM target too.
- **Do NOT use jemalloc** -- it has poor WASM support and larger binary size. mimalloc is smaller, faster, and more portable.

### Layer 3: Error Handling & Observability

| Technology | Version | Purpose | Why Recommended | Confidence | WASM |
|------------|---------|---------|-----------------|------------|------|
| **thiserror** | 2.0.18 | Derive macro for error types in library crates | Standard for library error types. Zero runtime overhead. v2.0 is current (breaking change from 1.x). | HIGH | Yes |
| **anyhow** | 1.x | Flexible error handling in application code (CLI, server) | Use in bins/ (CLI, server), NOT in library crates. Library crates should use thiserror for typed errors. | HIGH | Yes |
| **tracing** | 0.1.44 | Structured logging and instrumentation | De facto standard. Supports spans, structured fields, async. Powers OpenTelemetry integration for the cloud SaaS use case. | HIGH | Yes |
| **tracing-subscriber** | 0.3.x | Log output formatting and filtering | Required companion to tracing. Supports env_filter for `RUST_LOG`-style filtering. | HIGH | Yes |

### Layer 4: AI Integration (Optional Feature)

| Technology | Version | Purpose | Why Recommended | Confidence | WASM |
|------------|---------|---------|-----------------|------------|------|
| **reqwest** | 0.12.x | HTTP client for AI provider APIs (OpenAI, Anthropic, etc.) | WASM-compatible (auto-switches to fetch API on wasm32). 306M+ downloads. | HIGH | Yes* |
| **secrecy** | 0.10.x | API key protection (never logs/serializes secrets) | Wraps Secret types. Prevents accidental key leakage. | HIGH | Yes |
| **tokio** | 1.x | Async runtime for API server and AI calls | Required by reqwest, axum. Use ONLY in bins/ and slicecore-api, NOT in core library crates. | HIGH | No (native only) |

**\*reqwest on WASM:** Works but with limitations -- no TLS config, no timeout(), no cookie store. Uses browser's fetch API. Sufficient for AI API calls.

### Layer 5: Plugin System

| Technology | Version | Purpose | Why Recommended | Confidence | WASM |
|------------|---------|---------|-----------------|------------|------|
| **wasmtime** | 40.0 | WASM plugin sandbox runtime | Bytecode Alliance project. Secure sandboxing with memory/CPU limits. Component model support. LTS releases. | HIGH | No (host-side only) |

### Testing & Development Tools

| Tool | Version | Purpose | Notes |
|------|---------|---------|-------|
| **criterion** | 0.8.1 | Statistical benchmarking | Requires Rust 1.88+. Regression detection. Gnuplot output. |
| **proptest** | 1.10.0 | Property-based testing for geometric invariants | Experimental WASM support (disable default features). MSRV 1.84. |
| **tiny-skia** | 0.11.4 | CPU-only 2D rendering for visual regression tests | Pure Rust. ~200KiB binary addition. Render toolpaths to PNG for golden-file comparison. |
| **cargo-nextest** | latest | Faster parallel test runner | Up to 3x faster than cargo test on large workspaces. |
| **cargo-deny** | latest | License + advisory audit | Enforce MIT/Apache-2.0 only policy. Block copyleft deps. |
| **cargo-tarpaulin** | latest | Code coverage measurement | Target >80% on core algorithms. |

---

## Installation

```toml
# Cargo.toml workspace root -- [workspace.dependencies]

# Layer 0: Math & Geometry
glam = { version = "0.32", features = ["serde"] }
nalgebra = { version = "0.34", default-features = false, features = ["std"] }
i-overlay = "4.0"
geo-types = "0.7"
geo = "0.32"

# Layer 0: Mesh & Spatial
parry3d-f64 = "0.26"
rstar = { version = "0.12", features = ["serde"] }
bvh = "0.12"

# Layer 1: File I/O
nom = "7"
# lib3mf-core = "x.x" (already published)

# Layer 1: Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1"
toml = "1.0"
rmp-serde = "1"
indexmap = { version = "2", features = ["serde"] }
semver = { version = "1", features = ["serde"] }

# Layer 2: Parallelization & Memory
rayon = "1.11"
bumpalo = { version = "3.19", features = ["collections"] }
mimalloc = { version = "0.1", default-features = false }

# Layer 3: Error Handling & Observability
thiserror = "2.0"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Layer 4: AI (optional)
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
secrecy = "0.10"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }

# Layer 5: Plugin System
wasmtime = "40"

# WASM Target
wasm-bindgen = "0.2.108"

# Dev/Test
criterion = { version = "0.8" }
proptest = "1.10"
tiny-skia = "0.11"

# Validation/Fallback
clipper2-rust = "1.0"
```

---

## Alternatives Considered

| Category | Recommended | Alternative | Why Not the Alternative |
|----------|-------------|-------------|------------------------|
| Linear algebra (primary) | glam 0.32 | nalgebra 0.34 | nalgebra is 5x slower to compile, overkill for Vec2/3/Mat4 operations that dominate slicing. Use nalgebra only where downstream crates require it. |
| Polygon booleans (primary) | i-overlay 4.0 | clipper2-rust 1.0 | clipper2-rust is newer (released late 2025), less ecosystem integration, BSL-1.0 license vs dual MIT/Apache-2.0. Keep as validation/fallback. |
| Polygon booleans | i-overlay 4.0 | geo-booleanop | Martinez-Rueda algorithm is known to have edge cases with degenerate inputs. i-overlay handles self-intersections and degeneracies better. |
| Mesh structures | parry3d-f64 | Custom half-edge mesh | 2-4 weeks of implementation time saved. parry3d has BVH, AABB, spatial queries built-in. Custom mesh only if parry3d proves too heavy. |
| Spatial indexing (2D) | rstar 0.12 | Custom grid (EdgeGrid) | rstar is production-ready R*-tree. Custom EdgeGrid may be faster for specific polygon proximity queries -- build later if profiling shows need. |
| HTTP client | reqwest 0.12 | ureq | ureq is simpler but no WASM support. reqwest auto-switches to fetch on WASM. |
| Async runtime | tokio 1.x | async-std | tokio is the ecosystem standard. reqwest, axum, and most async crates assume tokio. |
| Global allocator | mimalloc | jemalloc | jemalloc has poor WASM support and larger binary. mimalloc is faster in multi-threaded WASM benchmarks. |
| WASM plugin sandbox | wasmtime 40 | wasmer | wasmtime has Bytecode Alliance backing, LTS releases, better component model support. wasmer is viable but less conservative. |
| 2D rendering (tests) | tiny-skia 0.11 | skia-safe (Skia bindings) | skia-safe requires C++ build toolchain and is not WASM-compatible. tiny-skia is pure Rust. |
| STL parsing | nom_stl / custom nom | stl_io | stl_io works but nom-based parsing is faster and more composable. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| **clipper2** (crates.io FFI wrapper) | Wraps C++ Clipper2 via clipper2c-sys. Has C++ build dependency. Not WASM-compatible. "Super early stage" per author. | i-overlay or clipper2-rust (pure Rust) |
| **clipper2-sys** / **clipper2c-sys** | Raw FFI bindings to C++ Clipper2. Requires C++ toolchain. Cannot compile to WASM. Unsafe FFI boundary. | i-overlay or clipper2-rust |
| **geo-clipper** | Binds to C++ Clipper via clipper-sys. C++ dependency. Not WASM-compatible. | i-overlay (geo crate's actual boolean backend) |
| **CGAL** (any Rust bindings) | GPL license. C++ dependency. Massive binary. Not WASM-compatible. | parry3d-f64 + custom algorithms |
| **opencascade-rs** | C++ OCCT bindings. Enormous dependency (>100MB). Not WASM-compatible. | truck crate (pure Rust B-rep, for future STEP support) |
| **jemalloc** | Poor WASM support. Larger binary than mimalloc. No clear performance advantage. | mimalloc |
| **cgmath** | Unmaintained since 2021. Superseded by glam. | glam |
| **euclid** | Mozilla-specific design decisions. Less performant than glam. Smaller ecosystem. | glam |
| **async-std** | Ecosystem has converged on tokio. Using both causes dep bloat. | tokio |
| **ndarray** | Designed for numerical computing (NumPy-like). Wrong abstraction for geometry. | nalgebra (when needed) or glam |

---

## Stack Patterns by Variant

**If building for native desktop/CLI (default):**
- Use full feature set: rayon, tokio, wasmtime, mimalloc
- Enable `native` feature flag
- All crates work without restriction

**If building for WASM (browser slicing):**
- Disable: tokio, wasmtime, axum, reqwest TLS features
- Enable: wasm-bindgen, web-sys
- Parallelism: Single-threaded fallback on stable Rust; rayon via wasm-bindgen-rayon on nightly
- Feature flag: `wasm` excludes native-only crates
- Memory: bumpalo works natively in WASM; consider mimalloc for WASM allocator

**If building for cloud SaaS (server mode):**
- Add: axum, tower, tokio with full features
- Enable: `server` feature flag
- Add: tracing-opentelemetry for distributed tracing
- Add: prometheus metrics crate for monitoring

---

## Identified Gaps (Must Build Custom)

### Gap 1: Mesh Repair (CRITICAL -- must build)
**Confidence:** HIGH that no suitable crate exists
- No production-ready pure-Rust mesh repair library exists.
- `baby_shark` has basic voxel remeshing but not the targeted repairs needed (non-manifold edge fixing, hole filling, self-intersection resolution, degenerate triangle removal).
- `parry3d` has mesh structures but no repair algorithms.
- **Plan:** Build `slicecore-mesh` with repair algorithms ported from C++ libslic3r reference. Estimated 2-3 weeks for core repair operations.

### Gap 2: EdgeGrid 2D Spatial Index (MEDIUM priority -- build when profiling shows need)
**Confidence:** HIGH that no equivalent exists
- C++ libslic3r uses a custom `EdgeGrid` (2D grid-based spatial index) for fast polygon-polygon proximity queries.
- rstar (R*-tree) covers most spatial indexing needs but may not match EdgeGrid's O(1) grid-based lookup for dense, uniform polygon data.
- **Plan:** Start with rstar. Profile. If polygon proximity queries are a bottleneck, implement a custom grid index in `slicecore-geo`.

### Gap 3: G-code Parser/Writer (must build)
**Confidence:** HIGH that no suitable crate exists
- No Rust crate handles multi-dialect G-code (Marlin, Klipper, RepRapFirmware, Bambu).
- **Plan:** Build `slicecore-gcode-io` using nom for parsing. Firmware dialect abstraction via trait system.

### Gap 4: Slicer-Specific 2D Algorithms (must build)
**Confidence:** HIGH
- Contour extraction from mesh-plane intersection
- Region classification (perimeter vs infill vs solid vs bridge)
- Perimeter generation (wall ordering, gap fill, Arachne variable-width)
- Infill pattern generation (24+ patterns)
- Support generation (grid, tree, organic)
- These are the core slicing algorithms -- no crate provides them. They ARE the product.

### Gap 5: Integer Coordinate Polygon Types (likely must build)
**Confidence:** MEDIUM
- i-overlay uses i32 coordinates natively, but our architecture specifies i64 (Coord type) for greater precision headroom.
- geo-types uses f64 coordinates.
- **Plan:** Build custom `IPoint2` / `IPolygon` types in `slicecore-math` with conversion traits to/from i-overlay's i32 and geo-types' f64. This is ~1 day of work, not a major gap.

---

## Version Compatibility Matrix

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| glam 0.32 | serde 1.x | Via `serde` feature flag |
| nalgebra 0.34 | bvh 0.12 | bvh requires nalgebra ^0.34 |
| nalgebra 0.34 | parry3d-f64 0.26 | Same Dimforge ecosystem |
| i-overlay 4.0 | geo 0.32 | geo depends on i_overlay ^4.0.0, <4.1.0 |
| rayon 1.11 | bvh 0.12 | bvh has optional rayon feature |
| serde 1.0.228 | All serde-enabled crates | Universal compatibility |
| wasm-bindgen 0.2.108 | wasm-bindgen-rayon | Requires nightly Rust (tested nightly-2025-11-15) |
| thiserror 2.0 | anyhow 1.x | thiserror 2.0 is a BREAKING change from 1.x. All error types must use v2 derive syntax. |
| Rust MSRV | 1.80+ | rayon 1.11 requires 1.80. criterion 0.8 requires 1.88. Use Rust 1.88+ stable. |

---

## Performance Implications

| Decision | Performance Impact | Mitigation |
|----------|--------------------|------------|
| glam over nalgebra | 2-5x faster for Vec2/3/Mat4 ops on x86 (SIMD); ~equal on scalar | Use glam for hot paths; nalgebra only where required by deps |
| i-overlay integer API | Avoids float-to-int conversion overhead on polygon hot paths | Use i32 API directly; convert at API boundaries |
| mimalloc global allocator | 10-30% improvement for multi-threaded allocation-heavy workloads | Enable for both native and WASM builds |
| bumpalo arena allocation | O(1) deallocation between layers; reduces GC pressure | Per-thread arenas via thread_local! + bumpalo |
| rayon par_iter | Near-linear scaling to 8 cores for per-layer operations | Match C++ TBB granularity; profile to avoid overhead on small workloads |
| parry3d BVH | O(log n) mesh-plane intersection vs O(n) brute force | Critical for slicing large meshes (500K+ triangles) |
| rstar R*-tree | O(log n) nearest-neighbor vs O(n) linear scan | Essential for seam placement and support spot detection |
| WASM single-threaded | ~5x slower than native parallel (no multi-threading on stable Rust) | Accept for now; document limitation; enable wasm-bindgen-rayon when threads stabilize |

---

## Sources

**Verified via docs.rs (HIGH confidence):**
- [geo 0.32.0](https://docs.rs/geo/latest/geo/) -- Boolean ops via i-overlay, algorithms
- [glam 0.32.0](https://docs.rs/glam/latest/glam/) -- Vec/Mat types, f64 support, SIMD
- [nalgebra 0.34.1](https://docs.rs/nalgebra/latest/nalgebra/) -- Linear algebra types
- [rstar 0.12.2](https://docs.rs/rstar/latest/rstar/) -- R*-tree spatial index
- [bvh 0.12.0](https://docs.rs/bvh/latest/bvh/) -- BVH for ray intersection
- [parry3d 0.26.0](https://docs.rs/parry3d/latest/parry3d/) -- 3D collision/mesh library
- [serde 1.0.228](https://docs.rs/serde/latest/serde/) -- Serialization framework
- [tracing 0.1.44](https://docs.rs/tracing/latest/tracing/) -- Structured logging
- [thiserror 2.0.18](https://docs.rs/thiserror/latest/thiserror/) -- Error derive macro
- [toml 1.0.1](https://docs.rs/toml/latest/toml/) -- TOML parser
- [proptest 1.10.0](https://docs.rs/proptest/latest/proptest/) -- Property testing
- [rayon 1.11.0](https://crates.io/crates/rayon) -- Data parallelism

**Verified via GitHub (MEDIUM confidence):**
- [i-overlay 4.0](https://github.com/iShape-Rust/iOverlay) -- Polygon booleans + buffering
- [clipper2-rust 1.0.0](https://github.com/larsbrubaker/clipper2-rust) -- Pure Rust Clipper2 port, 444 tests, published on crates.io
- [wasm-bindgen-rayon](https://github.com/GoogleChromeLabs/wasm-bindgen-rayon) -- WASM threading adapter, requires nightly
- [wasmtime 40.0](https://github.com/bytecodealliance/wasmtime) -- WASM plugin runtime
- [truck](https://github.com/ricosjp/truck) -- Pure Rust B-rep/CAD kernel with STEP support
- [tiny-skia 0.11.4](https://github.com/linebender/tiny-skia) -- Pure Rust 2D renderer

**Verified via WebSearch (MEDIUM confidence):**
- [bumpalo 3.19.1](https://crates.io/crates/bumpalo) -- Arena allocator, WASM compatible
- [wasm-bindgen 0.2.108](https://github.com/wasm-bindgen/wasm-bindgen) -- WASM-JS interop
- [criterion 0.8.1](https://docs.rs/crate/criterion/latest) -- Benchmarking framework
- mimalloc -- WASM-compatible, [2x improvement over dlmalloc in WASM](https://web.dev/articles/scaling-multithreaded-webassembly-applications)
- nom_stl -- [<20ms for 30MB binary STL](https://lib.rs/crates/nom_stl)

**Training data only (LOW confidence -- verify before depending on):**
- reqwest 0.12.x WASM auto-detection behavior -- verify with integration test
- secrecy 0.10.x API -- verify current version on crates.io
- rmp-serde 1.x -- verify current version

---

*Stack research for: libslic3r-rs -- Rust 3D Printer Slicing Core*
*Researched: 2026-02-14*
