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
**Plans:** 2/2 plans complete

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
**Plans:** 5 plans

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
**Plans:** 0 plans

Plans:
- [ ] TBD (run /gsd:plan-phase 21 to break down)
