# Technology Stack

**Analysis Date:** 2026-02-13

## Languages

**Primary:**
- Rust (Edition 2021+) - Core slicing library, all algorithmic work, performance-critical components

**Secondary:**
- C/C++ - Only for reference algorithm analysis from original LibSlic3r (not included in this codebase)

**Bindings:**
- Python - Via PyO3 for Python integration layer
- JavaScript/WebAssembly - WASM target for browser-based slicer

## Runtime

**Environment:**
- Rust compiler (rustc 1.70+)
- LLVM backend for native compilation

**Package Manager:**
- Cargo (Rust's package manager)
- Lockfile: `Cargo.lock` (required for reproducible builds)

**Target Platforms:**
- Linux x86_64, ARM64
- macOS x86_64, ARM64 (Apple Silicon)
- Windows x86_64
- WebAssembly (wasm32-unknown-unknown)

## Frameworks

**Core:**
- No UI framework included in core library (pure library design)
- Axum (async HTTP server framework) - `slicecore-server` binary for REST/gRPC API

**Concurrency:**
- Rayon (data-parallel iterator library) - Per-layer and per-region parallelism
- Tokio (async runtime) - For API server and async operations

**Serialization:**
- Serde (serialization framework) - Configuration, model metadata, API responses
- Serde JSON - API responses, analysis output
- TOML - Configuration files, printer/filament/quality profiles
- MessagePack - Efficient binary serialization for internal data

**Testing:**
- Criterion (benchmarking library) - Performance regression detection
- Proptest (property-based testing) - Geometric invariants
- Cargo test (built-in test runner)
- Nextest (faster parallel test runner, optional)

**Build/Dev:**
- Cargo workspace (monorepo organization)
- Clippy (linting)
- Rustfmt (code formatting)
- Cargo-deny (license + advisory audits)
- Cargo-fuzz (fuzzing targets for parsers)
- Cargo-tarpaulin (test coverage measurement)

## Key Dependencies

**Memory & Allocation:**
- Bumpalo (arena allocation) - Temporary geometry objects during slicing, O(1) reset between layers
- Indexmap (ordered hashmap) - Preserves insertion order for settings definitions

**Geometry & Math:**
- Custom math crate (`slicecore-math`) - Vec2, Vec3, Point2, Point3, Matrix3x3, Matrix4x4, BBox2, BBox3
- i-overlay OR geo crate (TBD) - High-performance polygon boolean operations
- Clipper2 (wrapped or ported) - Polygon offsetting and clipping

**3D Processing:**
- Custom mesh crate (`slicecore-mesh`) - TriangleMesh, BVH spatial index, mesh repair
- BVH library (spatial indexing) - Bounding volume hierarchy for ray/plane queries

**File I/O:**
- `lib3mf-core` - Native 3MF format support
- Custom parsers - Binary STL, ASCII STL, basic OBJ, STEP (future)

**Configuration:**
- Serde with TOML support - Settings loading and validation
- Custom validation framework - Dependency checking, constraint validation

**G-code:**
- Custom parser/writer (`slicecore-gcode-io`) - Firmware-agnostic G-code handling

**AI Integration:**
- Reqwest (HTTP client) - OpenAI, Anthropic, Google Vertex, OpenRouter APIs
- Secrecy (secret handling) - API key protection, never logs/serializes keys
- Async-trait (async trait support) - Provider abstraction

**Error Handling:**
- Thiserror - Error type derivation with custom messages
- Anyhow - Flexible error handling where needed

**Logging:**
- Tracing - Structured logging framework
- Tracing-subscriber - Log filtering and output configuration

**Utilities:**
- Semver - Version comparison for settings/plugin compatibility
- Num_cpus - CPU count detection for thread pool sizing

## Configuration

**Environment:**
- Runtime configuration via environment variables:
  - `SLICECORE_THREAD_COUNT` - Override thread pool size
  - `SLICECORE_LOG_LEVEL` - Tracing level (info, debug, trace)
  - `SLICECORE_AI_PROVIDER` - AI provider selection (ollama, openai, etc.)
  - `SLICECORE_AI_MODEL` - Model name for selected provider
  - `SLICECORE_AI_KEY` - API key for cloud providers
  - `SLICECORE_CACHE_DIR` - Cache directory for profiles and analysis

**Build:**
- `Cargo.toml` - Workspace root manifest defining all crates and features
- `.cargo/config.toml` - Cargo configuration (link args, target-specific settings)
- Feature flags:
  - `default` - Native build with full features
  - `native` - Enables rayon, tokio, axum (incompatible with WASM)
  - `wasm` - Enables wasm-bindgen, web-sys, js-sys (excludes native-only crates)
  - `ai` - Includes slicecore-ai and AI integration
  - `plugins` - Includes slicecore-plugin for dynamic plugin loading
  - `server` - Includes REST/gRPC server (slicecore-api)

**Settings Schema:**
- TOML-defined declarative schema at `crates/slicecore-config/schema/`
- ~400+ configurable settings organized by category
- Validation rules, constraints, and dependencies defined in schema
- Profile hierarchy: Defaults → Printer → Filament → Quality → User

## Platform Requirements

**Development:**
- Rust 1.70+ (or latest stable)
- Git (for repository management)
- Standard build tools (gcc/clang for C dependencies)
- Optional: WASM toolchain (`wasm-pack`, `wasm-opt`) for WASM target
- Optional: Python 3.8+ (for Python binding development)
- Recommended: `cargo-watch` (auto-rebuild on save)

**Production:**
- Deployment targets:
  - Linux: glibc 2.31+ (Debian 10+, Ubuntu 20.04+, RHEL 8+)
  - macOS: 10.12+
  - Windows: 10 22H2 or Windows Server 2019+
  - Browser: Modern browser with WASM support (Firefox 79+, Chrome 74+, Safari 14.1+)

**Runtime:**
- No runtime dependencies beyond standard C library
- AI integration requires network access (optional)
- Plugin system optionally requires Wasmtime (WASM plugin support)

---

*Stack analysis: 2026-02-13*
