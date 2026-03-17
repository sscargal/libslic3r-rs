# External Integrations

**Analysis Date:** 2026-02-13

## APIs & External Services

**AI/LLM Providers:**
- OpenAI (GPT-4, GPT-3.5-turbo) - Settings recommendation, troubleshooting, seam optimization
  - SDK/Client: `reqwest` HTTP client with custom wrapper
  - Auth: `SLICECORE_AI_KEY` environment variable (API key)
  - Use: Profile suggestion, print failure prediction, G-code explanation

- Anthropic (Claude models) - Settings recommendation, analysis explanation
  - SDK/Client: `reqwest` HTTP client via custom trait
  - Auth: `SLICECORE_AI_KEY` environment variable
  - Use: Structured analysis output, natural language optimization reasoning

- Google Vertex AI - Vision-based support placement, model orientation
  - SDK/Client: `reqwest` with custom authentication
  - Auth: GCP service account credentials (JSON file)
  - Use: Multi-modal analysis if image-based features are provided

- OpenRouter API - Unified interface to multiple LLM providers
  - SDK/Client: `reqwest` HTTP client
  - Auth: `SLICECORE_AI_KEY` environment variable
  - Use: Provider-agnostic LLM access with fallback support

**Local AI Providers:**
- Ollama (local LLM runtime) - Self-hosted model inference
  - Connection: HTTP via `base_url` parameter (default: `http://localhost:11434`)
  - Use: Private, offline-capable settings recommendation
  - Model: Configurable via `SLICECORE_AI_MODEL` (e.g., `mistral`, `llama2`)

- vLLM (high-throughput LLM serving) - Local inference server
  - Connection: HTTP via `base_url` parameter
  - Use: Fast batch inference for model analysis and optimization
  - Model: Configurable model name

**Custom/Local ML:**
- ONNX Runtime - Local ML models for orientation optimization, failure prediction
  - Model format: ONNX `.onnx` files
  - Use: Deterministic, offline machine learning
  - Location: Models loaded from `SLICECORE_CACHE_DIR` or user-specified paths

## Data Storage

**Databases:**
- Not used in core library - Stateless design
- Optional for `slicecore-server`:
  - Job database: Any SQL database (PostgreSQL, SQLite) for job persistence
  - Configuration: Local filesystem only (TOML/JSON profiles)

**File Storage:**
- Local filesystem only - No cloud storage integration
- Input: STL, 3MF, OBJ, STEP files from local disk
- Output: G-code files, JSON metadata, WASM binaries written locally
- Cache: Configuration profiles, analysis results cached to `$SLICECORE_CACHE_DIR` (default: `~/.cache/slicecore/`)

**Caching:**
- In-memory caching:
  - Settings schema (loaded once, cached per process)
  - Mesh repair results (cached within a slicing session)
  - AI response cache via Reqwest client (configurable TTL)
- Persistent cache:
  - Profile analysis results (opt-in)
  - Model feature extraction (for repeated analysis)
  - Location: `~/.cache/slicecore/` or `SLICECORE_CACHE_DIR`

## Authentication & Identity

**Auth Provider:**
- Custom (API key-based) - No centralized identity provider
- Implementation:
  - OpenAI: Bearer token in Authorization header
  - Anthropic: x-api-key header
  - Google Vertex: OAuth 2.0 service account flow
  - OpenRouter: Bearer token
  - Local providers (Ollama, vLLM): No authentication

**Secrets Management:**
- Stored in: Environment variables only (never committed to git)
- Protection: `secrecy` crate ensures keys never logged, displayed, or serialized
- Key types:
  - `SLICECORE_AI_KEY` - OpenAI/Anthropic/OpenRouter API key
  - `GOOGLE_APPLICATION_CREDENTIALS` - Path to GCP service account JSON

## Monitoring & Observability

**Error Tracking:**
- Not integrated - Errors reported via Result types and structured logging
- Optional: Application can integrate with Sentry/similar via `tracing` subscriber

**Logs:**
- Approach: Structured logging via `tracing` crate
- Levels: error, warn, info, debug, trace
- Configuration: `SLICECORE_LOG_LEVEL` environment variable
- Output: Stdout/stderr (application controls via `tracing-subscriber`)
- Content:
  - Slicing progress and stage transitions
  - Algorithm decisions (layer count, infill pattern selected, etc.)
  - AI provider calls (model name, latency, confidence scores)
  - Plugin lifecycle events (loaded, initialized, errors)
  - Mesh repair operations and warnings

**Metrics:**
- Prometheus-compatible metrics via `slicecore-server`:
  - Endpoint: `GET /api/v1/metrics`
  - Metrics: Slice time, layers processed, memory used, AI provider latency
  - Format: OpenMetrics text format

## CI/CD & Deployment

**Hosting:**
- Not a hosted service - Distributed as library and CLI
- Optional server hosting:
  - Docker container: `slicecore-server` binary in Docker image
  - Deployment: Kubernetes, Docker Compose, or standalone binary
  - Dependencies: None (stateless, single binary)

**CI Pipeline:**
- GitHub Actions (conceptual, not yet implemented):
  - Test matrix: Linux (glibc), macOS (x86_64 + ARM64), Windows (x86_64)
  - Stages:
    - `cargo fmt --check` - Code formatting validation
    - `cargo clippy` - Linting
    - `cargo deny check` - License + security audit
    - `cargo test` - Unit tests (parallel via nextest)
    - `cargo bench` - Benchmark regression detection (vs. main branch)
    - `cargo build --target wasm32-unknown-unknown` - WASM compilation
    - `cargo fuzz` - Fuzzing (weekly)

**Build Artifacts:**
- Native binaries: `slicecore-cli`, `slicecore-server`
- Libraries: Rust library crate (for embedding)
- Python wheel: PyO3 bindings
- WASM: `slicecore.wasm` + JavaScript bindings
- Docker image: `libslic3r-rs:latest`

## Environment Configuration

**Required env vars:**
- None (all have defaults or are optional)

**Optional env vars:**
- `SLICECORE_THREAD_COUNT` - Thread pool size (default: CPU count)
- `SLICECORE_LOG_LEVEL` - Logging level (default: info)
- `SLICECORE_AI_PROVIDER` - Provider: ollama, openai, anthropic, google, openrouter, onnx (default: none)
- `SLICECORE_AI_MODEL` - Model name (default: depends on provider)
- `SLICECORE_AI_KEY` - API key for cloud providers
- `SLICECORE_AI_BASE_URL` - Base URL for local providers (Ollama, vLLM)
- `SLICECORE_CACHE_DIR` - Cache directory path
- `SLICECORE_MAX_THREADS` - Hard limit on thread pool size
- `GOOGLE_APPLICATION_CREDENTIALS` - Path to GCP service account JSON (for Google Vertex)
- `RUST_LOG` - Tracing filter (advanced, overrides `SLICECORE_LOG_LEVEL`)

**Secrets location:**
- Environment variables only - Recommended approach via `.env` file (git-ignored)
- Files: Not stored; loaded at runtime from environment
- Never hardcoded in codebase

## Webhooks & Callbacks

**Incoming:**
- None - Library design, no webhook listeners

**Outgoing (Server only):**
- Optional progress callbacks via trait:
  - `ProgressReporter` trait for slicing progress (stage, percent complete, estimated time remaining)
  - Implemented by application (CLI, server, embedding) to report to UI or external systems
  - No HTTP callbacks; all synchronous

**Plugin Communication:**
- Extension points (traits) that plugins implement:
  - `InfillPattern::generate()` - Infill generation
  - `GcodeDialect::format_move()` - G-code formatting
  - `SupportStrategy::generate_supports()` - Support generation
  - `Analyzer::analyze()` - Custom model analysis
  - `Optimizer::optimize()` - Custom parameter optimization
  - `SeamStrategy::place_seams()` - Seam placement
  - `PostProcessor::process()` - Post-slicing G-code modification

## Multi-Material & Printer Support

**Printer Integration:**
- Not hardware-integrated - Output is G-code for any compatible printer
- Firmware dialects supported (via `slicecore-gcode-gen`):
  - Marlin (standard RepRap)
  - Klipper (Klipper firmware)
  - RepRapFirmware (Duet controllers)
  - Bambu (BambuLab proprietary)
  - Custom (user-provided dialect plugins)

**Tool System:**
- Multi-material support in `slicecore-engine`:
  - Tool changes via T-code
  - Purge tower generation for nozzle cleaning
  - Wipe sequences customizable per tool

---

*Integration audit: 2026-02-13*
