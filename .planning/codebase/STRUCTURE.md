# Codebase Structure

**Analysis Date:** 2026-02-13

## Directory Layout

```
libslic3r-rs/
├── Cargo.toml                    # Workspace root manifest
├── Cargo.lock                    # Locked dependency versions (committed)
├── crates/                       # Layer-organized library crates
│   ├── slicecore-math/           # Layer 0: Math primitives (Point2, Point3, Matrix, BBox)
│   ├── slicecore-geo/            # Layer 0: Computational geometry (Polygon, Boolean ops, Voronoi)
│   ├── slicecore-mesh/           # Layer 0: 3D mesh (TriangleMesh, BVH, repair, transforms)
│   ├── slicecore-fileio/         # Layer 1: File parsers (STL, 3MF, OBJ, STEP)
│   ├── slicecore-gcode-io/       # Layer 1: G-code parser and writer
│   ├── slicecore-config/         # Layer 1: Configuration system (schema, validation, TOML loading)
│   ├── slicecore-slicer/         # Layer 2: Core slicing (mesh → layers)
│   ├── slicecore-perimeters/     # Layer 2: Wall generation (Arachne, seam placement)
│   ├── slicecore-infill/         # Layer 2: Infill patterns (rectilinear, gyroid, etc.)
│   ├── slicecore-supports/       # Layer 2: Support generation (basic, tree, interface)
│   ├── slicecore-pathing/        # Layer 2: Toolpath optimization (travel, ordering)
│   ├── slicecore-planner/        # Layer 3: Motion planning (speed, accel, temp, fan)
│   ├── slicecore-gcode-gen/      # Layer 3: G-code emission (firmware dialects)
│   ├── slicecore-estimator/      # Layer 3: Time/material/cost estimation
│   ├── slicecore-analyzer/       # Layer 4: Model feature extraction and analysis
│   ├── slicecore-ai/             # Layer 4: AI provider abstraction (OpenAI, Ollama, etc.)
│   ├── slicecore-optimizer/      # Layer 4: Parameter optimization
│   ├── slicecore-engine/         # Layer 5: Pipeline orchestrator (full slicing workflow)
│   ├── slicecore-plugin/         # Layer 5: Plugin system (registry, loading, lifecycle)
│   └── slicecore-api/            # Layer 5: External interfaces (REST, Python, WASM, FFI)
├── bins/                         # Executable binaries
│   ├── slicecore-cli/            # Command-line interface
│   │   └── src/
│   │       ├── main.rs           # Entry point: parse args, invoke engine
│   │       └── commands/         # Command implementations (slice, analyze, optimize)
│   └── slicecore-server/         # REST/gRPC API server
│       └── src/
│           ├── main.rs           # Server startup and route registration
│           └── handlers/         # Route handlers (/api/v1/slice, /api/v1/analyze, etc.)
├── plugins/                      # Example and built-in plugins
│   ├── infill-gyroid/            # Example: Gyroid infill pattern plugin
│   └── gcode-klipper/            # Example: Klipper G-code dialect plugin
├── tests/                        # Integration and end-to-end tests
│   ├── models/                   # Test 3D models (calibration_cube_20mm.stl, benchy.stl, etc.)
│   ├── configs/                  # Test printer/filament profiles (pla_standard.toml, etc.)
│   ├── golden/                   # Golden file expectations (deterministic output hashes)
│   │   ├── calibration_cube_standard.gcode.sha256
│   │   ├── benchy_fine_quality.gcode.sha256
│   │   └── ...
│   └── integration_tests.rs      # End-to-end slice tests
├── benches/                      # Performance benchmarks
│   ├── slice_benchmark.rs        # Criterion benchmarks for slicing operations
│   └── geometry_benchmark.rs     # Benchmarks for geometric operations
├── fuzz/                         # Fuzz testing targets
│   ├── fuzz_targets/
│   │   ├── stl_parser.rs         # Fuzz STL parser with malformed input
│   │   ├── gcode_parser.rs       # Fuzz G-code parser
│   │   └── config_parser.rs      # Fuzz config TOML parser
├── designDocs/                   # Architecture and design documentation
│   ├── 01-PRODUCT_REQUIREMENTS.md
│   ├── 02-ARCHITECTURE.md
│   ├── 03-API-DESIGN.md
│   ├── 04-IMPLEMENTATION-GUIDE.md
│   ├── 05-CPP-ANALYSIS-GUIDE.md
│   ├── 06-NOVEL-IDEAS.md
│   ├── 07-MISSING-CONSIDERATIONS.md
│   └── 08-GLOSSARY.md
├── .github/workflows/            # CI/CD pipelines
│   └── ci.yml                    # Lint, test, benchmark, fuzz, WASM build
├── .planning/                    # GSD planning artifacts
│   └── codebase/                 # Architecture, structure, conventions, testing docs
├── target/                       # Compiled artifacts (gitignored)
│   ├── debug/
│   ├── release/
│   └── wasm32-unknown-unknown/
└── README.md                     # Project overview

```

## Directory Purposes

**`crates/`:**
- Purpose: Self-contained, reusable library crates organized by layer
- Contains: Source code, tests, and examples for each abstraction
- Key files: `Cargo.toml` (per-crate), `src/lib.rs` (public API), `tests/` (integration tests per crate)

**`bins/`:**
- Purpose: Executable binaries using crate libraries
- Contains: CLI tool and REST API server entry points
- Key files: `main.rs` (entry point), `handlers/` (request handling), `commands/` (CLI subcommands)

**`plugins/`:**
- Purpose: Example and reference implementations of plugin extension points
- Contains: Standalone crates demonstrating plugin loading and API
- Key files: `plugin.toml` (plugin metadata), `src/lib.rs` (plugin implementation)

**`tests/`:**
- Purpose: Integration tests and test fixtures
- Contains: Test models, configurations, golden file hashes
- Key files: `integration_tests.rs` (end-to-end tests), `models/*.stl` (test geometries), `golden/*.sha256` (deterministic expectations)

**`benches/`:**
- Purpose: Performance regression detection
- Contains: Criterion benchmarks for hot paths
- Key files: `slice_benchmark.rs` (full pipeline timing), `geometry_benchmark.rs` (per-operation profiling)

**`fuzz/`:**
- Purpose: Robustness and security validation via fuzzing
- Contains: Fuzz test targets for all parsers
- Key files: `fuzz_targets/*.rs` (per-format fuzz harnesses)

**`designDocs/`:**
- Purpose: Human-readable design documents guiding implementation
- Contains: Architecture decisions, API specs, implementation roadmap, analysis guides
- Key files: `02-ARCHITECTURE.md` (system design), `04-IMPLEMENTATION-GUIDE.md` (phase-by-phase roadmap)

## Key File Locations

**Entry Points:**
- `bins/slicecore-cli/src/main.rs`: CLI application entry point
- `bins/slicecore-server/src/main.rs`: REST API server entry point
- `crates/slicecore-engine/src/lib.rs`: Library public interface (Engine struct)
- `crates/slicecore-api/src/wasm.rs`: WASM interface (`WasmSlicer` struct)

**Configuration:**
- `Cargo.toml` (workspace root): Workspace metadata, dependencies, features, profiles
- `.github/workflows/ci.yml`: Continuous integration pipeline (lint, test, bench, fuzz)
- `designDocs/04-IMPLEMENTATION-GUIDE.md`: Detailed phase-by-phase roadmap with Gantt chart

**Core Logic:**
- `crates/slicecore-slicer/src/lib.rs`: Mesh-to-layer conversion (most critical algorithm)
- `crates/slicecore-perimeters/src/lib.rs`: Wall generation with seam placement strategies
- `crates/slicecore-infill/src/lib.rs`: Infill pattern implementations
- `crates/slicecore-planner/src/lib.rs`: Speed, acceleration, temperature planning
- `crates/slicecore-engine/src/lib.rs`: Pipeline orchestrator coordinating all stages

**Testing:**
- `tests/integration_tests.rs`: End-to-end tests (slice test models, compare golden hashes)
- `tests/models/`: Curated test geometries (calibration_cube_20mm.stl, benchy.stl, etc.)
- `tests/golden/`: SHA256 hashes of expected deterministic outputs
- `benches/slice_benchmark.rs`: Performance benchmarks with regression detection

## Naming Conventions

**Files:**
- Crate source: `src/lib.rs` (public API), `src/main.rs` (binary entry)
- Tests: `src/lib.rs` contains `#[cfg(test)] mod tests {}` for unit tests; `tests/integration_tests.rs` for integration tests
- Examples: `examples/basic_slice.rs` demonstrates crate usage
- Pattern: Snake_case for filenames; one responsibility per file

**Directories:**
- Crates: `slicecore-{layer}-{purpose}` or `slicecore-{domain}` (e.g., `slicecore-gcode-io`)
- Modules within crates: `geometry.rs`, `parser.rs`, `optimize.rs` (one concept per file)
- Tests: `tests/integration/`, `tests/golden/`, `tests/models/` by category
- Pattern: Hyphenated crate names; snake_case module names

**Types:**
- Structs: PascalCase (`TriangleMesh`, `SliceLayer`, `ExtrusionSegment`)
- Enums: PascalCase with variants PascalCase (`RegionType::Perimeter`)
- Traits: PascalCase with suffix `Trait` if generic (`Plugin`, `AiProvider`, `InfillPattern`)
- Errors: PascalCase with suffix `Error` (`MeshError`, `SlicingError`, `ConfigError`)

**Functions/Methods:**
- Snake_case for all functions and methods
- Builders: `pub fn new() -> Self`, `with_capacity()`, `build()`
- Getters: `pub fn mesh(&self) -> &TriangleMesh`
- Setters: `pub fn set_layer_height(&mut self, h: f64)`
- Validators: `pub fn validate() -> Result<()>`

## Where to Add New Code

**New Slicing Feature (e.g., new infill pattern):**
- Primary code: `crates/slicecore-infill/src/patterns.rs` (implement `InfillPattern` trait)
- Tests: `crates/slicecore-infill/src/lib.rs` unit tests + `tests/integration_tests.rs` golden file test
- Example: See `crates/slicecore-infill/src/patterns/rectilinear.rs` for reference implementation
- Pattern: Add trait method to `InfillPattern`, implement for new `struct`, register in plugin registry

**New File Format Support:**
- Primary code: `crates/slicecore-fileio/src/parsers/{format}.rs` (implement `FileParser` trait)
- Tests: `tests/models/` add test file, `crates/slicecore-fileio/src/lib.rs` add unit test
- Fuzzing: `fuzz/fuzz_targets/{format}_parser.rs` (add fuzz harness)
- Pattern: Implement `pub trait FileParser`, add to format detection in `detect_format()`

**New Optimization Strategy:**
- Primary code: `crates/slicecore-optimizer/src/strategies/{strategy}.rs`
- Tests: Unit tests in `src/lib.rs`, integration test in `tests/integration_tests.rs`
- AI Integration: If using LLM, add to `crates/slicecore-ai/src/providers.rs`
- Pattern: Implement `pub trait OptimizationStrategy`, register in `Optimizer`

**Utilities / Helpers:**
- Layer 0 (foundational): `crates/slicecore-math/src/utils.rs` or `crates/slicecore-geo/src/utils.rs`
- Layer 2+ (algorithm-specific): Within respective crate's `src/lib.rs` or `src/utils.rs`
- Avoid: Shared utilities directory — keep them close to usage site for cohesion

**Plugin Development:**
- Location: `plugins/{name}/` as a new crate
- Structure: Standard crate layout with `plugin.toml` manifest
- Example: `plugins/infill-gyroid/` shows plugin discovery, initialization, trait implementation
- Pattern: Implement `Plugin` + extension trait (e.g., `InfillPattern`), export via `#[no_mangle]` for dynamic loading

## Special Directories

**`target/`:**
- Purpose: Compiled artifacts and intermediate build products
- Generated: Yes (by cargo)
- Committed: No (in `.gitignore`)
- Note: Separate subdirectories for `debug/`, `release/`, `wasm32-unknown-unknown/` per target

**`.planning/codebase/`:**
- Purpose: Architecture and structure documentation consumed by GSD planning system
- Generated: Yes (by mapper agent)
- Committed: Yes (part of repo)
- Note: Contains ARCHITECTURE.md, STRUCTURE.md, CONVENTIONS.md, TESTING.md, CONCERNS.md, STACK.md, INTEGRATIONS.md

**`designDocs/`:**
- Purpose: Human-readable design specs and implementation guides
- Generated: No (hand-written design documents)
- Committed: Yes
- Note: Serves as reference; `.planning/` documents are auto-generated analyses of actual code

## Dependency Structure

**No circular dependencies — enforced by `cargo deny`.**

Dependency direction (Layers can only depend on lower layers):

```
Layer 5 (Integration)   → depends on → Layers 0-4
Layer 4 (Intelligence)  → depends on → Layers 0-3
Layer 3 (Planning)      → depends on → Layers 0-2
Layer 2 (Algorithms)    → depends on → Layers 0-1
Layer 1 (I/O & Data)    → depends on → Layer 0
Layer 0 (Foundation)    → no internal deps (only external crates)
```

Examples:
- `slicecore-engine` (Layer 5) can use `slicecore-slicer` (Layer 2) ✓
- `slicecore-slicer` (Layer 2) can use `slicecore-geo` (Layer 0) ✓
- `slicecore-perimeters` (Layer 2) cannot use `slicecore-planner` (Layer 3) ✗
- `slicecore-math` (Layer 0) cannot import from any slicecore crate ✗

---

*Structure analysis: 2026-02-13*
