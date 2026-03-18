# Phase 36: Plugins Subcommand - Research

**Researched:** 2026-03-18
**Domain:** CLI subcommand design, plugin state management, plugin discovery pipeline
**Confidence:** HIGH

## Summary

Phase 36 adds a `slicecore plugins` CLI subcommand with `list`, `enable`, `disable`, `info`, and `validate` sub-subcommands. The implementation extends the existing plugin system (`slicecore-plugin` crate) with per-plugin `.status` files and updates the CLI (`slicecore-cli` crate) with a new subcommand module following established patterns (`csg`, `calibrate`).

The codebase already has all foundation pieces: `PluginRegistry` with `discover_and_load()`, `PluginManifest` with full metadata, `PluginInfo` struct, `discover_plugins()` scanning, and established CLI patterns for nested subcommands with `--json` output and `comfy-table` formatting. The primary new work is: (1) `.status` file read/write logic in the discovery module, (2) a new `plugins_command.rs` CLI module, (3) promoting `--plugin-dir` to a global flag, and (4) updating `discover_and_load()` to respect enabled/disabled state.

**Primary recommendation:** Follow the `csg_command.rs` pattern for the subcommand module, extend `discovery.rs` with `.status` file handling, and add an `enabled` field to `PluginInfo`. Keep the `.status` file format minimal JSON (`{"enabled": true}`).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Nested subcommands under `slicecore plugins`: `list`, `enable <name>`, `disable <name>`, `info <name>`, `validate <name>`
- Reserve namespace for future `install`, `uninstall`, `upgrade`, `search` commands
- `plugins list` defaults to human-readable table: Name, Version, Type, Category, Status
- `--json` flag outputs flat array of plugin objects
- `--category infill|postprocessor` and `--status enabled|disabled|error` filters
- `plugins info <name>` shows full manifest fields, status, filesystem path, capabilities, API version
- Config-first with CLI override: `plugin_dir` from config.toml, `--plugin-dir` flag overrides
- `--plugin-dir` promoted to global CLI flag
- Per-plugin `.status` file inside each plugin's directory: `{"enabled": true}` or `{"enabled": false}`
- `.status` file auto-created with `{"enabled": true}` during discovery if missing
- `plugin.toml` remains single source of truth for metadata
- `plugins enable` validates by attempting load; if load fails, remains disabled with error
- `plugins disable` also validates (confirms plugin identity before disabling)
- `plugins validate <name>` is standalone health check
- Disabled plugin referenced in slice config: hard error with actionable message
- Broken/invalid plugins shown in `plugins list` with status "Error: <brief reason>" and ??? for unknown fields
- Update `discover_and_load()` to read `.status` files and skip disabled plugins
- Add `plugins` group to `scripts/qa_tests`

### Claude's Discretion
- Exact table formatting and column widths
- `.status` file JSON structure beyond `enabled` field
- Error message wording details
- clap argument parsing implementation details

### Deferred Ideas (OUT OF SCOPE)
- Plugin marketplace / registry (install, uninstall, upgrade, search commands)
- Plugin signing and verification
- Plugin dependency resolution
- Per-plugin configuration beyond enable/disable
- Plugin update checking
- Plugin sandboxing controls per-plugin (memory limits, CPU fuel overrides)
- Auto-update notifications
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.5 | CLI argument parsing with derive macros | Already in use, `#[command(subcommand)]` pattern established |
| comfy-table | 7 | Human-readable table output | Already in use for calibrate list, stats display |
| serde_json | workspace | JSON serialization for `--json` output and `.status` files | Already in use across CLI |
| serde | workspace | Derive Serialize/Deserialize for status structs | Already in use |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| toml | workspace | Reading `plugin.toml` manifests | Already used in `discovery.rs` |
| thiserror | workspace | Error type definitions | Extend `PluginSystemError` with new variants |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| JSON `.status` file | TOML `.status` file | JSON is simpler for single-field state; TOML used for manifests. JSON is fine for machine state |
| Per-plugin `.status` | Global `plugins.json` | Per-plugin chosen by user -- state co-located with plugin, no sync issues on install/uninstall |

**Installation:** No new dependencies required. All libraries already in workspace.

## Architecture Patterns

### Recommended Project Structure
```
crates/slicecore-cli/src/
  plugins_command.rs          # New: PluginsCommand enum + run_plugins() handler
crates/slicecore-plugin/src/
  discovery.rs                # Modified: add .status file read/write/auto-create
  registry.rs                 # Modified: add enabled field to PluginInfo, status-aware discover_and_load
  status.rs                   # New: PluginStatus struct, read/write helpers
  error.rs                    # Modified: add StatusFileError variant
scripts/
  qa_tests                    # Modified: expand group_plugin() with real tests
```

### Pattern 1: Nested Subcommand Module (follow csg_command.rs)
**What:** A separate module file with a `#[derive(Subcommand)]` enum and a `run_plugins()` entry point
**When to use:** For the plugins subcommand, same as `CsgCommand` and `CalibrateCommand`
**Example:**
```rust
// crates/slicecore-cli/src/plugins_command.rs
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum PluginsCommand {
    /// List installed plugins
    List {
        /// Output as JSON instead of table
        #[arg(long)]
        json: bool,
        /// Filter by category (infill, postprocessor)
        #[arg(long)]
        category: Option<String>,
        /// Filter by status (enabled, disabled, error)
        #[arg(long)]
        status: Option<String>,
    },
    /// Enable a plugin
    Enable {
        /// Plugin name
        name: String,
    },
    /// Disable a plugin
    Disable {
        /// Plugin name
        name: String,
    },
    /// Show detailed plugin information
    Info {
        /// Plugin name
        name: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Validate a plugin (health check)
    Validate {
        /// Plugin name
        name: String,
    },
}

pub fn run_plugins(cmd: PluginsCommand, plugin_dir: &Path) -> Result<(), anyhow::Error> {
    // dispatch to handlers
}
```

### Pattern 2: Global CLI Flag (promote --plugin-dir)
**What:** Move `--plugin-dir` from `Slice` variant to the top-level `Cli` struct
**When to use:** To share across `slice` and `plugins` subcommands
**Example:**
```rust
#[derive(Parser)]
struct Cli {
    /// Plugin directory (overrides config plugin_dir)
    #[arg(long, global = true)]
    plugin_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}
```
Note: clap's `global = true` propagates the flag to all subcommands.

### Pattern 3: Plugin Status File
**What:** A minimal JSON file in each plugin directory tracking host-side state
**When to use:** For persist enable/disable state between runs
**Example:**
```rust
// crates/slicecore-plugin/src/status.rs
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStatus {
    pub enabled: bool,
}

impl Default for PluginStatus {
    fn default() -> Self {
        Self { enabled: true }
    }
}

pub fn read_status(plugin_dir: &Path) -> Result<PluginStatus, PluginSystemError> {
    let path = plugin_dir.join(".status");
    if !path.exists() {
        let status = PluginStatus::default();
        write_status(plugin_dir, &status)?;
        return Ok(status);
    }
    let contents = std::fs::read_to_string(&path)?;
    serde_json::from_str(&contents).map_err(|e| PluginSystemError::ManifestError {
        path,
        reason: format!("Invalid .status file: {e}"),
    })
}

pub fn write_status(plugin_dir: &Path, status: &PluginStatus) -> Result<(), PluginSystemError> {
    let path = plugin_dir.join(".status");
    let json = serde_json::to_string_pretty(status)?;
    std::fs::write(&path, json)?;
    Ok(())
}
```

### Pattern 4: Discovery with Status Awareness
**What:** Extend `discover_plugins()` to return status alongside manifest; extend `discover_and_load()` to skip disabled plugins
**When to use:** During plugin discovery pipeline
**Example:**
```rust
// In discovery.rs -- new function for plugins command
pub fn discover_plugins_with_status(
    dir: &Path,
) -> Result<Vec<(PathBuf, PluginManifest, PluginStatus)>, PluginSystemError> {
    // Similar to discover_plugins but also reads .status files
}

// In registry.rs -- modify discover_and_load
// Skip plugins where status.enabled == false
```

### Pattern 5: Broken Plugin Handling in List
**What:** Attempt to parse manifest; if it fails, create a partial entry with "Error" status
**When to use:** For `plugins list` to show broken plugins rather than silently hiding them
**Example:**
```rust
// When manifest parsing fails in discovery, instead of propagating error:
// Return a partial DiscoveredPlugin with error info
pub struct DiscoveredPlugin {
    pub dir: PathBuf,
    pub manifest: Option<PluginManifest>,  // None if parse failed
    pub status: PluginStatus,
    pub error: Option<String>,             // Error message if broken
}
```

### Anti-Patterns to Avoid
- **Global state file for all plugins:** User explicitly chose per-plugin `.status` files. Do not create a single `plugins-state.json`.
- **Silently ignoring broken plugins in list:** User wants broken plugins shown with "Error" status, not hidden.
- **Loading plugins during list/info:** The `list` and `info` commands should read manifests and status files only -- no need to actually load plugin binaries (except `enable` and `validate` which test loading).
- **Modifying plugin.toml:** The `.status` file is host-side state. Never write enabled/disabled state into `plugin.toml`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Table output | Custom column padding | `comfy-table` with `UTF8_FULL` preset | Already used in calibrate, stats display; handles terminal width, alignment |
| JSON output | Manual JSON string building | `serde_json::to_string_pretty` | Type-safe, consistent with all other `--json` outputs |
| Arg parsing | Manual string matching | clap derive with `#[command(subcommand)]` | Gives help text, shell completions, error messages for free |
| File atomicity | Direct `fs::write` | Write to `.status.tmp` then rename | Prevents corrupt `.status` on crash (optional improvement) |

**Key insight:** The codebase already has every library needed. This phase is purely composing existing patterns into a new subcommand.

## Common Pitfalls

### Pitfall 1: Breaking the Slice Command's --plugin-dir
**What goes wrong:** Promoting `--plugin-dir` to global changes how the `Slice` variant receives it, breaking existing usage.
**Why it happens:** Moving from variant-level to struct-level arg requires updating the match arm that extracts it.
**How to avoid:** After moving to global, update the `Commands::Slice` match arm to read from the top-level `Cli` struct instead of the variant. Run existing slice QA tests to verify.
**Warning signs:** `cargo test` fails; `slicecore slice --plugin-dir ...` stops working.

### Pitfall 2: Discovery Error Propagation vs Collection
**What goes wrong:** Current `discover_plugins()` returns `Err` on the first bad manifest, which aborts discovery entirely. The `plugins list` command needs to show ALL plugins including broken ones.
**Why it happens:** The existing function was designed for the load pipeline where one bad manifest should fail fast.
**How to avoid:** Create a separate `discover_all_plugins()` function that collects errors per-plugin instead of propagating. The existing `discover_plugins()` can remain for backward compatibility or be refactored.
**Warning signs:** `plugins list` returns error instead of showing partial results.

### Pitfall 3: Race Condition on .status File
**What goes wrong:** Two CLI invocations could read/write `.status` simultaneously.
**Why it happens:** No file locking.
**How to avoid:** For v1, this is acceptable -- CLI commands are typically sequential. Document that concurrent plugin management is not supported. Optionally use write-to-temp-then-rename for crash safety.
**Warning signs:** Corrupt `.status` file after interrupted write.

### Pitfall 4: Category Mapping
**What goes wrong:** The `PluginCapability` enum uses `InfillPattern` and `GcodePostProcessor`, but the user wants `--category infill|postprocessor` filter.
**Why it happens:** String matching needs to map user-facing names to enum variants.
**How to avoid:** Use clap `ValueEnum` for the category filter, mapping to `PluginCapability` variants. Display as human-friendly names in the table.
**Warning signs:** Filter doesn't match any plugins because of string mismatch.

### Pitfall 5: PluginInfo Missing Fields
**What goes wrong:** Current `PluginInfo` has name, description, plugin_kind, version -- but `plugins list` needs category (capability) and status (enabled/disabled/error). `plugins info` needs even more.
**Why it happens:** `PluginInfo` was designed for the registry's internal use, not for CLI display.
**How to avoid:** Either extend `PluginInfo` with optional fields (status, capabilities, path) or create a richer `PluginDisplayInfo` struct for the CLI layer. The latter is cleaner since it avoids polluting the registry API.
**Warning signs:** Missing data in table output, forced to reach back into manifest for display.

## Code Examples

### Table Output with comfy-table (from calibrate/mod.rs)
```rust
// Source: crates/slicecore-cli/src/calibrate/mod.rs
use comfy_table::{presets::UTF8_FULL, Table};

let mut table = Table::new();
table.load_preset(UTF8_FULL);
table.set_header(vec!["Test", "Description", "Parameters"]);
// Add rows...
println!("{table}");
```

### Subcommand Registration in main.rs (from existing pattern)
```rust
// Source: crates/slicecore-cli/src/main.rs:570-577
#[derive(Subcommand)]
enum Commands {
    // ... existing variants ...

    /// Calibration test print generation.
    #[command(subcommand)]
    Calibrate(calibrate::CalibrateCommand),

    /// CSG operations on meshes.
    #[command(subcommand)]
    Csg(csg_command::CsgCommand),

    // New: add this
    /// Manage installed plugins.
    #[command(subcommand)]
    Plugins(plugins_command::PluginsCommand),
}
```

### Dispatch in main.rs (from existing pattern)
```rust
// Source: crates/slicecore-cli/src/main.rs:826-833
Commands::Calibrate(cal_cmd) => {
    if let Err(e) = calibrate::run_calibrate(cal_cmd) {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}
Commands::Csg(csg_cmd) => {
    if let Err(e) = csg_command::run_csg(csg_cmd) {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}
```

### Existing .status File JSON Format
```json
{"enabled": true}
```

### Full Manifest Reference (plugin.toml)
```toml
library_filename = "libzigzag_infill.so"
plugin_type = "native"
capabilities = ["infill_pattern"]

[metadata]
name = "zigzag-infill"
version = "1.0.0"
description = "Zigzag infill pattern with configurable angle"
author = "Test Author"
license = "MIT"
min_api_version = "0.1.0"
max_api_version = "0.2.0"
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `--plugin-dir` on slice only | Global `--plugin-dir` flag | This phase | All subcommands can access plugin dir |
| No enable/disable | Per-plugin `.status` files | This phase | Plugins can be managed without deletion |
| `discover_plugins()` fails on first error | Error-collecting variant needed | This phase | `plugins list` shows broken plugins |
| `PluginInfo` without status | Extended with enabled/status field | This phase | CLI can display plugin state |

## Open Questions

1. **Atomic .status file writes**
   - What we know: Simple `fs::write` works for most cases
   - What's unclear: Whether crash-safe writes (tmp+rename) are worth the complexity for v1
   - Recommendation: Use simple `fs::write` for now; atomic writes are a minor improvement for later

2. **discover_and_load refactoring scope**
   - What we know: Current function fails fast on errors. Need status-aware version for slicing, and error-collecting version for listing.
   - What's unclear: Whether to refactor the existing function or create parallel functions
   - Recommendation: Add a new `discover_all_with_status()` for the plugins command, modify `discover_and_load()` to skip disabled plugins. Keeps backward compatibility clean.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test + scripts/qa_tests (bash) |
| Config file | Cargo.toml workspace |
| Quick run command | `cargo test -p slicecore-plugin --lib` |
| Full suite command | `cargo test --workspace && scripts/qa_tests --group plugin` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| N/A-01 | .status file read/write/auto-create | unit | `cargo test -p slicecore-plugin status` | Wave 0 |
| N/A-02 | discover_all_with_status collects errors | unit | `cargo test -p slicecore-plugin discovery` | Extend existing |
| N/A-03 | discover_and_load skips disabled | unit | `cargo test -p slicecore-plugin registry` | Extend existing |
| N/A-04 | plugins list table output | integration | `scripts/qa_tests --group plugin` | Wave 0 |
| N/A-05 | plugins list --json | integration | `scripts/qa_tests --group plugin` | Wave 0 |
| N/A-06 | plugins enable/disable round-trip | integration | `scripts/qa_tests --group plugin` | Wave 0 |
| N/A-07 | plugins info output | integration | `scripts/qa_tests --group plugin` | Wave 0 |
| N/A-08 | plugins validate | integration | `scripts/qa_tests --group plugin` | Wave 0 |
| N/A-09 | broken plugin shown in list | unit | `cargo test -p slicecore-plugin discovery` | Wave 0 |
| N/A-10 | disabled plugin hard error in slice | integration | `scripts/qa_tests --group plugin` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-plugin -p slicecore-cli --lib`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green + `scripts/qa_tests --group plugin,errors`

### Wave 0 Gaps
- [ ] `crates/slicecore-plugin/src/status.rs` -- new module with PluginStatus read/write
- [ ] `crates/slicecore-cli/src/plugins_command.rs` -- new CLI module
- [ ] Expand `scripts/qa_tests` group_plugin() with real test cases using fixture plugin dirs

## Sources

### Primary (HIGH confidence)
- Direct codebase inspection of all referenced files (registry.rs, discovery.rs, main.rs, csg_command.rs, calibrate/mod.rs, metadata.rs, error.rs, postprocess.rs)
- Existing patterns verified in working code

### Secondary (MEDIUM confidence)
- clap 4.5 `global = true` flag behavior -- verified from existing clap usage patterns in the codebase

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all libraries already in workspace, no new deps
- Architecture: HIGH - follows established patterns (csg_command, calibrate), straightforward extension
- Pitfalls: HIGH - identified from direct code inspection of discovery pipeline and PluginInfo struct

**Research date:** 2026-03-18
**Valid until:** 2026-04-18 (stable -- internal architecture, no external API changes)
