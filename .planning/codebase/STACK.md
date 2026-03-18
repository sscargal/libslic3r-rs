# Technology Stack

**Analysis Date:** 2026-03-18

## Languages

**Primary:**
- Rust 2021 edition - All crates in the workspace

**Secondary:**
- TOML - Configuration files (`profiles/`, `plugin.toml` manifests, `.cargo/config.toml`)
- WIT (WebAssembly Interface Types) - Plugin API contract at `crates/slicecore-plugin/wit/slicecore-plugin.wit`

## Runtime

**Environment:**
- Native x86_64/aarch64 Linux, macOS (x86_64 + Apple Silicon), Windows MSVC
- WebAssembly: `wasm32-unknown-unknown` (no-std), `wasm32-wasip2` (WASI P2 component model)
- WASM targets have incremental build via `.cargo/config.toml`

**Package Manager:**
- Cargo 1.93+
- Lockfile: `Cargo.lock` present (committed)
- MSRV: 1.80 (enforced via `rust-version` in `[workspace.package]`)

## Frameworks

**Core:**
- None (no web framework — pure library + CLI binary)

**CLI:**
- `clap` 4.5 with `derive` feature — argument parsing in `crates/slicecore-cli`

**Testing:**
- Built-in `cargo test` — no separate test runner
- `proptest` 1 — property-based testing across most crates
- `criterion` 0.5 with `html_reports` — benchmarks in `slicecore-engine` and `slicecore-mesh`
- `libfuzzer-sys` 0.4 via `cargo-fuzz` — fuzz targets in `fuzz/` for STL binary, STL ASCII, OBJ parsers

**Async:**
- `tokio` 1 with `rt` + `macros` features — used only in `slicecore-ai` for HTTP calls
- `async-trait` 0.1 — async trait methods for `AiProvider`

**Build/Dev:**
- `rustfmt` (max_width=100, configured in `.rustfmt.toml`)
- `clippy` with `pedantic` + `cargo` lint groups (configured per-crate; global threshold in `clippy.toml`)
- `Swatinem/rust-cache@v2` in CI for build caching

## Key Dependencies

**Geometry / Math:**
- `clipper2-rust` 1.0 — polygon boolean operations (union, difference, intersection) in `slicecore-geo`
- `boostvoronoi` 0.11.1 — Voronoi diagram generation in `slicecore-engine`
- `robust` 1.1 — robust geometric predicates in `slicecore-mesh`
- `glam` 0.31 — linear algebra / matrix math in `slicecore-fileio`

**File Formats:**
- `tobj` 4 (no default features) — OBJ mesh parsing in `slicecore-fileio`
- `lib3mf-core` 0.4 (no default features) — 3MF mesh format core in `slicecore-fileio`
- `lib3mf-converters` 0.4 (no default features) — 3MF conversion utilities in `slicecore-fileio`
- `byteorder` 1 — binary STL parsing in `slicecore-fileio`
- `base64` 0.22 — G-code thumbnail encoding in `slicecore-gcode-io` and `slicecore-render`
- `png` 0.17 — PNG encoding for preview images in `slicecore-render`

**Serialization:**
- `serde` 1 with `derive` — workspace-wide, JSON + TOML + MessagePack
- `serde_json` 1 — workspace-wide JSON
- `toml` 0.8 — config files in `slicecore-engine`, `slicecore-ai`, `slicecore-cli`
- `rmp-serde` 1 — MessagePack serialization in `slicecore-engine`, `slicecore-cli`

**Plugin System:**
- `abi_stable` 0.11 — FFI-safe types for native (.so/.dll) plugins in `slicecore-plugin-api`, `slicecore-plugin`
- `wasmtime` 41 with `component-model` + `cranelift` features — WASM plugin runtime in `slicecore-plugin`
- `wasmtime-wasi` 41 — WASI support for WASM plugins
- `wit-bindgen` 0.53 — WIT bindings for WASM plugin authors (in `plugins/examples/wasm-spiral-infill`)
- `semver` 1 — plugin API version compatibility checks

**HTTP / AI:**
- `reqwest` 0.12 with `json` + `rustls-tls`, no default features — HTTP client in `slicecore-ai`
- `secrecy` 0.10 with `serde` — API key protection in `slicecore-ai`
- `url` 2 — URL parsing in `slicecore-ai`

**Parallelism:**
- `rayon` 1.11 — optional parallel processing in `slicecore-engine` (feature: `parallel`) and `slicecore-mesh` (feature: `parallel`)

**Utilities:**
- `thiserror` 2 — error types across all crates
- `anyhow` 1 — error propagation in `slicecore-cli`
- `approx` 0.5 — floating-point approximate equality in tests
- `walkdir` 2 — directory traversal for profile/plugin discovery
- `sha2` 0.10 — hashing in `slicecore-engine`
- `strsim` 0.11 — string similarity for fuzzy matching in `slicecore-engine`
- `comfy-table` 7 — terminal table rendering in `slicecore-cli`
- `indicatif` 0.17 — progress bars in `slicecore-cli`
- `tempfile` 3 — temporary files in tests

**Proc Macros:**
- `syn` 2 (full + derive + extra-traits + parsing features) — in `slicecore-config-derive`
- `quote` 1 — token generation in `slicecore-config-derive`
- `proc-macro2` 1 — proc macro support in `slicecore-config-derive`

## Configuration

**Environment:**
- `SLICECORE_PROFILES_DIR` — override print profile search path (read in `slicecore-engine/src/profile_resolve.rs` and `slicecore-cli/src/main.rs`)
- AI API keys are passed via `AiConfig` struct (deserialized from TOML), not from environment variables directly
- WASM build requires `getrandom_backend="wasm_js"` rustflag (set in `.cargo/config.toml`)

**Build:**
- `.cargo/config.toml` — incremental build enabled; WASM target rustflags
- `.rustfmt.toml` — max_width=100
- `clippy.toml` — too-many-arguments-threshold=8
- Per-crate `[lints.clippy]` sections enable pedantic + cargo lint groups

## Platform Requirements

**Development:**
- Rust stable toolchain (1.80+ MSRV, tested on 1.93.1)
- Nightly toolchain optional (for `cargo fuzz`)
- Installed targets: `wasm32-unknown-unknown`, `wasm32-wasip2` (for WASM builds/tests)

**Production:**
- No runtime dependencies beyond the compiled binary
- AI features require network access to OpenAI / Anthropic APIs or a local Ollama instance
- Native plugins require platform-shared-library loading (`.so`/`.dll`/`.dylib`)
- WASM plugins require no extra system dependencies (bundled wasmtime runtime)

---

*Stack analysis: 2026-03-18*
