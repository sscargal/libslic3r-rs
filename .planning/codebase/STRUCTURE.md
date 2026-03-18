# Codebase Structure

**Analysis Date:** 2026-03-18

## Directory Layout

```
libslic3r-rs/
├── crates/                         # All library crates (workspace members)
│   ├── slicecore-math/             # Foundation math types (no internal deps)
│   ├── slicecore-geo/              # 2D polygon operations
│   ├── slicecore-mesh/             # 3D triangle mesh + BVH + CSG
│   ├── slicecore-slicer/           # Mesh-to-contour slicing
│   ├── slicecore-fileio/           # STL, 3MF, OBJ parsers/exporters
│   ├── slicecore-gcode-io/         # G-code types, writer, validator
│   ├── slicecore-config-schema/    # Setting schema types and registry
│   ├── slicecore-config-derive/    # #[derive(ConfigSchema)] proc-macro
│   ├── slicecore-engine/           # Pipeline orchestrator (main library)
│   ├── slicecore-plugin-api/       # FFI-safe plugin interface types
│   ├── slicecore-plugin/           # Plugin registry, loaders, discovery
│   ├── slicecore-ai/               # LLM integration for profile suggestions
│   ├── slicecore-arrange/          # Build plate auto-arrangement
│   ├── slicecore-render/           # CPU software rasterizer for thumbnails
│   └── slicecore-cli/              # CLI binary (slicecore)
├── plugins/                        # Plugin examples and implementations
│   └── examples/
│       ├── native-zigzag-infill/   # Example native (cdylib) infill plugin
│       └── wasm-spiral-infill/     # Example WASM infill plugin
├── profiles/                       # Converted slicer profile library
│   ├── bambustudio/                # BambuStudio profiles (TOML)
│   ├── crealityprint/              # CrealityPrint profiles (TOML)
│   ├── orcaslicer/                 # OrcaSlicer profiles (TOML)
│   ├── prusaslicer/                # PrusaSlicer profiles (TOML)
│   └── index.json                  # Searchable profile index
├── fuzz/                           # cargo-fuzz harnesses
│   └── fuzz_targets/               # fuzz_stl_binary.rs, fuzz_stl_ascii.rs, fuzz_obj.rs, fuzz_csg.rs
├── designDocs/                     # Design reference documents (not code)
├── scripts/                        # Utility scripts
├── .planning/                      # GSD workflow state
│   ├── codebase/                   # Codebase analysis documents (this dir)
│   ├── phases/                     # Phase plans
│   └── config.json                 # GSD config
├── Cargo.toml                      # Workspace manifest
├── Cargo.lock                      # Lockfile
├── clippy.toml                     # Workspace clippy config
├── .rustfmt.toml                   # Rustfmt config
└── CLAUDE.md                       # AI assistant instructions
```

## Directory Purposes

**`crates/slicecore-math/src/`:**
- Purpose: Zero-dependency math primitives shared by all crates
- Contains: `point.rs`, `vec.rs`, `bbox.rs`, `matrix.rs`, `coord.rs`, `convert.rs`, `epsilon.rs`
- Key files: `crates/slicecore-math/src/lib.rs` (re-exports all types at crate root)

**`crates/slicecore-geo/src/`:**
- Purpose: 2D polygon types and boolean/offset/simplify operations
- Contains: `polygon.rs`, `boolean.rs`, `offset.rs`, `area.rs`, `polyline.rs`, `simplify.rs`, `convex_hull.rs`, `point_in_poly.rs`
- Key files: `crates/slicecore-geo/src/polygon.rs` (two-tier `Polygon`/`ValidPolygon`)

**`crates/slicecore-mesh/src/`:**
- Purpose: 3D mesh data structure with spatial queries
- Contains: `triangle_mesh.rs`, `bvh.rs`, `spatial.rs`, `transform.rs`, `stats.rs`, `repair.rs`, `repair/` dir, `csg/` dir
- Key files: `crates/slicecore-mesh/src/triangle_mesh.rs`

**`crates/slicecore-slicer/src/`:**
- Purpose: Mesh-to-layer slicing
- Contains: `layer.rs` (main `slice_mesh`), `contour.rs` (triangle-plane intersection), `adaptive.rs` (adaptive layer heights), `resolve.rs` (contour intersection resolution)

**`crates/slicecore-engine/src/`:**
- Purpose: Slicing pipeline orchestrator with all algorithms
- Contains: pipeline modules, profile management, config, event system
- Key files: `engine.rs` (Engine struct), `config.rs` (PrintConfig), `lib.rs` (re-exports + global registry)
- Subdirectories: `infill/` (10 pattern submodules), `support/` (12 support modules), `gcode_analysis/`

**`crates/slicecore-engine/src/infill/`:**
- Contains: `mod.rs` (dispatch + InfillPattern enum), `rectilinear.rs`, `grid.rs`, `honeycomb.rs`, `gyroid.rs`, `cubic.rs`, `adaptive_cubic.rs`, `lightning.rs`, `monotonic.rs`, `tpms_d.rs`, `tpms_fk.rs`

**`crates/slicecore-engine/src/support/`:**
- Contains: `mod.rs`, `config.rs`, `detect.rs`, `traditional.rs`, `tree.rs`, `tree_node.rs`, `interface.rs`, `bridge.rs`, `overhang_perimeter.rs`, `conflict.rs`, `override_system.rs`

**`crates/slicecore-cli/src/`:**
- Purpose: CLI binary with subcommand modules
- Contains: `main.rs` (clap setup + routing), `slice_workflow.rs`, `csg_command.rs`, `csg_info.rs`, `plugins_command.rs`, `schema_command.rs`, `analysis_display.rs`, `stats_display.rs`, `progress.rs`, `calibrate/`

**`crates/slicecore-plugin-api/src/`:**
- Purpose: Shared contract between host and plugins
- Contains: `types.rs` (InfillRequest, InfillResult, FfiInfillLine), `traits.rs` (InfillPatternPlugin), `metadata.rs` (PluginManifest), `postprocess_types.rs`, `postprocess_traits.rs`, `error.rs`

**`crates/slicecore-plugin/src/`:**
- Purpose: Plugin host infrastructure
- Contains: `registry.rs` (PluginRegistry), `discovery.rs` (directory scanning), `native.rs` (ABI-stable loader), `wasm.rs` (wasmtime loader), `sandbox.rs` (resource limits), `postprocess.rs`, `convert.rs`, `status.rs`
- Key files: `crates/slicecore-plugin/wit/slicecore-plugin.wit` (WIT interface definition)

**`crates/slicecore-ai/src/`:**
- Purpose: LLM provider integration
- Contains: `provider.rs` (AiProvider trait), `providers/` (OpenAI, Anthropic, Ollama), `config.rs`, `geometry.rs`, `profile.rs`, `prompt.rs`, `suggest.rs`, `types.rs`

**`crates/slicecore-arrange/src/`:**
- Purpose: Build plate packing
- Contains: `lib.rs` (arrange() entry point), `bed.rs`, `footprint.rs`, `placer.rs`, `grouper.rs`, `orient.rs`, `sequential.rs`, `config.rs`, `result.rs`

**`crates/slicecore-render/src/`:**
- Purpose: Software rasterizer for thumbnails
- Contains: `pipeline.rs`, `rasterizer.rs`, `framebuffer.rs`, `camera.rs`, `shading.rs`, `png_encode.rs`, `gcode_embed.rs`

**`plugins/examples/native-zigzag-infill/`:**
- Purpose: Reference implementation for native cdylib plugins
- Pattern: `crate-type = ["cdylib"]`, implements `InfillPatternPlugin`, exports `#[export_root_module]`

**`plugins/examples/wasm-spiral-infill/`:**
- Purpose: Reference implementation for WASM component plugins
- Pattern: `crate-type = ["cdylib"]`, built with `--target wasm32-wasip2`, implements WIT `Guest` trait

**`profiles/`:**
- Purpose: Imported and converted slicer profiles from upstream slicers
- Generated by: `slicecore import-profiles` CLI command
- Not committed as source code — imported at setup time
- Format: TOML files organized by `source/vendor/type/`

**`fuzz/fuzz_targets/`:**
- Purpose: Cargo-fuzz harnesses for parser fuzzing
- Generated: No — manually maintained
- Targets: `fuzz_stl_binary`, `fuzz_stl_ascii`, `fuzz_obj`, `fuzz_csg`

## Key File Locations

**Entry Points:**
- `crates/slicecore-cli/src/main.rs`: CLI binary, all subcommand routing
- `crates/slicecore-engine/src/engine.rs`: `Engine::slice()` — primary library entry point
- `crates/slicecore-arrange/src/lib.rs`: `arrange()` — build plate arrangement entry
- `crates/slicecore-fileio/src/lib.rs`: `load_mesh()` — unified mesh loading

**Configuration:**
- `Cargo.toml`: Workspace manifest with shared dependency versions
- `crates/slicecore-engine/src/config.rs`: `PrintConfig` — all slicing parameters
- `clippy.toml`: Workspace-wide clippy settings
- `.rustfmt.toml`: Formatting config

**Core Logic:**
- `crates/slicecore-engine/src/engine.rs`: Full slicing pipeline
- `crates/slicecore-engine/src/infill/mod.rs`: Infill dispatch + `InfillPattern` enum
- `crates/slicecore-mesh/src/triangle_mesh.rs`: `TriangleMesh` data structure
- `crates/slicecore-slicer/src/layer.rs`: `slice_mesh()` implementation
- `crates/slicecore-geo/src/polygon.rs`: `Polygon` / `ValidPolygon` types
- `crates/slicecore-plugin/src/registry.rs`: `PluginRegistry`
- `crates/slicecore-plugin/wit/slicecore-plugin.wit`: WIT interface definition

**Testing:**
- Integration tests: `crates/*/tests/` directories
- Benchmarks: `crates/slicecore-engine/benches/`, `crates/slicecore-mesh/benches/`
- Unit tests: inline `#[cfg(test)] mod tests` in source files
- Fuzz: `fuzz/fuzz_targets/*.rs`

## Naming Conventions

**Files:**
- Snake case: `triangle_mesh.rs`, `gcode_gen.rs`, `profile_import.rs`
- Module names match file names: `pub mod triangle_mesh;` → `triangle_mesh.rs`
- Test files named after the feature being tested: `fuzz_stl_binary.rs`

**Directories:**
- Kebab-case crate names: `slicecore-mesh`, `slicecore-plugin-api`
- Snake-case module subdirectories: `infill/`, `support/`, `gcode_analysis/`, `providers/`

**Types:**
- Structs and enums: PascalCase (`TriangleMesh`, `PrintConfig`, `InfillPattern`)
- Error types: `{Domain}Error` pattern (`MeshError`, `GeoError`, `EngineError`)
- Traits: PascalCase noun or adjective (`AiProvider`, `HasSettingSchema`, `InfillPatternPlugin`)

**Functions:**
- Snake case: `slice_mesh`, `load_mesh`, `generate_infill`, `arrange`
- Constructor-like functions: `new()`, `from_*()`, `parse()`
- Entry-point functions at crate root preferred over deep module paths

## Where to Add New Code

**New Infill Pattern:**
- Implementation: `crates/slicecore-engine/src/infill/{pattern_name}.rs`
- Register: Add variant to `InfillPattern` enum in `crates/slicecore-engine/src/infill/mod.rs`
- Dispatch: Add match arm in `generate_infill()`
- Tests: Add `#[cfg(test)]` module in the pattern file

**New CLI Subcommand:**
- Implementation: `crates/slicecore-cli/src/{command_name}.rs`
- Register: Add `mod {command_name};` and enum variant to `crates/slicecore-cli/src/main.rs`

**New File Format:**
- Parser: `crates/slicecore-fileio/src/{format}.rs`
- Register: Add to `detect_format()` in `crates/slicecore-fileio/src/detect.rs`
- Dispatch: Add match arm in `load_mesh()` in `crates/slicecore-fileio/src/lib.rs`
- Tests: `crates/slicecore-fileio/tests/`

**New Engine Pipeline Stage:**
- Module: `crates/slicecore-engine/src/{stage_name}.rs`
- Register: Add `pub mod {stage_name};` in `crates/slicecore-engine/src/lib.rs`
- Re-export primary types in `lib.rs`
- Integrate into `Engine::slice()` in `crates/slicecore-engine/src/engine.rs`

**New PrintConfig Field:**
- Add field to appropriate struct in `crates/slicecore-engine/src/config.rs`
- Annotate with `#[setting(...)]` for schema metadata
- Derive `ConfigSchema` is already on `PrintConfig` — field is auto-registered

**New AI Provider:**
- Implementation: `crates/slicecore-ai/src/providers/{provider_name}.rs`
- Register: Add variant to `ProviderType` enum in `crates/slicecore-ai/src/config.rs`
- Dispatch: Add match arm in `create_provider()` in `crates/slicecore-ai/src/providers/mod.rs`

**New Native Plugin:**
- Create new crate with `crate-type = ["cdylib"]`
- Depend only on `slicecore-plugin-api`
- Implement `InfillPatternPlugin` or `GcodePostProcessorPlugin`
- Export via `#[export_root_module]`
- Place in `plugins/` or external directory
- Reference example: `plugins/examples/native-zigzag-infill/`

**New WASM Plugin:**
- Create new crate with `crate-type = ["cdylib"]`
- Copy WIT from `crates/slicecore-plugin/wit/slicecore-plugin.wit`
- Use `wit_bindgen::generate!` and implement `Guest` trait
- Build with `cargo build --target wasm32-wasip2`
- Reference example: `plugins/examples/wasm-spiral-infill/`

**New Utility/Math Type:**
- If geometry-related: `crates/slicecore-math/src/` or `crates/slicecore-geo/src/`
- If mesh-related: `crates/slicecore-mesh/src/`
- Expose at crate root via `lib.rs` pub use

## Special Directories

**`.planning/`:**
- Purpose: GSD workflow state (phase plans, codebase docs, project context)
- Generated: Partially (phases generated by GSD commands)
- Committed: Yes

**`target/`:**
- Purpose: Cargo build artifacts
- Generated: Yes
- Committed: No (in `.gitignore`)

**`fuzz/target/`:**
- Purpose: Fuzz build artifacts
- Generated: Yes
- Committed: No

**`designDocs/`:**
- Purpose: Architecture reference documents, API design, implementation guides, glossary
- Committed: Yes — these are human-written design references
- Key files: `01-PRODUCT_REQUIREMENTS.md`, `02-ARCHITECTURE.md`, `04-IMPLEMENTATION-GUIDE.md`

---

*Structure analysis: 2026-03-18*
