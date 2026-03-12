# Phase 29: Mesh Boolean Operations (CSG) - Context

**Gathered:** 2026-03-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement true 3D mesh boolean operations (CSG) on `TriangleMesh` objects — union, intersection, difference, XOR — plus mesh primitives, plane splitting, hollowing, basic lattice generation, mesh offset, and a CLI subcommand. This is a general-purpose CSG API that enables multi-part assembly merging, modifier mesh cutting, support blocker subtraction, and model splitting. Lives in `slicecore-mesh` at Layer 0.

</domain>

<decisions>
## Implementation Decisions

### Supported Operations
- All four boolean operations: union, difference, intersection, XOR
- Pairwise API for all ops, plus optimized N-ary union (merge multiple meshes at once)
- Output is always a new `TriangleMesh` (no lazy CSG tree)
- Function-based API mirroring slicecore-geo::boolean: `mesh_union()`, `mesh_difference()`, `mesh_intersection()`, `mesh_xor()`, `mesh_union_many()`

### Algorithm Approach
- True 3D mesh booleans (compute intersection curves, classify triangles, re-triangulate)
- Correctness first, optimize later — CSG bugs are highly visible in prints
- Claude's discretion on precision approach (exact arithmetic vs floating-point with adaptive predicates)
- Symbolic perturbation for coplanar face handling (simulation of simplicity)
- Lives in `slicecore-mesh` (new module: `src/csg/` or `src/boolean.rs`)

### Mesh Splitting
- Dedicated `mesh_split_at_plane()` function returning both halves — optimized path
- Plane primitive also available for general CSG approach
- Split produces capped (watertight) meshes by default, with option to skip capping

### Mesh Hollowing
- `hollow_mesh(mesh, wall_thickness)` using mesh offset + CSG difference
- Optional drain hole (position + diameter parameter) — subtracts a cylinder

### Mesh Offset
- Robust Minkowski sum implementation for mesh offset (grow/shrink)
- Foundation for hollowing, chamfers, and clearance fitting

### Mesh Primitives
- Comprehensive set: box, cylinder, sphere, cone, torus, plane (half-space), wedge
- Configurable tessellation resolution with sensible defaults (e.g., 32 segments for cylinder)
- Generated at origin, positioned using existing `slicecore-mesh::transform` functions

### Basic Lattice Generation
- Implement one or two lattice patterns (cubic, gyroid) as mesh generators
- Combined with CSG difference, enables mesh-level lightweight parts

### Per-Triangle Attributes
- Extend `TriangleMesh` with optional per-triangle metadata (material ID, color)
- CSG operations preserve attributes through operations
- Backward-compatible: `None` = no attributes (existing code unaffected)

### Input Handling
- Auto-repair inputs using existing `slicecore-mesh::repair` before CSG operations
- If repair fails, return error

### Output Cleanup
- Auto-cleanup results: merge near-duplicate vertices, remove zero-area triangles, ensure consistent winding
- Guarantees clean mesh ready for slicing or export

### Diagnostics
- `CsgReport` returned alongside result mesh: input/output triangle counts, intersection curve count, repairs performed, warnings
- `CsgReport` derives `Serialize`/`Deserialize` for structured JSON output

### Error Handling
- Rich context errors with thiserror: which mesh had issues, triangle count, intersection region info
- No partial results on failure — operations succeed fully or return descriptive error
- `Result<(TriangleMesh, CsgReport), CsgError>`

### CLI Subcommand
- `slicecore csg <operation> <input1> <input2> -o <output>` with union/difference/intersection/xor
- Output format inferred from file extension — supports all export formats (STL, 3MF, OBJ)
- `--json` flag for structured CsgReport output

### Performance & Integration
- Cancellation support via existing `CancellationToken` from Phase 23
- Feature-gated rayon parallelism (`#[cfg(feature = "parallel")]`) — sequential fallback for WASM
- Warnings for large meshes (e.g., >500K triangles) but no hard limits
- No WASM-specific size limits — same code for all targets
- Criterion benchmarks: baseline measurements for cube pair, 10K-triangle, and 100K-triangle mesh pairs

### Plugin System
- CSG operations exposed through `slicecore-plugin-api` traits
- Plugins can both create meshes (primitives) and apply CSG operations
- Enables plugins for mesh manipulation (hollowing, lattice generation, custom cutting)

### Multi-material
- Claude's discretion on material-aware CSG — whether to track material IDs through operations or keep CSG geometry-only for now

### Claude's Discretion
- Exact arithmetic approach (Shewchuk predicates, `robust` crate, or adaptive)
- Modifier system integration (whether to wire up modifier.rs in this phase)
- Internal algorithm structure (BSP-based, plane-sweep, or other approach)
- Lattice pattern selection (which 1-2 patterns to implement)
- Material ID propagation strategy

</decisions>

<specifics>
## Specific Ideas

- "Ability to automatically slice very large models so they fit onto the build plates" — the split function + plane primitives provide the foundation for this
- "Same features as existing slicers" — CSG feature parity with PrusaSlicer/OrcaSlicer's mesh manipulation tools
- Export test meshes for visual inspection (gated behind env var/feature flag) for debugging geometric issues

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `slicecore-geo::boolean` — 2D polygon booleans via clipper2-rust (union, intersection, difference, XOR). API pattern to mirror.
- `slicecore-mesh::repair` — Mesh repair (stitching, intersection detection). Auto-repair before CSG.
- `slicecore-mesh::bvh` — SAH-based BVH spatial index. Reuse for broad-phase intersection detection.
- `slicecore-mesh::repair::intersect` — Moller triangle-triangle intersection test. Reuse for narrow-phase.
- `slicecore-mesh::transform` — Translate, rotate, scale, mirror. Use for positioning primitives.
- `slicecore-fileio::export` — STL/3MF/OBJ export. Reuse for CLI output and test mesh export.
- `fuzz/` directory — Existing fuzz testing infrastructure. Add CSG fuzz target.

### Established Patterns
- Arena+index pattern for `TriangleMesh` (vertices in flat Vec, triangles reference by index)
- `OnceLock<BVH>` for lazy spatial index construction
- `#[cfg(feature = "parallel")]` for rayon gating (Phase 25)
- `CancellationToken` for long operations (Phase 23)
- thiserror for error types with `#[source]` chains
- serde `Serialize`/`Deserialize` on report/result types

### Integration Points
- `slicecore-mesh/src/` — New CSG module alongside existing repair, transform, spatial
- `slicecore-mesh/src/triangle_mesh.rs` — Extend with optional per-triangle attributes
- `slicecore-engine/src/modifier.rs` — Potential consumer of 3D CSG (currently uses 2D booleans)
- `slicecore-plugin-api/src/traits.rs` — Add CSG operation traits for plugins
- `slicecore-cli/` — New `csg` subcommand
- `slicecore-mesh/benches/` — Criterion benchmarks

</code_context>

<deferred>
## Deferred Ideas

- **Auto cut-to-fit build plate** — Automatically split oversized models into build-plate-sized pieces. Uses split + CSG from this phase as foundation. Own phase.
- **CSG undo in UI/application layer** — Undo/history for CSG operations belongs in the GUI/application, not the library. TODO item.
- **Connector/alignment features** — Auto-generate dovetails, pins, alignment keys when splitting meshes. Uses primitives + CSG from this phase. Own phase.
- **Advanced lattice generation** — More lattice patterns and topology optimization. TODO item building on basic lattice from this phase.
- **Benchmark comparison with reference tools** — Compare CSG performance against OpenSCAD, Blender, and other CSG tools. TODO item.
- **Material-aware multi-material CSG** — Full material tracking through boolean operations if not covered by Claude's discretion in this phase.

</deferred>

---

*Phase: 29-mesh-boolean-operations-csg*
*Context gathered: 2026-03-12*
