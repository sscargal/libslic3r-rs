# Phase 10: CLI Feature Integration - Research

**Researched:** 2026-02-18
**Domain:** Rust CLI integration -- feature flags, clap subcommands, async runtime bridging
**Confidence:** HIGH

## Summary

Phase 10 is a narrow, well-scoped integration phase. All required functionality already exists in the library crates (`slicecore-engine`, `slicecore-ai`, `slicecore-plugin`). The work is purely **wiring**: enabling feature flags in `slicecore-cli/Cargo.toml`, adding a new `ai-suggest` clap subcommand, loading plugins from config, and writing integration tests.

The current CLI (`crates/slicecore-cli/src/main.rs`) has three subcommands (`slice`, `validate`, `analyze`) using clap 4.5 with derive macros. It depends on `slicecore-engine` **without** the `plugins` or `ai` features enabled. The engine already has `#[cfg(feature = "plugins")]` and `#[cfg(feature = "ai")]` impl blocks with `Engine::with_plugin_registry()` and `Engine::suggest_profile()` methods ready to use.

**Primary recommendation:** Enable `plugins` and `ai` features on the `slicecore-engine` dependency in `slicecore-cli/Cargo.toml`, add `slicecore-ai` and `slicecore-plugin` as direct dependencies (for AI config parsing and plugin registry creation), add a `tokio` dependency for async runtime, create an `AiSuggest` subcommand variant, wire plugin loading into `cmd_slice` when `plugin_dir` is set or `infill_pattern = { plugin = "..." }` is used, and write integration tests using `std::process::Command` against the compiled binary.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.5 | CLI argument parsing with derive macros | Already in use, derive API is the standard approach |
| tokio | 1 | Async runtime for AI provider HTTP calls | Already a dep of `slicecore-ai`; needed for `suggest_profile_sync` |
| slicecore-engine | workspace (features: plugins, ai) | Engine with plugin + AI features enabled | Feature-gated re-exports already exist |
| slicecore-ai | workspace | Direct dep for `AiConfig::from_toml()`, `create_provider()` | Needed for CLI-level AI config parsing |
| slicecore-plugin | workspace | Direct dep for `PluginRegistry::new()`, `discover_and_load()` | Needed for CLI-level plugin loading |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tempfile | 3 | Temp directories for integration tests | Already in dev-dependencies |
| serde_json | 1 | JSON output in integration tests | Already in dev-dependencies |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Direct `slicecore-ai` dep in CLI | Only use engine re-exports | Engine re-exports types but not `AiConfig::from_toml()` or `create_provider()` -- direct dep needed |
| Adding tokio to CLI | Using the existing `suggest_profile_sync` wrapper | `suggest_profile_sync` already creates its own tokio runtime internally, so CLI does NOT actually need tokio as a direct dependency -- the sync wrapper handles it |

## Architecture Patterns

### Current CLI Structure
```
crates/slicecore-cli/
  src/
    main.rs          # Single file: Cli struct, Commands enum, cmd_* functions
  tests/
    cli_output.rs    # Integration tests via std::process::Command
  Cargo.toml         # Currently: clap, slicecore-engine (no features), slicecore-fileio, etc.
```

### Target CLI Structure (After Phase 10)
```
crates/slicecore-cli/
  src/
    main.rs          # Extended: AiSuggest subcommand, plugin loading in cmd_slice
  tests/
    cli_output.rs    # Existing tests (unchanged)
    cli_ai.rs        # NEW: ai-suggest subcommand integration tests
    cli_plugins.rs   # NEW: plugin-based infill integration tests
  Cargo.toml         # Updated: slicecore-engine with features=["plugins","ai"], slicecore-ai, slicecore-plugin
```

### Pattern 1: Feature-Gated Engine Dependencies
**What:** Enable `plugins` and `ai` features on `slicecore-engine` in CLI's Cargo.toml
**When to use:** Always -- this is the primary mechanism for exposing these features

**Cargo.toml change:**
```toml
[dependencies]
slicecore-engine = { path = "../slicecore-engine", features = ["plugins", "ai"] }
slicecore-ai = { path = "../slicecore-ai" }
slicecore-plugin = { path = "../slicecore-plugin" }
```

**Source:** Verified from `crates/slicecore-engine/Cargo.toml` lines 9-12:
```toml
[features]
default = []
plugins = ["dep:slicecore-plugin"]
ai = ["dep:slicecore-ai"]
```

### Pattern 2: Clap Subcommand Addition
**What:** Add `AiSuggest` variant to the existing `Commands` enum
**When to use:** For the `slicecore ai-suggest input.stl` command

**Example:**
```rust
// Source: Verified pattern from existing main.rs Commands enum
#[derive(Subcommand)]
enum Commands {
    // ... existing variants ...

    /// Suggest optimal print settings using AI analysis of mesh geometry
    AiSuggest {
        /// Input mesh file (STL or 3MF)
        input: PathBuf,

        /// AI provider configuration file (TOML). Uses Ollama defaults if not specified.
        #[arg(short = 'a', long = "ai-config")]
        ai_config: Option<PathBuf>,

        /// Output format: "text" (default) or "json"
        #[arg(long, default_value = "text")]
        format: String,
    },
}
```

### Pattern 3: Plugin Loading in Slice Command
**What:** When config has `plugin_dir` set or `infill_pattern = { plugin = "..." }`, load plugins and attach registry to engine
**When to use:** In `cmd_slice` function, after config is loaded, before engine is created

**Example:**
```rust
// Source: Verified from engine.rs lines 291-307 and registry.rs lines 119-166
let mut engine = Engine::new(print_config.clone());

// Load plugins if plugin_dir is configured
if let Some(ref plugin_dir) = print_config.plugin_dir {
    let mut registry = slicecore_plugin::PluginRegistry::new();
    match registry.discover_and_load(std::path::Path::new(plugin_dir)) {
        Ok(loaded) => {
            if !loaded.is_empty() {
                eprintln!("Loaded {} plugin(s):", loaded.len());
                for info in &loaded {
                    eprintln!("  - {} ({})", info.name, info.description);
                }
            }
            engine = engine.with_plugin_registry(registry);
        }
        Err(e) => {
            eprintln!("Warning: Failed to load plugins from '{}': {}", plugin_dir, e);
        }
    }
}
```

### Pattern 4: AI Suggest Command Implementation
**What:** Load mesh, extract features, call LLM, display suggestion
**When to use:** When `ai-suggest` subcommand is invoked

**Example:**
```rust
// Source: Verified from suggest.rs lines 89-98 and config.rs lines 123-147
fn cmd_ai_suggest(input: &PathBuf, ai_config_path: Option<&std::path::Path>, format: &str) {
    // 1. Load mesh (same as cmd_analyze)
    let data = std::fs::read(input).unwrap_or_else(|e| { ... });
    let mesh = slicecore_fileio::load_mesh(&data).unwrap_or_else(|e| { ... });

    // 2. Load AI config
    let ai_config = if let Some(path) = ai_config_path {
        let toml_str = std::fs::read_to_string(path).unwrap_or_else(|e| { ... });
        slicecore_ai::AiConfig::from_toml(&toml_str).unwrap_or_else(|e| { ... })
    } else {
        slicecore_ai::AiConfig::default() // Ollama, llama3.2
    };

    // 3. Create engine and call suggest_profile
    let engine = slicecore_engine::Engine::new(slicecore_engine::PrintConfig::default());
    match engine.suggest_profile(&mesh, &ai_config) {
        Ok(suggestion) => {
            // Display results
        }
        Err(e) => {
            eprintln!("Error: AI suggestion failed: {}", e);
            std::process::exit(1);
        }
    }
}
```

### Anti-Patterns to Avoid
- **Adding tokio as a direct CLI dependency:** The `suggest_profile_sync()` wrapper in `slicecore-ai` already creates a single-threaded tokio runtime internally. The CLI should call `Engine::suggest_profile()` which uses this sync wrapper. No need for `#[tokio::main]` or tokio in CLI's Cargo.toml.
- **Conditional compilation in main.rs:** Since features are always enabled in the CLI binary (unconditionally), do NOT use `#[cfg(feature = "...")]` in main.rs. The engine's feature gates handle everything internally.
- **Blocking on async directly:** Do NOT call `suggest_profile()` (async) directly from CLI. Use `Engine::suggest_profile()` which internally calls `suggest_profile_sync()`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| AI config parsing | Custom CLI arg parsing for provider/model/key | `AiConfig::from_toml()` + optional TOML file | Already handles defaults, validation, secret handling |
| Plugin discovery | Manual directory scanning | `PluginRegistry::discover_and_load()` | Handles manifest parsing, version checks, error recovery |
| Async runtime for AI | Custom runtime management | `Engine::suggest_profile()` (sync wrapper) | Already creates single-threaded tokio runtime internally |
| JSON output of suggestions | Manual JSON formatting | `serde_json::to_string_pretty(&suggestion)` | `ProfileSuggestion` already derives `Serialize` |
| CLI argument parsing | Manual arg parsing | clap derive macros | Already used throughout the CLI |

**Key insight:** Everything needed for Phase 10 already exists as library APIs. The CLI work is purely integration/wiring with zero new algorithms.

## Common Pitfalls

### Pitfall 1: Forgetting to Enable Both Features
**What goes wrong:** Enabling only `plugins` or only `ai` but not both in `slicecore-engine` dependency
**Why it happens:** They're independent features that could be enabled separately
**How to avoid:** Set `features = ["plugins", "ai"]` in a single dependency line. Success criterion 1 explicitly requires both.
**Warning signs:** Compilation succeeds but `Engine::suggest_profile()` or `Engine::with_plugin_registry()` is not available

### Pitfall 2: Plugin Loading Without plugin_dir Config
**What goes wrong:** User specifies `infill_pattern = { plugin = "zigzag" }` in config but doesn't set `plugin_dir`, so plugin isn't loaded and engine returns `EngineError::Plugin { plugin: "zigzag", message: "... not found" }`
**Why it happens:** Config has two related but independent fields
**How to avoid:** In `cmd_slice`, if infill_pattern is `Plugin(_)` and plugin_dir is None, emit a helpful error message explaining that `plugin_dir` must be set. Or: accept `--plugin-dir` as a CLI flag too.
**Warning signs:** Test with plugin infill passes in library tests but fails in CLI tests

### Pitfall 3: AI Suggest Requires Running Ollama
**What goes wrong:** Integration tests for `ai-suggest` fail in CI because Ollama isn't running
**Why it happens:** Default AI config points to `localhost:11434` which won't exist in CI
**How to avoid:** Integration tests for AI should test CLI argument parsing and error handling (e.g., connection refused is expected), not actual LLM responses. Use `--ai-config` pointing to a nonexistent server and verify the CLI produces a sensible error. Alternatively, mock via a test fixture.
**Warning signs:** Tests pass locally (Ollama running) but fail in CI

### Pitfall 4: Native Plugin .so Path Issues
**What goes wrong:** Integration tests that load the native zigzag plugin fail because the `.so` file isn't at the expected path
**Why it happens:** The plugin is in a separate workspace-excluded crate; `cargo test` doesn't build it automatically
**How to avoid:** Integration tests must first build the plugin (or have a pre-built fixture). The plugin manifest specifies `library = "libnative_zigzag_infill.so"` but the actual built artifact may be at `target/debug/libnative_zigzag_infill.so`. Tests should either: (a) build the plugin as a test setup step, or (b) use a mock/builtin plugin for testing. Option (b) is more reliable for CI.
**Warning signs:** Plugin test fails with "library not found" or "no such file"

### Pitfall 5: Help Text Sprawl
**What goes wrong:** Adding verbose plugin and AI documentation in clap `about` strings makes `--help` unusable
**Why it happens:** Trying to document everything in help text
**How to avoid:** Keep clap `about` strings concise. Add a `long_about` or `after_help` section for detailed documentation. Consider a `--help-ai` or `--help-plugins` flag pattern, or point to a documentation URL.
**Warning signs:** `slicecore --help` outputs more than ~40 lines

## Code Examples

Verified patterns from the existing codebase:

### Enabling Features in Cargo.toml
```toml
# Source: crates/slicecore-engine/Cargo.toml (existing feature definitions)
# These features ALREADY exist and are tested:
[features]
default = []
plugins = ["dep:slicecore-plugin"]
ai = ["dep:slicecore-ai"]
```

### Engine AI Suggest (Already Implemented)
```rust
// Source: crates/slicecore-engine/src/engine.rs lines 1805-1828
#[cfg(feature = "ai")]
impl Engine {
    pub fn suggest_profile(
        &self,
        mesh: &slicecore_mesh::TriangleMesh,
        ai_config: &slicecore_ai::AiConfig,
    ) -> Result<slicecore_ai::ProfileSuggestion, slicecore_ai::AiError> {
        let provider = slicecore_ai::create_provider(ai_config)?;
        slicecore_ai::suggest_profile_sync(provider.as_ref(), mesh)
    }
}
```

### Engine Plugin Registry (Already Implemented)
```rust
// Source: crates/slicecore-engine/src/engine.rs lines 304-307
#[cfg(feature = "plugins")]
pub fn with_plugin_registry(mut self, registry: slicecore_plugin::PluginRegistry) -> Self {
    self.plugin_registry = Some(registry);
    self
}
```

### Plugin TOML Config Pattern
```toml
# Source: crates/slicecore-engine/src/engine.rs line 3098 (test)
# This is how a user config enables a plugin infill pattern:
infill_pattern = { plugin = "zigzag" }
plugin_dir = "./plugins"
```

### AI Config TOML Format
```toml
# Source: crates/slicecore-ai/src/config.rs tests
# Ollama (default, no key needed):
provider = "ollama"
model = "llama3.2"
base_url = "http://localhost:11434"

# OpenAI:
provider = "open_ai"
model = "gpt-4o"
api_key = "sk-..."

# Anthropic:
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key = "sk-ant-..."
```

### CLI Integration Test Pattern (Existing)
```rust
// Source: crates/slicecore-cli/tests/cli_output.rs lines 64-74
fn cli_binary() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.push("slicecore");
    path
}

// Usage in tests:
let output = Command::new(cli_binary())
    .args(["ai-suggest", stl_path.to_str().unwrap()])
    .output()
    .expect("failed to run slicecore CLI");
```

### ProfileSuggestion Display Fields
```rust
// Source: crates/slicecore-ai/src/profile.rs lines 26-74
// All fields that should be displayed in CLI output:
pub struct ProfileSuggestion {
    pub layer_height: f64,        // mm
    pub wall_count: u32,
    pub infill_density: f64,      // 0.0-1.0
    pub infill_pattern: String,
    pub support_enabled: bool,
    pub support_overhang_angle: f64, // degrees
    pub perimeter_speed: f64,     // mm/s
    pub infill_speed: f64,        // mm/s
    pub nozzle_temp: f64,         // C
    pub bed_temp: f64,            // C
    pub brim_width: f64,          // mm
    pub reasoning: String,
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| N/A (new integration) | Feature-gated optional deps in Cargo.toml | Rust 2021 edition / Cargo | Clean conditional compilation without code duplication |
| Manual arg parsing | clap 4.x derive macros | clap 3->4 migration | Less boilerplate, automatic help generation |

**Deprecated/outdated:**
- None relevant. The existing codebase is already on current versions of all dependencies.

## Open Questions

1. **Should `--plugin-dir` be a CLI flag in addition to config?**
   - What we know: Config has `plugin_dir` field; success criteria mention "config file with `infill_pattern = { plugin = "zigzag" }`"
   - What's unclear: Whether a `--plugin-dir` CLI flag should also exist for convenience
   - Recommendation: Add `--plugin-dir` as an optional CLI flag on the `slice` subcommand that overrides the config value. This makes testing and one-off usage easier.

2. **How should ai-suggest handle Ollama not running?**
   - What we know: Default provider is Ollama at localhost:11434. If not running, reqwest will get connection refused.
   - What's unclear: What error message to show, how integration tests should handle this
   - Recommendation: Catch the HTTP error and display a user-friendly message: "Failed to connect to Ollama at localhost:11434. Is Ollama running? Start it with 'ollama serve' or configure a different provider with --ai-config."

3. **Integration test strategy for plugin loading**
   - What we know: Plugin examples are workspace-excluded crates that must be built separately. The native zigzag plugin produces a platform-specific `.so`/`.dll`/`.dylib`.
   - What's unclear: Whether integration tests should build the plugin or use a test fixture
   - Recommendation: Use `PluginRegistry::register_infill_plugin()` for unit tests (no filesystem needed). For CLI integration tests, build the native plugin as a test setup step using `cargo build --manifest-path plugins/examples/native-zigzag-infill/Cargo.toml` and construct a temporary plugin directory. If this is too fragile, verify plugin CLI plumbing with a config that specifies a nonexistent plugin and assert the error message is helpful.

## Sources

### Primary (HIGH confidence)
- `crates/slicecore-cli/Cargo.toml` -- Current CLI dependencies (no features enabled)
- `crates/slicecore-cli/src/main.rs` -- Current CLI structure (278 lines, 3 subcommands)
- `crates/slicecore-engine/Cargo.toml` -- Feature definitions (plugins, ai)
- `crates/slicecore-engine/src/engine.rs` -- `Engine::suggest_profile()` (line 1820), `Engine::with_plugin_registry()` (line 305)
- `crates/slicecore-engine/src/lib.rs` -- Feature-gated re-exports (lines 98-107)
- `crates/slicecore-engine/src/config.rs` -- `plugin_dir` field (line 218)
- `crates/slicecore-ai/src/config.rs` -- `AiConfig::from_toml()` (line 144)
- `crates/slicecore-ai/src/suggest.rs` -- `suggest_profile_sync()` (line 89)
- `crates/slicecore-plugin/src/registry.rs` -- `PluginRegistry::discover_and_load()` (line 146)
- `crates/slicecore-cli/tests/cli_output.rs` -- Existing integration test patterns
- `.planning/v1.0-MILESTONE-AUDIT.md` -- Gap analysis confirming PLUGIN-05 and AI-03 partial

### Secondary (MEDIUM confidence)
- `.planning/ROADMAP.md` -- Phase 10 success criteria and planned plan files (10-01, 10-02, 10-03)
- `plugins/examples/native-zigzag-infill/` -- Example plugin structure and manifest format

### Tertiary (LOW confidence)
- None. All findings are verified from codebase inspection.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- All libraries already in use, no new dependencies except forwarding existing ones to CLI
- Architecture: HIGH -- Extending an existing CLI with well-documented engine APIs
- Pitfalls: HIGH -- Based on direct inspection of existing code, error paths, and test patterns

**Research date:** 2026-02-18
**Valid until:** 2026-03-18 (stable -- no external dependencies changing, purely internal integration)
