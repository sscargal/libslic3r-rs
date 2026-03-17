# Architecture

**Analysis Date:** 2026-02-13

## Pattern Overview

**Overall:** Layered, data-oriented, multi-crate architecture with strict dependency rules.

**Key Characteristics:**
- Five-layer modular design with unidirectional dependencies (no upward deps, no cycles)
- Data-oriented design preferring flat arrays and structure-of-arrays over object hierarchies
- Zero-cost abstractions using traits and generics over dynamic dispatch
- Deterministic execution ensuring identical outputs from identical inputs
- Plugin-first extensibility with built-in, dynamic, and WASM sandboxed plugins
- Progressive complexity — simple things remain simple while complex operations are possible

## Layers

**Layer 0: Foundation (`slicecore-math`, `slicecore-geo`, `slicecore-mesh`):**
- Purpose: Provides geometric primitives and data structures for all higher layers
- Location: `crates/slicecore-math/`, `crates/slicecore-geo/`, `crates/slicecore-mesh/`
- Contains: 3D points, vectors, matrices, transformations, polygons, polylines, mesh representations, boolean operations, spatial indexing
- Depends on: External crates only (serde, thiserror, bumpalo)
- Used by: All layers above

**Layer 1: I/O & Data (`slicecore-fileio`, `slicecore-gcode-io`, `slicecore-config`):**
- Purpose: File format parsing/writing and configuration management
- Location: `crates/slicecore-fileio/`, `crates/slicecore-gcode-io/`, `crates/slicecore-config/`
- Contains: STL/3MF/OBJ/STEP parsers, G-code parsers/writers, hierarchical config system, schema validation, profile management
- Depends on: Layer 0 types
- Used by: Layers 2-5

**Layer 2: Algorithms (`slicecore-slicer`, `slicecore-perimeters`, `slicecore-infill`, `slicecore-supports`, `slicecore-pathing`):**
- Purpose: Core slicing and toolpath generation algorithms
- Location: `crates/slicecore-slicer/`, `crates/slicecore-perimeters/`, `crates/slicecore-infill/`, `crates/slicecore-supports/`, `crates/slicecore-pathing/`
- Contains: Layer contour extraction, region classification, perimeter generation, infill pattern generation, support structure generation, toolpath ordering and travel optimization
- Depends on: Layers 0-1
- Used by: Layers 3-5

**Layer 3: Planning & Generation (`slicecore-planner`, `slicecore-gcode-gen`, `slicecore-estimator`):**
- Purpose: Motion planning, G-code emission, and resource estimation
- Location: `crates/slicecore-planner/`, `crates/slicecore-gcode-gen/`, `crates/slicecore-estimator/`
- Contains: Speed/acceleration planning, temperature/fan control, extrusion calculation, G-code formatting, time/material/cost estimation
- Depends on: Layers 0-2
- Used by: Layers 4-5

**Layer 4: Intelligence (`slicecore-ai`, `slicecore-optimizer`, `slicecore-analyzer`):**
- Purpose: AI/ML integration, model analysis, and parameter optimization
- Location: `crates/slicecore-ai/`, `crates/slicecore-optimizer/`, `crates/slicecore-analyzer/`
- Contains: Model feature extraction, provider-agnostic AI abstraction (OpenAI, Anthropic, local Ollama), parameter optimization, printability scoring
- Depends on: Layers 0-3
- Used by: Layer 5

**Layer 5: Integration (`slicecore-engine`, `slicecore-plugin`, `slicecore-api`):**
- Purpose: Pipeline orchestration, plugin system, and external interfaces
- Location: `crates/slicecore-engine/`, `crates/slicecore-plugin/`, `crates/slicecore-api/`
- Contains: Full slicing pipeline orchestration, plugin registry/loading, REST/gRPC server, CLI interface, Python bindings, WASM interface, FFI
- Depends on: Layers 0-4
- Used by: External applications (CLI, desktop, cloud, browsers)

## Data Flow

**Full Slicing Pipeline:**

1. **Input Loading** — `slicecore-fileio` parses STL/3MF/OBJ into `TriangleMesh` (Layer 1)
2. **Model Preparation** — `slicecore-mesh` repairs and transforms mesh to build-plate orientation (Layer 0)
3. **Layer Generation** — `slicecore-slicer` intersects mesh with horizontal planes to produce `SliceLayer` contours (Layer 2)
4. **Region Classification** — Each contour region classified as perimeter, infill, support, bridge, overhang, etc. (Layer 2)
5. **Perimeter Generation** — `slicecore-perimeters` generates wall extrusions via inward offset and seam placement (Layer 2)
6. **Infill Generation** — `slicecore-infill` generates pattern-based fill paths within classified infill regions (Layer 2)
7. **Support Generation** — `slicecore-supports` creates support structures under overhangs (Layer 2)
8. **Toolpath Ordering** — `slicecore-pathing` optimizes extrusion ordering and travel moves within layers (Layer 2)
9. **Motion Planning** — `slicecore-planner` assigns speeds, accelerations, temperatures, fan speeds, retractions (Layer 3)
10. **G-code Generation** — `slicecore-gcode-gen` emits firmware-specific G-code (Layer 3)
11. **Metadata & Estimation** — `slicecore-estimator` computes time, filament, cost; generates JSON metadata (Layer 3)
12. **Output** — `slicecore-gcode-io` writes G-code file; `slicecore-api` returns result via configured interface

**State Management:**

- **Immutable Input:** Mesh and config are immutable throughout pipeline
- **Per-Layer Processing:** Each layer processed independently where possible (enables parallelism via rayon)
- **Arena Allocation:** Transient geometry objects (clipped polygons, intermediate paths) allocated in per-layer arenas with O(1) reset between layers
- **Output Streaming:** G-code emitted per-layer as completed (enables streaming to printer before full slice completes)

## Key Abstractions

**TriangleMesh:**
- Purpose: Represents 3D printable model
- Examples: `crates/slicecore-mesh/src/lib.rs`
- Pattern: Indexed vertex array + face indices with lazy-computed normals and cached spatial index (BVH)

**SliceLayer:**
- Purpose: 2D horizontal slice at a specific Z height
- Examples: `crates/slicecore-slicer/src/lib.rs`
- Pattern: Contains Z coordinate, layer height, outer contours, holes, and region classifications

**ExtrusionSegment:**
- Purpose: Single extrusion motion from point A to B
- Examples: `crates/slicecore-gcode-gen/src/lib.rs`
- Pattern: Start/end points, extrusion width/height, flow rate, region type, attributes (seam, bridge, etc.)

**RegionType:**
- Purpose: Semantic classification of extrusion purpose
- Examples: Perimeter{wall_index}, Infill{density}, Support, Bridge, Overhang{angle}, TopSurface, BottomSurface
- Pattern: Enum driving behavior decisions (speed, temperature, fan speed, preview coloring)

**PlannedMove:**
- Purpose: Physical motion with all parameters resolved
- Examples: `crates/slicecore-planner/src/lib.rs`
- Pattern: Start/end points, feedrate, acceleration, extrusion amount, temperature, fan speed, move type

**Plugin Trait:**
- Purpose: Extension point for algorithm plugins
- Examples: `slicecore-perimeters` exports `InfillPattern`, `SupportStrategy`, `GcodeDialect` traits
- Pattern: Base `Plugin` trait + specialized extension traits (e.g., `InfillPattern::generate()`)

**AiProvider:**
- Purpose: Provider-agnostic AI/LLM abstraction
- Examples: `crates/slicecore-ai/src/lib.rs` implements OpenAiProvider, AnthropicProvider, OllamaProvider, etc.
- Pattern: Async trait with `complete()` for text generation, `embed()` for embeddings, `capabilities()` for feature detection

## Entry Points

**CLI (`bins/slicecore-cli/`):**
- Location: `bins/slicecore-cli/src/main.rs`
- Triggers: User runs `slicecore slice model.stl -o output.gcode`
- Responsibilities: Parse command-line args, load config, instantiate engine, invoke slice pipeline, write output

**REST API Server (`bins/slicecore-server/`):**
- Location: `bins/slicecore-server/src/main.rs`
- Triggers: HTTP POST to `/api/v1/slice` with multipart model + config JSON
- Responsibilities: Listen on configured port, deserialize requests, invoke engine, return JSON response with gcode + metadata

**Library Entry (`crates/slicecore-engine/`):**
- Location: `crates/slicecore-engine/src/lib.rs` exports `pub struct Engine`
- Triggers: External code calls `engine.slice(SliceJob { ... })?`
- Responsibilities: Coordinate full pipeline, manage thread pool, report progress, handle cancellation

**WASM Entry (`crates/slicecore-api/src/wasm.rs`):**
- Location: `crates/slicecore-api/src/wasm.rs` exports `#[wasm_bindgen] pub struct WasmSlicer`
- Triggers: JavaScript calls `new WasmSlicer().slice(model_bytes, config_json)`
- Responsibilities: Deserialize JS inputs, invoke engine, return JavaScript objects with gcode string and metadata

## Error Handling

**Strategy:** Layered error types with context, using `thiserror` for ergonomic error definitions.

**Patterns:**

- **Top-level:** `SliceCoreError` enum aggregates all crate-specific errors via `#[from]`
- **Layer 0 (Mesh):** `MeshError` for topology issues (non-manifold, self-intersection, degenerate triangles)
- **Layer 1 (IO):** `IoError` for parse failures with suggestions (e.g., "STL appears truncated")
- **Layer 2 (Algorithms):** Domain-specific errors (e.g., `SlicingError` with layer number context)
- **Layer 4 (AI):** `AiError` for provider unavailability/rate-limiting with fallback hints
- **Recovery:** Where possible, errors include `suggestion: Option<String>` for programmatic recovery hints
- **Warnings vs Errors:** Non-fatal issues (e.g., "thin wall below 1.2mm nozzle width") collected separately in `Vec<SliceWarning>` and returned alongside results

## Cross-Cutting Concerns

**Logging:** Structured logging via `tracing` crate with span-based context.
- All layer-level operations emit debug events
- Slice progress reported via `Progress` trait implementation
- Production: JSON logging to stdout; development: pretty-printed to stderr

**Validation:** Schema-driven validation in `slicecore-config`.
- All settings have declared types, ranges, constraints
- Dependent settings validated (e.g., "outer_wall_speed must be ≤ max_print_speed")
- Expression-based validation (e.g., "layer_height ≤ nozzle_diameter × 0.8")

**Authentication:** For cloud/remote operations in Layer 5.
- API keys handled via `secrecy` crate (never logged)
- CORS configured permissively for WASM origin
- JWT tokens optional for stateful API sessions

**Parallelism:** Data parallelism via `rayon` in Layers 2-5.
- Per-layer operations (`slice_layers`, `generate_perimeters`, `generate_infill`) use `rayon::par_iter()`
- Thread pool size configurable; defaults to `num_cpus::get()`
- Cancellation via `CancellationToken` checked between stages

---

*Architecture analysis: 2026-02-13*
