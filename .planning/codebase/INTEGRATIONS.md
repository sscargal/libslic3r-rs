# External Integrations

**Analysis Date:** 2026-03-18

## APIs & External Services

**LLM Providers (all optional, via `slicecore-ai`):**

- **Anthropic Claude** — print profile suggestions, geometry analysis
  - SDK/Client: `reqwest` 0.12 (direct HTTP, no Anthropic SDK)
  - Endpoint: `https://api.anthropic.com/v1/messages`
  - Auth: `x-api-key` header; key stored in `AiConfig.api_key` as `secrecy::SecretString`
  - Version header: `anthropic-version: 2023-06-01`
  - Implementation: `crates/slicecore-ai/src/providers/anthropic.rs`

- **OpenAI** — print profile suggestions, geometry analysis
  - SDK/Client: `reqwest` 0.12 (direct HTTP, no OpenAI SDK)
  - Endpoint: `https://api.openai.com` (configurable via `AiConfig.base_url`)
  - Auth: Bearer token in Authorization header
  - Implementation: `crates/slicecore-ai/src/providers/openai.rs`

- **Ollama** — local LLM inference (default provider)
  - SDK/Client: `reqwest` 0.12 (direct HTTP)
  - Endpoint: `http://localhost:11434` (default; configurable via `AiConfig.base_url`)
  - Auth: None required
  - Implementation: `crates/slicecore-ai/src/providers/ollama.rs`

**Provider abstraction:**
- `AiProvider` trait in `crates/slicecore-ai/src/provider.rs` — async `complete()` + `capabilities()` + `name()`
- Provider selected at runtime via `AiConfig.provider` (`ProviderType` enum: `OpenAi`, `Anthropic`, `Ollama`)
- Config loaded from TOML via `AiConfig::from_toml()` — `crates/slicecore-ai/src/config.rs`

## Data Storage

**Databases:**
- None — no embedded or external database

**File Storage:**
- Local filesystem only
- Print profiles stored as TOML/JSON files in `profiles/` directory (subdirs: `prusaslicer/`, `orcaslicer/`, `bambustudio/`, `crealityprint/`)
- Profile search path: adjacent to binary → `SLICECORE_PROFILES_DIR` env var override → platform user data dir
- Discovery implementation: `crates/slicecore-engine/src/profile_resolve.rs`

**Caching:**
- None (no runtime cache layer; CI uses `Swatinem/rust-cache@v2` for build artifacts)

## Authentication & Identity

**Auth Provider:**
- None — no user authentication system
- AI API keys managed via `AiConfig` struct with `secrecy::SecretString` (prevents `Debug` leakage)
- Keys sourced from TOML config files provided by the caller — no hardcoded keys, no env var convention enforced in code

## Monitoring & Observability

**Error Tracking:**
- None — no Sentry, Datadog, or similar integration

**Logs:**
- No logging framework (no `tracing`, `log`, or `env_logger` dependency detected)
- CLI uses `indicatif` 0.17 for progress display and `comfy-table` 7 for tabular output
- Errors surfaced via `anyhow` chains in the CLI binary (`crates/slicecore-cli`)

## CI/CD & Deployment

**Hosting:**
- Library crates — no deployment target (distributed as source / Cargo dependency)
- CLI binary — cross-platform, self-contained native binary

**CI Pipeline:**
- GitHub Actions: `.github/workflows/ci.yml`
- Jobs:
  - `fmt` — `cargo fmt --all -- --check`
  - `clippy` — `cargo clippy --workspace -- -D warnings`
  - `test` — matrix across Ubuntu, macOS (aarch64 + x86_64), Windows
  - `test-linux-arm` — `aarch64-unknown-linux-gnu` via `houseabsolute/actions-rust-cross@v0`
  - `wasm` — build check for `wasm32-unknown-unknown` and `wasm32-wasip2`
  - `doc` — `cargo doc --no-deps --workspace` with `-D warnings`
- Triggers: push to `main`, `master`, `phase-*` branches; pull requests to `main`/`master`
- Actions used: `actions/checkout@v4`, `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`, `houseabsolute/actions-rust-cross@v0`

## Plugin System (External Code Loading)

**Native Plugins (.so / .dll / .dylib):**
- Loaded via `abi_stable` 0.11 FFI-safe shared library interface
- Plugin manifest: `plugin.toml` alongside the shared library
- Discovery: `crates/slicecore-plugin/src/discovery.rs`
- Registry: `crates/slicecore-plugin/src/registry.rs`
- Example: `plugins/examples/native-zigzag-infill/`

**WASM Plugins (.wasm component):**
- Loaded via `wasmtime` 41 with Component Model + Cranelift JIT
- WASI support via `wasmtime-wasi` 41 (WASI P2)
- Contract defined in WIT: `crates/slicecore-plugin/wit/slicecore-plugin.wit`
- Plugin authors use `wit-bindgen` 0.53 to generate bindings
- Sandboxed execution — fuel exhaustion tested in `crates/slicecore-plugin/tests/integration_tests.rs`
- Example: `plugins/examples/wasm-spiral-infill/` (targets `wasm32-wasip2`)

## Environment Configuration

**Required env vars:**
- None strictly required — all defaults are sensible
- `SLICECORE_PROFILES_DIR` — optional override for print profile search directory

**Secrets location:**
- AI API keys passed in-process via `AiConfig` (loaded from caller-supplied TOML)
- No `.env` files, secrets directories, or credential files detected in the repository

## Webhooks & Callbacks

**Incoming:**
- None

**Outgoing:**
- None — all external HTTP calls are outbound LLM API requests initiated synchronously per-request

---

*Integration audit: 2026-03-18*
