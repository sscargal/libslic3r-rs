# Architecture Research

**Domain:** Modular Rust 3D slicing library with plugin architecture
**Researched:** 2026-02-14
**Confidence:** HIGH

---

## Standard Architecture

### System Overview

```
                          Consumer Layer
  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐
  │ Desktop   │  │   CLI     │  │ REST/gRPC │  │  Browser  │
  │ GUI (FFI) │  │  Binary   │  │  Server   │  │  (WASM)   │
  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘
        │              │              │              │
────────┴──────────────┴──────────────┴──────────────┴──────────
                    Public API Boundary
────────────────────────────────────────────────────────────────

  Layer 5: Integration
  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
  │ slicecore-   │  │ slicecore-   │  │ slicecore-   │
  │ engine       │  │ plugin       │  │ api          │
  │ (orchestrate)│  │ (load/run)   │  │ (expose)     │
  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘
         │                 │                 │
  Layer 4: Intelligence (optional, feature-gated)
  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
  │ slicecore-   │  │ slicecore-   │  │ slicecore-   │
  │ ai           │  │ optimizer    │  │ analyzer     │
  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘
         │                 │                 │
  Layer 3: Planning & Generation
  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
  │ slicecore-   │  │ slicecore-   │  │ slicecore-   │
  │ planner      │  │ gcode-gen    │  │ estimator    │
  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘
         │                 │                 │
  Layer 2: Algorithms
  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐
  │slicecore-│ │slicecore-│ │slicecore-│ │slicecore-│ │slicecore-│
  │slicer    │ │perimeters│ │infill    │ │supports  │ │pathing   │
  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘
       │            │            │            │            │
  Layer 1: I/O & Data
  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
  │ slicecore-   │  │ slicecore-   │  │ slicecore-   │
  │ fileio       │  │ gcode-io     │  │ config       │
  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘
         │                 │                 │
  Layer 0: Foundation (no internal dependencies)
  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
  │ slicecore-   │  │ slicecore-   │  │ slicecore-   │
  │ math         │  │ geo          │  │ mesh         │
  └──────────────┘  └──────────────┘  └──────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| `slicecore-math` | Vector/matrix math, coordinate transforms, floating-point utilities | Thin wrapper over `nalgebra` or `glam`, plus integer coordinate types (`Coord = i64`) |
| `slicecore-geo` | 2D computational geometry: polygon booleans, offsetting, point-in-polygon, area | Wraps `i-overlay` for boolean ops; custom offset algorithms; arena-friendly allocation |
| `slicecore-mesh` | Half-edge mesh, BVH spatial index, mesh repair, manifold checks | Indexed `TriangleMesh` with lazy BVH construction; `bumpalo` arena for temporaries |
| `slicecore-fileio` | STL, 3MF, OBJ, AMF, STEP parsers and writers | Streaming parsers; `lib3mf-core` for 3MF; all parsers fuzz-tested |
| `slicecore-gcode-io` | G-code parser and semantic analyzer | Line-by-line streaming parser; structured `GcodeCommand` output |
| `slicecore-config` | Settings schema, hierarchical profiles, validation, serialization | Declarative `ConfigSchema` driving validation, UI generation, and AI prompts |
| `slicecore-slicer` | Contour extraction at Z heights, layer generation | Mesh-plane intersection via BVH; produces `Vec<SliceLayer>` |
| `slicecore-perimeters` | Wall generation, Arachne variable-width, gap fill | Polygon offset chains; thin-wall detection via two-stage offset |
| `slicecore-infill` | Pattern generation (rectilinear, gyroid, honeycomb, etc.), adaptive density | Plugin-extensible via `InfillPattern` trait; built-in patterns as default plugins |
| `slicecore-supports` | Auto/tree/organic support generation | Plugin-extensible via `SupportStrategy` trait; overhang analysis drives strategy selection |
| `slicecore-pathing` | Toolpath ordering, travel optimization, seam placement | Per-layer TSP heuristic; cross-layer continuity optimization |
| `slicecore-planner` | Speed/acceleration/temperature/cooling/retraction planning | Per-segment physical parameter resolution; firmware-aware limits |
| `slicecore-gcode-gen` | G-code emission with firmware dialect support | Plugin-extensible via `GcodeDialect` trait; streaming layer-by-layer emission |
| `slicecore-estimator` | Print time, material usage, cost estimation | Firmware-model-aware time estimation; structured analytics JSON output |
| `slicecore-ai` | LLM/ML provider abstraction, prompt templates, response parsing | Feature-gated (`ai`); provider-agnostic `AiProvider` trait |
| `slicecore-optimizer` | Parameter search, profile optimization | Uses `slicecore-engine` for evaluation; search strategies (grid, bayesian) |
| `slicecore-analyzer` | Model geometry analysis, printability scoring, risk assessment | Feature detection (holes, thin walls, overhangs); produces `AnalysisReport` |
| `slicecore-engine` | Full pipeline orchestrator; the main entry point for slicing | Owns `rayon::ThreadPool`; wires all stages; supports cancellation and progress |
| `slicecore-plugin` | Plugin loading, lifecycle management, registry | Dual-mode: native (`abi_stable`) + WASM (`wasmtime`); capability-based security |
| `slicecore-api` | REST/gRPC server, CLI interface, C FFI, Python bindings | Feature-gated per target; all use `slicecore-engine` internally |

---

## Recommended Project Structure

```
libslic3r-rs/                        # Workspace root (virtual manifest)
├── Cargo.toml                       # [workspace] members = ["crates/*", "bins/*"]
├── Cargo.lock                       # Shared lockfile
├── crates/
│   ├── slicecore-math/              # Layer 0: Math primitives
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   └── lib.rs
│   │   └── tests/
│   ├── slicecore-geo/               # Layer 0: 2D computational geometry
│   ├── slicecore-mesh/              # Layer 0: Mesh data structures + spatial index
│   ├── slicecore-fileio/            # Layer 1: File format I/O
│   ├── slicecore-gcode-io/          # Layer 1: G-code I/O
│   ├── slicecore-config/            # Layer 1: Configuration system
│   ├── slicecore-slicer/            # Layer 2: Slicing algorithms
│   ├── slicecore-perimeters/        # Layer 2: Perimeter generation
│   ├── slicecore-infill/            # Layer 2: Infill generation
│   ├── slicecore-supports/          # Layer 2: Support generation
│   ├── slicecore-pathing/           # Layer 2: Toolpath planning
│   ├── slicecore-planner/           # Layer 3: Motion/thermal planning
│   ├── slicecore-gcode-gen/         # Layer 3: G-code generation
│   ├── slicecore-estimator/         # Layer 3: Time/material estimation
│   ├── slicecore-ai/                # Layer 4: AI/ML abstraction (feature-gated)
│   ├── slicecore-optimizer/         # Layer 4: Parameter optimization
│   ├── slicecore-analyzer/          # Layer 4: Model analysis
│   ├── slicecore-engine/            # Layer 5: Pipeline orchestrator
│   ├── slicecore-plugin/            # Layer 5: Plugin system
│   ├── slicecore-plugin-api/        # Plugin interface crate (shared types/traits)
│   └── slicecore-api/               # Layer 5: External interfaces
├── bins/
│   ├── slicecore-cli/               # CLI binary
│   └── slicecore-server/            # API server binary (feature-gated)
├── plugins/
│   ├── infill-gyroid/               # Example built-in plugin
│   └── gcode-klipper/              # Example firmware dialect plugin
├── xtask/                           # Build automation (cargo xtask pattern)
│   └── src/main.rs
├── benches/                         # Workspace-level benchmarks
├── tests/                           # Integration tests
└── fuzz/                            # Fuzz testing targets
```

### Structure Rationale

- **Virtual manifest at root:** The workspace root contains only `Cargo.toml` (virtual manifest), `Cargo.lock`, and workspace-level config. No `src/` at root. This follows the flat-crate best practice from matklad's large workspace guidance and Bevy's architecture. Using `--workspace` by default is never needed; each crate is self-contained.

- **`crates/` directory with flat layout:** All crates live in `crates/` with their full name as the directory name (e.g., `crates/slicecore-geo/`, not `crates/geo/`). This matches the Cargo.toml dependency names exactly, simplifying navigation and renames. At ~20 crates, a flat layout is optimal; hierarchical grouping (e.g., `crates/layer0/`, `crates/layer2/`) adds indirection without real benefit at this scale.

- **`slicecore-plugin-api` as a separate crate:** This crate contains only the trait definitions and shared types that plugins implement. It has minimal dependencies (only `slicecore-math`, `slicecore-geo`, and `slicecore-config` types). Plugin authors depend on this crate, not the full engine. This is the "interface crate" from the `abi_stable` three-crate pattern and the WIT-equivalent for WASM plugins.

- **`bins/` separate from `crates/`:** Binary crates are distinct from library crates. The CLI and server are thin wrappers over `slicecore-engine` and `slicecore-api`.

- **`plugins/` for example plugins:** Ships example plugins that also serve as integration tests for the plugin system. These demonstrate the plugin API without being part of the core.

- **`xtask/` for automation:** Following the `cargo xtask` pattern (used by rust-analyzer, Bevy, and others), all build automation is written in Rust rather than scattered Makefiles or shell scripts. Tasks include: running benchmarks, generating golden files, checking feature flag combinations, and publishing crates.

---

## Architectural Patterns

### Pattern 1: Layered Dependency Graph with Strict Upward Rule

**What:** Crates are organized into numbered layers (0-5). A crate may only depend on crates in lower-numbered layers. No lateral dependencies within the same layer unless explicitly documented. No circular dependencies.

**When to use:** Always. This is the fundamental organizational principle.

**Trade-offs:**
- Pro: Compile-time enforcement (via `cargo deny` or custom CI check), fast incremental builds (changing a Layer 2 crate does not recompile Layer 0), clear mental model
- Con: Occasionally forces indirection (e.g., `slicecore-slicer` and `slicecore-perimeters` cannot directly share helpers; shared code must live in Layer 0/1)

**Enforcement:**

```toml
# In xtask: validate no upward dependencies
# Layer assignments encoded in crate metadata or xtask config
[layer_rules]
layer_0 = ["slicecore-math", "slicecore-geo", "slicecore-mesh"]
layer_1 = ["slicecore-fileio", "slicecore-gcode-io", "slicecore-config"]
layer_2 = ["slicecore-slicer", "slicecore-perimeters", "slicecore-infill", "slicecore-supports", "slicecore-pathing"]
layer_3 = ["slicecore-planner", "slicecore-gcode-gen", "slicecore-estimator"]
layer_4 = ["slicecore-ai", "slicecore-optimizer", "slicecore-analyzer"]
layer_5 = ["slicecore-engine", "slicecore-plugin", "slicecore-api"]
```

**Real-world precedent:** Bevy uses layered crate dependencies (render depends on ECS but not vice versa). rustc organizes compiler passes as a DAG where later phases depend on earlier ones.

### Pattern 2: Trait-Based Extension Points (Plugin Traits)

**What:** Core algorithmic behaviors (infill patterns, support strategies, G-code dialects, seam strategies) are defined as traits in `slicecore-plugin-api`. The engine dispatches to implementations via `Box<dyn Trait>`. Built-in algorithms implement these same traits -- there is no special path for built-in vs plugin code.

**When to use:** For any behavior the user should be able to replace or extend.

**Trade-offs:**
- Pro: Uniform interface for built-in and external code; testable via mock implementations; plugins are first-class citizens
- Con: Dynamic dispatch overhead (~1-3ns per call); trait objects cannot use generics (must be dyn-compatible); some API design constraints

**Example:**

```rust
// In slicecore-plugin-api/src/infill.rs

/// Extension point: Infill pattern generation
pub trait InfillPattern: Send + Sync {
    /// Unique identifier for this pattern
    fn id(&self) -> &str;

    /// Human-readable name
    fn name(&self) -> &str;

    /// Generate infill paths for a bounded region
    fn generate(
        &self,
        boundary: &[Polygon],
        config: &InfillConfig,
        layer: &LayerInfo,
    ) -> Result<Vec<ExtrusionSegment>, PluginError>;

    /// Optional: declare what configuration parameters this pattern accepts
    fn parameters(&self) -> Vec<ParameterDef> {
        Vec::new()
    }
}

// In slicecore-infill/src/rectilinear.rs (built-in implementation)
pub struct RectilinearInfill;

impl InfillPattern for RectilinearInfill {
    fn id(&self) -> &str { "rectilinear" }
    fn name(&self) -> &str { "Rectilinear" }
    fn generate(&self, boundary: &[Polygon], config: &InfillConfig, layer: &LayerInfo)
        -> Result<Vec<ExtrusionSegment>, PluginError>
    {
        // ... algorithm implementation ...
    }
}
```

**Real-world precedent:** Bevy's `Plugin` trait -- every subsystem (rendering, input, audio) implements the same `Plugin::build()` method. serde's `Serializer`/`Deserializer` traits define extension points that any format can implement.

### Pattern 3: Dual-Mode Plugin Loading (Native + WASM)

**What:** The plugin system supports two loading mechanisms behind a unified `PluginRegistry`:

1. **Native plugins** via `abi_stable` + `libloading`: Full performance, Rust-only, requires same `slicecore-plugin-api` version. Used for performance-critical plugins (infill patterns, perimeter algorithms) on native targets.

2. **WASM plugins** via `wasmtime` + Component Model: Sandboxed, language-agnostic (plugins can be written in Rust, C, or any language targeting WASM), safe for untrusted code. Used for community marketplace plugins, post-processors, and custom analyzers.

Both types register through the same `PluginRegistry` and implement the same trait interfaces (native traits for native; WIT-generated bindings for WASM that map to the same trait semantics).

**When to use:** Native for performance-critical, trusted plugins; WASM for untrusted, community, or cross-platform plugins.

**Trade-offs:**
- Pro: Best of both worlds -- performance where needed, safety where needed; WASM plugins work on all platforms including browsers (when run via browser WASM runtime)
- Con: Two loading paths to maintain; WIT interface definitions must be kept in sync with Rust traits; WASM plugins have memory copy overhead at the boundary

**Architecture:**

```rust
// In slicecore-plugin/src/registry.rs

pub struct PluginRegistry {
    infill_patterns: Vec<Box<dyn InfillPattern>>,
    support_strategies: Vec<Box<dyn SupportStrategy>>,
    gcode_dialects: Vec<Box<dyn GcodeDialect>>,
    // ...
}

impl PluginRegistry {
    /// Load a native plugin (.so/.dll/.dylib)
    #[cfg(feature = "plugin-native")]
    pub fn load_native(&mut self, path: &Path) -> Result<(), PluginError> {
        // Uses abi_stable for safe Rust-to-Rust FFI
        // Version check, layout verification, then register
    }

    /// Load a WASM plugin (.wasm)
    #[cfg(feature = "plugin-wasm")]
    pub fn load_wasm(&mut self, path: &Path, limits: ResourceLimits) -> Result<(), PluginError> {
        // Uses wasmtime Component Model
        // WIT bindings auto-generated; sandboxed execution
    }

    /// Register a built-in plugin (compiled in)
    pub fn register_builtin<P: InfillPattern + 'static>(&mut self, plugin: P) {
        self.infill_patterns.push(Box::new(plugin));
    }
}
```

**Real-world precedent:** Extism uses wasmtime for WASM plugin hosting with a similar host/guest separation. The Zed editor uses WASM extensions for language support plugins while keeping core editor logic native.

### Pattern 4: Compile-Time Feature Flags for Target Selection

**What:** Use Cargo feature flags to conditionally compile entire subsystems. WASM builds exclude AI, server, and native plugin loading. Minimal builds exclude everything except core slicing.

**When to use:** When the library targets multiple platforms (native, WASM, embedded) with different capability sets.

**Trade-offs:**
- Pro: Zero-cost exclusion (excluded code is never compiled); smaller binaries; no runtime branching
- Con: Combinatorial testing burden (must test all valid feature combinations); features must be additive (cannot use features to switch between implementations)

**Feature flag design:**

```toml
# Root workspace Cargo.toml or slicecore-engine/Cargo.toml

[features]
default = ["full"]

# Meta-features (profiles)
full = ["ai", "server", "plugin-native", "plugin-wasm", "python"]
core = []  # Pure slicing, no extras
wasm-compat = []  # Suitable for wasm32 targets

# Individual features
ai = ["dep:slicecore-ai"]
server = ["dep:slicecore-api", "dep:tokio", "dep:tonic"]
plugin-native = ["dep:abi_stable", "dep:libloading"]
plugin-wasm = ["dep:wasmtime"]
python = ["dep:pyo3"]
ffi = []
```

**Real-world precedent:** Bevy has 80+ feature-gated crates composed into profiles (`2d`, `3d`, `ui`). The `reqwest` crate uses features to switch between `native-tls` and `rustls` backends.

### Pattern 5: Pipeline Orchestration with Owned Stage Results

**What:** The slicing pipeline is a sequence of stages where each stage consumes data from previous stages and produces owned results. No shared mutable state between stages. The engine passes data forward through the pipeline; each stage receives owned or borrowed immutable inputs and returns owned outputs.

**When to use:** For the main slicing pipeline (mesh -> layers -> regions -> toolpaths -> planned moves -> G-code).

**Trade-offs:**
- Pro: No global state; each stage is independently testable; natural fit for Rust ownership; enables incremental re-slicing (cache stage outputs, invalidate downstream)
- Con: Large intermediate data structures must be moved (not a performance issue in practice due to move semantics)

**Example:**

```rust
// In slicecore-engine/src/pipeline.rs

pub struct SliceEngine {
    pool: rayon::ThreadPool,
    registry: PluginRegistry,
    progress: Arc<dyn ProgressReporter>,
    cancel: CancellationToken,
}

impl SliceEngine {
    pub fn slice(&self, job: SliceJob) -> Result<SliceResult, SliceCoreError> {
        self.pool.install(|| {
            // Stage 1: Load and repair mesh (owned TriangleMesh)
            let mesh = self.load_and_repair(&job)?;

            // Stage 2: Slice into layers (owned Vec<SliceLayer>)
            let layers = self.slice_layers(&mesh, &job.config)?;

            // Stage 3: Classify regions (owned Vec<ClassifiedLayer>)
            let classified = self.classify_regions(&layers, &job.config)?;

            // Stage 4: Generate toolpaths (owned Vec<LayerToolpath>)
            // Uses plugin registry for infill/support/perimeter selection
            let toolpaths = self.generate_toolpaths(&classified, &job.config)?;

            // Stage 5: Plan motion (owned Vec<PlannedLayer>)
            let planned = self.plan_motion(&toolpaths, &job.config)?;

            // Stage 6: Emit G-code (streaming, sequential)
            let gcode = self.emit_gcode(&planned, &job.config)?;

            Ok(SliceResult { gcode, metadata: self.compute_metadata(&planned) })
        })
    }
}
```

**Real-world precedent:** rustc's query system -- each compiler pass produces owned results cached by query key. Nushell's pipeline -- data flows through stages with ownership transfer.

### Pattern 6: Context/Builder Pattern for Dependency Injection

**What:** Instead of global state or a DI container, use explicit context structs and builders to inject dependencies. The `SliceEngine` is constructed with all its dependencies via a builder. Each subsystem receives a context struct containing only what it needs. No singletons, no global registries.

**When to use:** For all component initialization and for passing cross-cutting concerns (progress reporting, cancellation, logging) through the pipeline.

**Trade-offs:**
- Pro: Fully testable (inject mocks); no hidden dependencies; compile-time DI (no runtime overhead); clear dependency graph
- Con: Slightly more verbose initialization code; context structs must be designed carefully to avoid "god object" anti-pattern

**Example:**

```rust
// Builder for the engine
let engine = SliceEngine::builder()
    .thread_count(8)
    .progress_reporter(Arc::new(ConsoleProgress))
    .plugin_dir("/usr/share/slicecore/plugins")
    .memory_budget(MemoryBudget::default())
    .build()?;

// Context passed to subsystems
pub struct SlicingContext<'a> {
    pub config: &'a PrintConfig,
    pub progress: &'a dyn ProgressReporter,
    pub cancel: &'a CancellationToken,
    pub arena: &'a SlicingArena,
}
```

**Real-world precedent:** Axum's `State` extractor for dependency injection without globals. Bevy's `Res<T>` and `ResMut<T>` for explicit resource access instead of global state.

---

## Data Flow

### Primary Slicing Pipeline

```
[Input: Model File + Print Config]
    │
    ▼
[Stage 1: File I/O]
    Load STL/3MF/OBJ → TriangleMesh
    Repair: fix non-manifold, holes, normals
    Transform: scale, rotate, translate to bed
    │
    ▼ (owned TriangleMesh)
[Stage 2: Slicing]
    Build BVH spatial index on mesh
    For each Z height (parallel per-layer via rayon):
        Intersect mesh with Z plane → contour polygons
    Output: Vec<SliceLayer { z, contours, holes }>
    │
    ▼ (owned Vec<SliceLayer>)
[Stage 3: Region Classification]
    For each layer (parallel):
        Identify: outer perimeter, inner perimeters, top/bottom surfaces,
                  infill regions, bridges, overhangs
    Output: Vec<ClassifiedLayer { regions: Vec<(RegionType, Polygon)> }>
    │
    ▼ (owned Vec<ClassifiedLayer>)
[Stage 4: Toolpath Generation] ← Plugin extension points here
    For each layer (parallel):
        Perimeters: offset inward, generate wall paths
        Infill: select pattern (via InfillPattern plugin), fill regions
        Supports: generate support structures (via SupportStrategy plugin)
    Output: Vec<LayerToolpath { segments, travels, retractions }>
    │
    ▼ (owned Vec<LayerToolpath>)
[Stage 5: Motion Planning]
    For each layer (parallel):
        Assign speeds per feature type
        Compute acceleration/deceleration
        Plan retractions and wipe moves
        Apply cooling (fan speed per segment)
        Temperature changes
    Output: Vec<PlannedLayer { moves: Vec<PlannedMove> }>
    │
    ▼ (owned Vec<PlannedLayer>)
[Stage 6: G-code Generation] ← Plugin extension point (GcodeDialect)
    Sequential (ordered output required):
        Emit start G-code (firmware-specific)
        For each layer:
            Emit Z move
            For each planned move:
                Format as G-code line (dialect-specific)
        Emit end G-code
    Output: G-code bytes + metadata JSON + analytics JSON
    │
    ▼
[Output: G-code File + Structured Metadata]
```

### Cross-Cutting Data Flows

1. **Progress reporting:** Every stage reports progress to the `ProgressReporter` trait object (injected at engine construction). Consumers (CLI, GUI, server) implement this trait for their display needs. Flow is unidirectional: stages -> reporter.

2. **Cancellation:** A `CancellationToken` (atomically-checked flag) is passed to every stage. Parallel iterators check `is_cancelled()` periodically. On cancellation, stages return `Err(SliceCoreError::Cancelled)`. Flow is unidirectional: caller -> stages.

3. **Warnings:** Non-fatal issues (thin walls, excessive overhangs, unsupported features) are collected into a `Vec<SliceWarning>` using a thread-safe collector. Flow is unidirectional: stages -> warning collector -> final report.

4. **Plugin invocation:** The engine holds a `PluginRegistry`. When a stage needs a specific capability (e.g., "generate gyroid infill"), it looks up the registered plugin by ID and invokes it. Data flows: stage -> plugin (input polygons + config) -> stage (output segments).

### State Management: No Global State

The C++ LibSlic3r suffers from pervasive global state (print config as global, model state as global). LibSlic3r-RS eliminates this entirely:

| C++ Pattern | Rust Replacement |
|-------------|-----------------|
| Global `PrintConfig` | Owned `PrintConfig` in `SliceJob`, passed by reference to stages |
| Global `Model` singleton | Owned `TriangleMesh` passed through pipeline stages |
| Global `Print` object with mutable state | Immutable stage results; new owned data at each stage |
| Thread-local state in TBB tasks | `rayon` closures capture only what they need (borrow checker enforces) |
| Mutable static variables for caching | Per-engine `PluginRegistry` and per-slice arena |

---

## Crate Dependency Graph

The dependency graph is strictly acyclic. Layer N crates depend only on Layer 0..N-1 crates.

```
slicecore-math ─────────────────────────────────────────────────┐
    │                                                           │
    ▼                                                           │
slicecore-geo ──────────────────────────────────────────────┐   │
    │                                                       │   │
    ▼                                                       │   │
slicecore-mesh ─────────────────────────────────────────┐   │   │
    │                                                   │   │   │
    ├──────────────────────────────┐                    │   │   │
    ▼                              ▼                    │   │   │
slicecore-fileio            slicecore-config ◄──────────┼───┼───┘
    │                              │                    │   │
    │    ┌─────────────────────────┤                    │   │
    │    │    ┌────────────────────┤                    │   │
    ▼    ▼    ▼                    ▼                    ▼   ▼
slicecore-slicer ◄─── slicecore-perimeters ◄─── slicecore-infill
    │                      │                        │
    │                      │                        │
    ▼                      ▼                        ▼
slicecore-supports    slicecore-pathing       (all Layer 2 depend
    │                      │                  on geo + mesh)
    ▼                      ▼
slicecore-planner ◄──── slicecore-gcode-gen ◄── slicecore-estimator
    │                      │                        │
    ▼                      ▼                        ▼
slicecore-analyzer   slicecore-optimizer    slicecore-ai (optional)
    │                      │                        │
    ▼                      ▼                        ▼
slicecore-engine ◄──── slicecore-plugin ──── slicecore-api
```

**Special crate: `slicecore-plugin-api`**

This crate sits outside the layer hierarchy. It contains only trait definitions and shared types. It depends on the absolute minimum (types from `slicecore-math`, `slicecore-geo`, and `slicecore-config`). Both the engine and plugin implementations depend on it, but it creates no circular dependency because it contains no implementation logic.

```
slicecore-plugin-api
    ├── depends on: slicecore-math (types only)
    ├── depends on: slicecore-geo (Polygon, Point2 types)
    ├── depends on: slicecore-config (InfillConfig, PrintConfig types)
    │
    ├── consumed by: slicecore-engine (to invoke plugins)
    ├── consumed by: slicecore-plugin (to load/wrap plugins)
    ├── consumed by: slicecore-infill (built-in patterns implement its traits)
    ├── consumed by: slicecore-supports (built-in strategies implement its traits)
    ├── consumed by: plugins/* (external plugins implement its traits)
    └── consumed by: slicecore-gcode-gen (built-in dialects implement its traits)
```

---

## Plugin Architecture Design

### Plugin Lifecycle

```
[Discovery] → [Loading] → [Initialization] → [Invocation] → [Shutdown]
     │              │              │                │              │
     │              │              │                │              │
  Scan dirs     Native: abi_stable  Plugin::init()   Trait method    Plugin::shutdown()
  Read manifests  load + verify    with context      calls via       Drop impl
  Check version  WASM: wasmtime                     Box<dyn Trait>   Unload library
  compatibility   compile + inst.
```

### Plugin Manifest (`plugin.toml`)

Every plugin ships with a manifest declaring its capabilities:

```toml
[plugin]
name = "gyroid-infill"
version = "1.2.0"
description = "Gyroid infill pattern with variable density support"
authors = ["Plugin Author <author@example.com>"]
license = "MIT"
min_engine_version = "0.5.0"

[capabilities]
extension_points = ["InfillPattern"]
# What the plugin needs from the host
requires = ["read_polygon_data", "read_config"]
# What the plugin must not have access to
denies = ["filesystem", "network"]

[native]
# For native plugins: shared library name
library = "libgyroid_infill"

[wasm]
# For WASM plugins: component file
component = "gyroid_infill.wasm"
memory_limit = "32MiB"
cpu_timeout = "10s"
```

### WIT Interface Definition (for WASM plugins)

```wit
// slicecore-plugin.wit

package slicecore:plugin@0.1.0;

interface types {
    record point2 { x: f64, y: f64 }
    record polygon { points: list<point2> }
    record extrusion-segment {
        start: point2,
        end: point2,
        width: f64,
        height: f64,
        flow-rate: f64,
    }
    record infill-config {
        density: f64,
        angle: f64,
        line-spacing: f64,
    }
    record layer-info {
        z: f64,
        layer-height: f64,
        layer-index: u32,
    }
}

interface infill-pattern {
    use types.{polygon, extrusion-segment, infill-config, layer-info};

    id: func() -> string;
    name: func() -> string;
    generate: func(
        boundary: list<polygon>,
        config: infill-config,
        layer: layer-info,
    ) -> result<list<extrusion-segment>, string>;
}

world infill-plugin {
    export infill-pattern;
}
```

### Native vs WASM Plugin Comparison

| Aspect | Native (`abi_stable`) | WASM (Component Model) |
|--------|-----------------------|------------------------|
| Performance | Near-native (~3-4ns dispatch overhead) | 10-100x slower due to sandbox + serialization |
| Safety | Same address space; crash can kill host | Fully sandboxed; crash contained |
| Language | Rust only (same `abi_stable` version) | Any language targeting WASM |
| Trust level | Trusted (first-party, vetted) | Untrusted (community, marketplace) |
| Platform | Native targets only | All targets including browsers |
| Resource limits | OS-level only | Fine-grained (memory, CPU, capabilities) |
| Use cases | Core infill patterns, perimeter algorithms | Community patterns, post-processors, analyzers |
| Build complexity | Same Rust toolchain | Requires WASI SDK or `cargo component` |

**Recommendation:** Use native plugins for the built-in algorithm library (performance-critical code paths like infill and perimeters). Use WASM plugins for the community marketplace and for any untrusted extensions. Both register through the same `PluginRegistry` and implement the same logical interface.

### Plugin Registration Pattern

```rust
// Engine initialization wires up built-in + loaded plugins

let mut registry = PluginRegistry::new();

// Built-in plugins (compiled in, zero overhead)
registry.register_infill(Box::new(RectilinearInfill));
registry.register_infill(Box::new(GridInfill));
registry.register_infill(Box::new(GyroidInfill));
// ... all 24+ built-in patterns

registry.register_dialect(Box::new(MarlinDialect));
registry.register_dialect(Box::new(KlipperDialect));

// Native plugins (from plugin directory)
#[cfg(feature = "plugin-native")]
for path in discover_native_plugins("/usr/share/slicecore/plugins")? {
    registry.load_native(&path)?;
}

// WASM plugins (from plugin directory)
#[cfg(feature = "plugin-wasm")]
for path in discover_wasm_plugins("/usr/share/slicecore/plugins")? {
    registry.load_wasm(&path, ResourceLimits::default())?;
}

// Engine uses registry to dispatch
let engine = SliceEngine::builder()
    .registry(registry)
    .build()?;
```

---

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Single model, desktop | Default config; `rayon` thread pool sized to physical cores; all data in RAM |
| Batch CLI (100s of models) | Sequential model processing; share thread pool; per-model arena reset; stream G-code to disk |
| Cloud server (concurrent requests) | Per-request `SliceEngine` instances; `tokio` for async I/O + `rayon` for compute; request queuing with backpressure |
| WASM in browser | Single-threaded (no rayon); streaming output; reduced feature set; `wasm-compat` feature flag |
| Print farm (1000s of jobs/day) | Job queue (Redis/NATS); horizontal scaling of stateless slice workers; shared config store; results to object storage |

### Scaling Priorities

1. **First bottleneck (polygon clipping):** At ~1,425 Clipper call sites in the C++ reference, polygon boolean and offset operations dominate slicing time. The `slicecore-geo` crate must be fast. Use `i-overlay` for boolean ops (pure Rust, outperforms Clipper2 in benchmarks). Arena allocation for intermediates. Polygon caching for prismatic models.

2. **Second bottleneck (memory for large models):** Models with 10M+ triangles can consume gigabytes. Use `mmap` for large STL files, BVH spatial indexing to avoid loading all triangles, and streaming layer processing (process and discard layers top-to-bottom if not needed for cross-layer analysis).

3. **Third bottleneck (parallel efficiency):** The C++ codebase has 47+ `parallel_for` sites with varying granularity. Not all are worth parallelizing in Rust. Profile first; parallelize only stages where wall-clock time > 100ms on typical models. `rayon`'s work-stealing handles load imbalance automatically.

---

## Anti-Patterns

### Anti-Pattern 1: Global Mutable State

**What people do:** Store configuration, model state, or caches in `static mut` variables or `lazy_static` singletons, replicating the C++ pattern.

**Why it's wrong:** Prevents concurrent slicing of multiple models; makes testing require global setup/teardown; creates hidden dependencies between modules; Rust's borrow checker cannot protect global state (requires `unsafe` or `Mutex`, both of which mask the architectural problem).

**Do this instead:** Own all state in the `SliceEngine` instance. Pass config and context as function parameters or struct fields. Use per-engine caches. Each `SliceEngine` is independent; you can run multiple simultaneously without conflict.

### Anti-Pattern 2: God Trait (One Trait to Rule Them All)

**What people do:** Define a single `Plugin` trait with methods for every extension point, requiring plugins to implement stubs for capabilities they don't provide.

**Why it's wrong:** Violates Interface Segregation Principle; bloats the vtable; makes it impossible to statically know a plugin's capabilities; forces version breaks when any extension point changes.

**Do this instead:** Separate traits per extension point (`InfillPattern`, `SupportStrategy`, `GcodeDialect`, etc.). A plugin crate can implement one or many. The base `Plugin` trait provides only metadata and lifecycle; extension traits are independent.

### Anti-Pattern 3: Feature Flags as Implementation Switches

**What people do:** Use feature flags to switch between two implementations of the same functionality (`#[cfg(feature = "fast-mode")]` changes an algorithm).

**Why it's wrong:** Features must be additive per Cargo convention. Non-additive features create an exponential testing matrix and can cause compilation failures when combined.

**Do this instead:** Use runtime configuration or generic type parameters for implementation selection. Feature flags should gate *presence* of functionality (include/exclude a crate), not *behavior* of functionality.

### Anti-Pattern 4: Premature Crate Splitting

**What people do:** Split every module into its own crate from day one, creating 50+ crates before the API is stable.

**Why it's wrong:** Unstable APIs between crates cause version churn; moving types between crates is a breaking change; compile-time overhead from excessive crate boundaries; cognitive load of navigating many small crates.

**Do this instead:** Start with the layered structure but keep early crates coarser. For example, `slicecore-geo` can initially contain both polygon booleans and offsetting. Split into `slicecore-geo-bool` and `slicecore-geo-offset` only when there's a concrete need (e.g., a crate needs offsetting but not booleans). The design doc's 20-crate structure is the *target*, not the starting point.

### Anti-Pattern 5: Blocking on AI in the Slicing Pipeline

**What people do:** Make AI calls (LLM inference, model analysis) synchronous blocking calls within the slicing pipeline, causing the entire slice to wait for network round-trips.

**Why it's wrong:** AI services have unpredictable latency (100ms to 30s); network failures should not fail a slice; slicing must work offline.

**Do this instead:** AI is always optional and asynchronous. The analyzer produces structured data that *can* be fed to an AI, but the pipeline never blocks on AI. AI suggestions are pre-computed before slicing begins, or applied as post-processing. The `slicecore-ai` crate is feature-gated and excluded from WASM builds.

---

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| AI providers (OpenAI, Anthropic, Ollama) | `AiProvider` trait in `slicecore-ai`; async HTTP clients | Feature-gated; provider-agnostic; user provides API keys |
| Printer firmware (Marlin, Klipper, RRF) | `GcodeDialect` trait in `slicecore-gcode-gen` | Each firmware is a plugin implementation; quirks database in TOML |
| OctoPrint / Moonraker | REST API client in `slicecore-api` | For job submission; not in core library (consumer concern) |
| File formats (3MF via lib3mf-core) | Direct dependency in `slicecore-fileio` | `lib3mf-core` already on crates.io |
| WASM runtime (wasmtime) | Embedded in `slicecore-plugin` | Feature-gated (`plugin-wasm`); excluded from WASM target builds |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| `slicecore-engine` <-> `slicecore-plugin` | `PluginRegistry` owned by engine; plugins invoked via trait objects | Synchronous invocation; plugin returns owned data |
| `slicecore-engine` <-> consumers (CLI/GUI/server) | `SliceJob` in, `SliceResult` out; `ProgressReporter` callbacks | Engine is a pure library; no event loop, no async runtime |
| `slicecore-api` (server) <-> `slicecore-engine` | `tokio::spawn_blocking` wraps synchronous engine calls | Async server dispatches to sync compute via blocking thread pool |
| Pipeline stages <-> each other | Owned data transfer (return value of one stage = input to next) | No shared mutable state; borrow checker enforces stage isolation |
| WASM plugins <-> host | Component Model calls; data serialized across sandbox boundary | WIT-generated bindings provide type safety; memory isolation |

---

## Suggested Build Order

Build crates in this order. Each step produces a testable, usable artifact before proceeding.

### Phase 1: Foundation (implement first)

1. **`slicecore-math`** -- Coordinate types, integer/float conversion, basic vector math. ~1 week. No dependencies. This unblocks everything.

2. **`slicecore-geo`** -- Polygon type, boolean operations (via `i-overlay`), offset, area, point-in-polygon. ~3-4 weeks. Depends on `slicecore-math`. This is the most critical crate; every algorithm crate depends on it.

3. **`slicecore-mesh`** -- `TriangleMesh`, BVH construction, mesh-plane intersection. ~2-3 weeks. Depends on `slicecore-math` and `slicecore-geo`.

### Phase 2: Vertical Slice (prove the architecture)

4. **`slicecore-config`** -- Settings schema, validation, profile loading. ~2 weeks. Depends on `slicecore-math`. Start with minimal settings; expand as algorithm crates need them.

5. **`slicecore-fileio`** -- STL parser (binary + ASCII), basic 3MF via `lib3mf-core`. ~2 weeks. Depends on `slicecore-mesh`.

6. **`slicecore-slicer`** -- Contour extraction from mesh at Z heights. ~2-3 weeks. Depends on `slicecore-mesh` + `slicecore-geo`. This is the core algorithm.

7. **`slicecore-perimeters`** -- Wall generation via polygon offsetting. ~2-3 weeks. Depends on `slicecore-geo`.

8. **`slicecore-infill`** -- Rectilinear pattern only (simplest pattern). ~1-2 weeks. Depends on `slicecore-geo`. Define the `InfillPattern` trait here.

9. **`slicecore-gcode-gen`** -- Basic Marlin G-code output. ~1-2 weeks. Depends on `slicecore-math`. Define `GcodeDialect` trait.

10. **`slicecore-engine`** -- Wire stages 5-9 together. ~1-2 weeks. This is the first end-to-end slice: STL in, G-code out.

11. **CLI binary** -- Thin wrapper: parse args, call engine, write output. ~1 week.

**Milestone: "Hello World" slice -- a calibration cube from STL to G-code.**

### Phase 3: Correctness and Completeness

12. **`slicecore-planner`** -- Speed, acceleration, retraction, cooling planning.
13. **`slicecore-pathing`** -- Travel optimization, seam placement.
14. **`slicecore-supports`** -- Basic line supports; define `SupportStrategy` trait.
15. **`slicecore-estimator`** -- Time and material estimation.
16. **`slicecore-gcode-io`** -- G-code parser for validation and analysis.
17. **Golden file test infrastructure** -- Deterministic output verification.

### Phase 4: Plugin System and Extensibility

18. **`slicecore-plugin-api`** -- Extract traits into standalone crate.
19. **`slicecore-plugin`** -- Native loading (abi_stable) + WASM loading (wasmtime).
20. **Additional infill patterns** -- Gyroid, honeycomb, cubic, etc. as plugins.
21. **Additional firmware dialects** -- Klipper, RepRapFirmware as plugins.

### Phase 5: Intelligence and API (feature-gated)

22. **`slicecore-analyzer`** -- Model analysis, printability scoring.
23. **`slicecore-ai`** -- AI provider abstraction.
24. **`slicecore-optimizer`** -- Parameter search.
25. **`slicecore-api`** -- REST/gRPC server, Python bindings.

---

## Examples from Real Rust Projects

### Bevy Engine (game engine, ~80 crates)

- **Pattern adopted:** Hierarchical feature flags; `Plugin` trait for modular composition; `App` builder for initialization; virtual manifest workspace; crates in `crates/` directory.
- **Key insight:** Bevy proves that 80+ crates in a single workspace works well when there is a clear dependency DAG and feature flags control composition.
- **Relevance:** LibSlic3r-RS can use the same `Plugin::build()` pattern for registering algorithms with the engine, and the same feature-flag profiles for target selection.
- **Source:** [Bevy Core Architecture](https://deepwiki.com/bevyengine/bevy/1.1-core-architecture-and-project-structure)

### rust-analyzer (IDE, ~200K lines, flat workspace)

- **Pattern adopted:** Flat crate layout in `crates/` with full crate names as directory names; virtual manifest; `cargo xtask` for automation; versioned internal crates at `0.0.0`.
- **Key insight:** The flat layout scales to hundreds of thousands of lines. Hierarchical grouping adds indirection without benefit until you exceed ~100 crates.
- **Relevance:** LibSlic3r-RS should follow this flat layout for its ~20 crates.
- **Source:** [Large Rust Workspaces](https://matklad.github.io/2021/08/22/large-rust-workspaces.html)

### Extism (WASM plugin framework)

- **Pattern adopted:** Host/guest separation; wasmtime as runtime; manifest-driven plugin discovery; resource limits; cross-language plugin support.
- **Key insight:** The WASM Component Model with WIT definitions provides type-safe, language-agnostic plugin interfaces that are far superior to raw function exports.
- **Relevance:** LibSlic3r-RS should use the Component Model (not raw WASM modules) for its WASM plugin system, and Extism's architecture validates the approach.
- **Source:** [Extism Plugin System](https://extism.org/docs/concepts/plug-in-system/)

### `abi_stable` (Rust-to-Rust FFI)

- **Pattern adopted:** Three-crate pattern (interface/implementation/user); `StableAbi` derive for layout verification; `#[repr(C)]` types; prefix types for forward compatibility.
- **Key insight:** Native Rust plugins are viable with abi_stable's automatic layout checking, but plugins must be rebuilt when the interface crate's minor version changes. The `AbortBomb` pattern prevents panic propagation across FFI.
- **Relevance:** LibSlic3r-RS should use abi_stable for its native plugin path, with `slicecore-plugin-api` as the interface crate.
- **Source:** [NullDeref: Plugins in Rust with abi_stable](https://nullderef.com/blog/plugin-abi-stable/)

### Zed Editor (WASM extensions)

- **Pattern adopted:** Language extensions as WASM plugins; core editor remains native Rust; extensions loaded on demand; well-defined extension API.
- **Key insight:** Real-world validation that WASM plugins are practical for extending Rust applications. Performance-critical code stays native; extensibility happens through WASM.
- **Relevance:** Same pattern for LibSlic3r-RS: core slicing is native Rust for performance; community extensions (custom infill, analyzers, post-processors) run in WASM sandbox.

### GladiusSlicer (Rust 3D slicer, alpha)

- **Pattern adopted:** Rust-native slicer with modular design.
- **Key insight:** Validates that a Rust-based slicer is feasible and can produce printable G-code. Currently alpha-quality, which underscores the complexity of the domain.
- **Relevance:** Reference implementation for Rust-specific slicing patterns, though LibSlic3r-RS targets a much larger scope.
- **Source:** [GladiusSlicer GitHub](https://github.com/GladiusSlicer/GladiusSlicer)

---

## Sources

- [Bevy Core Architecture and Project Structure](https://deepwiki.com/bevyengine/bevy/1.1-core-architecture-and-project-structure) -- HIGH confidence (official Bevy wiki)
- [Large Rust Workspaces (matklad)](https://matklad.github.io/2021/08/22/large-rust-workspaces.html) -- HIGH confidence (author is rust-analyzer lead)
- [Plugins in Rust (Michael F Bryan)](https://adventures.michaelfbryan.com/posts/plugins-in-rust) -- HIGH confidence (comprehensive reference with code)
- [Plugins in Rust: abi_stable (NullDeref)](https://nullderef.com/blog/plugin-abi-stable/) -- HIGH confidence (detailed benchmarks and code)
- [Building Native Plugin Systems with WebAssembly Components (Sy Brand)](https://tartanllama.xyz/posts/wasm-plugins/) -- HIGH confidence (detailed implementation guide)
- [Extism Plugin System](https://extism.org/docs/concepts/plug-in-system/) -- MEDIUM confidence (official docs)
- [WASM Component Model in Rust (Medium)](https://autognosi.medium.com/rust-wa-component-model-serverless-plugin-system-in-245-lines-with-container-optimization-b4312b1187a0) -- MEDIUM confidence (recent 2026 article)
- [Cargo Features Documentation](https://doc.rust-lang.org/cargo/reference/features.html) -- HIGH confidence (official Rust docs)
- [Effective Rust: Be Wary of Feature Creep](https://www.lurklurk.org/effective-rust/features.html) -- HIGH confidence (published Rust reference)
- [Rust Trait Objects (official book)](https://doc.rust-lang.org/book/ch18-02-trait-objects.html) -- HIGH confidence (official)
- [Global State in Rust (Michael F Bryan)](https://adventures.michaelfbryan.com/posts/pragmatic-global-state) -- HIGH confidence
- [GladiusSlicer](https://github.com/GladiusSlicer/GladiusSlicer) -- MEDIUM confidence (alpha-state reference)

---
*Architecture research for: libslic3r-rs modular Rust slicing library*
*Researched: 2026-02-14*
