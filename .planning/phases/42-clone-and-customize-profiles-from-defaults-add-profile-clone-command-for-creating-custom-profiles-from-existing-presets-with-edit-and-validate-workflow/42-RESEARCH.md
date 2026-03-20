# Phase 42: Clone and Customize Profiles from Defaults - Research

**Researched:** 2026-03-20
**Domain:** CLI subcommand group, TOML profile manipulation, schema-driven validation
**Confidence:** HIGH

## Summary

Phase 42 adds a `slicecore profile` subcommand group with clone, set, get, reset, edit, validate, delete, and rename operations, plus aliases for existing top-level profile commands. The codebase already has all the foundational pieces: `ProfileResolver` for name resolution with user/library source tracking, `PrintConfig` with full TOML serde round-trip support, `SettingRegistry` with search and validation, and two reference implementations of subcommand groups (`plugins_command.rs`, `schema_command.rs`). The implementation is primarily CLI plumbing and file I/O -- no new algorithms or complex data structures are needed.

The key technical decisions are: (1) use `toml` crate (already in workspace at 0.8) for full deserialize-modify-serialize workflow since `toml_edit` is not in the project and the full-copy clone approach means comment preservation is not a concern, (2) leverage `ProfileResolver` + `ProfileSource` enum to enforce the library-immutability rule, and (3) use `SettingRegistry::search()` for "did you mean?" suggestions on unknown keys.

**Primary recommendation:** Implement as a single `profile_command.rs` module following the `plugins_command.rs` pattern, with `ProfileCommand` enum deriving `Subcommand`, and a top-level `run_profile_command()` dispatcher.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- New `slicecore profile` subcommand group with clone, set, get, reset, edit, validate, delete, rename subcommands
- Existing top-level commands (list-profiles, show-profile, search-profiles, diff-profiles) remain as-is for backwards compatibility
- Add aliases under `profile` group: profile list, profile show, profile diff, profile search
- Implementation in dedicated `profile_command.rs`
- Add `Profile(ProfileCommand)` variant to `Commands` enum in main.rs
- All operations work uniformly on all profile types (machine, filament, process)
- User profiles stored in `~/.slicecore/profiles/` organized by type (machine/, filament/, process/)
- Library profiles are always immutable -- modification commands refuse with error suggesting clone
- Clone creates full standalone TOML copy with ALL settings from source
- Sets `inherits` metadata field, `is_custom = true`, records clone source in `[metadata]` section
- Profile names restricted to alphanumeric + hyphens + underscores
- Name conflicts error with message suggesting `--force` to overwrite or different name
- `profile set` validates against SettingRegistry (type, range, constraints)
- `profile edit` opens TOML in $EDITOR/$VISUAL, validates after close, saves regardless
- `profile validate` reports all errors and warnings
- `profile delete` requires `--yes` flag or interactive confirmation
- `profile rename` is atomic (moves file + updates metadata.name)

### Claude's Discretion
- Internal data structures for profile metadata parsing
- TOML manipulation approach (toml_edit vs full deserialize-modify-serialize)
- Error message wording and formatting details
- Test strategy and fixtures
- $EDITOR detection fallback chain
- How `reset` resolves the original value from the inherits chain
- Whether aliases use clap aliases or separate handler routing

### Deferred Ideas (OUT OF SCOPE)
- Profile create from scratch (guided/template)
- Profile import/export
- Profile versioning/changelog
- Profile groups/tags
- profile list --custom filter
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| API-02 | Full-featured CLI interface (slice, validate, analyze commands) | Profile subcommand group adds clone/set/get/reset/edit/validate/delete/rename operations, extending CLI coverage. Builds on existing ProfileResolver, SettingRegistry, and PrintConfig infrastructure. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.5 | CLI argument parsing with derive macros | Already in workspace, used by all CLI commands |
| toml | 0.8 | TOML serialization/deserialization | Already in workspace, used for PrintConfig round-trips |
| serde | 1.x | Serialization framework | Already in workspace |
| serde_json | 1.x | JSON output for --json flag | Already in workspace |
| anyhow | 1.x | Application error handling | Already in workspace, used in CLI handlers |
| comfy-table | 7 | Table formatting for terminal output | Already in workspace, used by plugins/diff commands |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tempfile | (workspace) | Temporary files for tests | Test fixtures |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| toml 0.8 (deserialize-modify-serialize) | toml_edit (in-place editing) | toml_edit preserves comments and formatting, but is not in the workspace and clone creates full copies from scratch anyway. Not worth adding a dependency. |

**Installation:** No new dependencies needed. All libraries are already in the workspace.

## Architecture Patterns

### Recommended Project Structure
```
crates/slicecore-cli/src/
  profile_command.rs      # New: ProfileCommand enum + run_profile_command() + all subcommand handlers
  main.rs                 # Modified: add Profile(ProfileCommand) variant, mod profile_command
```

### Pattern 1: Subcommand Group (from plugins_command.rs)
**What:** A `#[derive(Subcommand)]` enum with one variant per subcommand, dispatched by a top-level `run_*()` function.
**When to use:** Every subcommand group in this CLI.
**Example:**
```rust
// Source: crates/slicecore-cli/src/plugins_command.rs
#[derive(Subcommand)]
pub enum ProfileCommand {
    /// Clone a profile to create a custom copy
    Clone { source: String, name: String, #[arg(long)] force: bool },
    /// Set a single setting value
    Set { name: String, key: String, value: String },
    /// Get a single setting value
    Get { name: String, key: String },
    // ... etc
}

pub fn run_profile_command(cmd: ProfileCommand, profiles_dir: Option<&Path>) -> Result<(), anyhow::Error> {
    match cmd {
        ProfileCommand::Clone { source, name, force } => cmd_clone(&source, &name, force, profiles_dir),
        // ...
    }
}
```

### Pattern 2: ProfileResolver for Name Resolution
**What:** Use existing `ProfileResolver::resolve()` to look up profile names, checking user dir first then library. The `ResolvedProfile.source` field (`ProfileSource::User` vs `Library` vs `BuiltIn`) determines mutability.
**When to use:** Every command that accepts a profile name argument.
**Example:**
```rust
// Source: crates/slicecore-engine/src/profile_resolve.rs
let resolver = ProfileResolver::new(profiles_dir);
let resolved = resolver.resolve(&source_name, "filament")?;
match resolved.source {
    ProfileSource::User => { /* allow modification */ },
    ProfileSource::Library { .. } | ProfileSource::BuiltIn => {
        anyhow::bail!("Cannot modify library profile '{}'. Clone it first: slicecore profile clone {} my-copy",
            source_name, source_name);
    },
}
```

### Pattern 3: Type-Agnostic Resolution for Clone Source
**What:** The clone command must resolve the source profile without knowing its type upfront. The current `resolve()` requires an `expected_type`. Resolution should try all three types or use a type-agnostic path.
**When to use:** `profile clone`, `profile show`, `profile validate`, and other commands that accept any profile type.
**Implementation options:**
1. Try each type in order (`machine`, `filament`, `process`), take the first match
2. Add a `resolve_any()` method to `ProfileResolver` that searches all types
3. Accept an optional `--type` flag to disambiguate

**Recommendation:** Try all three types in order, collecting matches. If exactly one match, use it. If multiple, report ambiguity. This avoids modifying the engine crate for a CLI concern. If the user needs disambiguation, they can use `--type filament`.

### Pattern 4: Clone as Full TOML Copy with Metadata
**What:** Load source as `PrintConfig`, serialize to TOML, prepend metadata header.
**When to use:** `profile clone` command.
**Example:**
```rust
let config = PrintConfig::from_file(&resolved.path)?;
let toml_body = toml::to_string_pretty(&config)?;

let metadata = format!(
    "[metadata]\nname = \"{}\"\nis_custom = true\ninherits = \"{}\"\nclone_source = \"{}\"\n\n",
    new_name, source_name, resolved.path.display()
);

let full_content = format!("{metadata}{toml_body}");
let dest_dir = user_profiles_dir.join(&resolved.profile_type);
std::fs::create_dir_all(&dest_dir)?;
let dest_path = dest_dir.join(format!("{new_name}.toml"));
std::fs::write(&dest_path, &full_content)?;
```

### Pattern 5: Set with Schema Validation
**What:** For `profile set`, validate the key exists in `SettingRegistry`, check the value against constraints, then modify the TOML.
**When to use:** `profile set` command.
**Example:**
```rust
use slicecore_config_schema::{SettingRegistry, SettingKey};

let registry = slicecore_engine::setting_registry();
let setting_key = SettingKey(key.clone());
match registry.get(&setting_key) {
    Some(def) => {
        // Validate value against def.constraints (Range, etc.)
        // Parse value string to expected type
        // Load profile TOML, modify key, write back
    },
    None => {
        // "Did you mean?" using registry.search(&key)
        let suggestions = registry.search(&key);
        anyhow::bail!("Unknown setting key '{}'. Did you mean: {}?",
            key, suggestions.iter().take(3).map(|d| d.key.0.as_str()).collect::<Vec<_>>().join(", "));
    }
}
```

### Anti-Patterns to Avoid
- **Modifying library profile files directly:** Always check `ProfileSource` before any write operation.
- **Using `toml_edit` for clone:** The clone creates a full copy from scratch; there are no comments to preserve. Using `toml` (deserialize/serialize) is simpler and already available.
- **Hard-coding user profile paths:** Use `ProfileResolver`'s user dir detection (`~/.slicecore/profiles/`) or read it from the resolver.
- **Skipping name validation on clone:** Profile names become filenames -- must reject characters that are problematic on Windows/Linux filesystems.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Profile name resolution | Custom file scanning | `ProfileResolver::resolve()` | Already handles user-first search, substring matching, "did you mean?" suggestions, type checking |
| Setting validation | Manual range checks | `SettingRegistry::validate_config()` | Covers range constraints, dependency checks, deprecated warnings |
| Setting search/suggestions | Levenshtein distance | `SettingRegistry::search()` | Already implements ranked search across key, display name, description, tags |
| TOML round-trip | Manual string manipulation | `toml::to_string_pretty()` + `toml::from_str()` | Handles nested tables, arrays, escaping correctly |
| Table formatting | Manual column alignment | `comfy-table` | Already used by plugins and diff commands |
| User home directory | Manual `$HOME` parsing | `home::home_dir()` (via ProfileResolver) | Cross-platform, handles edge cases |

**Key insight:** The entire validation, resolution, and search infrastructure already exists in the engine and config-schema crates. The profile_command module is pure CLI glue.

## Common Pitfalls

### Pitfall 1: Type-Agnostic Resolution
**What goes wrong:** `ProfileResolver::resolve()` requires an `expected_type` parameter, but clone/validate/show commands don't know the source profile's type.
**Why it happens:** The resolver was designed for the slice workflow where type is always known.
**How to avoid:** Implement a `try_resolve_any()` helper in the CLI that tries all three types (`machine`, `filament`, `process`) and handles ambiguity. Alternatively, add an optional `--type` flag.
**Warning signs:** Tests that only work with one profile type.

### Pitfall 2: Metadata Section vs PrintConfig Fields
**What goes wrong:** `PrintConfig` uses `#[serde(default)]` and will silently ignore unknown TOML sections like `[metadata]`. When loading a cloned profile that has `[metadata]`, the metadata is lost in the `PrintConfig` struct but the TOML file retains it.
**Why it happens:** `[metadata]` is not a field on `PrintConfig`.
**How to avoid:** For operations that need metadata (reset, get-metadata), parse the TOML as `toml::Value` directly rather than going through `PrintConfig`. For clone, prepend metadata to the serialized TOML output. Consider whether `PrintConfig` should have an optional `metadata` field with `#[serde(default, skip_serializing_if)]`, or use a separate metadata parser.
**Warning signs:** Metadata disappearing after a set-then-save cycle.

### Pitfall 3: TOML Key Path Mapping
**What goes wrong:** `SettingRegistry` keys use dotted paths like `speed.perimeter`, but `PrintConfig` serializes nested structs as TOML tables (`[speed]\nperimeter = 50`). The `profile set` command needs to map from the dotted key to the correct TOML table path.
**Why it happens:** TOML has two representations: dotted keys and table headers.
**How to avoid:** Parse the profile as `toml::Value::Table`, then navigate the key path to set the value. The `toml` crate handles this correctly when you manipulate the `Value` tree.
**Warning signs:** `profile set speed.perimeter 100` failing to find the key or creating a literal `speed.perimeter` key instead of nested `[speed] perimeter`.

### Pitfall 4: $EDITOR Handling Edge Cases
**What goes wrong:** `profile edit` opens $EDITOR but fails on systems without EDITOR set, or when the editor returns non-zero, or when the file is deleted during editing.
**Why it happens:** Interactive editor spawning has many edge cases.
**How to avoid:** Fallback chain: `$VISUAL` -> `$EDITOR` -> `nano` -> `vi`. Check editor exit status. Verify file still exists after editor closes. Validate TOML syntax before attempting schema validation.
**Warning signs:** Tests that shell out to real editors.

### Pitfall 5: Profile Name as Filename Safety
**What goes wrong:** User provides a name like `../../../etc/passwd` or `CON` (Windows reserved name) and the clone creates files in unexpected locations.
**Why it happens:** Profile names map directly to filenames.
**How to avoid:** Validate names against `^[a-zA-Z0-9_-]+$` regex. Reject empty names, names starting with `.` or `-`, and Windows reserved names.
**Warning signs:** Path traversal in test fixtures.

### Pitfall 6: Atomic Rename Race Conditions
**What goes wrong:** `profile rename` moves the file but another process reads the old path between move and metadata update.
**Why it happens:** Rename is not a single atomic operation at the TOML-metadata level.
**How to avoid:** Write the new file first (with updated metadata.name), then delete the old file. This way the profile always exists at one of the two paths. Use `std::fs::rename()` for the actual filesystem operation which is atomic on most systems.
**Warning signs:** Tests that only verify the happy path.

## Code Examples

### Clone Command Implementation Pattern
```rust
// Source: project patterns from plugins_command.rs + profile_resolve.rs

fn cmd_clone(
    source: &str,
    new_name: &str,
    force: bool,
    profiles_dir: Option<&Path>,
) -> Result<(), anyhow::Error> {
    // Validate name
    if !is_valid_profile_name(new_name) {
        anyhow::bail!("Invalid profile name '{}'. Use only letters, numbers, hyphens, and underscores.", new_name);
    }

    // Resolve source (try all types)
    let resolver = ProfileResolver::new(profiles_dir);
    let resolved = try_resolve_any(&resolver, source)?;

    // Load source config
    let config = PrintConfig::from_file(&resolved.path)?;
    let toml_body = toml::to_string_pretty(&config)?;

    // Build destination path
    let user_dir = home::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?
        .join(".slicecore/profiles")
        .join(&resolved.profile_type);
    std::fs::create_dir_all(&user_dir)?;
    let dest_path = user_dir.join(format!("{new_name}.toml"));

    // Check for conflicts
    if dest_path.exists() && !force {
        anyhow::bail!(
            "Profile '{}' already exists at {}. Use --force to overwrite or choose a different name.",
            new_name, dest_path.display()
        );
    }

    // Write with metadata
    let metadata = format!(
        "# Custom profile cloned from {source}\n\
         [metadata]\n\
         name = \"{new_name}\"\n\
         is_custom = true\n\
         inherits = \"{source}\"\n\n"
    );
    std::fs::write(&dest_path, format!("{metadata}{toml_body}"))?;

    println!("Created custom profile '{new_name}' at {}", dest_path.display());
    println!("\nNext steps:");
    println!("  slicecore profile show {new_name}");
    println!("  slicecore profile set {new_name} <key> <value>");
    println!("  slicecore profile edit {new_name}");

    Ok(())
}
```

### Name Validation
```rust
fn is_valid_profile_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !name.starts_with('-')
}
```

### Type-Agnostic Resolution Helper
```rust
fn try_resolve_any(
    resolver: &ProfileResolver,
    query: &str,
) -> Result<ResolvedProfile, anyhow::Error> {
    let types = ["machine", "filament", "process"];
    let mut matches = Vec::new();

    for profile_type in &types {
        match resolver.resolve(query, profile_type) {
            Ok(resolved) => matches.push(resolved),
            Err(ProfileError::NotFound { .. }) => continue,
            Err(ProfileError::TypeMismatch { .. }) => continue,
            Err(e) => return Err(e.into()),
        }
    }

    match matches.len() {
        0 => anyhow::bail!("Profile '{}' not found in any type (machine, filament, process)", query),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => anyhow::bail!(
            "Ambiguous profile '{}': found in types {}. Use --type to disambiguate.",
            query,
            matches.iter().map(|m| m.profile_type.as_str()).collect::<Vec<_>>().join(", ")
        ),
    }
}
```

### Set Command with Validation
```rust
fn cmd_set(name: &str, key: &str, value: &str, profiles_dir: Option<&Path>) -> Result<(), anyhow::Error> {
    let resolver = ProfileResolver::new(profiles_dir);
    let resolved = try_resolve_any(&resolver, name)?;

    // Enforce immutability
    if !matches!(resolved.source, ProfileSource::User) {
        anyhow::bail!(
            "Cannot modify {} profile '{}'. Clone it first:\n  slicecore profile clone {} my-{}",
            resolved.source, name, name, name
        );
    }

    // Validate key against schema
    let registry = slicecore_engine::setting_registry();
    let setting_key = slicecore_config_schema::SettingKey(key.to_string());
    let def = registry.get(&setting_key).ok_or_else(|| {
        let suggestions: Vec<_> = registry.search(key).iter().take(3).map(|d| d.key.0.clone()).collect();
        if suggestions.is_empty() {
            anyhow::anyhow!("Unknown setting key '{key}'")
        } else {
            anyhow::anyhow!("Unknown setting key '{key}'. Did you mean: {}?", suggestions.join(", "))
        }
    })?;

    // Parse and validate value against constraints
    // ... (parse value to correct type, check Range constraints)

    // Load TOML as Value, modify, write back
    let contents = std::fs::read_to_string(&resolved.path)?;
    let mut doc: toml::Value = toml::from_str(&contents)?;
    // Navigate key path and set value
    // ...
    let output = toml::to_string_pretty(&doc)?;
    std::fs::write(&resolved.path, output)?;

    println!("Set {key} = {value} in profile '{name}'");
    Ok(())
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Top-level profile commands (list-profiles, show-profile) | Profile subcommand group with aliases | Phase 42 | Better CLI organization, backwards-compatible |
| Profile library is read-only | User profiles in ~/.slicecore/profiles/ writable | Phase 30 | Enables clone-and-customize workflow |

**Deprecated/outdated:** None for this phase. All dependencies are current.

## Open Questions

1. **PrintConfig metadata field**
   - What we know: `[metadata]` section in TOML is not represented in `PrintConfig` struct. The `passthrough` `BTreeMap<String, String>` field may capture some extra keys but not nested tables.
   - What's unclear: Whether `toml::from_str` will silently drop the `[metadata]` section or error on it. Whether `profile set` followed by save will lose metadata.
   - Recommendation: Test this during implementation. If metadata is lost, parse as `toml::Value` for set/get operations instead of going through `PrintConfig`. Consider adding an optional `metadata` table to `PrintConfig` with `#[serde(default, skip_serializing_if = "Option::is_none")]`.

2. **Profile type detection without --type flag**
   - What we know: `ProfileResolver::resolve()` requires `expected_type`. There is no `resolve_any()` method.
   - What's unclear: Whether trying all three types causes ambiguity issues in practice (e.g., a profile name that exists in both filament and machine).
   - Recommendation: Implement `try_resolve_any()` as a CLI helper function (see code example). Add optional `--type` flag for disambiguation.

3. **Clap aliases vs separate handler routing**
   - What we know: Clap supports `#[command(alias = "...")]` on subcommand variants. The existing top-level commands have their own handler functions.
   - What's unclear: Whether clap aliases can map `profile list` to the existing `cmd_list_profiles()` handler or if a new wrapper is needed.
   - Recommendation: Use clap `alias` on the `ProfileCommand` variants that correspond to existing top-level commands. The handler can call the existing top-level handler function directly.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml [dev-dependencies] |
| Quick run command | `cargo test -p slicecore-cli --lib` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| API-02 | profile clone creates TOML copy in user dir | integration | `cargo test -p slicecore-cli --test cli_profile` | Wave 0 |
| API-02 | profile clone sets metadata fields | unit | `cargo test -p slicecore-cli profile_command` | Wave 0 |
| API-02 | profile clone refuses duplicate without --force | unit | `cargo test -p slicecore-cli profile_command` | Wave 0 |
| API-02 | profile set validates against schema | unit | `cargo test -p slicecore-cli profile_command` | Wave 0 |
| API-02 | profile set refuses library profile modification | unit | `cargo test -p slicecore-cli profile_command` | Wave 0 |
| API-02 | profile get reads single setting | unit | `cargo test -p slicecore-cli profile_command` | Wave 0 |
| API-02 | profile reset reverts to inherited value | unit | `cargo test -p slicecore-cli profile_command` | Wave 0 |
| API-02 | profile validate reports errors and warnings | unit | `cargo test -p slicecore-cli profile_command` | Wave 0 |
| API-02 | profile delete removes file with --yes | unit | `cargo test -p slicecore-cli profile_command` | Wave 0 |
| API-02 | profile rename moves file and updates metadata | unit | `cargo test -p slicecore-cli profile_command` | Wave 0 |
| API-02 | profile name validation rejects invalid chars | unit | `cargo test -p slicecore-cli profile_command` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-cli --lib`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/slicecore-cli/src/profile_command.rs` -- new module with inline #[cfg(test)] tests
- [ ] `crates/slicecore-cli/tests/cli_profile.rs` -- integration tests for profile subcommands
- [ ] Test fixtures: sample TOML profiles for clone/set/validate test scenarios (use tempfile)

## Sources

### Primary (HIGH confidence)
- `crates/slicecore-cli/src/plugins_command.rs` -- Reference subcommand group pattern (467 lines)
- `crates/slicecore-cli/src/schema_command.rs` -- Reference subcommand group pattern with SettingRegistry
- `crates/slicecore-cli/src/diff_profiles_command.rs` -- Profile resolution pattern in CLI
- `crates/slicecore-engine/src/profile_resolve.rs` -- ProfileResolver API, ProfileSource enum, 19 tests
- `crates/slicecore-engine/src/config.rs` -- PrintConfig serde, from_file(), from_toml()
- `crates/slicecore-config-schema/src/validate.rs` -- Schema validation with ValidationIssue/Severity
- `crates/slicecore-config-schema/src/registry.rs` -- SettingRegistry::get(), all()
- `crates/slicecore-config-schema/src/search.rs` -- SettingRegistry::search() ranked results
- `crates/slicecore-cli/Cargo.toml` -- Confirmed dependency versions: clap 4.5, toml (workspace 0.8), anyhow, comfy-table 7

### Secondary (MEDIUM confidence)
- None -- all findings verified against source code

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all dependencies already in workspace, versions confirmed from Cargo.toml
- Architecture: HIGH -- subcommand group pattern directly observed in plugins_command.rs and schema_command.rs
- Pitfalls: HIGH -- identified from reading actual code (ProfileResolver API surface, PrintConfig serde behavior, SettingRegistry search)

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable domain, no external dependencies changing)
