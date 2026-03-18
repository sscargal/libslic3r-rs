# Architecture

**Analysis Date:** 2026-03-18

## Pattern Overview

**Overall:** Layered Rust workspace with domain-separated crates

**Key Characteristics:**
- 15-crate Cargo workspace where each crate owns one domain (math, geometry, mesh, slicing, engine, I/O, plugins, AI)
- Strict dependency layering: foundation crates have no internal deps; upper crates compose lower ones
- Plugin system uses a three-crate architecture (api / host / plugin-impl) with both native (ABI-stable FFI) and WASM backends
- Feature flags (`plugins`, `ai`, `arrange`, `parallel`) allow opt-in capabilities at the engine level
- WASM-compatible by design: no `std::time::Instant` on wasm32, pure-Rust I/O libs, cfg-gated `rayon`

## Layers

**Foundation Math Layer:**
- Purpose: Shared primitive types for all geometric operations
- Location: `crates/slicecore-math/`
- Contains: `Point2`, `Point3`, `Vec2`, `Vec3`, `BBox2/3`, `Matrix3x3/4x4`, `IPoint2` (integer coords), `Coord`
- Depends on: `serde`, `approx` only
- Used by: every other crate in the workspace
- Notes: Two coordinate spaces — float (mm) and integer (nanometers, 1mm = 1_000_000 units)

**Geometry Layer:**
- Purpose: 2D polygon operations used throughout the slicing pipeline
- Location: `crates/slicecore-geo/`
- Contains: `Polygon`, `ValidPolygon` (two-tier validation), boolean ops, offsetting, simplification, convex hull
- Depends on: `slicecore-math`, `clipper2-rust`
- Used by: `slicecore-slicer`, `slicecore-engine`, `slicecore-arrange`

**Mesh Layer:**
- Purpose: 3D triangle mesh representation with spatial queries
- Location: `crates/slicecore-mesh/`
- Contains: `TriangleMesh` (arena+index: `Vec<Point3>` + `Vec<[u32;3]>`), lazy SAH-BVH, CSG boolean ops, mesh repair, transforms
- Depends on: `slicecore-math`, `robust`, optional `rayon`
- Used by: `slicecore-slicer`, `slicecore-fileio`, `slicecore-render`, `slicecore-engine`

**Slicer Layer:**
- Purpose: Triangle-plane intersection to produce layer contours
- Location: `crates/slicecore-slicer/`
- Contains: `slice_mesh`, `slice_at_height`, `compute_layer_heights`, adaptive layer height, contour chaining
- Depends on: `slicecore-math`, `slicecore-mesh`, `slicecore-geo`
- Used by: `slicecore-engine`

**I/O Layer:**
- Purpose: File format parsers and G-code output
- Location: `crates/slicecore-fileio/`, `crates/slicecore-gcode-io/`
- `slicecore-fileio` Contains: STL (binary + ASCII), 3MF, OBJ parsers and exporters; `load_mesh` auto-detects format
- `slicecore-gcode-io` Contains: structured `GcodeCommand` enum, dialect-aware `GcodeWriter` (Marlin, Klipper, RepRap, Bambu), arc fitting, validation
- Depends on: `slicecore-math`, `slicecore-mesh`, `tobj`, `lib3mf-core`, `lib3mf-converters`, `byteorder`
- Used by: `slicecore-engine`, `slicecore-cli`

**Config Layer:**
- Purpose: Setting schema types, derive macro, and registry
- Location: `crates/slicecore-config-schema/`, `crates/slicecore-config-derive/`
- Contains: `SettingDefinition`, `SettingRegistry`, `HasSettingSchema` trait, JSON Schema generation, `#[derive(ConfigSchema)]` proc-macro
- Depends on: `serde`, `syn`/`quote` (proc-macro only)
- Used by: `slicecore-engine`

**Engine Layer:**
- Purpose: Full slicing pipeline orchestrator
- Location: `crates/slicecore-engine/`
- Contains: `Engine` struct (single entry point), `PrintConfig`, all pipeline stages (perimeter, infill, support, toolpath, gcode_gen, planner), profile management (import, library, compose, resolve), statistics, multi-material, sequential printing, event system
- Depends on: all lower layers plus optional `slicecore-ai`, `slicecore-arrange`, `slicecore-plugin`
- Used by: `slicecore-cli`
- Feature flags: `plugins`, `ai`, `arrange`, `parallel`

**Plugin Layer:**
- Purpose: Extensible infill patterns and G-code post-processors
- Location: `crates/slicecore-plugin-api/`, `crates/slicecore-plugin/`
- `slicecore-plugin-api`: FFI-safe types (`RVec`, `RString`, `abi_stable::StableAbi`), `InfillPatternPlugin` and `GcodePostProcessorPlugin` traits, WASM WIT bindings
- `slicecore-plugin`: `PluginRegistry`, discovery, native loader (ABI-stable cdylib), WASM loader (wasmtime component model), sandbox config
- Depends on: `abi_stable`, `wasmtime` (optional), `semver`, `toml`
- Used by: `slicecore-engine` (via feature flag), `slicecore-cli`

**AI Layer:**
- Purpose: LLM-based geometry analysis and print profile suggestions
- Location: `crates/slicecore-ai/`
- Contains: `AiProvider` async trait, providers (OpenAI, Anthropic, Ollama), `AiConfig`, geometry feature extraction, profile suggestion, secure API key storage (`secrecy`)
- Depends on: `slicecore-mesh`, `slicecore-math`, `reqwest`, `tokio`, `async-trait`, `secrecy`
- Used by: `slicecore-engine` (feature `ai`), `slicecore-cli`

**Utility Crates:**
- `crates/slicecore-arrange/`: Build plate auto-arrangement with material grouping, sequential mode, gantry clearance
- `crates/slicecore-render/`: CPU software rasterizer (orthographic + z-buffer), PNG encoding for thumbnails

**CLI Layer:**
- Purpose: User-facing binary and subcommand dispatch
- Location: `crates/slicecore-cli/`
- Contains: `slicecore` binary, subcommands (slice, validate, analyze, ai-suggest, csg, arrange, schema, etc.)
- Depends on: all crates (the CLI is the top-level consumer)

## Data Flow

**Primary Slice Flow:**

1. CLI reads mesh file bytes → `slicecore_fileio::load_mesh` → `TriangleMesh`
2. CLI loads/builds `PrintConfig` (TOML files, profile library, overrides)
3. `Engine::slice(mesh, config, writer, opts)` is called
4. `slicecore_slicer::slice_mesh` intersects triangles with Z planes → `Vec<SliceLayer>`
5. Per layer (optionally parallel via `rayon`): perimeters, surface classification, infill, gap fill, ironing, support, toolpath assembly
6. First-layer extras: skirt/brim prepended to layer 0
7. `gcode_gen::generate_full_gcode` converts `LayerToolpath` → `Vec<GcodeCommand>`
8. `GcodeWriter` serializes to bytes with dialect-aware start/end sequences
9. `SliceResult` returned with G-code bytes, layer count, time estimate, filament usage, statistics, preview

**Plugin Dispatch Flow:**

1. `PluginRegistry::discover_and_load(dir)` scans for `plugin.toml` manifests
2. Native plugins: `abi_stable::load_root_module_in_file` verifies ABI then calls `generate()`
3. WASM plugins: `wasmtime` instantiates component, CPU fuel/memory limits enforced
4. Engine checks `InfillPattern::Plugin(name)` before calling `generate_infill`, routes to registry

**AI Suggestion Flow:**

1. `extract_geometry_features(mesh)` computes dimensions, volume, overhangs, thin walls
2. `build_profile_prompt(features)` creates structured LLM prompt
3. Provider sends HTTP request (Ollama/OpenAI/Anthropic)
4. Response JSON parsed by `parse_profile_suggestion` into `ProfileSuggestion`

**State Management:**
- No global mutable state except `LazyLock<SettingRegistry>` singleton in `slicecore-engine`
- `CancellationToken` uses `Arc<AtomicBool>` for thread-safe cooperative cancellation
- `EventBus` pattern with `EventSubscriber` trait for slice progress events
- `TriangleMesh` BVH is built lazily via `OnceLock` (thread-safe, built once)

## Key Abstractions

**TriangleMesh:**
- Purpose: Core 3D model representation
- Examples: `crates/slicecore-mesh/src/triangle_mesh.rs`
- Pattern: Arena+index (`Vec<Point3>` vertices, `Vec<[u32;3]>` triangle indices), lazy BVH

**PrintConfig:**
- Purpose: All slicing parameters in one serializable struct
- Examples: `crates/slicecore-engine/src/config.rs`
- Pattern: Nested structs with `#[serde(default)]`, `#[derive(ConfigSchema)]` for setting metadata

**ValidPolygon:**
- Purpose: Geometric invariant enforcement at type level
- Examples: `crates/slicecore-geo/src/polygon.rs`
- Pattern: Two-tier — `Polygon` (raw, mutable) → `Polygon::validate()` → `ValidPolygon` (guaranteed non-degenerate, known winding)

**Engine:**
- Purpose: Single entry point for the slicing pipeline
- Examples: `crates/slicecore-engine/src/engine.rs`
- Pattern: Takes `TriangleMesh + PrintConfig`, produces `SliceResult` with G-code bytes

**InfillPatternPlugin / GcodePostProcessorPlugin:**
- Purpose: FFI-safe extension points for custom behaviors
- Examples: `crates/slicecore-plugin-api/src/traits.rs`
- Pattern: `abi_stable::sabi_trait` macro generates vtable-based trait objects safe across dylib boundaries

**SettingRegistry:**
- Purpose: Runtime metadata for all `PrintConfig` fields (display names, constraints, dependencies)
- Examples: `crates/slicecore-engine/src/lib.rs` (global singleton), `crates/slicecore-config-schema/src/registry.rs`
- Pattern: `LazyLock` singleton populated via `#[derive(ConfigSchema)]`-generated code

## Entry Points

**CLI Binary:**
- Location: `crates/slicecore-cli/src/main.rs`
- Triggers: `cargo run --bin slicecore` or installed binary
- Responsibilities: Subcommand parsing via `clap`, orchestrating all user-facing workflows

**Engine::slice:**
- Location: `crates/slicecore-engine/src/engine.rs`
- Triggers: Called by CLI `slice_workflow.rs` or any library consumer
- Responsibilities: Full pipeline from `TriangleMesh + PrintConfig` to G-code bytes

**Plugin Entry (Native):**
- Location: Each plugin exposes `#[export_root_module]` returning `InfillPluginMod_Ref`
- Examples: `plugins/examples/native-zigzag-infill/`
- Responsibilities: Implement `InfillPatternPlugin::generate(&InfillRequest) -> InfillResult`

**Plugin Entry (WASM):**
- Location: Each plugin implements WIT `Guest` trait
- Examples: `plugins/examples/wasm-spiral-infill/`
- WIT definition: `crates/slicecore-plugin/wit/slicecore-plugin.wit`
- Responsibilities: Compiled to `wasm32-wasip2`, loaded by wasmtime component model

## Error Handling

**Strategy:** `thiserror`-derived enums at each crate boundary; `anyhow` only in the CLI binary

**Patterns:**
- Each crate defines its own `Error` enum (e.g., `MeshError`, `GeoError`, `FileIOError`, `EngineError`, `PluginSystemError`, `AiError`, `ArrangeError`)
- Errors propagate upward via `?` with `#[from]` for automatic conversions at crate boundaries
- `EngineError::Cancelled` for cooperative cancellation via `CancellationToken`
- Plugin errors wrapped as `EngineError::Plugin { plugin, message }` at engine boundary
- `anyhow::Result` used throughout `slicecore-cli` for ergonomic error display

## Cross-Cutting Concerns

**Logging:** No logging framework — CLI uses `eprintln!` and `indicatif` progress bars; engine emits `SliceEvent` via `EventBus`
**Validation:** `PrintConfig` validated via `config_validate::validate_config` returning `Vec<ValidationIssue>`; polygon invariants enforced by `ValidPolygon`
**Authentication:** AI API keys stored as `secrecy::SecretString`; redacted from `Debug` output
**Parallelism:** Optional `rayon` feature in `slicecore-engine` and `slicecore-mesh`; per-layer processing uses `maybe_par_iter` wrapper that degrades to serial when feature disabled
**WASM Compatibility:** `cfg(target_arch = "wasm32")` gates on `Instant`, timer, and native plugin loading; `getrandom` uses `wasm_js` feature

---

*Architecture analysis: 2026-03-18*
