# Phase 30: CLI Profile Composition and Slice Workflow - Research

**Researched:** 2026-03-14
**Domain:** CLI workflow, TOML profile merging, provenance tracking, progress reporting
**Confidence:** HIGH

## Summary

Phase 30 implements the core user-facing workflow for composing multiple profile layers (machine + filament + process) into a final PrintConfig, with provenance tracking and an enhanced `slice` command. The existing codebase already has extensive infrastructure for TOML-based profile management: `toml::Value` tree manipulation in `profile_library.rs` and `profile_convert.rs`, `ProfileIndexEntry` with 21k+ profiles, and `find_profiles_dir()` for directory discovery. The new work builds on these foundations.

The central architectural decision -- operating on `toml::Value` trees rather than building a parallel Option<T> struct -- is validated by the existing `merge_inheritance()` function in `profile_library.rs` which already does exactly this pattern: serialize to `toml::Value::Table`, merge tables, deserialize back to `PrintConfig`. The new profile composition system generalizes this pattern to a 5-layer merge with provenance tracking.

**Primary recommendation:** Build a `ProfileResolver` + `ProfileComposer` in `slicecore-engine` that operates on `toml::Value::Table` trees, reusing the existing `toml::Value` merge pattern from `profile_library.rs`. Add `indicatif` for progress bars and `sha2` for profile checksums. The CLI changes are extensive but structurally straightforward -- extending the existing clap derive-based command structure.

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions
- 5-layer merge with provenance tracking: Defaults -> Machine -> Filament -> Process -> User overrides -> --set CLI flags
- Machine + filament are required (safety-critical). Process defaults to built-in "Standard" quality
- `--unsafe-defaults` escape hatch allows slicing without profiles (dev/testing only, with warning)
- Profiles declare their type via `profile_type` field, validated against the flag used
- Single-level `inherits` field supported for user profiles extending library profiles
- Parallel provenance map: `HashMap<String, FieldSource>` alongside PrintConfig
- `FieldSource` records: source type, file path, what it overrode
- PrintConfig itself stays clean -- no generics or wrappers per field
- Profile checksums (SHA256) included in G-code header and saved configs
- TOML partial deserialization + field-level tracking via toml::Value tree deep-merge
- `--set` values auto-coerced from string as TOML literals
- `--set` validates keys against known PrintConfig field paths with "did you mean?" suggestions
- Start/end G-code template variables resolved during merge using final merged values
- New CLI flags: `-m/--machine`, `-f/--filament`, `-p/--process`, `--overrides`, `--set key=value`, `--save-config`, `--show-config`, `--dry-run`, `--no-log`, `--log-file`, `--force`
- `--config` kept as replay path, mutually exclusive with -m/-f/-p
- Name-or-path auto-detection: values containing `/` or ending in `.toml` treated as file paths
- ProfileResolver as shared module in slicecore-engine
- Search order: user profiles (~/.slicecore/profiles/) -> library index -> library filesystem scan
- Case-insensitive substring matching, exact ID match prioritized
- User profiles win over library profiles for same short name
- Existing --config flag stays, works as before
- Existing CLI tests using --config continue to work
- G-code output only (model.stl -> model.gcode)
- Embedded config in G-code header with reproduce command, profile IDs, checksums, version, timestamp
- Log file always created by default (model.log), --no-log to suppress
- indicatif for progress bar, text fallback for non-TTY
- Structured exit codes: 0=success, 1=general, 2=config/profile, 3=mesh, 4=safety validation
- Config validation: warn on suspicious values, error on dangerous values, --force to override
- list-profiles, search-profiles, show-profile updated to use ProfileResolver
- Ship minimal built-in profiles (Generic PLA, Generic PETG, common printers)
- Config merge logic is WASM-safe (no filesystem access needed)
- ProfileResolver is CLI/server only

### Claude's Discretion
- Inheritance depth limit enforcement approach
- Exact built-in profile set (which printers/filaments to ship)
- indicatif progress bar styling and layout details
- Internal ProfileResolver data structures and caching strategy
- Performance optimization (defer unless profiling shows issues)
- --overrides file format auto-detection implementation
- Help text and documentation wording

### Deferred Ideas (OUT OF SCOPE)
- Job output directory system
- Slice manifest file (.slicecore.toml)
- 3MF project output
- Multi-extruder CLI support (multiple -f flags)
- Profile management commands (create/edit/delete)
- Custom metadata via --set
- Stdin pipe support
- Profile library distribution/versioning
- Batch slicing

</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| toml | 0.8 | TOML parsing, Value tree manipulation, serialization | Already in workspace, used extensively for profile work |
| clap | 4.5 | CLI argument parsing with derive macros | Already used for all CLI commands |
| serde / serde_json | 1 | Serialization framework | Already in workspace |
| indicatif | 0.17 | Terminal progress bars and spinners | De-facto standard for Rust CLI progress reporting |
| sha2 | 0.10 | SHA-256 checksums for profile files | Standard RustCrypto crate, pure Rust, WASM-compatible |
| dirs | 5 | Cross-platform user directory resolution (~/.slicecore/) | Standard for home directory access |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| strsim | 0.11 | String similarity for "did you mean?" suggestions | --set key validation, profile name typo suggestions |
| console | 0.15 | Terminal detection (is_term) for TTY/non-TTY handling | Already a transitive dependency of indicatif |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| indicatif | println! progress | indicatif handles TTY detection, multi-line, spinner -- worth the dependency |
| sha2 | sha256 (simpler) | sha2 is the standard RustCrypto implementation; sha256 crate wraps it anyway |
| dirs | home | dirs provides more paths (config_dir, data_dir); home only gives home |
| strsim | edit_distance | strsim provides multiple algorithms (Jaro-Winkler better for typo detection) |

**Installation:**
```bash
cargo add indicatif@0.17 sha2@0.10 dirs@5 strsim@0.11
```

Note: `toml`, `clap`, `serde`, `serde_json` are already workspace dependencies.

## Architecture Patterns

### Recommended Project Structure
```
crates/slicecore-engine/src/
  profile_compose.rs       # NEW: TOML value-tree merge, provenance tracking, validation
  profile_resolve.rs       # NEW: ProfileResolver (name->path resolution, search)
  config.rs                # EXISTING: PrintConfig (unchanged struct, new helper methods)
  profile_library.rs       # EXISTING: batch conversion, index (minor updates)
  profile_convert.rs       # EXISTING: conversion utils (reuse round_floats_in_value)

crates/slicecore-cli/src/
  main.rs                  # MODIFIED: enhanced Slice command, updated profile commands
  slice_workflow.rs        # NEW: orchestrate resolve->compose->validate->slice->output
  progress.rs              # NEW: indicatif progress bar wrapper with TTY detection
```

### Pattern 1: TOML Value Tree Merge with Provenance
**What:** Deep-merge `toml::Value::Table` trees layer by layer, recording which layer set each field.
**When to use:** Profile composition (the core of this phase).
**Example:**
```rust
// Source: existing pattern in profile_library.rs:189-285
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum SourceType {
    Default,
    Machine,
    Filament,
    Process,
    UserOverride,
    CliSet,
}

#[derive(Debug, Clone)]
pub struct FieldSource {
    pub source_type: SourceType,
    pub file_path: Option<String>,
    pub overrode: Option<Box<FieldSource>>,
}

pub struct ComposedConfig {
    pub config: PrintConfig,
    pub provenance: HashMap<String, FieldSource>,
    pub warnings: Vec<String>,
    pub profile_checksums: Vec<(String, String)>, // (path, sha256)
}

/// Deep-merge a layer's TOML table into the base table, recording provenance.
fn merge_layer(
    base: &mut toml::map::Map<String, toml::Value>,
    layer: &toml::map::Map<String, toml::Value>,
    source: &FieldSource,
    provenance: &mut HashMap<String, FieldSource>,
    prefix: &str,
) {
    for (key, value) in layer {
        let full_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };

        match (base.get(key), value) {
            // Recursive merge for nested tables
            (Some(toml::Value::Table(_)), toml::Value::Table(child_table)) => {
                if let Some(toml::Value::Table(base_table)) = base.get_mut(key) {
                    merge_layer(base_table, child_table, source, provenance, &full_key);
                }
            }
            // Leaf value: override and record provenance
            _ => {
                let previous = provenance.get(&full_key).cloned();
                let mut new_source = source.clone();
                if let Some(prev) = previous {
                    new_source.overrode = Some(Box::new(prev));
                }
                provenance.insert(full_key, new_source);
                base.insert(key.clone(), value.clone());
            }
        }
    }
}
```

### Pattern 2: Profile Resolution Chain
**What:** Resolve a profile name/path to a concrete TOML file, searching multiple locations.
**When to use:** When -m, -f, or -p flag values need to be resolved.
**Example:**
```rust
pub struct ProfileResolver {
    user_dir: Option<PathBuf>,       // ~/.slicecore/profiles/
    library_dirs: Vec<PathBuf>,      // imported profile directories
    index: Option<ProfileIndex>,     // loaded index.json
}

pub struct ResolvedProfile {
    pub path: PathBuf,
    pub source: ProfileSource, // User or Library
    pub profile_type: String,  // machine/filament/process
    pub name: String,
    pub checksum: String,      // SHA-256 of file contents
}

pub enum ProfileSource {
    User,
    Library { vendor: String },
}

impl ProfileResolver {
    pub fn resolve(
        &self,
        query: &str,
        expected_type: &str,
    ) -> Result<ResolvedProfile, ProfileError> {
        // 1. Check if query is a file path (contains '/' or ends with '.toml')
        // 2. Search user profiles (exact match, then substring)
        // 3. Search library index (exact ID match, then substring)
        // 4. Error with suggestions if ambiguous or not found
    }
}
```

### Pattern 3: Config Validation with Severity Levels
**What:** Validate merged PrintConfig values, categorizing issues as warnings vs errors.
**When to use:** After merge, before slicing.
**Example:**
```rust
pub enum ValidationSeverity {
    Warning,  // Suspicious but allowed
    Error,    // Dangerous, blocked unless --force
}

pub struct ValidationIssue {
    pub field: String,
    pub message: String,
    pub severity: ValidationSeverity,
    pub value: String,
}

pub fn validate_config(config: &PrintConfig) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    // Warning: layer height > nozzle diameter
    if config.layer_height > config.machine.nozzle_diameter() {
        issues.push(ValidationIssue {
            field: "layer_height".into(),
            message: format!(
                "Layer height ({:.2}mm) exceeds nozzle diameter ({:.2}mm)",
                config.layer_height, config.machine.nozzle_diameter()
            ),
            severity: ValidationSeverity::Warning,
            ..
        });
    }

    // Error: nozzle temp exceeds machine max
    if let Some(max) = config.machine.max_nozzle_temp {
        if config.filament.nozzle_temp() > max {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                ..
            });
        }
    }

    issues
}
```

### Anti-Patterns to Avoid
- **Option<T> wrapper per field:** Creating a parallel struct with `Option<f64>` for every PrintConfig field. The TOML Value tree approach avoids this entirely -- the existing codebase already validates this pattern works.
- **Deserializing partial profiles directly into PrintConfig:** This would fill missing fields with defaults, losing the distinction between "not set" and "set to default". Always work with `toml::Value::Table` for partial profiles.
- **Hardcoding profile paths:** Always use `ProfileResolver` for all profile access, including the existing `list-profiles`, `search-profiles`, and `show-profile` commands.
- **Blocking progress bar on non-TTY:** Always check `std::io::stderr().is_terminal()` before using animated progress bars; fall back to text lines for pipes/CI.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Progress bars | Custom print-based progress | `indicatif::ProgressBar` | Handles TTY detection, multi-line, elapsed time, ETA, thread safety |
| SHA-256 hashing | Custom hash function | `sha2::Sha256` | Cryptographic correctness, pure Rust, WASM-safe |
| Home directory | `$HOME` env var parsing | `dirs::home_dir()` | Cross-platform (Windows %USERPROFILE%, macOS, Linux) |
| String similarity | Levenshtein distance | `strsim::jaro_winkler` | Better for short-string typo detection than raw edit distance |
| TOML deep merge | Field-by-field manual merge | `toml::Value::Table` recursive merge | Already proven in profile_library.rs, handles nested structs automatically |
| Terminal detection | Manual isatty check | `std::io::IsTerminal` (already imported) | Standard library trait, already in use in main.rs |

**Key insight:** The existing `merge_inheritance()` in `profile_library.rs` already implements the core merge pattern. The new system generalizes it with provenance tracking and multi-layer support rather than reinventing it.

## Common Pitfalls

### Pitfall 1: Default Value Ambiguity in Merge
**What goes wrong:** When a profile explicitly sets a field to the same value as `PrintConfig::default()`, the merge system can't distinguish it from "not set" if comparing against defaults.
**Why it happens:** `toml::Value::Table` serialization of a default PrintConfig includes all fields, so comparing child table against default table misses explicit-same-as-default values.
**How to avoid:** For the 5-layer merge, only include fields that are actually present in the source TOML file. Parse each profile TOML into `toml::Value::Table` directly (not through PrintConfig round-trip). The raw TOML parse naturally omits fields not in the file.
**Warning signs:** Filament profile sets `nozzle_temp = 200.0` (same as default) but it gets dropped during merge because it matches the default.

### Pitfall 2: Nested Table Merge Depth
**What goes wrong:** Flat key override replaces an entire nested table instead of merging individual fields within it.
**Why it happens:** `base.insert("speeds", layer_speeds_table)` replaces the whole `speeds` sub-table rather than merging speed fields individually.
**How to avoid:** Recursive merge -- when both base and layer have a Table for the same key, recurse into the table rather than replacing it. The provenance keys must use dotted paths (e.g., `speeds.perimeter`) for nested fields.
**Warning signs:** Setting `-f` filament profile overrides all speed values even though it only specified one speed field.

### Pitfall 3: --set Key Validation False Positives
**What goes wrong:** `--set speeds.perimeter=40.0` works but `--set speed.perimeter=40.0` silently fails or gives a confusing error.
**Why it happens:** The key validation only checks top-level field names, not nested paths.
**How to avoid:** Build the valid key set by recursively walking the default PrintConfig serialized as `toml::Value::Table`, collecting all dotted paths. Use this set for both validation and "did you mean?" suggestions.
**Warning signs:** Users report that `--set` doesn't seem to take effect for nested config fields.

### Pitfall 4: Profile Type Mismatch
**What goes wrong:** User passes a filament profile to `-m` (machine flag) and gets confusing errors.
**Why it happens:** No type validation on resolved profiles.
**How to avoid:** Require a `profile_type` field in all profile TOML files. Validate that resolved profile type matches the flag that specified it. Provide clear error: "Profile 'PLA_Basic' is a filament profile but was passed to --machine. Did you mean --filament?"
**Warning signs:** Slicing produces bizarre results because machine-specific fields came from a filament profile.

### Pitfall 5: Circular Inheritance
**What goes wrong:** Profile A inherits from B which inherits from A, causing infinite loop.
**Why it happens:** User creates profiles with `inherits = "..."` forming a cycle.
**How to avoid:** Track visited profile IDs during inheritance resolution. Error if a cycle is detected. The decision limits to single-level inherits, but even at depth 1, self-reference is possible.
**Warning signs:** CLI hangs when resolving a profile.

### Pitfall 6: Log File Write Failures
**What goes wrong:** Log file creation fails silently and user loses diagnostic info.
**Why it happens:** Output directory is read-only, disk full, or path is invalid.
**How to avoid:** Attempt log file creation early. If it fails, warn on stderr but continue slicing. Never let log file issues block the actual slice operation.
**Warning signs:** Users report missing .log files without any error message.

## Code Examples

### TOML Value Auto-Coercion for --set
```rust
// Parse a --set value string as a TOML literal value
fn parse_set_value(value_str: &str) -> toml::Value {
    // Try parsing as TOML literal: "42" -> Integer, "3.14" -> Float,
    // "true" -> Boolean, bare string -> String
    if let Ok(v) = value_str.parse::<i64>() {
        return toml::Value::Integer(v);
    }
    if let Ok(v) = value_str.parse::<f64>() {
        return toml::Value::Float(v);
    }
    if value_str == "true" {
        return toml::Value::Boolean(true);
    }
    if value_str == "false" {
        return toml::Value::Boolean(false);
    }
    toml::Value::String(value_str.to_string())
}

// Insert a dotted key path into a TOML table
fn set_dotted_key(
    table: &mut toml::map::Map<String, toml::Value>,
    key_path: &str,
    value: toml::Value,
) -> Result<(), String> {
    let parts: Vec<&str> = key_path.split('.').collect();
    let mut current = table;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            current.insert(part.to_string(), value);
            return Ok(());
        }
        current = current
            .entry(part.to_string())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
            .as_table_mut()
            .ok_or_else(|| format!("'{}' is not a table", part))?;
    }
    unreachable!()
}
```

### Progress Bar with TTY Detection
```rust
use indicatif::{ProgressBar, ProgressStyle};
use std::io::IsTerminal;

fn create_progress(total: u64) -> ProgressBar {
    let pb = if std::io::stderr().is_terminal() {
        let pb = ProgressBar::new(total);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                .unwrap()
                .progress_chars("=>-"),
        );
        pb
    } else {
        // Non-TTY: hidden progress bar, emit text updates
        ProgressBar::hidden()
    };
    pb
}
```

### G-code Header Config Embedding
```rust
fn format_gcode_header(
    config: &PrintConfig,
    provenance: &HashMap<String, FieldSource>,
    profile_checksums: &[(String, String)],
    cli_command: &str,
) -> String {
    let mut header = String::new();
    header.push_str("; Generated by SliceCore v");
    header.push_str(env!("CARGO_PKG_VERSION"));
    header.push('\n');
    header.push_str(&format!("; Timestamp: {}\n", chrono_or_manual_timestamp()));
    header.push_str(&format!("; Reproduce: {}\n", cli_command));
    header.push('\n');

    for (path, checksum) in profile_checksums {
        header.push_str(&format!("; Profile: {} (sha256:{})\n", path, &checksum[..16]));
    }
    header.push('\n');

    // Serialize config as TOML comments
    let toml_str = toml::to_string_pretty(config).unwrap_or_default();
    for line in toml_str.lines() {
        header.push_str(&format!("; {}\n", line));
    }

    header
}
```

### Structured Exit Codes
```rust
#[repr(i32)]
enum ExitCode {
    Success = 0,
    GeneralError = 1,
    ConfigProfileError = 2,
    MeshError = 3,
    SafetyValidationError = 4,
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single --config flag | -m/-f/-p layered composition | This phase | Real-world profile workflow |
| No provenance | Per-field source tracking | This phase | Debuggable config issues |
| PrintConfig::default() when no config | Required machine+filament | This phase | Safety-critical defaults |
| No progress bar | indicatif progress reporting | This phase | Better UX for long slices |
| No log file | Automatic .log file | This phase | Post-mortem debugging |

**Deprecated/outdated:**
- `SettingOverrides` struct: The 7-field manual merge pattern is superseded by the TOML value tree merge for profile composition. `SettingOverrides` remains for modifier mesh regions (Phase 6 feature).

## Open Questions

1. **Timestamp format in G-code header**
   - What we know: Need timestamp for reproducibility tracking.
   - What's unclear: Whether to add `chrono` dependency or use manual UTC formatting.
   - Recommendation: Use `std::time::SystemTime` + manual formatting to avoid new dependency. Format as ISO 8601.

2. **Built-in profile storage mechanism**
   - What we know: Need minimal built-in profiles (Generic PLA, Generic PETG, common printers).
   - What's unclear: Whether to use `include_str!()` for compiled-in profiles or ship as files.
   - Recommendation: Use `include_str!()` for a small set (~5 profiles) so the binary is self-contained for first-time users. Ship as const TOML strings in a `builtin_profiles` module.

3. **Provenance serialization in --save-config**
   - What we know: Saved config should include provenance as TOML comments.
   - What's unclear: Exact comment format (inline vs header block).
   - Recommendation: Per-section header comments: `# [speeds] Source: filament "PLA_Basic" (~/.slicecore/profiles/filament/PLA_Basic.toml)`.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test + cargo test |
| Config file | Cargo.toml [dev-dependencies] |
| Quick run command | `cargo test -p slicecore-engine --lib profile_compose` |
| Full suite command | `cargo test -p slicecore-engine -p slicecore-cli` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| N/A-01 | 5-layer TOML value merge | unit | `cargo test -p slicecore-engine profile_compose` | No -- Wave 0 |
| N/A-02 | Provenance tracking per field | unit | `cargo test -p slicecore-engine profile_compose::provenance` | No -- Wave 0 |
| N/A-03 | --set key=value parsing and coercion | unit | `cargo test -p slicecore-engine profile_compose::set_parsing` | No -- Wave 0 |
| N/A-04 | Profile name resolution | integration | `cargo test -p slicecore-engine profile_resolve` | No -- Wave 0 |
| N/A-05 | Type-constrained search | integration | `cargo test -p slicecore-engine profile_resolve::type_constraint` | No -- Wave 0 |
| N/A-06 | Config validation warnings/errors | unit | `cargo test -p slicecore-engine profile_compose::validation` | No -- Wave 0 |
| N/A-07 | Slice with -m/-f/-p E2E | integration | `cargo test -p slicecore-cli cli_slice_profiles` | No -- Wave 0 |
| N/A-08 | --dry-run exits without slicing | integration | `cargo test -p slicecore-cli cli_slice_dry_run` | No -- Wave 0 |
| N/A-09 | --save-config writes merged TOML | integration | `cargo test -p slicecore-cli cli_save_config` | No -- Wave 0 |
| N/A-10 | Exit codes match spec | integration | `cargo test -p slicecore-cli cli_exit_codes` | No -- Wave 0 |
| N/A-11 | --config and -m/-f/-p mutual exclusion | unit | `cargo test -p slicecore-cli cli_mutex` | No -- Wave 0 |
| N/A-12 | Log file creation | integration | `cargo test -p slicecore-cli cli_log_file` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-engine --lib`
- **Per wave merge:** `cargo test -p slicecore-engine -p slicecore-cli`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/slicecore-engine/src/profile_compose.rs` -- new module with unit tests
- [ ] `crates/slicecore-engine/src/profile_resolve.rs` -- new module with unit tests
- [ ] `crates/slicecore-cli/tests/cli_slice_profiles.rs` -- E2E tests for new workflow
- [ ] `crates/slicecore-cli/src/slice_workflow.rs` -- orchestrator module
- [ ] `crates/slicecore-cli/src/progress.rs` -- progress bar wrapper
- [ ] Dependencies: `cargo add -p slicecore-engine sha2@0.10 dirs@5 strsim@0.11` and `cargo add -p slicecore-cli indicatif@0.17`

## Sources

### Primary (HIGH confidence)
- Existing codebase: `crates/slicecore-engine/src/profile_library.rs` -- TOML Value tree merge pattern (lines 185-295)
- Existing codebase: `crates/slicecore-engine/src/config.rs` -- PrintConfig structure (lines 545-728), from_file (lines 1085-1095)
- Existing codebase: `crates/slicecore-engine/src/profile_convert.rs` -- round_floats_in_value, diff-against-default pattern
- Existing codebase: `crates/slicecore-cli/src/main.rs` -- cmd_slice (lines 690-1027), find_profiles_dir (lines 1545-1583), cmd_list_profiles/search/show
- toml 0.8 API: `toml::Value::Table`, `toml::map::Map` for tree manipulation -- verified via existing usage
- clap 4.5 derive: already used throughout CLI -- verified via existing Slice command definition

### Secondary (MEDIUM confidence)
- [indicatif 0.17](https://docs.rs/indicatif/latest/indicatif/) -- ProgressBar, ProgressStyle, MultiProgress
- [sha2 0.10](https://docs.rs/sha2) -- Sha256 digest, pure Rust
- [dirs 5](https://crates.io/crates/dirs) -- home_dir(), config_dir()
- [strsim](https://crates.io/crates/strsim) -- jaro_winkler for "did you mean?" suggestions

### Tertiary (LOW confidence)
- None -- all critical patterns verified in existing codebase

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all core libraries already in workspace or well-established crates
- Architecture: HIGH -- merge pattern already proven in profile_library.rs, extends naturally
- Pitfalls: HIGH -- default-value ambiguity and nested table merge are documented with existing workarounds in the codebase

**Research date:** 2026-03-14
**Valid until:** 2026-04-14 (stable domain, no fast-moving dependencies)
