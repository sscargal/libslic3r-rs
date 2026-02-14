# Project Research Summary

**Project:** libslic3r-rs
**Domain:** Computational Geometry / 3D Printer Slicing Engine (Rust)
**Researched:** 2026-02-14
**Confidence:** HIGH

## Executive Summary

LibSlic3r-RS is a ground-up Rust rewrite of the C++ LibSlic3r slicing core that powers PrusaSlicer, OrcaSlicer, BambuStudio, and CrealityPrint. Research across Rust ecosystem gaps, C++ algorithm analysis, and cross-fork feature comparison confirms this project is architecturally feasible. The critical blocker identified in earlier design work -- finding a pure-Rust replacement for Clipper2's 1,425+ polygon boolean call sites -- is now resolved: `i-overlay` (the `geo` crate's backend) provides production-ready polygon booleans and offsetting with native integer coordinate support, and `clipper2-rust` (a line-by-line port of Clipper2) serves as validation fallback. The rest of the Rust stack (glam for math, rayon for parallelism, bumpalo for arena allocation, parry3d-f64 for mesh structures) is mature and battle-tested.

The recommended approach is a 6-layer modular workspace with ~20 crates organized in a strict acyclic dependency graph. The architecture eliminates C++ LibSlic3r's biggest design flaw -- global mutable state -- through explicit context passing and owned stage results in a pipeline model. All algorithmic extension points (infill patterns, support strategies, G-code dialects, seam placement) are defined as traits, enabling both compiled-in implementations and runtime plugins via WASM sandboxing. Feature research across all four slicer forks identifies ~850 unique settings, with ~200 required for MVP (P0), ~450 for feature parity (P1), and the remainder for best-of-breed status (P2/P3). The five highest-value innovations to port from the fork ecosystem are: OrcaSlicer's scarf joint seam system, per-feature flow control, 4-tier overhang speed/fan modulation, tree support with polygon nodes, and Arachne variable-width perimeters.

The key risks are: (1) floating-point robustness in geometric predicates -- mitigated by using integer coordinates (i-overlay's i32 API) for all polygon operations, (2) mesh repair has no production Rust crate -- must be built custom (~2-3 weeks), (3) WASM multi-threading requires nightly Rust -- mitigated by designing single-threaded fallback from day one, and (4) the sheer scope of ~850 settings risks timeline creep -- mitigated by strict P0/P1/P2 prioritization where MVP delivers a printable calibration cube before any advanced features.

## Key Findings

### Recommended Stack

The Rust ecosystem has matured enough to build this project entirely in pure Rust with no C/C++ dependencies, enabling WASM compilation. The stack is organized in layers matching the architecture: foundation math/geometry, file I/O, parallelization, error handling, and optional AI/plugin subsystems.

**Core technologies:**
- **glam 0.32** (primary math): 2-5x faster than nalgebra for Vec2/3/Mat4 ops that dominate slicing; 5x faster compile times; first-class WASM simd128 support
- **i-overlay 4.0** (polygon booleans): Powers `geo` crate's boolean ops; native i32 integer API eliminates float robustness issues; includes polygon offsetting (critical for perimeter generation); Apache-2.0/MIT dual license
- **clipper2-rust 1.0** (validation fallback): Faithful line-by-line Clipper2 port; identical API to C++ reference; 444 tests passing; use for cross-validation and Minkowski operations
- **parry3d-f64 0.26** (mesh/spatial): TriMesh with built-in BVH, AABB, spatial queries; eliminates 2-4 weeks of custom mesh implementation
- **rstar 0.12** (2D spatial index): R*-tree for nearest-neighbor and range queries; essential for seam placement and support detection
- **rayon 1.11** (parallelism): Maps directly to C++'s 47+ TBB parallel_for sites; requires careful granularity tuning -- do not blindly convert all loops
- **bumpalo 3.19** (arena allocation): O(1) per-layer deallocation for transient polygon data; WASM compatible
- **mimalloc** (global allocator): 10-30% multi-threaded allocation improvement; works in WASM
- **serde + toml** (config): De facto standard; TOML for human-editable profiles matching existing slicer conventions
- **wasmtime 40** (plugin sandbox): WASM Component Model for untrusted community plugins with memory/CPU limits

**Critical version requirements:** Rust 1.88+ stable (criterion 0.8 requires it). nalgebra 0.34 required by bvh 0.12 and parry3d 0.26. thiserror 2.0 (breaking change from 1.x).

### Expected Features

Feature analysis draws from the cross-fork comparison of PrusaSlicer (538 settings), OrcaSlicer (716 settings), BambuStudio (692 settings), and CrealityPrint (670 settings), producing a unified superset of ~850 settings.

**Must have (table stakes -- P0, ~200 settings):**
- Mesh ingestion (STL, 3MF), repair, transformation
- Contour extraction at configurable layer heights
- Classic + Arachne perimeter generation with gap fill
- 5 core infill patterns (rectilinear, grid, honeycomb, cubic, gyroid)
- Traditional line/grid support material
- Bridge and overhang detection with 4-tier dynamic speed/fan
- Basic seam placement (aligned, rear, random, nearest)
- Retraction, speed planning, temperature/fan control
- G-code generation for Marlin, Klipper, RepRapFirmware
- Brim, skirt, elephant foot compensation
- CLI binary producing printable G-code

**Should have (competitive -- P1, ~250 additional settings):**
- All 24 PrusaSlicer infill patterns + plugin trait for extensibility
- Tree support (Cura-based algorithm) with polygon node enhancement
- Scarf joint seam system (OrcaSlicer's 12-parameter seam innovation)
- Per-feature flow ratios (10+ independent flow controls from OrcaSlicer)
- Adaptive layer heights, adaptive pressure advance basics
- Arc fitting (G2/G3), ironing, fuzzy skin
- Multi-material support (SEMM, wipe tower, purge volume matrix)
- Wall direction control, brim ears, overhang reversal
- Per-feature acceleration and jerk control
- Plugin system (native abi_stable + WASM wasmtime)
- REST/gRPC server, Python bindings

**Defer (v2+):**
- TPMS infill patterns (Diamond, Fischer-Koch) -- very high complexity
- Organic support (PrusaSlicer mesh-based tree support) -- ~3,660 lines of C++
- Adaptive pressure advance (Orca, Klipper-specific)
- Binary G-code (.bgcode, Prusa-specific)
- Vendor-specific features (multi-plate beds, camera integration, CR_PNG thumbnails)
- STEP file import (use `truck` crate when ready)

### Architecture Approach

The architecture follows a 6-layer modular workspace with strict acyclic dependency ordering. Layer 0 (math, geo, mesh) has zero internal dependencies and forms the foundation everything builds on. The pipeline pattern -- where each slicing stage consumes owned input and produces owned output -- eliminates global mutable state and naturally enables both parallel execution and incremental re-slicing. Extension points use trait-based dispatch (`InfillPattern`, `SupportStrategy`, `GcodeDialect`, `SeamStrategy`) with a dual-mode plugin system: native plugins via `abi_stable` for performance-critical paths, WASM plugins via wasmtime Component Model for untrusted community extensions.

**Major components (20 crates across 6 layers):**
1. **slicecore-math** (Layer 0) -- Coordinate types (i64 Coord, IPoint2), glam wrappers, float/int conversion
2. **slicecore-geo** (Layer 0) -- Polygon booleans (i-overlay), offsetting, validation, area/containment tests
3. **slicecore-mesh** (Layer 0) -- TriangleMesh with BVH, mesh repair, manifold checks, mesh-plane intersection
4. **slicecore-fileio** (Layer 1) -- STL/3MF/OBJ parsers, streaming, fuzz-tested
5. **slicecore-config** (Layer 1) -- Declarative settings schema (~850 settings), hierarchical profiles, validation
6. **slicecore-slicer** (Layer 2) -- Contour extraction at Z heights, layer generation
7. **slicecore-perimeters** (Layer 2) -- Classic offset + Arachne variable-width wall generation
8. **slicecore-infill** (Layer 2) -- Pattern generation with InfillPattern plugin trait
9. **slicecore-supports** (Layer 2) -- Auto/tree support with SupportStrategy plugin trait
10. **slicecore-engine** (Layer 5) -- Pipeline orchestrator, rayon ThreadPool, progress/cancellation

### Critical Pitfalls

1. **Floating-point robustness in geometric predicates** -- Use integer coordinates (i-overlay i32 API) for all polygon boolean and offset operations. Establish the coordinate precision strategy (Coord = i64, COORD_SCALE = 1,000,000) as the very first implementation decision in Phase 1. Never use `f64::EPSILON` as a tolerance; use application-specific tolerances or the `robust` crate for exact predicates.

2. **Borrow checker vs. graph-like mesh data structures** -- Use arena allocation with index-based references (Vec<Vertex> + usize indices), not Rc<RefCell<T>>. This eliminates lifetime hell, enables Send+Sync for rayon, and matches cache-friendly SoA layout. The mesh data structure design must be settled in Phase 1 before any algorithms are built on top of it.

3. **Rayon over-parallelization overhead** -- Do NOT convert all 47+ TBB parallel_for sites to par_iter() blindly. Benchmark each site with criterion, require >20% speedup proof, use with_min_len() for chunk sizes, and pin threads to cores for cache locality. Get sequential algorithms correct first; parallelize selectively in later phases.

4. **WASM threading and memory constraints** -- Design for single-threaded WASM from day one. Feature-gate rayon behind `#[cfg(not(target_arch = "wasm32"))]`. Add `cargo build --target wasm32-unknown-unknown` as a CI gate in Phase 1. Avoid Mutex/blocking on main thread. 4GB memory ceiling means streaming/chunked processing for large meshes.

5. **Polygon degeneracy and self-intersection handling** -- Real-world STL files contain zero-area spikes, collinear vertices, duplicate points, and self-intersections. Always validate and clean input polygons before boolean operations. Use Rust's type system: `ValidPolygon` (cleaned) vs raw `Polygon` (unchecked). Build a comprehensive degenerate test suite.

## Implications for Roadmap

Based on combined research, the project should follow a dependency-driven build order that produces testable artifacts at each phase. The key insight is: the slicing pipeline is a strict DAG, and the polygon geometry layer is the most critical dependency -- every algorithm crate depends on it. Build bottom-up, prove the architecture with a vertical slice (STL to G-code) as early as possible, then expand features horizontally.

### Phase 1: Foundation Types and Geometry Core
**Rationale:** Every algorithm depends on coordinate types and polygon operations. The integer coordinate strategy, mesh data structures, and polygon boolean API must be locked in before anything else. This phase addresses the #1 and #2 critical pitfalls (float robustness, borrow checker vs. graph structures).
**Delivers:** `slicecore-math`, `slicecore-geo`, `slicecore-mesh` crates with comprehensive tests including degenerate geometry cases. WASM CI gate established.
**Features addressed:** FR-001 (mesh ingestion foundation), FR-002 (mesh repair), FR-003 (mesh transforms)
**Pitfalls avoided:** Floating-point robustness (integer coords from day 1), borrow checker (arena+index pattern), WASM (CI gate), global state (context passing pattern)
**Estimated duration:** 6-8 weeks
**Research needed:** Benchmark i-overlay i32 vs f64 API for coordinate range safety. Verify parry3d-f64 binary size impact. Validate integer coordinate overflow with 200mm+ build plates.

### Phase 2: Vertical Slice (STL to G-code)
**Rationale:** Prove the full pipeline works end-to-end before investing in breadth. A printable calibration cube validates the architecture. This phase builds the minimum viable set: file I/O, slicing, basic perimeters, one infill pattern, basic G-code.
**Delivers:** `slicecore-fileio`, `slicecore-config` (minimal), `slicecore-slicer`, `slicecore-perimeters` (classic only), `slicecore-infill` (rectilinear only), `slicecore-gcode-gen` (Marlin only), `slicecore-engine` (basic orchestrator), `slicecore-cli` binary.
**Features addressed:** FR-004 (slicing), FR-006 (basic perimeters), FR-007 (one infill pattern), FR-017 (basic G-code), FR-062 (CLI)
**Milestone:** Printable calibration cube -- STL in, Marlin G-code out, prints successfully on a real printer.
**Pitfalls avoided:** Premature crate splitting (start coarser, split later), over-parallelization (sequential first)
**Estimated duration:** 8-10 weeks
**Research needed:** Validate contour extraction algorithm against C++ reference output. Cross-compare G-code output with PrusaSlicer for same model+config.

### Phase 3: Core Algorithm Completeness (P0 Features)
**Rationale:** With the pipeline proven, fill in the remaining P0 features that make the slicer usable for real prints: Arachne perimeters, 4 more infill patterns, traditional support, bridge/overhang handling, speed planning, retraction, and full seam placement.
**Delivers:** Arachne perimeters, 5 core infill patterns, traditional support, bridge detection, overhang 4-tier speed/fan, all basic seam modes, retraction+wipe, speed/acceleration planning, temperature control, brim/skirt, Klipper+RRF G-code flavors.
**Features addressed:** FR-005 through FR-020 (all P0 core slicing), FR-017 (3 firmware dialects)
**Pitfalls avoided:** Selective parallelization (benchmark per call site), polygon degeneracy (validated input types)
**Estimated duration:** 12-16 weeks
**Research needed:** Arachne/Voronoi algorithm Rust implementation (no existing crate). Tree-based support algorithms. Motion planning firmware-model accuracy.

### Phase 4: Feature Parity (P1 Differentiators)
**Rationale:** Expand from functional to competitive. Add the remaining 19 infill patterns, tree support, scarf seam system, per-feature flow control, multi-material support, and advanced quality features. This is where the plugin trait system gets exercised with real diversity.
**Delivers:** All 24 PrusaSlicer infill patterns, tree support, scarf joint seam (12 params), per-feature flow (10+ ratios), adaptive layer heights, arc fitting, ironing, fuzzy skin, MMU/wipe tower, wall direction control, overhang reversal.
**Features addressed:** FR-030 through FR-045 (all P1 advanced features)
**Estimated duration:** 16-24 weeks
**Research needed:** Scarf seam gradient implementation details. Lightning infill tree algorithm. Tree support collision volume caching.

### Phase 5: Plugin System and Extensibility
**Rationale:** Extract proven trait interfaces into `slicecore-plugin-api`, implement native (abi_stable) and WASM (wasmtime) loading. Move some built-in patterns to plugins to validate the system. This enables community contributions without forking.
**Delivers:** `slicecore-plugin-api`, `slicecore-plugin`, example plugins, WIT interface definitions, plugin manifest format.
**Features addressed:** FR-039 (plugin system)
**Estimated duration:** 6-8 weeks
**Research needed:** abi_stable three-crate pattern versioning. WASM Component Model WIT generation for geometry types. Plugin memory/CPU sandboxing limits.

### Phase 6: Intelligence, API, and WASM Target
**Rationale:** With a stable core, add the differentiating features: AI provider abstraction, parameter optimizer, model analyzer, REST/gRPC server, Python bindings, and WASM browser build.
**Delivers:** `slicecore-ai`, `slicecore-optimizer`, `slicecore-analyzer`, `slicecore-api` (REST/gRPC), Python bindings (PyO3), WASM browser build with streaming output.
**Features addressed:** FR-050 through FR-067 (AI, API, WASM, Python)
**Estimated duration:** 12-16 weeks
**Research needed:** WASM memory budget for real-world models. AI structured output reliability for settings recommendation. WASM threading status on stable Rust.

### Phase Ordering Rationale

- **Bottom-up dependency order:** Layer 0 (math/geo/mesh) must exist before Layer 2 (algorithms) can be implemented. The dependency graph is not negotiable.
- **Vertical slice before horizontal expansion:** Phase 2 proves the full pipeline with minimal features, catching architectural mistakes before they propagate across 20 crates. The C++ analysis shows that mesh slicing + perimeters + infill + G-code constitute the minimum complete pipeline.
- **P0 before P1 before P2:** Feature research shows ~200 P0 settings are sufficient for a usable slicer. Adding P1 features is additive -- it does not require reworking P0 code.
- **Plugin system after algorithms stabilize:** Extracting traits into a stable plugin API requires the trait interfaces to be proven by real algorithm implementations first. Premature plugin API design leads to breaking changes.
- **AI/WASM last:** These are multipliers on a working core, not core functionality. They depend on everything below them.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 1:** i-overlay integer coordinate range validation -- need to confirm i32 range is sufficient for 300mm+ build plates at COORD_SCALE 1,000,000 (300mm * 1M = 300M, fits i32 max 2.1B, but offset operations can expand coordinates -- needs empirical testing)
- **Phase 3:** Arachne variable-width perimeter algorithm -- Voronoi skeleton computation in Rust has no off-the-shelf crate; need to evaluate porting C++ implementation (~5,000 lines in Arachne/)
- **Phase 3:** Support generation algorithms -- tree support is Very High complexity (~3,000+ lines C++); organic support even higher
- **Phase 4:** Scarf seam implementation -- OrcaSlicer's 12-parameter system is novel with limited documentation outside the source code
- **Phase 5:** WASM Component Model for geometry types -- WIT interface design for efficient polygon data transfer across sandbox boundary

Phases with standard patterns (skip research-phase):
- **Phase 2 (file I/O, basic slicing):** STL/3MF parsing is well-documented; mesh-plane intersection is a textbook algorithm; rectilinear infill is trivial
- **Phase 2 (CLI):** Standard clap/structopt patterns; nothing novel
- **Phase 4 (most infill patterns):** Algorithm implementations are documented in C++ source with pseudocode; straightforward ports
- **Phase 6 (REST API, Python bindings):** axum + PyO3 are well-documented with extensive examples

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All core crates verified via docs.rs/crates.io with version numbers. i-overlay and clipper2-rust verified via GitHub. Only reqwest WASM behavior and secrecy API need integration testing. |
| Features | HIGH | Cross-fork feature matrix built from source code analysis of all 4 forks. ~850 settings enumerated with priority assignments. Feature complexity estimates based on C++ LOC analysis. |
| Architecture | HIGH | Pattern research grounded in real Rust projects (Bevy, rust-analyzer, Extism, Zed). 6-layer crate structure validated against Cargo workspace best practices. Plugin dual-mode (native+WASM) validated by Extism and Zed precedents. |
| Pitfalls | HIGH | All 6 critical pitfalls verified with multiple sources, real-world examples, and Rust community consensus. Recovery strategies documented with time estimates. |

**Overall confidence:** HIGH

### Gaps to Address

- **Mesh repair algorithms:** No production Rust crate exists. Must port from C++ reference. Estimated 2-3 weeks. Validate by running repair on known-broken STL test files from the PrusaSlicer test suite.
- **Arachne/Voronoi perimeters:** No Rust Voronoi skeleton crate suitable for variable-width perimeter generation. Must evaluate porting C++ Arachne (~5,000 lines). Consider using Boost.Voronoi via FFI as a temporary bridge or finding a pure-Rust Voronoi implementation.
- **Integer coordinate overflow:** i-overlay uses i32 natively. With COORD_SCALE=1,000,000, a 300mm coordinate = 300,000,000 which fits i32 (max 2,147,483,647). But polygon offset operations can expand coordinates beyond the original bounding box. Need benchmarking in Phase 1 to determine if i64 fallback or f64 API is needed.
- **WASM binary size:** Target is <5 MiB gzipped. With parry3d-f64, i-overlay, glam, and serde, the base binary may exceed this. Need size profiling early. Consider LTO and `wasm-opt` as mitigations.
- **Deterministic output:** NFR-020 requires bit-for-bit identical G-code across runs. This requires deterministic polygon clipping (i-overlay uses integer coords, which helps), deterministic parallel execution (rayon work-stealing is non-deterministic -- may need canonical ordering passes), and avoiding HashMap iteration order dependence. Needs design attention in Phase 1.
- **G-code parser/writer (multi-dialect):** No Rust crate handles Marlin + Klipper + RepRapFirmware + Bambu dialects. Must build `slicecore-gcode-io` using nom. The firmware quirks database should be data-driven (TOML) rather than code-driven.

## Sources

### Primary (HIGH confidence)

**Rust Ecosystem (docs.rs/crates.io verified):**
- glam 0.32.0, nalgebra 0.34.1, rstar 0.12.2, bvh 0.12.0, parry3d 0.26.0
- serde 1.0.228, tracing 0.1.44, thiserror 2.0.18, rayon 1.11.0
- proptest 1.10.0, criterion 0.8.1, toml 1.0.1

**Rust Ecosystem (GitHub verified):**
- i-overlay 4.0 (iShape-Rust/iOverlay)
- clipper2-rust 1.0.0 (larsbrubaker/clipper2-rust) -- 444 tests, published on crates.io
- wasmtime 40.0 (bytecodealliance/wasmtime)
- wasm-bindgen-rayon (GoogleChromeLabs)

**Architecture References:**
- Bevy Core Architecture (deepwiki.com/bevyengine/bevy)
- Large Rust Workspaces (matklad.github.io)
- Plugins in Rust (adventures.michaelfbryan.com, nullderef.com)
- WASM Component Model (tartanllama.xyz)

**C++ Algorithm Analysis:**
- PrusaSlicer libslic3r source code (~250K lines analyzed)
- Cross-fork feature matrix (PrusaSlicer, OrcaSlicer, BambuStudio, CrealityPrint)

### Secondary (MEDIUM confidence)

- Rayon optimization analysis (gendignoux.com/blog) -- detailed benchmarks on parallelization overhead
- NPB-Rust benchmarks (arxiv.org) -- Rayon vs OpenMP academic comparison
- iOverlay performance benchmarks (ishape-rust.github.io) -- vendor benchmarks showing 6-22x faster than Clipper2
- GladiusSlicer (github.com) -- alpha-state Rust slicer reference
- Extism plugin system (extism.org) -- WASM plugin hosting patterns

### Tertiary (LOW confidence, needs validation)

- reqwest 0.12.x WASM auto-detection behavior -- verify with integration test
- WASM multi-threading stabilization timeline -- currently requires nightly Rust
- parry3d-f64 WASM binary size impact -- needs measurement
- mimalloc WASM allocator performance -- cited as 2x improvement over dlmalloc, vendor claim

---
*Research completed: 2026-02-14*
*Ready for roadmap: yes*
