# Requirements: libslic3r-rs

**Defined:** 2026-02-14
**Core Value:** The plugin architecture and AI integration must work from day one -- modularity and intelligence are not bolt-ons.

## v1 Requirements

Requirements for initial proof-of-concept release. Each maps to roadmap phases.

### Foundation (FOUND)

- [ ] **FOUND-01**: Pure Rust implementation with no FFI to C/C++/Python/Go
- [ ] **FOUND-02**: Multi-platform support: macOS (ARM/x86), Linux (ARM/x86), Windows (ARM/x86)
- [ ] **FOUND-03**: WASM compilation target (wasm32-wasi and wasm32-unknown-unknown)
- [ ] **FOUND-04**: Coordinate precision strategy locked (f64 vs i64 vs hybrid)
- [ ] **FOUND-05**: Polygon boolean operations work (i-overlay or clipper2-rust)
- [ ] **FOUND-06**: Performance matches or beats C++ libslic3r (>=1.0x, targeting >=1.5x)
- [ ] **FOUND-07**: Memory usage <=80% of C++ libslic3r
- [ ] **FOUND-08**: Test coverage >80% on core algorithms

### Mesh I/O (MESH)

- [ ] **MESH-01**: Import STL files (binary and ASCII)
- [ ] **MESH-02**: Import 3MF files via lib3mf-core
- [ ] **MESH-03**: Import OBJ files
- [ ] **MESH-04**: Export G-code in multiple dialects (Marlin, Klipper, RepRapFirmware, Bambu)
- [ ] **MESH-05**: Auto-repair non-manifold geometry
- [ ] **MESH-06**: Auto-repair self-intersecting meshes
- [ ] **MESH-07**: Auto-repair degenerate triangles
- [ ] **MESH-08**: Mesh transformations: scale, rotate, translate, mirror
- [ ] **MESH-09**: Validate input (ValidPolygon type prevents degeneracies)

### Core Slicing (SLICE)

- [ ] **SLICE-01**: Layer slicing at configurable heights
- [ ] **SLICE-02**: Adaptive layer heights based on surface curvature
- [ ] **SLICE-03**: Contour extraction from mesh cross-sections
- [ ] **SLICE-04**: Generate slicing preview data (layer-by-layer visualization)
- [ ] **SLICE-05**: Deterministic output (same input + config = identical G-code)

### Perimeters (PERIM)

- [ ] **PERIM-01**: Generate perimeters with configurable wall count
- [ ] **PERIM-02**: Arachne variable-width perimeters for thin walls
- [ ] **PERIM-03**: Wall ordering control (inner-first or outer-first)
- [ ] **PERIM-04**: Gap fill between perimeters
- [ ] **PERIM-05**: Seam placement strategies (aligned, random, rear, smart hiding)
- [ ] **PERIM-06**: Scarf joint seam with 12 configurable parameters

### Infill (INFILL)

- [ ] **INFILL-01**: Rectilinear infill pattern
- [ ] **INFILL-02**: Grid infill pattern
- [ ] **INFILL-03**: Honeycomb infill pattern
- [ ] **INFILL-04**: Gyroid infill pattern
- [ ] **INFILL-05**: Adaptive cubic infill pattern
- [ ] **INFILL-06**: Cubic infill pattern
- [ ] **INFILL-07**: Lightning infill pattern
- [ ] **INFILL-08**: Monotonic infill pattern
- [ ] **INFILL-09**: OrcaSlicer TPMS-D pattern
- [ ] **INFILL-10**: OrcaSlicer TPMS-FK pattern
- [ ] **INFILL-11**: Configurable infill density (0-100%)
- [ ] **INFILL-12**: Top/bottom solid layers

### Supports (SUPP)

- [ ] **SUPP-01**: Automatic support generation based on overhang angle
- [ ] **SUPP-02**: Manual support painting (enforcers/blockers)
- [ ] **SUPP-03**: Traditional grid/line support structures
- [ ] **SUPP-04**: Tree supports (Cura-based algorithm)
- [ ] **SUPP-05**: Organic tree supports (mesh-based)
- [ ] **SUPP-06**: Bridge detection and handling
- [ ] **SUPP-07**: Overhang detection with 4-tier angle control (0-25%, 25-50%, 50-75%, 75%+)
- [ ] **SUPP-08**: Support interface layers for better surface finish

### G-code Generation (GCODE)

- [ ] **GCODE-01**: Marlin dialect G-code output
- [ ] **GCODE-02**: Klipper dialect G-code output
- [ ] **GCODE-03**: RepRapFirmware dialect G-code output
- [ ] **GCODE-04**: Bambu dialect G-code output
- [ ] **GCODE-05**: Speed planning (per-feature speed control)
- [ ] **GCODE-06**: Acceleration and jerk control
- [ ] **GCODE-07**: Retraction configuration (distance, speed, z-hop, wipe)
- [ ] **GCODE-08**: Temperature planning (layer-based, first-layer overrides)
- [ ] **GCODE-09**: Cooling/fan control (layer time-based, bridge-specific)
- [ ] **GCODE-10**: Skirt/brim/raft generation for bed adhesion
- [ ] **GCODE-11**: Arc fitting (convert line segments to G2/G3 arcs)
- [x] **GCODE-12**: Print time estimation
- [x] **GCODE-13**: Filament usage estimation (weight, length, cost)

### Plugin System (PLUGIN)

- [ ] **PLUGIN-01**: Plugin trait API defined (extension points for infill, supports, etc.)
- [ ] **PLUGIN-02**: PluginRegistry for discovery and registration
- [ ] **PLUGIN-03**: Native plugin loading via abi_stable
- [ ] **PLUGIN-04**: WASM plugin loading via Component Model
- [ ] **PLUGIN-05**: Example custom infill pattern plugin (loadable without modifying core)
- [ ] **PLUGIN-06**: Plugin sandboxing (plugins can't crash core)
- [ ] **PLUGIN-07**: Plugin API documentation with examples

### AI Integration (AI)

- [ ] **AI-01**: Provider-agnostic LLM abstraction layer
- [ ] **AI-02**: Geometry analysis API (extract features from mesh)
- [ ] **AI-03**: Profile suggestion endpoint (send geometry -> receive settings)
- [ ] **AI-04**: Local LLM support (ollama, llama.cpp)
- [ ] **AI-05**: Cloud LLM support (OpenAI, Anthropic, custom endpoints)
- [ ] **AI-06**: Example: AI-driven profile suggestion for uploaded model

### API & CLI (API)

- [ ] **API-01**: Well-documented Rust public API (all public items have rustdoc)
- [ ] **API-02**: Full-featured CLI interface (slice, validate, analyze commands)
- [ ] **API-03**: JSON structured output (settings, slicing results, metadata)
- [ ] **API-04**: MessagePack structured output option
- [ ] **API-05**: Event system for progress, warnings, errors (pub/sub)
- [ ] ~~**API-06**: C FFI layer for cross-language use (C-compatible API)~~ **EXCLUDED** -- conflicts with PROJECT.md Out of Scope (no FFI bindings)
- [ ] ~~**API-07**: Python bindings via PyO3~~ **EXCLUDED** -- conflicts with PROJECT.md Out of Scope (no FFI bindings)

### Quality & Testing (TEST)

- [ ] **TEST-01**: Unit tests for all core algorithms
- [ ] **TEST-02**: Integration tests (STL -> G-code validation)
- [ ] **TEST-03**: Golden file tests (compare output to PrusaSlicer/OrcaSlicer)
- [ ] **TEST-04**: Fuzz testing on mesh parsers
- [ ] **TEST-05**: Benchmark suite (performance regression detection)
- [ ] **TEST-06**: WASM CI gate (ensure no incompatible dependencies)
- [ ] **TEST-07**: Line coverage >80% measured by cargo-tarpaulin

### Advanced Features (ADV)

- [ ] **ADV-01**: Multi-material support (MMU tool changes, purge tower)
- [ ] **ADV-02**: Sequential printing (object-by-object with collision detection)
- [ ] **ADV-03**: Modifier meshes (region-specific setting overrides)
- [ ] **ADV-04**: Custom G-code injection (per-layer, per-feature hooks)
- [ ] **ADV-05**: Per-feature flow control (10+ feature types)
- [ ] **ADV-06**: Pressure advance calibration pattern generation
- [ ] **ADV-07**: Hole-to-polyhole conversion for dimensional accuracy
- [ ] **ADV-08**: Ironing for top surface quality

## v2 Requirements

Deferred to future milestones. Not in v1 roadmap.

### Intelligence (v2)

- **AI-INT-01**: Print failure prediction (warping, stringing, adhesion issues)
- **AI-INT-02**: Automatic part orientation for strength/quality/speed
- **AI-INT-03**: Optimal nesting/packing of multiple parts
- **AI-INT-04**: Topology-aware infill (stress analysis integration)
- **AI-INT-05**: Feedback loop (correlate print results with settings)

### Advanced G-code (v2)

- **ADV-GC-01**: REST/gRPC API for cloud operation
- **ADV-GC-02**: Real-time slicing server (streaming mode)
- **ADV-GC-03**: Distributed slicing (multi-node parallelization)
- **ADV-GC-04**: Firmware simulation for time estimation accuracy

### Progressive Disclosure UI (v2)

- **UI-01**: Settings tier system (5 tiers: AI Auto, Simple, Intermediate, Advanced, Developer)
- **UI-02**: Settings metadata (affects/affected_by graph)
- **UI-03**: Interactive setting exploration (what-if scenarios)
- **UI-04**: Profile hierarchy (printer -> filament -> print quality)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Native GUI application | Separate project consuming library API; not part of core |
| Printer communication (OctoPrint/Moonraker) | Integration layer lives outside core slicing |
| Resin/SLA/DLP slicing | FDM only for v1; different algorithms entirely |
| FFI bindings to C++/Go | Pure Rust constraint; build Rust crates instead |
| C FFI layer (API-06) | Conflicts with pure Rust constraint; excluded from v1 |
| Python bindings (API-07) | Conflicts with pure Rust constraint; excluded from v1 |
| Material science database | External service; not embedded in slicer |
| Filament drying/storage management | Physical workflow, not slicing software |
| Direct printer firmware development | Core generates G-code; firmware is separate |

## Traceability

Updated during roadmap creation. Each requirement maps to exactly one phase.

| Requirement | Phase | Status |
|-------------|-------|--------|
| FOUND-01 | Phase 1 | Pending |
| FOUND-04 | Phase 1 | Pending |
| FOUND-05 | Phase 1 | Pending |
| FOUND-08 | Phase 1 | Pending |
| MESH-09 | Phase 1 | Pending |
| MESH-01 | Phase 2 | Pending |
| MESH-02 | Phase 2 | Pending |
| MESH-03 | Phase 2 | Pending |
| MESH-04 | Phase 2 | Pending |
| MESH-05 | Phase 2 | Pending |
| MESH-06 | Phase 2 | Pending |
| MESH-07 | Phase 2 | Pending |
| MESH-08 | Phase 2 | Pending |
| SLICE-01 | Phase 3 | Pending |
| SLICE-03 | Phase 3 | Pending |
| SLICE-05 | Phase 3 | Pending |
| PERIM-01 | Phase 3 | Pending |
| PERIM-03 | Phase 3 | Pending |
| INFILL-01 | Phase 3 | Pending |
| INFILL-11 | Phase 3 | Pending |
| INFILL-12 | Phase 3 | Pending |
| GCODE-01 | Phase 3 | Pending |
| GCODE-05 | Phase 3 | Pending |
| GCODE-07 | Phase 3 | Pending |
| GCODE-08 | Phase 3 | Pending |
| GCODE-09 | Phase 3 | Pending |
| GCODE-10 | Phase 3 | Pending |
| API-02 | Phase 3 | Pending |
| SLICE-02 | Phase 4 | Pending |
| SLICE-04 | Phase 4 | Pending |
| PERIM-02 | Phase 4 | Pending |
| PERIM-04 | Phase 4 | Pending |
| PERIM-05 | Phase 4 | Pending |
| PERIM-06 | Phase 4 | Pending |
| INFILL-02 | Phase 4 | Pending |
| INFILL-03 | Phase 4 | Pending |
| INFILL-04 | Phase 4 | Pending |
| INFILL-05 | Phase 4 | Pending |
| INFILL-06 | Phase 4 | Pending |
| INFILL-07 | Phase 4 | Pending |
| INFILL-08 | Phase 4 | Pending |
| SUPP-01 | Phase 5 | Pending |
| SUPP-02 | Phase 5 | Pending |
| SUPP-03 | Phase 5 | Pending |
| SUPP-04 | Phase 5 | Pending |
| SUPP-05 | Phase 5 | Pending |
| SUPP-06 | Phase 5 | Pending |
| SUPP-07 | Phase 5 | Pending |
| SUPP-08 | Phase 5 | Pending |
| GCODE-02 | Phase 6 | Pending |
| GCODE-03 | Phase 6 | Pending |
| GCODE-04 | Phase 6 | Pending |
| GCODE-06 | Phase 6 | Pending |
| GCODE-11 | Phase 6 | Pending |
| GCODE-12 | Phase 6 | Complete |
| GCODE-13 | Phase 6 | Complete |
| ADV-01 | Phase 6 | Pending |
| ADV-02 | Phase 6 | Pending |
| ADV-03 | Phase 6 | Pending |
| ADV-04 | Phase 6 | Pending |
| ADV-05 | Phase 6 | Pending |
| ADV-06 | Phase 6 | Pending |
| ADV-07 | Phase 6 | Pending |
| ADV-08 | Phase 6 | Pending |
| INFILL-09 | Phase 6 | Pending |
| INFILL-10 | Phase 6 | Pending |
| PLUGIN-01 | Phase 7 | Pending |
| PLUGIN-02 | Phase 7 | Pending |
| PLUGIN-03 | Phase 7 | Pending |
| PLUGIN-04 | Phase 7 | Pending |
| PLUGIN-05 | Phase 7 | Pending |
| PLUGIN-06 | Phase 7 | Pending |
| PLUGIN-07 | Phase 7 | Pending |
| AI-01 | Phase 8 | Pending |
| AI-02 | Phase 8 | Pending |
| AI-03 | Phase 8 | Pending |
| AI-04 | Phase 8 | Pending |
| AI-05 | Phase 8 | Pending |
| AI-06 | Phase 8 | Pending |
| FOUND-02 | Phase 9 | Pending |
| FOUND-03 | Phase 9 | Pending |
| FOUND-06 | Phase 9 | Pending |
| FOUND-07 | Phase 9 | Pending |
| API-01 | Phase 9 | Pending |
| API-03 | Phase 9 | Pending |
| API-04 | Phase 9 | Pending |
| API-05 | Phase 9 | Pending |
| TEST-01 | Phase 9 | Pending |
| TEST-02 | Phase 9 | Pending |
| TEST-03 | Phase 9 | Pending |
| TEST-04 | Phase 9 | Pending |
| TEST-05 | Phase 9 | Pending |
| TEST-06 | Phase 9 | Pending |
| TEST-07 | Phase 9 | Pending |
| API-06 | EXCLUDED | Scope conflict with PROJECT.md |
| API-07 | EXCLUDED | Scope conflict with PROJECT.md |

**Coverage:**
- v1 requirements: 86 total
- Mapped to phases: 84
- Excluded (scope conflict): 2 (API-06, API-07)
- Unmapped: 0

---
*Requirements defined: 2026-02-14*
*Last updated: 2026-02-14 after roadmap creation*
