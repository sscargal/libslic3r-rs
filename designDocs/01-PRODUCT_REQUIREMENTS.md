# LibSlic3r-RS: Product Requirements Document (PRD)

**Version:** 1.0.0-draft
**Author:** Steve Scargall / SliceCore-RS Architecture Team
**Date:** 2026-02-13
**Status:** Draft — Review & Iterate

---

## 1. Executive Summary

LibSlic3r-RS is a ground-up Rust rewrite of the C++ LibSlic3r slicing core that powers every major open-source FDM slicer (PrusaSlicer, BambuStudio, OrcaSlicer, CrealityPrint). The goal is to create a **modular, pluggable, high-performance slicer core** that serves as the foundation for:

- Native desktop slicers (macOS, Linux, Windows — ARM & x86_64)
- Headless CLI tooling for automation and print farms
- Cloud SaaS products with AI-driven optimization
- WebAssembly-powered browser-based slicing
- API-first integrations for third-party tooling

The project addresses fundamental limitations in the existing C++ ecosystem: monolithic architecture, lack of APIs, no headless operation, poor AI integration, and an overwhelming user experience that alienates beginners while frustrating experts.

---

## 2. Problem Statement

### 2.1 The Fork Tree & Its Consequences

```
Slic3r (Alessandro Ranellucci, 2011)
  └── PrusaSlicer (Prusa Research)
        └── BambuStudio (BambuLab)
              └── OrcaSlicer (Community)
                    └── CrealityPrint (Creality)
```

Each fork carries forward ~15 years of C++ technical debt. Features diverge. Bug fixes don't propagate. The community fragments. Innovation stalls.

### 2.2 User Frustrations by Experience Level

#### Beginners (< 6 months experience)
| Problem | Impact |
|---------|--------|
| 300+ settings with cryptic names | Overwhelmed, use defaults, get poor prints |
| No guided workflows | Don't know what to change or why |
| Profile management is confusing | Wrong profile = failed print = wasted filament |
| Error messages are unhelpful | "Slicing failed" with no actionable guidance |
| No real-time feedback on what settings do | Trial-and-error learning curve |
| Multiple slicers needed for different printers | Fragmented ecosystem |

#### Intermediate / DIY Users (6 months — 3 years)
| Problem | Impact |
|---------|--------|
| Settings interactions are undocumented | Changing one setting breaks another |
| No A/B comparison tooling | Can't see impact of changes before printing |
| Profile sharing is primitive (ini/json files) | Community knowledge is siloed |
| No batch processing | One model at a time is tedious |
| Preview is slow and limited | Can't inspect critical layers efficiently |
| Support generation is hit-or-miss | Manual painting is time-consuming |

#### Expert / Print Farm Operators (3+ years)
| Problem | Impact |
|---------|--------|
| No headless/CLI mode suitable for automation | Can't build CI/CD for print pipelines |
| No programmatic API | Can't integrate with MES/ERP systems |
| No cloud-native operation | Print farms need centralized slicing |
| Poor multi-printer fleet management | Each printer needs separate profile management |
| No telemetry or feedback loops | Can't correlate settings → print quality |
| C++ codebase repels contributors | AI tooling and modern dev practices don't apply |
| Plugin systems are weak or nonexistent | Can't extend without forking |
| No adaptive slicing based on geometry | Uniform layer heights waste time |

### 2.3 Technical Debt in C++ LibSlic3r

- **Monolithic architecture:** Geometry, slicing, G-code, and UI deeply coupled
- **Global state everywhere:** Thread safety is bolted on, not designed in
- **No serialization boundaries:** Can't extract the engine from the GUI
- **Memory management:** Manual memory management with smart pointers inconsistently applied
- **Build system complexity:** CMake + vcpkg + platform-specific hacks
- **No test harness:** Minimal unit tests; integration testing is "print it and see"
- **No profiling infrastructure:** Performance bottlenecks are addressed ad-hoc
- **Intel TBB dependency:** 47+ parallel_for sites, 2 parallel_reduce operations — maps cleanly to Rust's rayon but requires careful granularity tuning

---

## 3. Product Vision

> **"Make 3D printing as reliable as 2D printing — press print and it works."**

LibSlic3r-RS delivers this vision through three pillars:

1. **Intelligence:** AI-driven defaults, adaptive algorithms, and predictive optimization
2. **Accessibility:** Progressive disclosure — simple for beginners, powerful for experts
3. **Extensibility:** Plugin architecture, API-first design, and cloud-native operation

---

## 4. Target Users & Use Cases

### 4.1 User Personas

| Persona | Description | Primary Need |
|---------|-------------|--------------|
| **Maker Maya** | Hobbyist, prints toys and household items | "Just work" defaults, guided setup |
| **Engineer Erik** | Designs functional parts, needs dimensional accuracy | Precise control, material profiles, simulation |
| **Farm Owner Fiona** | Operates 50+ printers, runs a business | Automation, fleet management, cost optimization |
| **Developer Dev** | Builds tools on top of slicing | API access, headless mode, WebAssembly |
| **AI Researcher Aria** | Trains models on slicing data | Structured output, batch processing, feature extraction |

### 4.2 Use Cases

| ID | Use Case | Personas | Priority |
|----|----------|----------|----------|
| UC-01 | Slice a single STL/3MF to G-code | All | P0 |
| UC-02 | Headless CLI batch slicing | Fiona, Dev | P0 |
| UC-03 | AI-optimized print profile selection | Maya, Erik | P0 |
| UC-04 | Cloud SaaS: upload → optimize → download | All | P1 |
| UC-05 | Real-time slicing preview in browser (WASM) | Maya, Dev | P1 |
| UC-06 | Plugin: custom infill pattern | Erik, Dev | P1 |
| UC-07 | Multi-printer fleet job distribution | Fiona | P1 |
| UC-08 | Print quality prediction before printing | Erik, Fiona | P2 |
| UC-09 | Automated support structure optimization | All | P1 |
| UC-10 | G-code analysis and post-processing | Erik, Dev, Fiona | P1 |
| UC-11 | Model repair and mesh optimization | All | P0 |
| UC-12 | STEP file native slicing | Erik | P2 |

---

## 5. Functional Requirements

### 5.1 Core Slicing Pipeline (P0)

| ID | Requirement | Description |
|----|-------------|-------------|
| FR-001 | Mesh ingestion | Import STL, 3MF (via `lib3mf-core`), OBJ, STEP, AMF |
| FR-002 | Mesh repair | Auto-repair non-manifold, self-intersecting, degenerate meshes |
| FR-003 | Mesh transformations | Scale, rotate, translate, mirror, split, merge |
| FR-004 | Slicing | Contour extraction at configurable layer heights |
| FR-005 | Adaptive layer heights | Variable layers based on surface curvature |
| FR-006 | Perimeter generation | Configurable wall count, ordering, gap fill |
| FR-007 | Infill generation | Multiple patterns: 24 PrusaSlicer-standard patterns (rectilinear, grid, gyroid, cubic, honeycomb, lightning, adaptive cubic, monotonic, etc.) plus 11 OrcaSlicer innovations (TPMS-D, TPMS-FK, CrossHatch, lateral patterns) |
| FR-008 | Support generation | Auto and manual supports with configurable parameters; traditional grid/line support, tree supports (3 implementations: Cura-based, organic mesh-based, polygon-node), and support enforcers/blockers |
| FR-009 | Bridge detection | Detect and handle unsupported spans |
| FR-010 | Overhang detection | Identify overhangs requiring support or special treatment |
| FR-011 | Top/bottom surface generation | Solid layers, ironing, surface quality control |
| FR-012 | Skirt/brim/raft | Bed adhesion helpers |
| FR-013 | Retraction & wipe | Configurable retraction, z-hop, wipe moves |
| FR-014 | Speed planning | Per-feature speed, acceleration, jerk control |
| FR-015 | Temperature planning | Layer-based temperature changes, first-layer overrides |
| FR-016 | Cooling/fan control | Layer time-based, bridge-specific, overhang-specific |
| FR-017 | G-code generation | Marlin, Klipper, RepRapFirmware, Bambu, custom dialects |
| FR-018 | G-code preview data | Layer-by-layer visualization data (line type, speed, flow) |
| FR-019 | Multi-material support | Tool changes, purge tower/bucket, color painting |
| FR-020 | Sequential printing | Object-by-object printing with collision avoidance |

### 5.2 Advanced Features (P1)

| ID | Requirement | Description |
|----|-------------|-------------|
| FR-030 | Tree supports | Organic tree-shaped supports for reduced scarring |
| FR-031 | Arachne perimeters | Variable-width perimeters for thin walls |
| FR-032 | Arc fitting | Convert linear segments to G2/G3 arcs |
| FR-033 | Pressure advance calibration | Built-in PA/LA tuning data generation |
| FR-034 | Seam painting/placement | Intelligent seam hiding, configurable strategy |
| FR-035 | Modifier meshes | Region-specific settings overrides |
| FR-036 | Custom G-code injection | Per-layer, per-feature, per-tool-change hooks |
| FR-037 | Print time estimation | Accurate time estimates considering firmware behavior |
| FR-038 | Filament usage estimation | Weight, length, and cost estimation |
| FR-039 | Plugin system | Dynamic load/unload of slicing extensions |
| FR-040 | Profile system | Hierarchical profiles: printer → filament → print quality |
| FR-041 | Scarf joint seam | Gradient flow/speed seam hiding with 12 configurable parameters (OrcaSlicer innovation) |
| FR-042 | Per-feature flow control | Independent flow ratios for 10+ feature types: outer wall, inner wall, top/bottom surface, overhang, gap fill, support, etc. |
| FR-043 | 4-tier overhang control | Dynamic speed and fan speed adjustment based on overhang angle (0-25%, 25-50%, 50-75%, 75%+) |
| FR-044 | Wall direction control | Configurable inner/outer wall print direction for quality optimization |
| FR-045 | Hole-to-polyhole | Convert circular holes to polygons for dimensional accuracy |

### 5.3 AI & Optimization Features (P1-P2)

| ID | Requirement | Description |
|----|-------------|-------------|
| FR-050 | AI profile suggestion | Analyze model geometry → suggest optimal settings |
| FR-051 | Print failure prediction | Predict warping, stringing, layer adhesion issues |
| FR-052 | Automatic orientation | Optimal build plate orientation for strength/quality/speed |
| FR-053 | Nesting/packing | Optimal arrangement of multiple parts on bed |
| FR-054 | Topology-aware infill | Vary infill based on stress analysis |
| FR-055 | AI model abstraction | Provider-agnostic LLM/ML integration layer |
| FR-056 | Feedback loop | Correlate print results with slicing parameters |

### 5.4 API & Integration (P0)

| ID | Requirement | Description |
|----|-------------|-------------|
| FR-060 | Rust library API | Well-documented public Rust API |
| FR-061 | C FFI | C-compatible foreign function interface for cross-language use |
| FR-062 | CLI interface | Full-featured command-line interface |
| FR-063 | REST/gRPC API | Network API for cloud and remote operation |
| FR-064 | WebAssembly target | Compile core to WASM for browser execution |
| FR-065 | Python bindings | PyO3-based Python bindings for scripting and AI |
| FR-066 | Event system | Pub/sub event system for progress, warnings, errors |
| FR-067 | Structured output | JSON/MessagePack serialized slicing results |

---

## 6. Non-Functional Requirements

### 6.1 Performance

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-001 | Slice time for 50mm cube | < 2 seconds on 4-core consumer laptop |
| NFR-002 | Slice time for complex model (500K triangles) | < 30 seconds on 4-core consumer laptop |
| NFR-003 | Memory usage for complex model | < 2 GiB peak |
| NFR-004 | WASM slice time | < 5x native performance |
| NFR-005 | Startup time (CLI) | < 100ms to ready |
| NFR-006 | Parallel scaling | Near-linear up to 8 cores, graceful beyond |

### 6.2 Portability

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-010 | Platform support | macOS (ARM/x86), Linux (ARM/x86), Windows (ARM/x86) |
| NFR-011 | Minimum hardware | 4 vCPUs, 8 GiB RAM, integrated GPU |
| NFR-012 | No GPU required for slicing | GPU optional, used only for preview acceleration |
| NFR-013 | WASM target | wasm32-wasi and wasm32-unknown-unknown |

### 6.3 Reliability

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-020 | Deterministic output | Same input + config = identical G-code (bit-for-bit) |
| NFR-021 | Crash-free slicing | No panics; all errors handled and reported |
| NFR-022 | Test coverage | > 80% line coverage on core algorithms |
| NFR-023 | Fuzz testing | All parsers and mesh operations fuzz-tested |

### 6.4 Developer Experience

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-030 | Build time (clean) | < 3 minutes on developer laptop |
| NFR-031 | Build time (incremental) | < 15 seconds |
| NFR-032 | Documentation | Rustdoc on all public APIs with examples |
| NFR-033 | CI/CD | Full test suite on every PR |
| NFR-034 | AI-friendly codebase | Clear module boundaries, descriptive types, minimal macros |

---

## 7. Settings Architecture

### 7.1 Progressive Disclosure Model

```
┌──────────────────────────────────────────────────────┐
│                    SETTINGS TIERS                     │
├──────────────────────────────────────────────────────┤
│                                                      │
│  Tier 0: AI Auto          (0 settings visible)       │
│  "Just print it"          AI picks everything         │
│                                                      │
│  Tier 1: Simple           (~15 settings)             │
│  Quality, Speed, Strength  Curated presets            │
│                                                      │
│  Tier 2: Intermediate     (~60 settings)             │
│  Per-feature control       Grouped by function        │
│                                                      │
│  Tier 3: Advanced         (~200 settings)            │
│  Full parameter access     Expert tuning              │
│                                                      │
│  Tier 4: Developer        (All ~850+ settings)       │
│  Internal parameters       Algorithm tuning           │
│                                                      │
└──────────────────────────────────────────────────────┘
```

### 7.2 Settings Categories

Each setting belongs to exactly one category and has metadata:

```rust
struct SettingDefinition {
    key: SettingKey,              // e.g., "perimeters.wall_count"
    display_name: String,         // e.g., "Wall Count"
    description: String,          // Human-readable explanation
    tier: SettingTier,            // Which disclosure level
    category: SettingCategory,    // Grouping
    value_type: ValueType,        // Int, Float, Enum, Bool, etc.
    default: Value,               // Default value
    constraints: Constraints,     // Min, max, step, dependencies
    affects: Vec<SettingKey>,     // Settings this influences
    affected_by: Vec<SettingKey>, // Settings that influence this
    units: Option<Units>,         // mm, mm/s, °C, %, etc.
    tags: Vec<String>,           // Searchable tags
}
```

---

## 8. Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Feature parity with OrcaSlicer | 95% of P0+P1 features (~450 settings) within 18 months; full superset (~850 settings) within 30 months | Feature checklist |
| Slice speed vs. C++ LibSlic3r | ≥ 1.5x faster | Benchmark suite |
| Memory usage vs. C++ LibSlic3r | ≤ 80% | Benchmark suite |
| WASM bundle size | < 5 MiB (gzipped) | Build output |
| API response time (cloud) | < 5s for typical model | P95 latency |
| Test coverage | > 80% | cargo-tarpaulin |
| Crates published | Core + 10 sub-crates | crates.io |

---

## 9. Out of Scope (This Phase)

- Native GUI application (will be separate project consuming the API)
- Printer firmware development
- Filament material science database (external service)
- Direct printer communication protocols (OctoPrint/Moonraker integration instead)
- Resin/SLA/DLP slicing (FDM only for v1)

---

## 10. Dependencies & Assumptions

### 10.1 Dependencies

| Dependency | Purpose | Status |
|------------|---------|--------|
| `lib3mf-core` | 3MF file parsing | ✅ Published on crates.io |
| `rayon` | Parallel computation | ✅ Stable |
| `serde` | Serialization | ✅ Stable |
| `geo` / `geo-types` | 2D geometry operations | ✅ Stable |
| `nalgebra` | Linear algebra | ✅ Stable |
| `wasm-bindgen` | WASM target | ✅ Stable |
| `i-overlay` or `clipper2` | Polygon boolean/offset ops | 🔲 Benchmark needed — 1,425+ Clipper call sites in C++ |
| Custom crates (TBD) | Gaps identified during development | 🔲 To be created |

### 10.2 Assumptions

- Rust's `no_std` compatibility is NOT required; `std` is available on all targets except WASM (which uses `wasm-bindgen`)
- The slicer core produces G-code; it does not send it to printers
- AI model inference happens out-of-process; the core provides structured data for AI consumption
- The project is developed by a sole developer with AI coding assistants

---

## 11. Risks & Mitigations

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Algorithm complexity underestimated | High | High | Start with well-known algorithms; optimize later |
| Rust ecosystem gaps for computational geometry | Medium | Medium | Identify and build missing crates |
| WASM performance insufficient | Medium | Medium | Profile early; fallback to server-side slicing |
| Feature parity takes too long | High | High | Prioritize P0 features; accept 80/20 rule |
| Solo developer burnout | High | Critical | Modular design enables incremental progress |
| Patent risk from existing algorithms | Low | High | Document novel approaches; prior art search |

---

*Next Document: [02-ARCHITECTURE.md](./02-ARCHITECTURE.md)*