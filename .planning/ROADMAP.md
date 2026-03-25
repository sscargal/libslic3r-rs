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
- [x] **Phase 6: G-code Completeness and Advanced Features** - All firmware dialects, multi-material, modifier meshes, advanced print features
- [x] **Phase 7: Plugin System** - Plugin trait API, registry, native and WASM loading, example plugin
- [x] **Phase 8: AI Integration** - LLM abstraction, geometry analysis, profile suggestions
- [x] **Phase 9: API Polish, Testing, and Platform Validation** - Public API, structured output, cross-platform, performance/memory targets, test coverage
- [ ] **Phase 10: CLI Feature Integration** - Enable plugins and AI in CLI binary, add ai-suggest subcommand
- [ ] **Phase 11: Config Integration** - Wire plugin_dir, sequential, and multi-material into Engine pipeline
- [ ] **Phase 12: Mesh Repair Completion** - Implement self-intersection resolution
- [x] **Phase 13: JSON Profile Support** - Import OrcaSlicer/BambuStudio JSON profiles with auto-format detection
- [x] **Phase 14: Profile Conversion Tool (JSON to TOML)** - Convert JSON profiles to native TOML format with selective output and round-trip fidelity
- [x] **Phase 15: Printer and Filament Profile Library** - Build extensive profile library from OrcaSlicer/BambuStudio with CLI discovery commands
- [x] **Phase 16: PrusaSlicer Profile Migration** - Convert PrusaSlicer INI profiles to TOML, extending library with ~9,500 profiles across 33 FFF vendors

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
**Plans:** 9 plans

Plans:
- [x] 06-01-PLAN.md -- Firmware dialect enrichment, GcodeCommand extension, configurable dialect in engine (GCODE-02, GCODE-03, GCODE-04, GCODE-06)
- [x] 06-02-PLAN.md -- Per-feature flow control, custom G-code injection, ironing (ADV-04, ADV-05, ADV-08)
- [x] 06-03-PLAN.md -- TPMS-D and TPMS-FK infill patterns (INFILL-09, INFILL-10)
- [x] 06-04-PLAN.md -- Arc fitting algorithm and engine integration (GCODE-11)
- [x] 06-05-PLAN.md -- Print time estimation (trapezoid model) and filament usage estimation (GCODE-12, GCODE-13)
- [x] 06-06-PLAN.md -- Modifier meshes and polyhole conversion (ADV-03, ADV-07)
- [x] 06-07-PLAN.md -- Multi-material support, purge tower, sequential printing (ADV-01, ADV-02)
- [x] 06-08-PLAN.md -- Pressure advance calibration pattern generation (ADV-06)
- [x] 06-09-PLAN.md -- Integration tests and Phase 6 success criteria verification

### Phase 7: Plugin System
**Goal**: External developers can write custom infill patterns, support strategies, or G-code post-processors as plugins and load them without modifying or recompiling the core -- the core architectural differentiator works
**Depends on**: Phase 4 (stable trait interfaces needed)
**Requirements**: PLUGIN-01, PLUGIN-02, PLUGIN-03, PLUGIN-04, PLUGIN-05, PLUGIN-06, PLUGIN-07
**Success Criteria** (what must be TRUE):
  1. A custom infill pattern plugin (implementing the InfillPattern trait) can be compiled separately, loaded at runtime via abi_stable, and produces valid infill toolpaths that slice and print correctly
  2. A WASM plugin loaded via wasmtime Component Model can provide a custom infill pattern, and a bug/crash in the WASM plugin does not crash or corrupt the host process
  3. PluginRegistry discovers, validates, and manages plugins -- listing available plugins, their capabilities, and version compatibility
  4. Plugin API is documented with rustdoc and includes at least two working example plugins (one native, one WASM) with build instructions that a developer can follow to create their own
**Plans:** 7 plans

Plans:
- [ ] 07-01-PLAN.md -- slicecore-plugin-api crate (FFI-safe types, sabi_trait InfillPatternPlugin, metadata)
- [ ] 07-02-PLAN.md -- slicecore-plugin crate scaffold, PluginRegistry, native plugin loader via abi_stable
- [ ] 07-03-PLAN.md -- WASM plugin loader (wasmtime Component Model), WIT interface, sandboxing
- [ ] 07-04-PLAN.md -- Engine integration (InfillPattern::Plugin variant, generate_infill dispatch, feature gating)
- [ ] 07-05-PLAN.md -- Example native zigzag-infill plugin (cdylib with abi_stable)
- [ ] 07-06-PLAN.md -- Example WASM spiral-infill plugin (Component Model with wit-bindgen)
- [ ] 07-07-PLAN.md -- Integration tests (SC1-SC3) and rustdoc documentation (SC4)

### Phase 8: AI Integration
**Goal**: Users can send a 3D model and receive intelligent print profile suggestions from a local or cloud LLM -- the second core differentiator works end-to-end
**Depends on**: Phase 3 (needs working pipeline to suggest settings for)
**Requirements**: AI-01, AI-02, AI-03, AI-04, AI-05, AI-06
**Success Criteria** (what must be TRUE):
  1. Geometry analysis extracts meaningful features from a mesh (bounding box, overhang areas, thin wall regions, surface area, volume) and presents them in a structured format suitable for LLM consumption
  2. Profile suggestion works end-to-end: upload a model, geometry is analyzed, features are sent to an LLM, and the response is parsed into a valid print profile that can be used for slicing
  3. Both local LLM (ollama) and cloud LLM (OpenAI or Anthropic API) providers work through the same abstraction layer -- switching providers requires only a config change, not code changes
  4. AI suggestions are reasonable: for a model with large overhangs, the AI suggests enabling supports; for a model with thin walls, the AI suggests appropriate wall settings
**Plans:** 5 plans

Plans:
- [ ] 08-01-PLAN.md -- slicecore-ai crate scaffold, core types, AiProvider trait, AiConfig, AiError (AI-01)
- [ ] 08-02-PLAN.md -- OpenAI, Anthropic, and Ollama provider implementations (AI-04, AI-05)
- [ ] 08-03-PLAN.md -- Geometry feature extraction, prompt templates, profile suggestion parsing (AI-02)
- [ ] 08-04-PLAN.md -- End-to-end suggest pipeline, sync wrapper, engine integration with ai feature flag (AI-03)
- [ ] 08-05-PLAN.md -- Integration tests verifying all success criteria with mock providers (AI-06)

### Phase 9: API Polish, Testing, and Platform Validation
**Goal**: The library is production-ready -- documented public API, structured output, cross-platform builds pass, performance and memory targets are met, and test coverage exceeds 80%
**Depends on**: Phase 6, Phase 7, Phase 8
**Requirements**: FOUND-02, FOUND-03, FOUND-06, FOUND-07, API-01, API-03, API-04, API-05, TEST-01, TEST-02, TEST-03, TEST-04, TEST-05, TEST-07
**Success Criteria** (what must be TRUE):
  1. All public API items have rustdoc documentation, and `cargo doc --no-deps` produces clean output with zero warnings
  2. JSON and MessagePack structured output work for slicing results, settings export, and metadata -- external tools can consume the output programmatically
  3. The library builds and passes tests on macOS (ARM + x86), Linux (ARM + x86), and Windows (ARM + x86), verified by CI matrix
  4. WASM compilation (wasm32-wasip2 and wasm32-unknown-unknown) succeeds and a browser-based slicing demo produces correct G-code
  5. Performance matches or beats C++ libslic3r on a benchmark suite of 5 models, and memory usage is at or below 80% of C++ libslic3r, measured by cargo-criterion and peak RSS comparison
**Plans:** 8 plans

Plans:
- [ ] 09-01-PLAN.md -- Fix rustdoc warnings (broken intra-doc links)
- [ ] 09-02-PLAN.md -- Add module-level doc comments to all pub mods
- [ ] 09-03-PLAN.md -- Add Serialize/Deserialize derives to all public API types
- [ ] 09-04-PLAN.md -- Event system (SliceEvent, EventBus, subscribers) and structured output (JSON + MessagePack) with CLI flags
- [ ] 09-05-PLAN.md -- WASM getrandom fix for wasm32-unknown-unknown and multi-platform CI matrix (macOS/Linux/Windows/ARM)
- [ ] 09-06-PLAN.md -- Criterion benchmark suite (5 synthetic models, geometry hot-path micro-benchmarks)
- [ ] 09-07-PLAN.md -- Fuzz testing targets for mesh parsers and golden file tests for G-code regression detection
- [ ] 09-08-PLAN.md -- Integration tests, coverage measurement (>= 80%), and Phase 9 success criteria verification

### Phase 10: CLI Feature Integration
**Goal**: CLI users can access plugin loading and AI profile suggestions -- the core v1.0 differentiating features are exposed through the binary, not just the library API
**Depends on**: Phase 7, Phase 8, Phase 9
**Gap Closure**: Addresses PLUGIN-05 partial, AI-03 partial, integration gaps (CLI feature gates), Flow F (ai-suggest command)
**Success Criteria** (what must be TRUE):
  1. `slicecore` binary is compiled with `features = ["plugins", "ai"]` enabled in Cargo.toml
  2. `slicecore ai-suggest input.stl` subcommand exists and successfully calls `Engine::suggest_profile()` with default Ollama provider
  3. Plugin-based infill patterns work via CLI: config file with `infill_pattern = { plugin = "zigzag" }` loads and executes the plugin
  4. CLI help text documents plugin and AI features, including how to configure providers and plugin directories
  5. Integration tests verify both features work end-to-end via the CLI binary (not just library API)
**Plans:** 3 plans

Plans:
- [x] 10-01-PLAN.md -- Enable plugins and ai features in CLI Cargo.toml, verify compilation
- [x] 10-02-PLAN.md -- Add ai-suggest CLI subcommand with provider configuration
- [x] 10-03-PLAN.md -- Update CLI help, add integration tests, verify Phase 10 success criteria

### Phase 11: Config Integration
**Goal**: All PrintConfig fields are wired into the Engine pipeline -- users' TOML settings actually affect slicing behavior without requiring direct API calls
**Depends on**: Phase 10
**Gap Closure**: Addresses integration gaps (plugin_dir orphaned, sequential/multi-material not wired), Flow E (plugin_dir), Flow G (sequential)
**Success Criteria** (what must be TRUE):
  1. Setting `plugin_dir = "/path/to/plugins"` in config TOML triggers automatic plugin discovery and loading via `PluginRegistry::discover_and_load()` in Engine constructor
  2. Setting `sequential.enabled = true` in config TOML triggers collision detection and object ordering in `Engine::slice()` pipeline
  3. Setting `multi_material.enabled = true` with multiple tool configs triggers tool changes and purge tower generation in `Engine::slice()` pipeline
  4. Integration tests verify all three config-driven features work without requiring manual API calls to specialized methods
  5. RepairReport or warnings notify users if plugin_dir is set but contains no valid plugins

**Plans:** 4 plans

Plans:
- [x] 11-01-PLAN.md -- Wire plugin_dir auto-loading into Engine constructor and update CLI to prevent double-loading
- [x] 11-02-PLAN.md -- Add connected_components() to TriangleMesh and wire sequential printing into Engine pipeline
- [x] 11-03-PLAN.md -- Wire multi-material validation and purge tower generation into Engine pipeline
- [x] 11-04-PLAN.md -- Integration tests for all config-driven features and Phase 11 success criteria verification

### Phase 12: Mesh Repair Completion
**Goal**: Self-intersecting meshes are automatically repaired, not just detected -- users get clean geometry without external preprocessing tools
**Depends on**: Phase 2
**Gap Closure**: Addresses MESH-06 (self-intersection resolution missing)
**Success Criteria** (what must be TRUE):
  1. Self-intersection resolution uses Clipper2 boolean union via per-slice contour union (not direct 3D mesh modification)
  2. RepairReport shows detection metrics: intersecting triangle pairs, affected Z-range, and resolution status flag -- contours in the affected Z-range produce clean merged output after per-slice union
  3. Test suite includes self-intersecting test models (programmatically generated overlapping geometry avoiding licensing issues) that successfully slice with clean contour output
  4. Resolved contours pass validation: positive area, correct winding (CCW outer, CW holes), no degenerate polygons -- mesh-level detection confirms intersections are flagged for deferred resolution
  5. Performance is acceptable: detection + resolution completes in <5 seconds for models with <10k triangles

**Plans:** 3 plans

Plans:
- [x] 12-01-PLAN.md -- BVH-accelerated self-intersection detection with pair reporting and per-slice contour resolution via Clipper2 union
- [x] 12-02-PLAN.md -- Wire contour resolution into engine pipeline, programmatic self-intersecting test meshes, end-to-end tests
- [x] 12-03-PLAN.md -- Integration tests for all 5 success criteria, performance verification, phase completion

### Phase 13: JSON Profile Support
**Goal**: Users can import printer and filament profiles from OrcaSlicer and BambuStudio JSON format files, with auto-detection of file format (JSON vs TOML) and field mapping from upstream schema to PrintConfig
**Depends on**: Phase 12
**Success Criteria** (what must be TRUE):
  1. Config file format (JSON vs TOML) is auto-detected by content sniffing, not file extension
  2. OrcaSlicer/BambuStudio JSON profiles (process, filament, machine types) are mapped to PrintConfig fields with correct value conversion (string-to-number, percentage stripping, array unwrapping, nil sentinel handling)
  3. ImportResult reports both mapped and unmapped fields so users know what was and wasn't imported
  4. CLI --config flag accepts both TOML and JSON files without user intervention
  5. Real upstream profiles from OrcaSlicer and BambuStudio load without errors and produce reasonable config values
**Plans:** 2 plans

Plans:
- [ ] 13-01-PLAN.md -- Profile import module with format detection, JSON field mapping, and ImportResult
- [ ] 13-02-PLAN.md -- CLI integration, integration tests with real upstream profiles, phase verification

## Coverage Notes

**Requirements with scope conflicts (flagged for user decision):**

API-06 (C FFI layer) and API-07 (Python bindings via PyO3) are listed as v1 requirements in REQUIREMENTS.md, but PROJECT.md explicitly states "FFI bindings to C/C++/Python/Go" are **Out of Scope** with rationale "Pure Rust ecosystem; build missing crates instead." These two requirements are **excluded from this roadmap** pending clarification. If they should be included, they would map to Phase 9.

**Adjusted coverage:** 84 of 86 v1 requirements mapped. 2 excluded due to scope conflict (API-06, API-07).

**Gap closure phases (10-12):**

Phases 10-12 address gaps identified by milestone audit (2026-02-18):
- Phase 10 completes PLUGIN-05 and AI-03 (CLI accessibility)
- Phase 11 fixes config integration (plugin_dir, sequential, multi-material)
- Phase 12 completes MESH-06 (self-intersection resolution)

After Phases 10-12, all v1.0 requirements will be fully satisfied with no partial implementations.

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7 -> 8 -> 9 -> 10 -> 11 -> 12 -> 13
(Phases 4 and 5 can run in parallel after Phase 3. Phase 7 can start after Phase 4. Phase 8 can start after Phase 3. Phase 12 can run in parallel with Phases 10-11.)

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation Types and Geometry Core | 4/4 | ✓ Complete | 2026-02-15 |
| 2. Mesh I/O and Repair | 5/5 | ✓ Complete | 2026-02-16 |
| 3. Vertical Slice (STL to G-code) | 6/6 | ✓ Complete | 2026-02-16 |
| 4. Perimeter and Infill Completeness | 10/10 | ✓ Complete | 2026-02-17 |
| 5. Support Structures | 8/8 | ✓ Complete | 2026-02-17 |
| 6. G-code Completeness and Advanced Features | 9/9 | ✓ Complete | 2026-02-17 |
| 7. Plugin System | 7/7 | ✓ Complete | 2026-02-17 |
| 8. AI Integration | 5/5 | ✓ Complete | 2026-02-17 |
| 9. API Polish, Testing, and Platform Validation | 8/8 | ✓ Complete | 2026-02-18 |
| 10. CLI Feature Integration | 3/3 | ✓ Complete | 2026-02-18 |
| 11. Config Integration | 4/4 | ✓ Complete | 2026-02-18 |
| 12. Mesh Repair Completion | 3/3 | ✓ Complete | 2026-02-18 |
| 13. JSON Profile Support | 2/2 | ✓ Complete | 2026-02-18 |
| 14. Profile Conversion Tool (JSON to TOML) | 2/2 | ✓ Complete | 2026-02-18 |
| 15. Printer and Filament Profile Library | 3/3 | ✓ Complete | 2026-02-18 |
| 16. PrusaSlicer Profile Migration | 2/2 | ✓ Complete | 2026-02-19 |
| 17. BambuStudio Profile Migration | 1/1 | ✓ Complete | 2026-02-19 |
| 42. Clone and Customize Profiles | 2/2 | Complete    | 2026-03-20 |
| 43. Enable/Disable Printer and Filament Profiles | 3/3 | Complete    | 2026-03-21 |
| 44. Search and Filter Profiles by Compatibility | 3/3 | Complete    | 2026-03-23 |
| 45. Global and Per-Object Settings Override System | 11/11 | Complete    | 2026-03-24 |
| 46. Job Output Directories | 3/3 | Complete    | 2026-03-25 |
| 47. Variable Layer Height Algorithms | 3/4 | In Progress|  |
| 48. Selective Adaptive Z-Hop Control | 0/0 | ○ Pending | - |
| 49. Hybrid Sequential Printing | 0/0 | ○ Pending | - |
| 50. 3MF Project Output with Embedded G-code | 0/0 | ○ Pending | - |
| 51. Comprehensive Documentation Suite | 0/0 | ○ Pending | - |

### Phase 14: Profile Conversion Tool (JSON to TOML)

**Goal:** Users can convert OrcaSlicer/BambuStudio JSON profiles to slicecore's native TOML format via a CLI subcommand, with selective output (only mapped fields), multi-file merge, and round-trip fidelity
**Depends on:** Phase 13
**Success Criteria** (what must be TRUE):
  1. `slicecore convert-profile input.json` produces valid TOML containing only fields that were mapped from the source (not all 86 PrintConfig defaults)
  2. JSON -> PrintConfig -> TOML -> PrintConfig round-trip preserves all mapped field values within floating-point tolerance
  3. Multiple input files (process + filament + machine) merge correctly into a single unified TOML profile
  4. Conversion report on stderr shows source metadata, mapped field count, and unmapped field names
  5. Float values in TOML output are clean (no IEEE 754 artifacts like 0.15000000000000002)
**Plans:** 2 plans

Plans:
- [x] 14-01-PLAN.md -- Profile conversion module (selective TOML output, multi-file merge) and CLI convert-profile subcommand
- [x] 14-02-PLAN.md -- Integration tests (round-trip, merge, real profiles) and phase verification

### Phase 15: Printer and Filament Profile Library

**Goal:** Build an extensive library of printer and filament profiles from upstream slicers (OrcaSlicer, BambuStudio, PrusaSlicer) stored in profiles/ directory with logical organization by vendor/material/properties, and provide CLI commands for searching and listing profiles
**Depends on:** Phase 14
**Requirements:**
  - Import existing printer and filament profiles from /home/steve/slicer-analysis/
  - Store profiles in profiles/ with logical directory structure (by vendor, material, nozzle size, etc.)
  - Add CLI subcommands for profile discovery: list, search, show
  - Integration tests comparing original JSON profiles vs converted TOML profiles
**Plans:** 3 plans

Plans:
- [x] 15-01-PLAN.md -- Batch conversion module with inheritance resolution, profile index, CLI import-profiles subcommand
- [x] 15-02-PLAN.md -- CLI profile discovery subcommands (list-profiles, search-profiles, show-profile) and profile library generation
- [x] 15-03-PLAN.md -- Integration tests for conversion fidelity, round-trip verification, and phase success criteria

### Phase 16: PrusaSlicer Profile Migration

**Goal:** Convert PrusaSlicer printer and filament profiles from INI format to native TOML, extending the profile library with ~9,500 profiles across 33 FFF vendors, using the same output structure and CLI commands established in Phase 15
**Depends on:** Phase 15
**Success Criteria** (what must be TRUE):
  1. PrusaSlicer INI files parse correctly with typed section headers, key-value pairs, and abstract/concrete profile discrimination
  2. Multi-parent inheritance (semicolon-separated) resolves left-to-right with recursive depth, producing correct merged field values
  3. PrusaSlicer field names map to PrintConfig with correct value conversion (percentage stripping, comma-separated multi-extruder values)
  4. Batch conversion produces TOML files in profiles/prusaslicer/vendor/type/ directory structure
  5. Merged index.json contains entries from both OrcaSlicer and PrusaSlicer sources without clobbering
  6. CLI list-profiles, search-profiles, and show-profile work with the combined multi-source profile library
**Plans:** 2 plans

Plans:
- [x] 16-01-PLAN.md -- INI parser module, PrusaSlicer field mapping, batch conversion, index merge, CLI integration
- [x] 16-02-PLAN.md -- Generate PrusaSlicer profile library, integration tests, phase success criteria verification

### Phase 17: BambuStudio Profile Migration

**Goal:** Import ~2,348 BambuStudio profiles into the profile library using the existing batch conversion pipeline (zero code changes), extending the library to 3 sources and ~17,600 total profiles
**Depends on:** Phase 16
**Success Criteria** (what must be TRUE):
  1. BambuStudio JSON profiles convert via existing `batch_convert_profiles()` with zero code changes
  2. ~2,348 instantiated profiles convert without fatal errors
  3. Merged index.json contains entries from all three sources (orcaslicer + prusaslicer + bambustudio)
  4. CLI list-profiles, search-profiles, show-profile work with the combined 3-source library
  5. BambuStudio-unique profiles (H2C, H2S, P2S) are present in the library
  6. Integration tests verify batch conversion, index merge, and TOML round-trip fidelity
**Plans:** 1 plan

Plans:
- [x] 17-01-PLAN.md -- Generate BambuStudio profile library via CLI, integration tests for batch conversion and index merge

### Phase 18: CrealityPrint Profile Migration. Find and convert the printer/machine and filament/material profiles in /home/steve/slicer-analysis/CrealityPrint to profiles/ in this project like we did in the previous phase.

**Goal:** Import ~3,940 CrealityPrint profiles into the profile library using the existing batch conversion pipeline (zero code changes), extending the library to 4 sources and ~21,544 total profiles
**Depends on:** Phase 17
**Success Criteria** (what must be TRUE):
  1. CrealityPrint JSON profiles convert via existing `batch_convert_profiles()` with zero code changes
  2. ~3,940 instantiated profiles convert without fatal errors
  3. Merged index.json contains entries from all four sources (orcaslicer + prusaslicer + bambustudio + crealityprint)
  4. CLI list-profiles, search-profiles, show-profile work with the combined 4-source library
  5. CrealityPrint-unique profiles (K2, GS-01, SPARKX i7) are present in the library
  6. Integration tests verify batch conversion, index merge, and TOML round-trip fidelity
**Plans:** 1 plan

Plans:
- [ ] 18-01-PLAN.md -- Generate CrealityPrint profile library via CLI, integration tests for batch conversion and index merge

### Phase 19: Slicing Summary and Print Statistics

**Goal:** Generate detailed per-feature slicing statistics after G-code generation, presenting print time, filament usage, and per-feature breakdowns in user-selectable formats (ASCII table, CSV, JSON) with configurable precision, sorting, and support subtotals
**Depends on:** Phase 18
**Requirements:** GCODE-12, GCODE-13
**Success Criteria** (what must be TRUE):
  1. Per-feature statistics (time, filament, distance, segment count) are computed from LayerToolpath segments and available in SliceResult after every slice
  2. G-code metrics (retraction count/distance, z-hop count/distance, wipe count/distance) are extracted from the GcodeCommand stream
  3. ASCII table, CSV, and JSON output formats display per-feature breakdown with both percentage-of-total and percentage-of-print columns
  4. CLI flags control statistics format (--stats-format), quiet mode (--quiet), file output (--stats-file), time precision (--time-precision), sort order (--sort-stats)
  5. When --json is used for slice output, statistics are included by default (--json-no-stats to exclude)
  6. All features appear in output even when unused (zero values), and support features have separate subtotals
**Plans:** 3 plans (2 complete + 1 gap closure)

Plans:
- [ ] 19-01-PLAN.md -- Engine statistics module (PrintStatistics types, per-feature computation, G-code metrics extraction, pipeline integration)
- [ ] 19-02-PLAN.md -- CLI formatting (ASCII table via comfy-table, CSV, JSON), CLI flags, integration tests

### Phase 20: Expand PrintConfig Field Coverage and Profile Mapping

**Goal:** Expand PrintConfig to include the ~50 most impactful upstream slicer fields (layer_height, nozzle_diameter, retract_length, line widths, per-feature speeds, bed size, start/end G-code, cooling settings, max volumetric speed) and update the JSON/INI-to-TOML profile mapping so converted profiles capture enough settings for meaningful apples-to-apples slicer output comparison
**Depends on:** Phase 19
**Success Criteria** (what must be TRUE):
  1. PrintConfig includes all critical process fields: layer_height, bottom_solid_layers, bridge_speed, inner_wall_speed, gap_fill_speed, top_surface_speed, and all line width fields (outer_wall, inner_wall, infill, top_surface, initial_layer)
  2. PrintConfig includes all critical machine fields: nozzle_diameter, retract_length, bed_size, printable_area, start_gcode, end_gcode, max_acceleration_(x/y/z/e), max_speed_(x/y/z/e)
  3. PrintConfig includes all critical filament fields: retract_length (filament override), max_volumetric_speed, fan_max_speed, fan_min_speed, slow_down_layer_time, slow_down_min_speed, filament_type
  4. JSON profile mapper (OrcaSlicer/BambuStudio) maps at least 50 upstream fields to PrintConfig (up from ~24 for process, ~9 for machine, ~10 for filament)
  5. INI profile mapper (PrusaSlicer) maps the same expanded field set
  6. Re-converted BambuStudio X1C profiles (0.20mm Standard + Generic PLA + X1C 0.4mm) contain all settings needed for a representative slice comparison
  7. All existing tests pass with no regressions
**Plans:** 5/5 plans complete

Plans:
- [ ] 20-01-PLAN.md -- Add nested sub-config structs (LineWidthConfig, SpeedConfig, CoolingConfig, RetractionConfig, MachineConfig, AccelerationConfig, FilamentPropsConfig) and passthrough BTreeMap to PrintConfig
- [ ] 20-02-PLAN.md -- Expand JSON profile mapper from ~43 to 100+ mapped fields with passthrough storage
- [ ] 20-03-PLAN.md -- Expand PrusaSlicer INI profile mapper to match JSON mapper coverage
- [ ] 20-04-PLAN.md -- Migrate existing flat fields into sub-configs and update all engine call sites
- [ ] 20-05-PLAN.md -- Re-convert all ~21k profiles with expanded mappers and integration tests

### Phase 21: G-code Analysis and Comparison Tool

**Goal:** Build a G-code parser and analysis module that can ingest any G-code file (from BambuStudio, OrcaSlicer, PrusaSlicer, or our own output) and extract structured metrics for comparison: layer count, Z heights, feature annotations (;TYPE: comments), per-feature move counts/distances/extrusion amounts, speed distributions, retraction counts, and time/filament totals from headers. Expose via CLI `analyze-gcode` subcommand for standalone use and slicer output comparison.
**Depends on:** Phase 20
**Success Criteria** (what must be TRUE):
  1. G-code parser extracts layer boundaries (Z changes), move counts, travel/extrusion distances, and total filament per layer
  2. Feature type annotations (`;TYPE:` comments from BambuStudio/OrcaSlicer/PrusaSlicer) are parsed and per-feature metrics accumulated
  3. Retraction count/distance, z-hop count/distance, and speed distribution (min/max/mean per feature) extracted
  4. Header metadata (slicer name/version, estimated time, filament usage, layer count) parsed from comment blocks
  5. `slicecore analyze-gcode <file>` CLI subcommand outputs analysis as ASCII table, CSV, or JSON (reusing stats_display patterns)
  6. `slicecore compare-gcode <file1> <file2>` CLI subcommand shows side-by-side metrics comparison with delta columns
  7. Analysis of BambuStudio, OrcaSlicer, and PrusaSlicer G-code files produces correct metrics validated against header-reported values
**Plans:** 3/3 plans complete

Plans:
- [ ] 21-01-PLAN.md -- Core G-code parser, slicer detection, metric types, and header metadata extraction
- [ ] 21-02-PLAN.md -- N-file comparison logic, CLI subcommands (analyze-gcode, compare-gcode), and display formatting
- [ ] 21-03-PLAN.md -- Integration tests with synthetic and real G-code files, phase success criteria verification

### Phase 22: Migrate from lib3mf to lib3mf-core ecosystem

**Goal:** Replace lib3mf 0.1.3 with lib3mf-core 0.2.0 in slicecore-fileio, eliminating the C dependency (zstd-sys) that blocks WASM compilation, and enabling 3MF parsing on all targets including WASM
**Depends on:** Phase 21
**Requirements:** MESH-02, FOUND-01, FOUND-03
**Success Criteria** (what must be TRUE):
  1. lib3mf-core 0.2.0 replaces lib3mf 0.1.3 as the 3MF dependency, with no C/C++ libraries in the dependency tree
  2. 3MF parsing works identically (behavioral equivalence) -- same vertex and triangle counts from the same input files
  3. The WASM cfg gate is removed -- 3MF parsing is available on all targets including wasm32-unknown-unknown and wasm32-wasip2
  4. No references to old lib3mf crate remain in any source file
  5. WASM compilation of slicecore-fileio succeeds, proving the migration unlocked WASM 3MF support
**Plans:** 3 plans (2 complete + 1 gap closure)

Plans:
- [ ] 22-01-PLAN.md -- Swap lib3mf to lib3mf-core, rewrite threemf.rs parser + tests, remove WASM cfg gates from lib.rs
- [ ] 22-02-PLAN.md -- Verify WASM compilation, add integration test, CI validation, final phase verification

### Phase 23: Progress/Cancellation API

**Goal:** Add rich progress reporting and cooperative cancellation to the slicing engine by extending the existing EventBus with SliceEvent::Progress (percentage, ETA, elapsed time, throughput) and introducing a CancellationToken (Arc<AtomicBool> wrapper) passed as Option<CancellationToken> on all public slice methods -- enabling GUI, web service, and print farm applications to track slicing progress and cancel operations mid-flight
**Requirements**: API-05
**Depends on:** Phase 22
**Success Criteria** (what must be TRUE):
  1. CancellationToken type exists with new(), cancel(), is_cancelled() using Arc<AtomicBool>, re-exported at crate root
  2. All 5 public Engine slice methods accept Option<CancellationToken> as final parameter -- existing callers pass None for backward compatibility
  3. Engine checks cancellation token once per layer and returns Err(EngineError::Cancelled) -- clean error, no partial results
  4. SliceEvent::Progress emitted after each layer with overall_percent (10-90%), stage_percent (0-100%), ETA via rolling average over last 20 layers, elapsed_seconds, and layers_per_second
  5. WASM compilation works: timing gracefully disabled (elapsed_seconds=0.0, eta_seconds=None on wasm32)
  6. Integration tests verify cancellation (pre-flight and mid-flight), progress event accuracy, and ETA estimation
**Plans:** 3 plans (2 complete + 1 gap closure)

Plans:
- [ ] 23-01-PLAN.md -- Core types (CancellationToken, EngineError::Cancelled, SliceEvent::Progress) and method signature changes with call site updates
- [ ] 23-02-PLAN.md -- Progress emission logic, cancellation checking, WASM-safe timing, integration tests

### Phase 24: Mesh Export (STL/3MF Write)

**Goal:** Add bidirectional mesh I/O to slicecore-fileio by upgrading lib3mf-core to 0.4, adding lib3mf-converters 0.4 for STL/OBJ export, implementing save_mesh/save_mesh_to_writer with ExportFormat enum, and adding a CLI convert subcommand for mesh format conversion
**Depends on:** Phase 23
**Success Criteria** (what must be TRUE):
  1. save_mesh writes valid 3MF, Binary STL, and OBJ files that round-trip back through load_mesh
  2. save_mesh_to_writer works with any Write+Seek destination (File, Cursor)
  3. ExportFormat is auto-detected from file extension (.stl, .3mf, .obj)
  4. lib3mf-core upgraded from 0.3 to 0.4 with no regressions in existing 3MF import
  5. CLI `slicecore convert input.ext output.ext` converts between mesh formats
**Plans:** 3 plans (2 complete + 1 gap closure)

Plans:
- [ ] 24-01-PLAN.md -- lib3mf-core 0.4 upgrade, lib3mf-converters dep, export module (save_mesh, ExportFormat, round-trip tests)
- [ ] 24-02-PLAN.md -- CLI convert subcommand, integration tests, phase verification

### Phase 25: Parallel Slicing Pipeline (rayon)

**Goal:** Add rayon-based parallelism to the per-layer processing pipeline, enabling multi-core speedup for perimeter generation, surface classification, infill, and toolpath assembly while maintaining bit-identical output via two-pass seam alignment and lightning infill sequential fallback
**Requirements**: FOUND-06
**Depends on:** Phase 24
**Success Criteria** (what must be TRUE):
  1. Per-layer processing runs in parallel via rayon par_iter when parallel_slicing config is true and the parallel Cargo feature is enabled
  2. Parallel G-code output is byte-for-byte identical to sequential output for the same input mesh and config
  3. Lightning infill automatically falls back to sequential processing (cross-layer tree state dependency)
  4. WASM targets compile with parallel feature disabled, running single-threaded
  5. Criterion benchmarks show measurable wall-time speedup for parallel vs sequential on a 200-layer test mesh
**Plans:** 4/4 plans complete

Plans:
- [ ] 25-01-PLAN.md -- Rayon dependency, parallel feature flag, PrintConfig fields, maybe_par_iter! macro
- [ ] 25-02-PLAN.md -- Convert layer processing loops to parallel with two-pass seam, lightning fallback, determinism test
- [ ] 25-03-PLAN.md -- Criterion benchmarks comparing parallel vs sequential performance
- [ ] 25-04-PLAN.md -- Gap closure: Fix CI WASM build to exclude parallel feature (rayon)

### Phase 26: Thumbnail/Preview Rasterization

**Goal:** Rasterize 3D model meshes into PNG thumbnail images using a custom CPU-based software renderer with Gouraud shading, supporting 6 camera angles, configurable resolutions, and three output targets (3MF embedding, G-code header comments, standalone PNG files) -- all pure Rust, WASM-compatible, no GPU dependencies
**Requirements**: RENDER-01, RENDER-02, RENDER-03, RENDER-04, RENDER-05, RENDER-06, RENDER-07, RENDER-08, RENDER-09
**Depends on:** Phase 25
**Success Criteria** (what must be TRUE):
  1. A TriangleMesh can be rendered to an RGBA pixel buffer from any of 6 camera angles (front, back, left, right, top, isometric) with z-buffered triangle rasterization
  2. Gouraud shading with vertex normal interpolation produces smooth brightness variation across curved surfaces
  3. PNG encoding produces valid PNG files from RGBA buffers
  4. 3MF export can include a thumbnail at Metadata/thumbnail.png via lib3mf-core Model.attachments
  5. G-code output can include base64-encoded PNG thumbnails in header comments (PrusaSlicer/Creality formats)
  6. CLI `slicecore thumbnail` subcommand generates standalone PNG files; `slice --thumbnails` embeds thumbnails in output
  7. The render crate compiles for wasm32-unknown-unknown
**Plans:** 3/3 plans complete

Plans:
- [ ] 26-01-PLAN.md -- Core slicecore-render crate (framebuffer, rasterizer, camera, shading, PNG encoding, public API)
- [ ] 26-02-PLAN.md -- Integration (3MF thumbnail embedding, G-code comments, PrintConfig field, CLI subcommand)
- [ ] 26-03-PLAN.md -- Integration tests verifying all 9 RENDER requirements

### Phase 27: Build Plate Auto-Arrangement

**Goal:** Users can automatically position multiple parts on the build plate with optimal packing, auto-orientation for minimal support, material-aware grouping, multi-plate splitting, and sequential print collision avoidance
**Requirements**: ADV-02
**Depends on:** Phase 26
**Plans:** 5/5 plans complete

Plans:
- [ ] 27-01-PLAN.md -- slicecore-arrange crate scaffold, types, bed parsing, footprint computation
- [ ] 27-02-PLAN.md -- Expand PrintConfig with gantry clearance and multi-head detection fields
- [ ] 27-03-PLAN.md -- Auto-orient, bottom-left fill placer, multi-plate grouping, sequential support
- [ ] 27-04-PLAN.md -- CLI arrange subcommand, --auto-arrange on slice, engine integration
- [ ] 27-05-PLAN.md -- Integration tests for all arrangement features
### Phase 28: G-code Post-Processing Plugin Point

**Goal:** Extend the plugin system with G-code post-processing capabilities -- FFI-safe post-processor trait, bidirectional GcodeCommand conversion, 4 built-in post-processors (pause at layer, timelapse camera, fan speed override, custom G-code injection), engine pipeline integration with progress/cancellation, and standalone CLI post-process subcommand for re-processing G-code without re-slicing
**Requirements**: ADV-04, PLUGIN-01, PLUGIN-02
**Depends on:** Phase 27
**Success Criteria** (what must be TRUE):
  1. GcodePostProcessorPlugin sabi_trait defined with process_all and process_layer modes, FfiGcodeCommand mirrors all GcodeCommand variants
  2. PluginRegistry manages post-processor plugins alongside infill plugins with register/get/discover
  3. Four built-in post-processors (pause-at-layer, timelapse-camera, fan-speed-override, custom-gcode-injection) self-skip when unconfigured
  4. Post-processing runs after arc fitting and purge tower, before time estimation -- time/filament stats always reflect post-processed output
  5. Standalone `slicecore post-process` CLI subcommand reads existing G-code, applies plugins, writes output
  6. Integration tests verify all 4 built-ins in full pipeline, backward compatibility (disabled by default), and time estimation accuracy
**Plans:** 3/3 plans complete

Plans:
- [ ] 28-01-PLAN.md -- FFI-safe types/trait in plugin-api, conversion/adapter/registry in plugin crate
- [ ] 28-02-PLAN.md -- PostProcessConfig, 4 built-in post-processors, engine pipeline integration
- [ ] 28-03-PLAN.md -- CLI post-process subcommand, integration tests

### Phase 29: Mesh Boolean Operations (CSG)

**Goal:** True 3D mesh boolean operations (union, difference, intersection, XOR) plus 9 mesh primitives, plane splitting, hollowing, mesh offset, CLI subcommand, plugin API, benchmarks, and fuzz targets -- enabling multi-part assembly merging, modifier mesh cutting, and model splitting
**Requirements**: CSG-01 through CSG-13
**Depends on:** Phase 28
**Plans:** 7/7 plans complete

Plans:
- [ ] 29-01-PLAN.md -- CSG module foundation: types, error, report, per-triangle attributes, 9 mesh primitives
- [ ] 29-02-PLAN.md -- Core CSG algorithm: intersection curves, retriangulation, classification, symbolic perturbation
- [ ] 29-03-PLAN.md -- Public boolean API: mesh_union, mesh_difference, mesh_intersection, mesh_xor, mesh_union_many
- [ ] 29-04-PLAN.md -- Plane split, mesh offset, and hollow mesh operations
- [ ] 29-05-PLAN.md -- CancellationToken support, rayon parallelism, plugin API traits
- [ ] 29-06-PLAN.md -- CLI csg subcommand with info command and integration tests
- [ ] 29-07-PLAN.md -- Criterion benchmarks, fuzz target, full workspace verification

### Phase 30: CLI profile composition and slice workflow

**Goal:** CLI users can compose multiple profile layers (machine + filament + process) into a final PrintConfig with provenance tracking, use the enhanced slice command with real-world multi-profile workflow, and get progress feedback, log files, and embedded config in G-code output
**Requirements**: N/A-01, N/A-02, N/A-03, N/A-04, N/A-05, N/A-06, N/A-07, N/A-08, N/A-09, N/A-10, N/A-11, N/A-12
**Depends on:** Phase 29
**Plans:** 6/6 plans complete

Plans:
- [ ] 30-01-PLAN.md -- ProfileComposer core: TOML value tree merge with provenance tracking
- [ ] 30-02-PLAN.md -- ProfileResolver: name-to-path resolution with type-constrained search
- [ ] 30-03-PLAN.md -- Built-in profiles and config validation with severity levels
- [ ] 30-04-PLAN.md -- CLI flags and slice workflow orchestrator (resolve->compose->validate->slice)
- [ ] 30-05-PLAN.md -- Progress bar module and existing profile command migration to ProfileResolver
- [ ] 30-06-PLAN.md -- E2E integration tests for profile composition slice workflow
### Phase 31: CLI utility commands calibrate and estimate

**Goal:** CLI users can generate printer-specific calibration G-code (temperature tower, retraction test, flow rate, first layer) and get cost estimation breakdowns from G-code analysis, with multi-config comparison and volume-based rough estimation for model files
**Requirements**: TBD
**Depends on:** Phase 30
**Plans:** 6/6 plans complete

Plans:
- [ ] 31-01-PLAN.md -- Infrastructure: calibrate CLI skeleton, cost model, config additions
- [ ] 31-02-PLAN.md -- Extend analyze-gcode with cost estimation and volume-based rough estimation
- [ ] 31-03-PLAN.md -- Temperature tower and retraction test calibration commands
- [ ] 31-04-PLAN.md -- Flow rate and first layer calibration commands
- [ ] 31-05-PLAN.md -- Multi-config comparison, dry-run, save-model, output formats
- [ ] 31-06-PLAN.md -- E2E integration tests for all calibrate and estimate features

### Phase 32: P0 config gap closure - critical missing fields

**Goal:** Add ~16 critical config fields (dimensional compensation, surface patterns, bed types, chamber temperature, z offset, etc.) with full profile import mapping, template variables, validation, and G-code integration -- config-only, no engine behavior changes
**Requirements**: P32-01, P32-02, P32-03, P32-04, P32-05, P32-06, P32-07, P32-08, P32-09, P32-10
**Depends on:** Phase 31
**Plans:** 4/4 plans complete

Plans:
- [ ] 32-01-PLAN.md -- New enums, sub-structs, and all P0 fields in config.rs
- [ ] 32-02-PLAN.md -- OrcaSlicer JSON and PrusaSlicer INI profile import mappings
- [ ] 32-03-PLAN.md -- Template variables, validation rules, G-code comments and M-codes
- [ ] 32-04-PLAN.md -- Tests and profile re-conversion

### Phase 33: P1 config gap closure - profile fidelity fields

**Goal:** Add ~30 P1-priority config fields (FuzzySkinConfig, BrimSkirtConfig, InputShapingConfig, ToolChangeRetractionConfig sub-structs + extensions to AccelerationConfig, CoolingConfig, SpeedConfig, FilamentPropsConfig, MultiMaterialConfig) with OrcaSlicer JSON and PrusaSlicer INI import mappings, G-code template variables, and range validation -- config + mapping only, no engine behavior changes
**Requirements**: P33-01 through P33-16
**Depends on:** Phase 32
**Plans:** 4/4 plans complete

Plans:
- [ ] 33-01-PLAN.md -- New sub-structs, BrimType enum, extend existing sub-structs, top-level fields
- [ ] 33-02-PLAN.md -- OrcaSlicer JSON and PrusaSlicer INI field mappings
- [ ] 33-03-PLAN.md -- G-code template variables and range validation
- [ ] 33-04-PLAN.md -- Integration tests and profile re-conversion

### Phase 34: Support config and advanced feature profile import mapping

**Goal:** Map ALL remaining unmapped config sections from upstream profiles (OrcaSlicer/BambuStudio/PrusaSlicer) to achieve 100% typed field coverage. Covers SupportConfig, ScarfJointConfig, MultiMaterialConfig, CustomGcodeHooks, PostProcessConfig, ~20 P2 niche fields, G-code template variable translation, and coverage reporting.
**Requirements**: SUPPORT-MAP, SCARF-MAP, MULTI-MAP, GCODE-MAP, POST-MAP, P2-FIELDS, GCODE-TRANSLATE, PASSTHROUGH-THRESHOLD, ROUND-TRIP, RECONVERT
**Depends on:** Phase 33
**Success Criteria** (what must be TRUE):
  1. All 5 previously-0% sub-structs (SupportConfig, ScarfJoint, MultiMaterial, CustomGcode, PostProcess) have upstream field mappings in both JSON and INI importers
  2. All ~20 P2 niche fields have typed config representation with upstream mappings
  3. G-code template variable translation table exists and is wired into import pipeline
  4. Passthrough ratio is below 5% on representative profiles
  5. CONFIG_PARITY_AUDIT.md Section 4 reflects final coverage numbers
**Plans:** 6/6 plans complete

Plans:
- [ ] 34-01-PLAN.md -- Comprehensive field inventory from real profile scanning
- [ ] 34-02-PLAN.md -- SupportConfig + BridgeConfig + TreeSupportConfig field mapping (JSON + INI)
- [ ] 34-03-PLAN.md -- ScarfJoint + MultiMaterial + CustomGcode field mapping (JSON + INI)
- [ ] 34-04-PLAN.md -- PostProcess + P2 niche fields + straggler fields
- [ ] 34-05-PLAN.md -- G-code template variable translation table and dual storage
- [ ] 34-06-PLAN.md -- Integration tests, re-conversion, coverage report, audit update

### Phase 35: ConfigSchema system with setting metadata and JSON Schema generation

**Goal:** Build a per-field metadata system for all config settings using a proc-macro derive, populate a runtime SettingRegistry, and generate JSON Schema 2020-12 + flat metadata JSON output. Replace ad-hoc validation with schema-driven validation. Deliver a CLI schema command for querying and exporting. Annotate ALL ~387 fields with tier, description, units, constraints, affects, and category.
**Requirements**: TBD
**Depends on:** Phase 34
**Plans:** 7/7 plans complete

Plans:
- [ ] 35-01-PLAN.md -- Runtime types crate (SettingDefinition, SettingKey, ValueType, Tier, SettingCategory, SettingRegistry)
- [ ] 35-02-PLAN.md -- Proc-macro derive crate (#[derive(SettingSchema)] for structs and enums)
- [ ] 35-03-PLAN.md -- TIER_MAP.md design artifact with user review gate
- [ ] 35-04-PLAN.md -- Annotate all config structs and enums in config.rs
- [ ] 35-05-PLAN.md -- Annotate support/config.rs and cross-module enums
- [ ] 35-06-PLAN.md -- JSON Schema generation, flat metadata JSON, search API, global registry
- [ ] 35-07-PLAN.md -- CLI schema subcommand, schema-driven validation, integrity tests

### Phase 36: Add a plugins subcommand to allow users to list and manage installed plugins, such as enable or disable

**Goal:** Working `slicecore plugins` CLI subcommand with list, enable, disable, info, and validate commands; per-plugin .status files for state management; status-aware plugin discovery pipeline
**Requirements**: [PLG-STATUS, PLG-DISCOVERY, PLG-REGISTRY, PLG-CLI-LIST, PLG-CLI-ENABLE, PLG-CLI-DISABLE, PLG-CLI-INFO, PLG-CLI-VALIDATE, PLG-GLOBAL-PLUGINDIR, PLG-QA-TESTS, PLG-DISABLED-SLICE-ERROR]
**Depends on:** Phase 35
**Plans:** 3/3 plans complete

- [ ] 36-01-PLAN.md -- Plugin status module and status-aware discovery pipeline
- [ ] 36-02-PLAN.md -- CLI plugins subcommand and global --plugin-dir promotion
- [ ] 36-03-PLAN.md -- QA tests for plugins subcommand

### Phase 37: CI benchmark tracking with regression detection — integrate criterion benchmarks into CI pipeline with historical tracking, threshold-based regression alerts, and dashboard reporting

**Goal:** Integrate existing criterion benchmarks into CI with two-tier regression detection (5% warn / 15% block), per-PR comparison comments via criterion-compare-action, historical tracking on gh-pages dashboard via github-action-benchmark, peak memory tracking via /usr/bin/time, bench-ok label override, and developer documentation
**Requirements**: BENCH-CI, BENCH-COMPARE, BENCH-HISTORY, BENCH-MEMORY, BENCH-SKIP, BENCH-DOCS
**Depends on:** Phase 36
**Plans:** 3 plans (2 complete + 1 gap closure)

Plans:
- [ ] 37-01-PLAN.md -- CI workflow (changes filter + bench job) and memory tracking script
- [ ] 37-02-PLAN.md -- Benchmark documentation in CONTRIBUTING.md


### Phase 38: Profile diff command to compare presets side by side — implement slicecore profile diff CLI subcommand with settings comparison, category grouping, impact hints, and multiple output formats

**Goal:** Implement diff-profiles CLI subcommand comparing two PrintConfig instances with category-grouped table/JSON output, SettingRegistry metadata enrichment, and filtering by category/tier
**Requirements**: TBD
**Depends on:** Phase 37
**Plans:** 3 plans (2 complete + 1 gap closure)

Plans:
- [ ] 38-01-PLAN.md -- Core diff engine module (profile_diff.rs) with types, flatten, comparison, and registry enrichment
- [ ] 38-02-PLAN.md -- CLI diff-profiles subcommand with table/JSON display, all flags, and Commands wiring

### Phase 39: JPEG thumbnail export — add JPEG encoding option to render crate alongside existing PNG, with CLI flag, quality control, and 3MF/G-code thumbnail embedding support

**Goal:** Add JPEG encoding alongside PNG in the render crate, with CLI --format/--quality flags, auto-detection from file extension, and proper 3MF/G-code embedding behavior
**Requirements**: JPEG-01, JPEG-02, JPEG-03, JPEG-04, JPEG-05, JPEG-06
**Depends on:** Phase 38
**Plans:** 3 plans (2 complete + 1 gap closure)

Plans:
- [ ] 39-01-PLAN.md -- Render crate image crate migration, ImageFormat enum, JPEG encoding, field renames across workspace
- [ ] 39-02-PLAN.md -- CLI --format/--quality flags on thumbnail and slice commands with integration tests

### Phase 40: Adopt indicatif for consistent CLI progress display — replace ad-hoc println with progress bars, spinners, and step indicators across all CLI commands with quiet/json flag support

**Goal:** Replace ad-hoc println/eprintln progress output across all CLI commands with a unified CliOutput abstraction built on indicatif, add global --quiet and --color flags, step indicators for slice workflow
**Requirements**: CLI-PROGRESS-01, CLI-PROGRESS-02, CLI-PROGRESS-03
**Depends on:** Phase 39
**Plans:** 3/3 plans complete

Plans:
- [ ] 40-01-PLAN.md — CliOutput abstraction and global CLI flags
- [ ] 40-02-PLAN.md — Slice command migration to step-based workflow
- [ ] 40-03-PLAN.md — Spinners and --json for all other commands

### Phase 41: Travel move optimization with TSP algorithms — implement 2-opt and greedy edge insertion for toolpath ordering to reduce travel distance by 20-35% on multi-object plates with benchmark validation

**Goal:** Optimize toolpath ordering within layers using TSP heuristics (NN, greedy edge insertion, 2-opt) to reduce non-extrusion travel distance by 20-35% on multi-object plates, with criterion benchmarks and CI-enforcing integration tests.
**Requirements**: [GCODE-05]
**Depends on:** Phase 40
**Plans:** 4/4 plans complete

Plans:
- [ ] 41-01-PLAN.md — Core TSP algorithms and config types
- [ ] 41-02-PLAN.md — Engine integration (toolpath optimizer wiring + travel stats)
- [ ] 41-03-PLAN.md — CLI --no-travel-opt flag
- [ ] 41-04-PLAN.md — Benchmarks and integration tests (>= 20% reduction assertions)

### Phase 42: Clone and customize profiles from defaults — add profile clone command for creating custom profiles from existing presets with edit and validate workflow

**Goal:** Enable users to create custom profiles by cloning existing presets via `slicecore profile clone <source> <new-name>`, with subsequent editing via `slicecore profile set` and schema-based validation.
**Requirements**: [API-02]
**Depends on:** Phase 41
**Plans:** 3 plans (2 complete + 1 gap closure)

Plans:
- [ ] 42-01-PLAN.md — ProfileCommand enum, clone command, name validation, main.rs wiring
- [ ] 42-02-PLAN.md — Set/get/reset/edit/validate/delete/rename commands and alias wiring

### Phase 43: Enable/disable printer and filament profiles to narrow search scope — add profile activation system with first-run wizard and per-printer filament visibility

**Goal:** Implement an enable/disable system for printer and filament profiles using `~/.config/slicecore/enabled-profiles.toml`, with CLI commands (enable/disable/list/setup), interactive first-run wizard, and per-printer filament visibility filtering.
**Requirements**: [API-02]
**Depends on:** Phase 42
**Plans:** 3/3 plans complete

Plans:
- [ ] 43-01-PLAN.md -- EnabledProfiles data model, compatibility types, and ProfileResolver filtering
- [ ] 43-02-PLAN.md -- Enable, disable, status CLI commands and activation-aware list filtering
- [ ] 43-03-PLAN.md -- Interactive setup wizard, setup command, and slice trigger

### Phase 44: Search and filter profiles by printer and filament compatibility — add profile search with compatibility engine and enhanced list command

**Goal:** Add `slicecore profile search <query>` with filters (printer, material, nozzle, manufacturer), a compatibility engine (nozzle match, temp ranges, hardware requirements), enhanced `list` command with filtering, and profile sets for favorites.
**Requirements**: [API-02]
**Depends on:** Phase 43
**Plans:** 3/3 plans complete

Plans:
- [ ] 44-01-PLAN.md — Compatibility engine extensions + filter infrastructure + profile sets data model
- [ ] 44-02-PLAN.md — Search/list/compat CLI commands with filter flags and compatibility display
- [ ] 44-03-PLAN.md — Profile sets CLI commands + slice --set flag + pre-slice compatibility warnings

### Phase 45: Global and per-object settings override system — implement layered settings resolution with per-object and per-region overrides

**Goal:** Implement a layered settings override system (global → per-object → per-region) with proper cascading, validation, and serialization, enabling users to customize specific objects on multi-object plates with different infill, layer height, or other parameters.
**Requirements**: [ADV-03]
**Depends on:** Phase 44
**Plans:** 11/11 plans complete

Plans:
- [ ] 45-01-PLAN.md — Core data model: PlateConfig, ObjectConfig, FieldSource extension, OverrideSafety enum
- [ ] 45-02-PLAN.md — Override safety derive macro + OVERRIDE_SAFETY_MAP.md + user review
- [ ] 45-03-PLAN.md — 10-layer cascade resolution + Z-schedule computation + proptest
- [ ] 45-04-PLAN.md — Modifier mesh migration: replace SettingOverrides with TOML partial merge
- [ ] 45-05-PLAN.md — Engine integration: Engine::new(PlateConfig), per-object slicing, backward compat
- [ ] 45-06-PLAN.md — Override set CRUD CLI + plate init/from-3mf/to-3mf commands
- [ ] 45-07-PLAN.md — CLI integration: --plate, --object flags, multi-model, validation
- [ ] 45-08-PLAN.md — 3MF import/export of per-object settings and modifier meshes
- [ ] 45-09-PLAN.md — Serialization: G-code header, per-object stats, provenance, checksum
- [ ] 45-10-PLAN.md — E2E tests, test fixtures, criterion benchmarks, regression tests
- [ ] 45-11-PLAN.md — Gap closure: wire plate from-3mf/to-3mf to per-object-aware fileio functions

### Phase 46: Job output directories for isolated slice execution — add --job-dir flag with structured output directory containing G-code, logs, config snapshot, thumbnail, and manifest

**Goal:** Implement job directory concept with structured output (G-code, logs, config snapshot, thumbnail, manifest) via `--job-dir` flag, enabling isolated artifact management for parallel slicing, batch workflows, and future daemon/farm/SaaS features.
**Requirements**: [API-02]
**Depends on:** Phase 45
**Plans:** 3/3 plans complete

Plans:
- [ ] 46-01-PLAN.md -- JobDir module with struct, manifest, locking, artifact paths, and unit tests
- [ ] 46-02-PLAN.md -- CLI integration (--job-dir, --job-base flags) with cmd_slice wiring and integration tests
- [ ] 46-03-PLAN.md -- Gap closure: populate PrintStats in job-dir manifest




### Phase 47: Variable layer height algorithms — implement multi-objective VLH optimization with curvature, feature-aware heights, and Laplacian smoothing

**Goal:** Multi-objective VLH optimization with four objectives (quality, speed, strength, material), feature-aware height selection (overhangs, bridges, thin walls, holes), Laplacian smoothing for transition continuity, greedy and DP optimizers, and per-layer diagnostic events.
**Requirements**: [SLICE-05]
**Depends on:** Phase 46
**Plans:** 3/4 plans executed

Plans:
- [ ] 47-01-PLAN.md -- VLH module types, PrintConfig fields, and objective scoring functions
- [ ] 47-02-PLAN.md -- Feature map pre-pass and Laplacian smoothing
- [ ] 47-03-PLAN.md -- Greedy and DP optimizers
- [ ] 47-04-PLAN.md -- Public API integration, adaptive.rs wrapper refactor, diagnostics

### Phase 48: Selective adaptive z-hop control for top surfaces — implement surface-type-based z-hop with distance gating and height-proportional lift

**Goal:** Replace global z-hop with intelligent surface-type-based z-hop that activates only on top solids and ironing surfaces, with layer-position-based rules, height-proportional lift, and distance-gated activation to eliminate unnecessary stringing on interior layers.
**Requirements**: [GCODE-03]
**Depends on:** Phase 47
**Plans:** 0 plans

Plans:

### Phase 49: Hybrid sequential printing — first N layers all objects together, then switch to by-object sequential printing

**Goal:** Implement hybrid print mode where Phase 1 prints first N layers of all objects together for adhesion verification, then Phase 2 switches to sequential by-object printing for quality, with early failure detection and conditional object skipping.
**Requirements**: [ADV-02]
**Depends on:** Phase 48
**Plans:** 0 plans

Plans:

### Phase 50: 3MF project output with model settings and embedded G-code — implement full 3MF project write support with settings metadata, thumbnails, and Bambu printer compatibility

**Goal:** Enable saving complete slice sessions in 3MF project format containing model geometry, print settings metadata, thumbnail images, and embedded G-code, with Bambu/OrcaSlicer compatibility for direct-to-printer workflows.
**Requirements**: [MESH-03]
**Depends on:** Phase 49
**Plans:** 0 plans

Plans:

### Phase 51: Comprehensive documentation suite for users and developers — build mdBook-based docs with install guide, user guide, API reference, and developer guide

**Goal:** Create a complete mdBook-based documentation suite covering installation, user guide (CLI usage, profiles, slicing workflows), configuration reference, API reference (rustdoc integration), developer guide (architecture, contributing), and examples gallery.
**Requirements**: [API-01]
**Depends on:** Phase 50
**Plans:** 0 plans

Plans:
