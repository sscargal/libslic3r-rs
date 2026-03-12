# Phase 29: Mesh Boolean Operations (CSG) - Context

**Gathered:** 2026-03-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement true 3D mesh boolean operations (CSG) on `TriangleMesh` objects -- union, intersection, difference, XOR -- plus mesh primitives, plane splitting, hollowing, mesh offset, CLI subcommand with comprehensive mesh info, plugin API traits, benchmarks, and fuzz targets. Lives in `slicecore-mesh` at Layer 0.

**Not in scope:** Lattice generation (deferred), per-triangle attribute propagation (stubbed only), modifier.rs integration (deferred), mesh decimation/simplification (deferred to own phase).

</domain>

<decisions>
## Implementation Decisions

### Scope (Trimmed)
- Core booleans, 9 primitives, plane split, mesh offset, hollowing, plugin API, CLI, benchmarks, fuzz -- all in scope
- Lattice generation (cubic/gyroid) deferred -- it's a consumer of CSG, not core CSG
- Per-triangle attributes: add `Optional<Vec<TriangleAttr>>` field to `TriangleMesh` (stub), but CSG ops drop/ignore attributes for now. Wire up propagation later
- Modifier.rs integration deferred -- keep Phase 29 focused on the CSG engine itself
- Mesh decimation/simplification deferred to its own phase
- N-ary union (`mesh_union_many`) included, using sequential left-fold (optimize merge order later if benchmarks show need)
- CsgReport included with full diagnostics: input/output triangle counts, intersection curve count, repairs performed, warnings, volume, surface area

### Algorithm Robustness
- **Correctness first, always** -- never sacrifice correctness for speed. If exact predicates make it 10x slower, so be it. Optimize after correctness is proven
- Use exact/adaptive predicates (Shewchuk-style). Claude's discretion on whether to use `robust` crate or implement from scratch
- **Coplanar faces must be handled correctly** -- symbolic perturbation or explicit coplanar classification required. This is non-negotiable
- **Watertight output required** -- output must be manifold (every edge shared by exactly 2 triangles). If algorithm can't produce manifold output, it's a failure
- **Always validate output** -- run manifold check on CSG output. Optional validation behind feature flag in release builds, always-on in debug/test
- **Hard fail with context** on unprocessable meshes -- CsgError with detailed info (which triangles, intersection region). No partial results
- **Mandatory auto-repair** -- always auto-repair inputs using existing `slicecore-mesh::repair` before CSG. No opt-out
- No hard mesh size limit -- warn above 500K triangles but accept any size. Let benchmarks reveal practical limits
- Claude's discretion on debug API surface (intersection curves, classifications) behind feature flag

### Mesh Primitives (9 total)
- **In Phase 29:** box, rounded box, cylinder, sphere, cone, torus, plane, wedge, N-gon prism
- N-gon prism: regular polygon (N sides) + height. Subsumes hex prism (N=6). Regular only for now
- Rounded box: box with filleted edges/corners, defined by dimensions + fillet radius
- Plane: both analytical representation (normal + offset for `mesh_split_at_plane`) and mesh representation (large finite rectangle for general CSG)
- Configurable tessellation resolution with sensible default (32 segments for curved shapes)
- Positioning: generate at origin by default, but offer builder pattern with optional position/rotation for convenience
- All primitives accessible from CLI: `slicecore csg primitive <type> <params> -o <out>`
- **Deferred to TODO:** capsule, ellipsoid, arbitrary polygon extrusion

### Mesh Splitting
- Dedicated `mesh_split_at_plane()` using analytical plane (normal + offset)
- Returns both halves, capped (watertight) by default with option to skip capping
- Plane primitive also available for general CSG approach

### Mesh Offset & Hollowing
- Mesh offset via approximate Minkowski sum (accept simplified rather than full Minkowski)
- `hollow_mesh(mesh, wall_thickness)` using offset inward + CSG difference
- Drain hole: default at bottom-center of mesh, user can override position + diameter
- Drain hole shape: cylinder or tapered cone (user choice)
- Single drain hole per hollow operation (chain ops for multiple holes)
- Uniform wall thickness only -- variable thickness deferred to TODO
- CsgReport includes volume reduction (original vs hollow volume, percentage saved)
- Claude's discretion on thin-wall validation (when thickness causes self-intersection)

### CLI Subcommand
- `slicecore csg <op>` structure with operations as positional args
- **Boolean ops:** `slicecore csg <union|difference|intersection|xor> <a> <b> -o <out>`
- **Split:** `slicecore csg split <model> --plane <normal,offset> -o <top> <bottom>`
- **Hollow:** `slicecore csg hollow <model> --wall <thickness> -o <out>`
- **Primitive:** `slicecore csg primitive <type> <params> -o <out>` -- all primitives supported including rounded-box, n-gon-prism
- **Info:** `slicecore csg info <model>` -- comprehensive mesh inspection
- Verbose flag (-v) for progress, timing, triangle counts. Default is silent
- Output format inferred from file extension (STL, 3MF, OBJ)
- `--json` flag for structured CsgReport output
- File paths only (no stdin piping -- deferred to batch CLI TODO)
- Colored human-readable error output (anyhow/miette-style) to stderr. --json for structured JSON errors
- CsgReport to stdout only (no sidecar file option)

### CLI Info Command (Comprehensive)
- **Geometry stats:** triangle count, vertex count, volume, surface area, bounding box dimensions
- **Mesh quality:** manifold status, non-manifold edge count, degenerate triangles, holes
- **File info:** file format, file size, units (if detectable from 3MF)
- **Component count:** number of disconnected mesh shells
- **Printability warnings:** thin walls, overhangs > 45deg, small features below typical nozzle diameter
- **Repair suggestions:** if mesh has issues, suggest repair commands or auto-repair
- **Symmetry detection:** detect if mesh is symmetric along any axis
- **Optimal orientation hint:** suggest best print orientation based on overhang/support analysis
- **Estimated print time:** rough estimate based on volume and default settings (uses existing estimation module)
- Output as table by default, --json for structured output

### API Ergonomics
- Function-based API: `mesh_union()`, `mesh_difference()`, `mesh_intersection()`, `mesh_xor()`, `mesh_union_many()`
- Both borrowed (`&TriangleMesh`) and owned (`TriangleMesh`) overloads for boolean operations
- `CsgOptions` config struct for options (cancel token, parallel flag, etc.) with `Default::default()` for simple calls
- Claude's discretion on: module re-export strategy, primitive return type (Result vs infallible), CsgMesh trait vs functions only, method chaining sugar

### Performance & Integration
- Cancellation support via existing `CancellationToken` from Phase 23
- Feature-gated rayon parallelism (`#[cfg(feature = "parallel")]`) -- sequential fallback for WASM
- Plugin API traits in `slicecore-plugin-api` -- plugins can create primitives and apply CSG operations
- Warnings for large meshes (>500K triangles) but no hard limits

### Testing Strategy
- **All test mesh categories:** simple primitives, touching/tangent cases, real-world STLs, adversarial/degenerate inputs
- **Test fixtures:** generate programmatically using primitive API. User-provided STL/3MF files discovered from a configurable directory (skip gracefully if absent)
- **Fuzz targets:** both random triangle soups (crash finding) and mutated primitives (edge case finding)
- **Integration tests:** comprehensive real-world scenarios -- load STL, boolean, validate output, export. Edge cases, multi-op chains, hollow+split combos
- **WASM benchmarks:** benchmark CSG ops compiled to WASM vs native

### Benchmarking
- **Criterion benchmarks:** boolean op throughput (1K/10K/100K triangles), primitive generation, hollowing pipeline, plane split performance
- **Additional metrics:** memory usage, BVH construction time, output validation overhead, parallel vs sequential comparison, repair overhead, scaling behavior curves
- **WASM performance:** benchmark WASM vs native
- **CI regression detection:** Claude's discretion on threshold and integration approach
- Claude's discretion on benchmark result reporting format

### Mesh Metrics
- Volume calculation (signed volume via divergence theorem) included in CsgReport
- Surface area calculation included in CsgReport

### Documentation
- Comprehensive runnable doc examples (`/// # Examples`) on all public functions
- Error recovery guide in module-level docs (`//! # Error Handling` section)

### Claude's Discretion
- Exact arithmetic approach (Shewchuk `robust` crate vs custom implementation)
- Debug API surface behind feature flag
- Internal algorithm structure (BSP-based, plane-sweep, or other approach)
- Module re-export strategy (crate root vs submodule imports)
- Primitive return type (Result vs infallible)
- CsgMesh trait vs standalone functions
- Method chaining sugar (impl TriangleMesh methods vs function-only API)
- BVH strategy for output meshes (lazy rebuild vs eager)
- Post-CSG mesh healing scope (rely on existing repair module or add CSG-specific cleanup)
- Thin-wall validation for hollowing
- Benchmark CI regression threshold
- Benchmark result reporting format

</decisions>

<specifics>
## Specific Ideas

- "Ability to automatically slice very large models so they fit onto the build plates" -- the split function + plane primitives provide the foundation for this
- "Same features as existing slicers" -- CSG feature parity with PrusaSlicer/OrcaSlicer's mesh manipulation tools
- Export test meshes for visual inspection (gated behind env var/feature flag) for debugging geometric issues
- N-gon prism generalizes hex prism -- hex is just N=6

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `slicecore-geo::boolean` -- 2D polygon booleans via clipper2-rust (union, intersection, difference, XOR). API pattern to mirror.
- `slicecore-mesh::repair` -- Mesh repair (stitching, intersection detection). Mandatory auto-repair before CSG.
- `slicecore-mesh::bvh` -- SAH-based BVH spatial index. Reuse for broad-phase intersection detection.
- `slicecore-mesh::repair::intersect` -- Moller triangle-triangle intersection test. Reuse for narrow-phase.
- `slicecore-mesh::transform` -- Translate, rotate, scale, mirror. Use for positioning primitives.
- `slicecore-fileio::export` -- STL/3MF/OBJ export. Reuse for CLI output and test mesh export.
- `slicecore-engine::estimation` -- Print time estimation. Reuse for CLI info command.
- `fuzz/` directory -- Existing fuzz testing infrastructure. Add CSG fuzz targets.

### Established Patterns
- Arena+index pattern for `TriangleMesh` (vertices in flat Vec, triangles reference by index)
- `OnceLock<BVH>` for lazy spatial index construction
- `#[cfg(feature = "parallel")]` for rayon gating (Phase 25)
- `CancellationToken` for long operations (Phase 23)
- thiserror for error types with `#[source]` chains
- serde `Serialize`/`Deserialize` on report/result types

### Integration Points
- `slicecore-mesh/src/` -- New CSG module alongside existing repair, transform, spatial
- `slicecore-mesh/src/triangle_mesh.rs` -- Extend with optional per-triangle attributes (stub)
- `slicecore-plugin-api/src/traits.rs` -- Add CSG operation traits for plugins
- `slicecore-cli/` -- New `csg` subcommand with union/difference/intersection/xor/split/hollow/primitive/info
- `slicecore-mesh/benches/` -- Criterion benchmarks

</code_context>

<deferred>
## Deferred Ideas

### Future Phases
- **Lattice generation** -- Cubic and gyroid mesh patterns. Consumer of CSG. Own phase.
- **Auto cut-to-fit build plate** -- Automatically split oversized models into build-plate-sized pieces. Uses split + CSG from this phase as foundation.
- **Connector/alignment features** -- Auto-generate dovetails, pins, alignment keys when splitting meshes.
- **Mesh decimation/simplification** -- Edge collapse, quadric error metrics. Own phase.
- **Convex decomposition** -- V-HACD or similar. Foundation for physics/collision. Own phase.
- **Modifier.rs integration** -- Wire up 3D CSG for modifier mesh cutting (currently uses 2D booleans).
- **Per-triangle attribute propagation** -- Full material tracking through boolean operations.
- **CSG operation serialization** -- Save/replay boolean sequences for parametric modeling, undo, scripting.

### TODO Items
- **Capsule primitive** -- Cylinder + hemispheres. Common for slots and channels.
- **Ellipsoid primitive** -- Non-uniform sphere.
- **Arbitrary polygon extrusion** -- Accept any 2D polygon path and extrude to height.
- **Variable wall thickness hollowing** -- Thicker at stress points with smooth gradient.
- **Batch CLI operations** -- Chain multiple CSG ops in one command or via stdin piping.
- **CSG with transforms convenience API** -- mesh_difference_at(&a, &b, transform) sugar.
- **Advanced lattice generation** -- More lattice patterns and topology optimization.
- **Benchmark comparison with reference tools** -- Compare CSG performance against OpenSCAD, Blender.

</deferred>

---

*Phase: 29-mesh-boolean-operations-csg*
*Context gathered: 2026-03-12*
