# Phase 1: Foundation Types and Geometry Core - Context

**Gathered:** 2026-02-15
**Status:** Ready for planning

<domain>
## Phase Boundary

Low-level geometric primitives and data structures for a 3D slicing engine. This phase establishes the coordinate system, polygon boolean operations, polygon offsetting, and triangle mesh representation that all downstream algorithm crates depend on. These architectural decisions cannot change later without cascading rewrites, so they must be correct from day one.

Scope includes: integer coordinate types, float-to-int conversion, polygon validation, boolean operations, mesh data structures with spatial indexing, and WASM compilation validation.

Out of scope: file I/O (Phase 2), slicing algorithms (Phase 3+), multi-threading utilities (defer to when needed).

</domain>

<decisions>
## Implementation Decisions

### Coordinate precision strategy
All aspects delegated to Claude's discretion based on 3D printing precision requirements:
- Scaling factor (1e3, 1e6, or 1e9) — choose based on nozzle diameter precision vs model size limits
- Precision loss handling — appropriate strategy for slicing engine (silent rounding vs warnings vs validation)
- Bounds checking approach — balance safety vs performance for coordinate operations
- Type strategy (i32 vs i64) — choose based on typical 3D printing model sizes and memory concerns

### Polygon API ergonomics
All aspects delegated to Claude's discretion based on Rust idioms:
- Validation timing — eager, lazy, or two-tier type system (Polygon vs ValidPolygon)
- Construction API style — builder pattern, direct construction, or separate types for simple/complex polygons
- Mutability model — immutable (functional), mutable (imperative), or copy-on-write
- Error handling — Result, Option, or panic-on-invalid-input

### Mesh data structure trade-offs
All aspects delegated to Claude's discretion based on performance and Send+Sync requirements:
- Ownership pattern — arena+indices (recommended in question), Rc/Arc, or graph library
- Spatial indexing — BVH, uniform grid, or octree (choose based on slicing query patterns)
- Memory layout — Structure of Arrays, Array of Structures, or hybrid approach
- Mutability — immutable after construction, incremental modification, or copy-on-write

### WASM compatibility constraints
- **WASM target support:** wasm32-unknown-unknown only for MVP. Put other targets (wasm32-wasi, Emscripten) and features (filesystem, pthread support, maximum compatibility) on a roadmap for later phases.
- Thread safety (Send+Sync): Claude decides whether to enforce from day one or use conditional compilation
- Dependency restrictions: Claude decides strictness level (zero non-WASM deps, feature-gated exceptions, or dev-dependency exemptions)
- WASM bindings: Claude decides whether to add wasm-bindgen in Phase 1 or defer to Phase 9

### Claude's Discretion
Nearly all implementation details are at Claude's discretion. The user has delegated:
- Coordinate precision strategy (scaling factor, bounds checking, types)
- Polygon API design (validation, construction, mutability, errors)
- Mesh data structure choices (ownership, spatial index, memory layout, mutability)
- WASM threading and dependency policies

The only locked decision is: **wasm32-unknown-unknown only for MVP**, with other WASM targets deferred.

</decisions>

<specifics>
## Specific Ideas

**WASM targets:** Start with wasm32-unknown-unknown (browser-only, pure computation). Defer wasm32-wasi (filesystem access) and Emscripten (pthread support) to a future roadmap — these are enhancements, not MVP blockers.

**Design philosophy:** The architectural decisions that cannot change later must be locked in Phase 1. Downstream algorithms in later phases will depend on the coordinate types, polygon operations, and mesh structures established here. Get the foundation right before building on it.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

User mentioned putting WASM enhancements (wasm32-wasi, Emscripten, filesystem, pthread support) on a roadmap, which could be future phases or feature work beyond v1.

</deferred>

---

*Phase: 01-foundation-types-and-geometry-core*
*Context gathered: 2026-02-15*
