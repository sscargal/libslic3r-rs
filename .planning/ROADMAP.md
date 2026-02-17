# Roadmap: libslic3r-rs

## Overview

This roadmap delivers a modular Rust-based 3D printer slicing engine from the ground up. The build order follows the slicing pipeline's strict dependency graph: foundation geometry first, then a vertical slice proving the full STL-to-G-code pipeline, then horizontal expansion of algorithms, then the differentiating capabilities (plugins, AI, API polish). Nine phases, derived from 86 v1 requirements, each delivering a coherent and verifiable capability.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Foundation Types and Geometry Core** - Coordinate system, polygon booleans, mesh data structures, WASM CI gate
- [x] **Phase 2: Mesh I/O and Repair** - Import STL/3MF/OBJ, auto-repair, transformations, validation
- [x] **Phase 3: Vertical Slice (STL to G-code)** - Minimum pipeline producing a printable calibration cube
- [x] **Phase 4: Perimeter and Infill Completeness** - All perimeter modes, all standard infill patterns, adaptive layers
- [x] **Phase 5: Support Structures** - Automatic, manual, tree, organic supports with bridge/overhang handling
- [ ] **Phase 6: G-code Completeness and Advanced Features** - All firmware dialects, multi-material, modifier meshes, advanced print features
- [ ] **Phase 7: Plugin System** - Plugin trait API, registry, native and WASM loading, example plugin
- [ ] **Phase 8: AI Integration** - LLM abstraction, geometry analysis, profile suggestions
- [ ] **Phase 9: API Polish, Testing, and Platform Validation** - Public API, structured output, cross-platform, performance/memory targets, test coverage

## Phase Details

### Phase 1: Foundation Types and Geometry Core
**Goal**: All downstream algorithm crates can build on stable coordinate types, polygon boolean operations, and mesh data structures -- the architectural decisions that cannot change later are locked in
**Depends on**: Nothing (first phase)
**Requirements**: FOUND-01, FOUND-04, FOUND-05, FOUND-08, MESH-09
**Success Criteria** (what must be TRUE):
  1. Integer coordinate types (Coord/IPoint2) exist with documented precision strategy, and converting between float mesh coordinates and integer polygon coordinates round-trips without losing meaningful precision
  2. Polygon boolean operations (union, intersection, difference, XOR) produce correct results on a suite of 20+ test cases including degenerate geometry (zero-area spikes, collinear vertices, self-intersections)
  3. Polygon offsetting (inward and outward) produces correct results, validated against Clipper2 reference output for the same inputs
  4. TriangleMesh data structure exists with BVH-accelerated spatial queries, uses arena+index pattern (no Rc/RefCell), and is Send+Sync
  5. `cargo build --target wasm32-unknown-unknown` succeeds with zero errors on all Phase 1 crates, enforced by CI
**Plans:** 4 plans

Plans:
- [ ] 01-01-PLAN.md -- Cargo workspace and slicecore-math crate (coordinate types, points, vectors, bounding boxes, matrices)
- [ ] 01-02-PLAN.md -- slicecore-geo crate (polygon types, boolean operations, offsetting, geometry utilities)
- [ ] 01-03-PLAN.md -- slicecore-mesh crate (TriangleMesh, BVH spatial index, mesh statistics, transforms)
- [ ] 01-04-PLAN.md -- WASM compilation gate, CI configuration, phase verification

### Phase 2: Mesh I/O and Repair
**Goal**: Users can load real-world 3D model files from Thingiverse/Printables and get clean, valid meshes ready for slicing -- even when the source files have common defects
**Depends on**: Phase 1
**Requirements**: MESH-01, MESH-02, MESH-03, MESH-04, MESH-05, MESH-06, MESH-07, MESH-08
**Success Criteria** (what must be TRUE):
  1. Binary STL, ASCII STL, 3MF (via lib3mf-core), and OBJ files load successfully, tested against 10+ real models from Thingiverse/Printables
  2. Non-manifold meshes, self-intersecting meshes, and meshes with degenerate triangles are automatically detected and repaired, validated against PrusaSlicer's repair test suite
  3. Mesh transformations (scale, rotate, translate, mirror) produce correct results, verified by comparing bounding boxes and vertex positions before/after
  4. G-code writer can emit valid Marlin-dialect output (used by Phase 3; tested with G-code syntax validation, not yet print-tested)
  5. ValidPolygon type system enforces that only cleaned/validated geometry enters downstream algorithms -- raw Polygon cannot be passed where ValidPolygon is required
**Plans:** 5 plans

Plans:
- [ ] 02-01-PLAN.md -- slicecore-fileio crate scaffold, STL parsers (binary + ASCII), format detection
- [ ] 02-02-PLAN.md -- Mesh repair pipeline (degenerate removal, edge stitching, hole filling, normal fix, self-intersection detection)
- [ ] 02-03-PLAN.md -- slicecore-gcode-io crate (structured commands, writer, 4 firmware dialects, validator)
- [ ] 02-04-PLAN.md -- 3MF parser (lib3mf), OBJ parser (tobj), unified load_mesh(), WASM validation
- [ ] 02-05-PLAN.md -- Integration tests, synthetic test fixtures, Phase 2 success criteria verification

### Phase 3: Vertical Slice (STL to G-code)
**Goal**: The full slicing pipeline works end-to-end: a real STL file goes in and valid, printable G-code comes out -- proving the architecture before investing in breadth
**Depends on**: Phase 2
**Requirements**: SLICE-01, SLICE-03, SLICE-05, PERIM-01, PERIM-03, INFILL-01, INFILL-11, INFILL-12, GCODE-01, GCODE-05, GCODE-07, GCODE-08, GCODE-09, GCODE-10, API-02
**Success Criteria** (what must be TRUE):
  1. A 20mm calibration cube STL produces G-code that prints correctly on a real FDM printer running Marlin firmware -- walls are solid, top/bottom surfaces are filled, dimensions are within 0.2mm tolerance
  2. The CLI binary accepts `slice <input.stl> --config <profile.toml> --output <output.gcode>` and produces complete G-code with start/end sequences, temperature commands, retraction, speed control, and cooling
  3. Slicing is deterministic: the same STL + same config produces bit-for-bit identical G-code across multiple runs
  4. Layer slicing at configurable heights works correctly -- changing layer height from 0.2mm to 0.1mm doubles the layer count (within rounding tolerance) and produces valid G-code at both settings
  5. Skirt/brim generation works for bed adhesion, and infill density is configurable from 0-100%
**Plans:** 6 plans

Plans:
- [ ] 03-01-PLAN.md -- PrintConfig + slicecore-slicer crate (triangle-plane intersection, segment chaining, contour extraction)
- [ ] 03-02-PLAN.md -- Perimeter generation (polygon offset shells) and rectilinear infill pattern generation
- [ ] 03-03-PLAN.md -- Surface classification (top/bottom solid), extrusion math, toolpath segment types
- [ ] 03-04-PLAN.md -- Planner (skirt/brim, retraction, temperature, fan) and G-code generation from toolpaths
- [ ] 03-05-PLAN.md -- Engine orchestrator (full pipeline) and CLI binary (slice/validate/analyze)
- [ ] 03-06-PLAN.md -- Integration tests, determinism verification, phase success criteria validation

### Phase 4: Perimeter and Infill Completeness
**Goal**: Users have access to the full range of perimeter generation modes and infill patterns needed for real-world printing -- thin walls, seam control, and pattern variety
**Depends on**: Phase 3
**Requirements**: SLICE-02, SLICE-04, PERIM-02, PERIM-04, PERIM-05, PERIM-06, INFILL-02, INFILL-03, INFILL-04, INFILL-05, INFILL-06, INFILL-07, INFILL-08
**Success Criteria** (what must be TRUE):
  1. Arachne variable-width perimeters handle thin walls correctly -- a test model with 0.8mm walls prints cleanly without gaps, unlike classic perimeters which would leave voids
  2. All 8 standard infill patterns (rectilinear, grid, honeycomb, gyroid, adaptive cubic, cubic, lightning, monotonic) generate correct toolpaths, each visually distinct and structurally appropriate for its pattern type
  3. Seam placement strategies (aligned, random, rear, smart hiding) produce visually different seam lines on a cylindrical test model, and scarf joint seam produces a smooth, nearly invisible seam transition
  4. Adaptive layer heights vary layer thickness based on surface curvature -- a sphere model uses thinner layers at the equator (high curvature) and thicker layers at the poles (low curvature)
  5. Gap fill between perimeters produces solid walls without voids on a test model with varying wall thicknesses
**Plans:** 10 plans

Plans:
- [ ] 04-01-PLAN.md -- Infill module refactor, InfillPattern dispatch, Grid + Monotonic patterns
- [ ] 04-02-PLAN.md -- Seam placement strategies (aligned, random, rear, smart hiding)
- [ ] 04-03-PLAN.md -- Adaptive layer heights based on surface curvature
- [ ] 04-04-PLAN.md -- Honeycomb + Cubic infill patterns
- [ ] 04-05-PLAN.md -- Gyroid infill pattern (TPMS + marching squares)
- [ ] 04-06-PLAN.md -- Scarf joint seam with 12 configurable parameters
- [ ] 04-07-PLAN.md -- Adaptive Cubic + Lightning infill patterns
- [ ] 04-08-PLAN.md -- Gap fill between perimeters
- [ ] 04-09-PLAN.md -- Arachne variable-width perimeters (boostvoronoi)
- [ ] 04-10-PLAN.md -- Preview data, engine integration, integration tests + phase verification

### Phase 5: Support Structures
**Goal**: Users can print models with overhangs and bridges confidently -- automatic supports are generated where needed, tree supports minimize material waste, and support removal leaves clean surfaces
**Depends on**: Phase 3
**Requirements**: SUPP-01, SUPP-02, SUPP-03, SUPP-04, SUPP-05, SUPP-06, SUPP-07, SUPP-08
**Success Criteria** (what must be TRUE):
  1. Automatic support generation correctly identifies overhangs beyond a configurable angle threshold and generates traditional grid/line support structures that print and remove cleanly
  2. Tree supports generate branching structures that reach overhang areas with less material than traditional supports, validated by comparing filament usage on a test model with significant overhangs
  3. Bridge detection identifies unsupported spans and applies bridge-specific speed/fan/flow settings -- a bridge test model prints clean horizontal bridges at 20mm+ spans
  4. Manual support enforcers and blockers override automatic support placement -- enforcers add support where auto-detection missed, blockers remove support from areas where removal would damage the part
  5. Support interface layers (dense contact layers between support and part surface) produce better surface finish on the supported face compared to direct support contact
**Plans:** 8 plans

Plans:
- [ ] 05-01-PLAN.md -- Support configuration types and overhang detection (hybrid layer-diff + raycast)
- [ ] 05-02-PLAN.md -- Traditional grid/line support generation with XY gap
- [ ] 05-03-PLAN.md -- Bridge detection (combined angle/endpoint/span criteria) and G-code integration
- [ ] 05-04-PLAN.md -- Tree support generation (bottom-up growth, branching, merging, organic/geometric)
- [ ] 05-05-PLAN.md -- Support interface layers, quality presets, material-specific defaults
- [ ] 05-06-PLAN.md -- Manual override system (enforcers/blockers, volume modifiers, conflict resolution)
- [ ] 05-07-PLAN.md -- 4-tier overhang control, auto support type selection, engine pipeline integration
- [ ] 05-08-PLAN.md -- Integration tests and Phase 5 success criteria verification

### Phase 6: G-code Completeness and Advanced Features
**Goal**: Users can target any major firmware dialect and use advanced print features -- multi-material, per-region settings, and dimensional accuracy tools
**Depends on**: Phase 4, Phase 5
**Requirements**: GCODE-02, GCODE-03, GCODE-04, GCODE-06, GCODE-11, GCODE-12, GCODE-13, ADV-01, ADV-02, ADV-03, ADV-04, ADV-05, ADV-06, ADV-07, ADV-08, INFILL-09, INFILL-10
**Success Criteria** (what must be TRUE):
  1. G-code output for Klipper, RepRapFirmware, and Bambu firmware dialects passes firmware-specific syntax validation and contains correct dialect-specific commands (e.g., Klipper pressure advance, Bambu AMS commands)
  2. Multi-material prints generate correct tool change sequences and purge tower G-code -- a two-color test model slices with proper MMU commands
  3. Modifier meshes apply region-specific setting overrides -- a model with an internal modifier region uses different infill density inside vs. outside the modifier
  4. Print time and filament usage estimates are within 15% of actual measured values on a set of 5 test prints
  5. Arc fitting converts line segments to G2/G3 arcs where appropriate, reducing G-code file size by at least 20% on curved models while maintaining dimensional accuracy
**Plans**: TBD

Plans:
- [ ] 06-01: TBD
- [ ] 06-02: TBD
- [ ] 06-03: TBD

### Phase 7: Plugin System
**Goal**: External developers can write custom infill patterns, support strategies, or G-code post-processors as plugins and load them without modifying or recompiling the core -- the core architectural differentiator works
**Depends on**: Phase 4 (stable trait interfaces needed)
**Requirements**: PLUGIN-01, PLUGIN-02, PLUGIN-03, PLUGIN-04, PLUGIN-05, PLUGIN-06, PLUGIN-07
**Success Criteria** (what must be TRUE):
  1. A custom infill pattern plugin (implementing the InfillPattern trait) can be compiled separately, loaded at runtime via abi_stable, and produces valid infill toolpaths that slice and print correctly
  2. A WASM plugin loaded via wasmtime Component Model can provide a custom infill pattern, and a bug/crash in the WASM plugin does not crash or corrupt the host process
  3. PluginRegistry discovers, validates, and manages plugins -- listing available plugins, their capabilities, and version compatibility
  4. Plugin API is documented with rustdoc and includes at least two working example plugins (one native, one WASM) with build instructions that a developer can follow to create their own
**Plans**: TBD

Plans:
- [ ] 07-01: TBD
- [ ] 07-02: TBD

### Phase 8: AI Integration
**Goal**: Users can send a 3D model and receive intelligent print profile suggestions from a local or cloud LLM -- the second core differentiator works end-to-end
**Depends on**: Phase 3 (needs working pipeline to suggest settings for)
**Requirements**: AI-01, AI-02, AI-03, AI-04, AI-05, AI-06
**Success Criteria** (what must be TRUE):
  1. Geometry analysis extracts meaningful features from a mesh (bounding box, overhang areas, thin wall regions, surface area, volume) and presents them in a structured format suitable for LLM consumption
  2. Profile suggestion works end-to-end: upload a model, geometry is analyzed, features are sent to an LLM, and the response is parsed into a valid print profile that can be used for slicing
  3. Both local LLM (ollama) and cloud LLM (OpenAI or Anthropic API) providers work through the same abstraction layer -- switching providers requires only a config change, not code changes
  4. AI suggestions are reasonable: for a model with large overhangs, the AI suggests enabling supports; for a model with thin walls, the AI suggests appropriate wall settings
**Plans**: TBD

Plans:
- [ ] 08-01: TBD
- [ ] 08-02: TBD

### Phase 9: API Polish, Testing, and Platform Validation
**Goal**: The library is production-ready -- documented public API, structured output, cross-platform builds pass, performance and memory targets are met, and test coverage exceeds 80%
**Depends on**: Phase 6, Phase 7, Phase 8
**Requirements**: FOUND-02, FOUND-03, FOUND-06, FOUND-07, API-01, API-03, API-04, API-05, TEST-01, TEST-02, TEST-03, TEST-04, TEST-05, TEST-07
**Success Criteria** (what must be TRUE):
  1. All public API items have rustdoc documentation, and `cargo doc --no-deps` produces clean output with zero warnings
  2. JSON and MessagePack structured output work for slicing results, settings export, and metadata -- external tools can consume the output programmatically
  3. The library builds and passes tests on macOS (ARM + x86), Linux (ARM + x86), and Windows (ARM + x86), verified by CI matrix
  4. WASM compilation (wasm32-wasi and wasm32-unknown-unknown) succeeds and a browser-based slicing demo produces correct G-code
  5. Performance matches or beats C++ libslic3r on a benchmark suite of 5 models, and memory usage is at or below 80% of C++ libslic3r, measured by cargo-criterion and peak RSS comparison

## Coverage Notes

**Requirements with scope conflicts (flagged for user decision):**

API-06 (C FFI layer) and API-07 (Python bindings via PyO3) are listed as v1 requirements in REQUIREMENTS.md, but PROJECT.md explicitly states "FFI bindings to C/C++/Python/Go" are **Out of Scope** with rationale "Pure Rust ecosystem; build missing crates instead." These two requirements are **excluded from this roadmap** pending clarification. If they should be included, they would map to Phase 9.

**Adjusted coverage:** 84 of 86 v1 requirements mapped. 2 excluded due to scope conflict (API-06, API-07).

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7 -> 8 -> 9
(Phases 4 and 5 can run in parallel after Phase 3. Phase 7 can start after Phase 4. Phase 8 can start after Phase 3.)

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation Types and Geometry Core | 4/4 | ✓ Complete | 2026-02-15 |
| 2. Mesh I/O and Repair | 5/5 | ✓ Complete | 2026-02-16 |
| 3. Vertical Slice (STL to G-code) | 6/6 | ✓ Complete | 2026-02-16 |
| 4. Perimeter and Infill Completeness | 10/10 | ✓ Complete | 2026-02-17 |
| 5. Support Structures | 8/8 | ✓ Complete | 2026-02-17 |
| 6. G-code Completeness and Advanced Features | 0/TBD | Not started | - |
| 7. Plugin System | 0/TBD | Not started | - |
| 8. AI Integration | 0/TBD | Not started | - |
| 9. API Polish, Testing, and Platform Validation | 0/TBD | Not started | - |
