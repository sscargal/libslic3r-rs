# Requirements: libslic3r-rs

**Defined:** 2026-02-14
**Core Value:** The plugin architecture and AI integration must work from day one — modularity and intelligence are not bolt-ons.

## v1 Requirements

Requirements for initial proof-of-concept release. Each maps to roadmap phases.

### Foundation (FOUND)

- [ ] **FOUND-01**: Pure Rust implementation with no FFI to C/C++/Python/Go
- [ ] **FOUND-02**: Multi-platform support: macOS (ARM/x86), Linux (ARM/x86), Windows (ARM/x86)
- [ ] **FOUND-03**: WASM compilation target (wasm32-wasi and wasm32-unknown-unknown)
- [ ] **FOUND-04**: Coordinate precision strategy locked (f64 vs i64 vs hybrid)
- [ ] **FOUND-05**: Polygon boolean operations work (i-overlay or clipper2-rust)
- [ ] **FOUND-06**: Performance matches or beats C++ libslic3r (≥1.0x, targeting ≥1.5x)
- [ ] **FOUND-07**: Memory usage ≤80% of C++ libslic3r
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
- [ ] **GCODE-12**: Print time estimation
- [ ] **GCODE-13**: Filament usage estimation (weight, length, cost)

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
- [ ] **AI-03**: Profile suggestion endpoint (send geometry → receive settings)
- [ ] **AI-04**: Local LLM support (ollama, llama.cpp)
- [ ] **AI-05**: Cloud LLM support (OpenAI, Anthropic, custom endpoints)
- [ ] **AI-06**: Example: AI-driven profile suggestion for uploaded model

### API & CLI (API)

- [ ] **API-01**: Well-documented Rust public API (all public items have rustdoc)
- [ ] **API-02**: Full-featured CLI interface (slice, validate, analyze commands)
- [ ] **API-03**: JSON structured output (settings, slicing results, metadata)
- [ ] **API-04**: MessagePack structured output option
- [ ] **API-05**: Event system for progress, warnings, errors (pub/sub)
- [ ] **API-06**: C FFI layer for cross-language use (C-compatible API)
- [ ] **API-07**: Python bindings via PyO3

### Quality & Testing (TEST)

- [ ] **TEST-01**: Unit tests for all core algorithms
- [ ] **TEST-02**: Integration tests (STL → G-code validation)
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
- **UI-04**: Profile hierarchy (printer → filament → print quality)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Native GUI application | Separate project consuming library API; not part of core |
| Printer communication (OctoPrint/Moonraker) | Integration layer lives outside core slicing |
| Resin/SLA/DLP slicing | FDM only for v1; different algorithms entirely |
| FFI bindings to C++/Go | Pure Rust constraint; build Rust crates instead |
| Material science database | External service; not embedded in slicer |
| Filament drying/storage management | Physical workflow, not slicing software |
| Direct printer firmware development | Core generates G-code; firmware is separate |

## Traceability

Updated during roadmap creation. Each requirement maps to exactly one phase.

| Requirement | Phase | Status |
|-------------|-------|--------|
| (Populated by roadmapper) | - | - |

**Coverage:**
- v1 requirements: 86 total
- Mapped to phases: 0 (pending roadmap)
- Unmapped: 86 ⚠️

---
*Requirements defined: 2026-02-14*
*Last updated: 2026-02-14 after initial definition*
