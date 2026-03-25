# Codebase Structure

**Analysis Date:** 2026-03-25

## Directory Layout

```
libslic3r-rs/
├── crates/                          # All library crates (workspace members)
│   ├── slicecore-math/              # Foundation math types (no internal deps)
│   ├── slicecore-geo/               # 2D polygon operations
│   ├── slicecore-mesh/              # 3D triangle mesh + BVH + CSG + repair
│   ├── slicecore-slicer/            # Mesh-to-contour slicing
│   ├── slicecore-fileio/            # STL, 3MF, OBJ parsers/exporters
│   ├── slicecore-gcode-io/          # G-code types, writer, dialects, validator
│   ├── slicecore-config-schema/     # Setting schema types and registry
│   ├── slicecore-config-derive/     # #[derive(ConfigSchema)] proc-macro
│   ├── slicecore-engine/            # Pipeline orchestrator (primary library)
│   ├── slicecore-plugin-api/        # FFI-safe plugin interface types
│   ├── slicecore-plugin/            # Plugin registry, loaders, discovery
│   ├── slicecore-ai/                # LLM integration for profile suggestions
│   ├── slicecore-arrange/           # Build plate auto-arrangement
│   ├── slicecore-render/            # CPU software rasterizer for thumbnails
│   └── slicecore-cli/               # CLI binary (slicecore)
├── plugins/                         # Plugin examples and implementations
│   └── examples/
│       ├── native-zigzag-infill/    # Example native (cdylib) infill plugin
│       └── wasm-spiral-infill/      # Example WASM infill plugin
├── profiles/                        # Converted slicer profile library (TOML)
│   ├── bambustudio/                 # BambuStudio profiles
│   ├── crealityprint/               # CrealityPrint profiles
│   ├── orcaslicer/                  # OrcaSlicer profiles
│   ├── prusaslicer/                 # PrusaSlicer profiles
│   └── index.json                   # Searchable profile index
├── fuzz/                            # cargo-fuzz harnesses
│   ├── fuzz_targets/                # fuzz_stl_binary.rs, fuzz_stl_ascii.rs, fuzz_obj.rs, fuzz_csg.rs
│   └── corpus/                      # Seed corpus files for fuzzing
├── tests/                           # Workspace-level integration test fixtures
│   └── fixtures/
│       ├── override-sets/           # TOML override-set fixtures
│       └── plate-configs/           # TOML plate config fixtures
├── designDocs/                      # Design reference documents
├── scripts/                         # Utility and CI scripts
│   ├── bench-with-memory.sh         # Run criterion bench + capture peak RSS
│   ├── check-bench-regressions.sh   # Compare bench-results against thresholds
│   └── qa_tests                     # End-to-end QA smoke test script
├── .planning/                       # GSD workflow state
│   ├── codebase/                    # Codebase analysis documents
│   ├── phases/                      # Phase plans (one dir per phase)
│   ├── PROJECT.md                   # Project context and decisions
│   └── config.json                  # GSD config
├── .github/
│   └── workflows/ci.yml             # CI pipeline
├── Cargo.toml                       # Workspace manifest
├── Cargo.lock                       # Lockfile (committed)
├── clippy.toml                      # Workspace clippy config
├── .rustfmt.toml                    # Rustfmt config (max_width = 100)
└── CLAUDE.md                        # AI assistant instructions
```

## Directory Purposes

**`crates/slicecore-math/src/`:**
- Purpose: Zero-dependency math primitives shared by all crates
- Contains: `point.rs`, `vec.rs`, `bbox.rs`, `matrix.rs`, `coord.rs`, `convert.rs`, `epsilon.rs`
- Key files: `crates/slicecore-math/src/lib.rs` (re-exports all types at crate root)

**`crates/slicecore-geo/src/`:**
- Purpose: 2D polygon types and boolean/offset/simplify operations
- Contains: `polygon.rs`, `boolean.rs`, `offset.rs`, `area.rs`, `polyline.rs`, `simplify.rs`, `convex_hull.rs`, `point_in_poly.rs`, `error.rs`
- Key files: `crates/slicecore-geo/src/polygon.rs` (two-tier `Polygon`/`ValidPolygon`)

**`crates/slicecore-mesh/src/`:**
- Purpose: 3D mesh data structure, spatial indexing, CSG, mesh repair
- Contains: `triangle_mesh.rs`, `bvh.rs`, `spatial.rs`, `transform.rs`, `stats.rs`, `repair.rs`
- Subdirectory `repair/`: `degenerate.rs`, `holes.rs`, `intersect.rs`, `normals.rs`, `stitch.rs`
- Subdirectory `csg/`: `boolean.rs`, `classify.rs`, `error.rs`, `hollow.rs`, `intersect.rs`, `offset.rs`, `perturb.rs`, `primitives.rs`, `report.rs`, `retriangulate.rs`, `split.rs`, `types.rs`, `volume.rs`
- Benchmarks: `benches/csg_bench.rs`

**`crates/slicecore-slicer/src/`:**
- Purpose: Mesh-to-layer slicing (horizontal plane intersection)
- Contains: `layer.rs` (main `slice_mesh`), `contour.rs` (triangle-plane intersection), `adaptive.rs` (adaptive layer heights), `resolve.rs` (contour intersection resolution)

**`crates/slicecore-fileio/src/`:**
- Purpose: Mesh file format parsing and export
- Contains: `lib.rs` (`load_mesh`, `save_mesh`), `detect.rs` (format detection), `stl.rs`, `stl_binary.rs`, `stl_ascii.rs`, `obj.rs`, `threemf.rs`, `export.rs`, `error.rs`
- Tests: `tests/integration.rs`, `tests/wasm_3mf_test.rs`

**`crates/slicecore-gcode-io/src/`:**
- Purpose: G-code types, per-firmware dialects, writer, validation
- Contains: `lib.rs`, `commands.rs`, `writer.rs`, `dialect.rs`, `arc.rs`, `validate.rs`, `thumbnail.rs`, `error.rs`
- Dialect modules: `marlin.rs`, `klipper.rs`, `reprap.rs`, `bambu.rs`

**`crates/slicecore-config-schema/src/`:**
- Purpose: Setting metadata, JSON Schema generation, schema registry and search
- Contains: `lib.rs`, `types.rs`, `registry.rs`, `json_schema.rs`, `metadata_json.rs`, `search.rs`, `validate.rs`

**`crates/slicecore-config-derive/src/`:**
- Purpose: Procedural macro `#[derive(ConfigSchema)]` for `PrintConfig` fields
- Contains: `lib.rs`, `parse.rs`, `codegen.rs`
- Tests: `tests/derive_test.rs`

**`crates/slicecore-engine/src/`:**
- Purpose: Primary slicing pipeline orchestrator with all algorithms
- Contains: `engine.rs` (Engine struct), `config.rs` (PrintConfig), `lib.rs` (module declarations + re-exports)
- Subdirectory `infill/`: `mod.rs` + 10 pattern files (`rectilinear.rs`, `grid.rs`, `honeycomb.rs`, `gyroid.rs`, `cubic.rs`, `adaptive_cubic.rs`, `lightning.rs`, `monotonic.rs`, `tpms_d.rs`, `tpms_fk.rs`)
- Subdirectory `support/`: `mod.rs`, `config.rs`, `detect.rs`, `traditional.rs`, `tree.rs`, `tree_node.rs`, `interface.rs`, `bridge.rs`, `overhang_perimeter.rs`, `conflict.rs`, `override_system.rs`
- Subdirectory `gcode_analysis/`: `mod.rs`, `parser.rs`, `metrics.rs`, `comparison.rs`, `slicer_detect.rs`
- Benchmarks: `benches/` (5 bench files: `cascade_bench.rs`, `geometry_benchmark.rs`, `parallel_benchmark.rs`, `slice_benchmark.rs`, `travel_benchmark.rs`)
- Key files: `crates/slicecore-engine/src/engine.rs`, `crates/slicecore-engine/src/config.rs`

**`crates/slicecore-engine/src/` — notable modules:**
- `profile_library.rs`: Profile index loading and search
- `profile_compose.rs`: Profile composition (machine + filament + process)
- `profile_resolve.rs`: Profile resolution from multiple sources
- `profile_convert.rs`: Profile format conversion (upstream → native TOML)
- `profile_import.rs`, `profile_import_ini.rs`: INI/TOML import logic
- `profile_diff.rs`: Profile diff/comparison
- `enabled_profiles.rs`: Profile activation state management
- `cascade.rs`: Layered config cascade (printer → filament → process → overrides)
- `plate_config.rs`: Per-plate configuration
- `job_dir.rs`-equivalent lives in CLI — see `crates/slicecore-cli/src/job_dir.rs`
- `output.rs`: Slice output types
- `event.rs`: Event bus and cancellation API
- `parallel.rs`: Rayon-based parallel slicing utilities (private module)
- `travel_optimizer.rs`: 2-opt and greedy TSP for toolpath ordering
- `cost_model.rs`, `estimation.rs`, `statistics.rs`: Print analysis

**`crates/slicecore-cli/src/`:**
- Purpose: CLI binary with subcommand modules
- Binary entry: `main.rs` (clap `Parser` + `Subcommand` routing)
- Command modules: `slice_workflow.rs`, `csg_command.rs`, `csg_info.rs`, `plugins_command.rs`, `schema_command.rs`, `diff_profiles_command.rs`, `profile_command.rs`, `profile_wizard.rs`, `plate_cmd.rs`
- Support modules: `analysis_display.rs`, `stats_display.rs`, `cli_output.rs`, `override_set.rs`, `job_dir.rs`
- Subdirectory `calibrate/`: `mod.rs`, `common.rs`, `first_layer.rs`, `flow.rs`, `retraction.rs`, `temp_tower.rs`
- Tests: 14 integration test files in `tests/` (one per CLI feature)

**`crates/slicecore-plugin-api/src/`:**
- Purpose: Shared FFI contract between host and plugins
- Contains: `types.rs` (InfillRequest, InfillResult, FfiInfillLine), `traits.rs` (InfillPatternPlugin), `metadata.rs` (PluginManifest), `postprocess_types.rs`, `postprocess_traits.rs`, `error.rs`

**`crates/slicecore-plugin/src/`:**
- Purpose: Plugin host infrastructure
- Contains: `registry.rs` (PluginRegistry), `discovery.rs` (directory scanning), `native.rs` (ABI-stable loader), `wasm.rs` (wasmtime loader), `sandbox.rs` (resource limits), `postprocess.rs`, `postprocess_convert.rs`, `convert.rs`, `status.rs`
- WIT interface definition: `crates/slicecore-plugin/wit/slicecore-plugin.wit`

**`crates/slicecore-ai/src/`:**
- Purpose: LLM provider integration for print profile suggestions
- Contains: `provider.rs` (AiProvider trait), `config.rs`, `geometry.rs`, `profile.rs`, `prompt.rs`, `suggest.rs`, `types.rs`, `error.rs`
- Subdirectory `providers/`: `mod.rs`, `openai.rs`, `anthropic.rs`, `ollama.rs`

**`crates/slicecore-arrange/src/`:**
- Purpose: Build plate auto-arrangement (bin packing)
- Contains: `lib.rs` (`arrange()` entry), `bed.rs`, `footprint.rs`, `placer.rs`, `grouper.rs`, `orient.rs`, `sequential.rs`, `config.rs`, `result.rs`, `error.rs`

**`crates/slicecore-render/src/`:**
- Purpose: CPU software rasterizer for thumbnail preview images
- Contains: `lib.rs`, `pipeline.rs`, `rasterizer.rs`, `framebuffer.rs`, `camera.rs`, `shading.rs`, `encode.rs` (PNG + JPEG encoding), `gcode_embed.rs`, `types.rs`

**`plugins/examples/native-zigzag-infill/`:**
- Purpose: Reference implementation for native cdylib plugins
- Pattern: `crate-type = ["cdylib"]`, implements `InfillPatternPlugin`, exports `#[export_root_module]`
- Separate Cargo workspace (excluded from root `Cargo.toml` members)

**`plugins/examples/wasm-spiral-infill/`:**
- Purpose: Reference implementation for WASM component plugins
- Pattern: `crate-type = ["cdylib"]`, built with `--target wasm32-wasip2`, implements WIT `Guest` trait
- Separate Cargo workspace (excluded from root `Cargo.toml` members)

**`profiles/`:**
- Purpose: Imported and converted slicer profiles from upstream slicers
- Imported via: `slicecore import-profiles` CLI command
- Format: TOML files organized by `source/vendor/type/` (e.g., `bambustudio/Anker/filament/`)
- Profile types per vendor: `machine/`, `filament/`, `process/`

**`tests/fixtures/`:**
- Purpose: Shared TOML test fixtures for workspace-level integration tests
- Contains: `override-sets/` (fast-draft.toml, high-detail.toml), `plate-configs/` (simple.toml, multi-object.toml)

**`fuzz/fuzz_targets/`:**
- Purpose: cargo-fuzz harnesses for parser fuzzing
- Maintained manually — not generated
- Targets: `fuzz_stl_binary.rs`, `fuzz_stl_ascii.rs`, `fuzz_obj.rs`, `fuzz_csg.rs`
- Corpus: `fuzz/corpus/` with seed inputs per target

**`scripts/`:**
- `bench-with-memory.sh`: Runs criterion bench + captures peak RSS via `/usr/bin/time -v`
- `check-bench-regressions.sh`: Compares bench-results against threshold baselines
- `qa_tests`: End-to-end smoke test script exercising the built `slicecore` binary

**`designDocs/`:**
- Purpose: Human-written architecture reference documents, API design, implementation guides, glossary
- Key files: `01-PRODUCT_REQUIREMENTS.md`, `02-ARCHITECTURE.md`, `03-API-DESIGN.md`, `04-IMPLEMENTATION-GUIDE.md`, `08-GLOSSARY.md`
- Also contains G-code comparison samples: `gcode-bambu/`, `gcode-ours/`, `models/`, `SlicingResultsScreenshots/`

## Key File Locations

**Entry Points:**
- `crates/slicecore-cli/src/main.rs`: CLI binary, all subcommand routing
- `crates/slicecore-engine/src/engine.rs`: `Engine::slice()` — primary library entry point
- `crates/slicecore-engine/src/lib.rs`: All public re-exports at crate root
- `crates/slicecore-arrange/src/lib.rs`: `arrange()` — build plate arrangement
- `crates/slicecore-fileio/src/lib.rs`: `load_mesh()` / `save_mesh()` — unified mesh I/O

**Configuration:**
- `Cargo.toml`: Workspace manifest with shared dependency versions
- `crates/slicecore-engine/src/config.rs`: `PrintConfig` — all slicing parameters
- `crates/slicecore-engine/src/cascade.rs`: Layered config cascade system
- `clippy.toml`: Workspace-wide clippy settings (`too-many-arguments-threshold = 8`)
- `.rustfmt.toml`: Formatting config (`max_width = 100`, `edition = "2021"`)

**Core Logic:**
- `crates/slicecore-engine/src/engine.rs`: Full slicing pipeline
- `crates/slicecore-engine/src/infill/mod.rs`: Infill dispatch + `InfillPattern` enum
- `crates/slicecore-mesh/src/triangle_mesh.rs`: `TriangleMesh` data structure
- `crates/slicecore-slicer/src/layer.rs`: `slice_mesh()` implementation
- `crates/slicecore-geo/src/polygon.rs`: `Polygon` / `ValidPolygon` types
- `crates/slicecore-plugin/src/registry.rs`: `PluginRegistry`
- `crates/slicecore-plugin/wit/slicecore-plugin.wit`: WIT interface definition

**Profile Management:**
- `crates/slicecore-engine/src/profile_library.rs`: Profile index loading and search
- `crates/slicecore-engine/src/profile_compose.rs`: Profile composition
- `crates/slicecore-engine/src/profile_resolve.rs`: Profile source resolution
- `crates/slicecore-engine/src/enabled_profiles.rs`: Profile activation state
- `crates/slicecore-engine/src/builtin_profiles.rs`: Built-in profile definitions

**Schema System:**
- `crates/slicecore-config-schema/src/registry.rs`: Global schema registry
- `crates/slicecore-config-schema/src/json_schema.rs`: JSON Schema output
- `crates/slicecore-config-derive/src/lib.rs`: `#[derive(ConfigSchema)]` entry

**Testing:**
- Integration tests: `crates/*/tests/` directories in each crate
- Shared fixtures: `tests/fixtures/`
- Benchmarks: `crates/slicecore-engine/benches/`, `crates/slicecore-mesh/benches/`
- Unit tests: inline `#[cfg(test)] mod tests` in source files
- Fuzz: `fuzz/fuzz_targets/*.rs`
- QA smoke tests: `scripts/qa_tests`

## Naming Conventions

**Files:**
- Snake case throughout: `triangle_mesh.rs`, `gcode_gen.rs`, `profile_import.rs`, `job_dir.rs`
- Module name matches file name exactly: `pub mod triangle_mesh;` → `triangle_mesh.rs`
- Integration test files named after the feature: `cli_job_dir.rs`, `csg_boolean.rs`, `repair_integration.rs`
- Phase-specific tests named: `phase12_integration.rs`, `integration_phase20.rs`

**Directories:**
- Kebab-case crate names: `slicecore-mesh`, `slicecore-plugin-api`, `slicecore-config-derive`
- Snake-case module subdirectories: `infill/`, `support/`, `gcode_analysis/`, `providers/`, `repair/`, `csg/`, `calibrate/`

**Types:**
- Structs and enums: PascalCase (`TriangleMesh`, `PrintConfig`, `InfillPattern`, `PluginRegistry`)
- Error types: `{Domain}Error` pattern (`MeshError`, `GeoError`, `EngineError`, `FileIoError`)
- Traits: PascalCase noun or adjective (`AiProvider`, `HasSettingSchema`, `InfillPatternPlugin`)
- Config structs: `{Domain}Config` (`SupportConfig`, `ArachneConfig`, `IroningConfig`)

**Functions:**
- Snake case: `slice_mesh`, `load_mesh`, `generate_infill`, `arrange`
- Constructor-like: `new()`, `from_*()`, `parse()`, `load_*()`
- Entry-point functions re-exported at crate root via `pub use` in `lib.rs`

**Crate Names:**
- All crates prefixed `slicecore-` for namespace clarity
- Suffix describes domain: `-engine`, `-mesh`, `-geo`, `-fileio`, `-gcode-io`, `-plugin`, `-plugin-api`, `-ai`, `-render`, `-arrange`, `-slicer`, `-math`
- Proc-macro crate suffix: `-derive`
- Schema crate suffix: `-schema`

## Where to Add New Code

**New Infill Pattern:**
- Implementation: `crates/slicecore-engine/src/infill/{pattern_name}.rs`
- Register: Add variant to `InfillPattern` enum in `crates/slicecore-engine/src/infill/mod.rs`
- Dispatch: Add match arm in `generate_infill()` in `crates/slicecore-engine/src/infill/mod.rs`
- Tests: Add `#[cfg(test)]` module in the pattern file

**New CLI Subcommand:**
- Implementation: `crates/slicecore-cli/src/{command_name}.rs`
- Register: Add `mod {command_name};` and enum variant in `crates/slicecore-cli/src/main.rs`
- Tests: `crates/slicecore-cli/tests/cli_{command_name}.rs`

**New File Format:**
- Parser: `crates/slicecore-fileio/src/{format}.rs`
- Register: Add to `detect_format()` in `crates/slicecore-fileio/src/detect.rs`
- Dispatch: Add match arm in `load_mesh()` in `crates/slicecore-fileio/src/lib.rs`
- Tests: `crates/slicecore-fileio/tests/`

**New Engine Pipeline Stage:**
- Module: `crates/slicecore-engine/src/{stage_name}.rs`
- Register: Add `pub mod {stage_name};` in `crates/slicecore-engine/src/lib.rs`
- Re-export primary types via `pub use` in `crates/slicecore-engine/src/lib.rs`
- Integrate into `Engine::slice()` in `crates/slicecore-engine/src/engine.rs`

**New PrintConfig Field:**
- Add field to appropriate struct in `crates/slicecore-engine/src/config.rs`
- Annotate with `#[setting(...)]` for schema metadata
- `ConfigSchema` is already derived on `PrintConfig` — field is auto-registered

**New AI Provider:**
- Implementation: `crates/slicecore-ai/src/providers/{provider_name}.rs`
- Register: Add variant to `ProviderType` enum in `crates/slicecore-ai/src/config.rs`
- Dispatch: Add match arm in `create_provider()` in `crates/slicecore-ai/src/providers/mod.rs`

**New Native Plugin:**
- Create new crate with `crate-type = ["cdylib"]` under `plugins/`
- Depend only on `slicecore-plugin-api`
- Implement `InfillPatternPlugin` or `GcodePostProcessorPlugin`
- Export via `#[export_root_module]`
- Reference: `plugins/examples/native-zigzag-infill/`

**New WASM Plugin:**
- Create new crate with `crate-type = ["cdylib"]` under `plugins/`
- Copy WIT from `crates/slicecore-plugin/wit/slicecore-plugin.wit`
- Use `wit_bindgen::generate!` and implement `Guest` trait
- Build with `cargo build --target wasm32-wasip2`
- Reference: `plugins/examples/wasm-spiral-infill/`

**New Math/Geometry Type:**
- Pure math (points, vectors, matrices): `crates/slicecore-math/src/`
- 2D polygon operations: `crates/slicecore-geo/src/`
- 3D mesh operations: `crates/slicecore-mesh/src/`
- Expose at crate root via `pub use` in `lib.rs`

**New G-code Dialect:**
- Implementation: `crates/slicecore-gcode-io/src/{firmware}.rs`
- Register in `crates/slicecore-gcode-io/src/dialect.rs`

## Special Directories

**`.planning/`:**
- Purpose: GSD workflow state (phase plans, codebase docs, project context)
- Partially generated (phases generated by GSD commands)
- Committed: Yes

**`target/`:**
- Purpose: Cargo build artifacts
- Generated: Yes
- Committed: No (in `.gitignore`)

**`fuzz/target/`:**
- Purpose: Fuzz build artifacts
- Generated: Yes
- Committed: No

**`fuzz/corpus/`:**
- Purpose: Seed corpus files for fuzz targets
- Generated: Partially (seeds hand-crafted; new corpus entries generated by fuzzer)
- Committed: Yes (seed inputs only)

**`tmp/`:**
- Purpose: Scratch area for G-code and model comparison samples
- Contains: `gcode-bambu/`, `gcode-ours/`, `models/`, `SlicingResultsScreenshots/`
- Committed: Yes (reference samples)

**`designDocs/`:**
- Purpose: Human-written architecture reference, API design, implementation guides, glossary
- Committed: Yes — these are authoritative design references

**`.github/workflows/`:**
- Purpose: CI pipeline definitions
- Key file: `.github/workflows/ci.yml`

---

*Structure analysis: 2026-03-25*
