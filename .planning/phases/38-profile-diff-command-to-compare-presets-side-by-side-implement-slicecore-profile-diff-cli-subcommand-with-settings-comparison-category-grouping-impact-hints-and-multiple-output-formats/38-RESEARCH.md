# Phase 38: Profile Diff Command - Research

**Researched:** 2026-03-19
**Domain:** Rust CLI / Profile comparison / Terminal display
**Confidence:** HIGH

## Summary

This phase implements a `diff-profiles` CLI subcommand that compares two `PrintConfig` instances field-by-field, groups differences by `SettingCategory`, annotates with metadata from `SettingRegistry`, and outputs in table or JSON format. The entire infrastructure already exists: `PrintConfig` derives `Serialize`/`Deserialize` (enabling `serde_json::to_value` for field-by-field comparison), `SettingRegistry::global()` provides display names/categories/tiers/units/affects, `comfy-table` is already a dependency for table formatting, and the CLI uses established `clap` subcommand patterns.

The core algorithmic challenge is straightforward: serialize both configs to `serde_json::Value`, flatten nested structs to dotted keys matching `SettingKey` format, then iterate comparing values. The diff logic belongs in `slicecore-engine` as a new `profile_diff.rs` module (pure data, no CLI concerns). The CLI module (`diff_profiles_command.rs`) handles display formatting and clap args.

**Primary recommendation:** Use `serde_json::to_value()` to convert both `PrintConfig` instances to JSON `Value::Object`, recursively flatten to dotted-key maps, compare field-by-field, enrich with `SettingRegistry` metadata, group by `SettingCategory`, and render via `comfy-table` (table) or `serde_json` (JSON).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Accept both profile library names (e.g., `BBL/PLA_Basic`) and file paths (e.g., `config.toml`) -- reuse existing `ProfileResolver` logic
- Same-type comparison only -- error if comparing mismatched types (machine vs filament)
- `--defaults` flag compares a single profile against built-in `PrintConfig::default()`
- CLI command is top-level `diff-profiles` (matches existing `list-profiles`, `search-profiles`, `show-profile` pattern)
- Labels use actual resolved profile names as column headers (not A/B)
- Default: show only settings that differ between the two profiles (`--all` flag to show everything)
- Group by `SettingCategory` from the schema registry (15 variants)
- Categories with no differences are hidden by default
- Show both display name AND raw key: `First Layer Height (first_layer_height)`
- Units from SettingRegistry shown next to values: `45.0 mm/s`
- Summary header: total differences + per-category breakdown
- Impact hints are opt-in via `--verbose` / `-v` flag
- Default output is clean values-only table
- Verbose mode adds: `affects` list (from dependency graph) + setting description
- `--category <name>` flag filters diff to specific category (repeatable for multiple categories)
- `--tier <level>` flag filters to specific tier level (simple, intermediate, advanced, developer)
- Table format (default): aligned columns with category headers, terminal colors with auto-detection
- JSON format (`--json`): full metadata per entry
- Terminal colors: green/red for changed values, auto-detect TTY, `--color always/never/auto` override
- `--quiet` / `-q` flag: suppress output, exit code 0 = identical, 1 = different (for scripting)
- JSON summary includes total count and per-category breakdown
- Exit code behavior matches `diff` convention: 0 = identical, 1 = different, 2 = error

### Claude's Discretion
- Internal diff algorithm (field-by-field comparison via serde_json or reflection)
- Color scheme and exact ANSI codes
- How to handle nested struct comparison (flatten to dotted keys vs recursive)
- Table column width calculation and alignment logic
- Error messages for profile resolution failures
- How to compare enum values (display name vs variant name)
- Test strategy and integration test fixtures

### Deferred Ideas (OUT OF SCOPE)
- Multi-way diff (3+ profiles with matrix display)
- Markdown output format
- CSV output format
- `--only-keys` filter
- Profile merge command
- Interactive diff (TUI)
- Diff against upstream
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde_json | workspace | Serialize PrintConfig to Value for field comparison | Already used throughout, derives on all config types |
| comfy-table | 7.x | Terminal table formatting with alignment | Already a dependency in slicecore-cli |
| clap | 4.5 | CLI argument parsing with derive macros | Already the CLI framework |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| slicecore-config-schema | workspace | SettingRegistry, SettingCategory, Tier, SettingDefinition | Metadata enrichment for every diff entry |
| slicecore-engine | workspace | PrintConfig, ProfileResolver, setting_registry() | Profile loading and diff logic module |

### No New Dependencies Needed

All required functionality is covered by existing dependencies. No new crates need to be added.

## Architecture Patterns

### Recommended Module Structure
```
crates/slicecore-engine/src/
  profile_diff.rs          # Core diff logic (DiffEntry, DiffResult, diff_configs())

crates/slicecore-cli/src/
  diff_profiles_command.rs  # CLI args (DiffProfilesArgs) + display formatting
  main.rs                  # Add DiffProfiles variant to Commands enum
```

### Pattern 1: serde_json Value Flattening for Diff
**What:** Serialize both `PrintConfig` instances to `serde_json::Value`, then recursively flatten nested objects to dotted-key maps (`BTreeMap<String, serde_json::Value>`). This produces keys like `"print.speed.travel"` that match `SettingKey` format.
**When to use:** Always -- this is the recommended diff algorithm.
**Example:**
```rust
use serde_json::Value;
use std::collections::BTreeMap;

fn flatten_json(prefix: &str, value: &Value, out: &mut BTreeMap<String, Value>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_json(&key, v, out);
            }
        }
        _ => {
            out.insert(prefix.to_string(), value.clone());
        }
    }
}

fn diff_configs(left: &PrintConfig, right: &PrintConfig) -> Vec<DiffEntry> {
    let left_val = serde_json::to_value(left).unwrap();
    let right_val = serde_json::to_value(right).unwrap();
    let mut left_map = BTreeMap::new();
    let mut right_map = BTreeMap::new();
    flatten_json("", &left_val, &mut left_map);
    flatten_json("", &right_val, &mut right_map);
    // Compare all keys present in either map
    // ...
}
```

### Pattern 2: Metadata Enrichment from SettingRegistry
**What:** For each differing key, look up `SettingRegistry::global().get_by_str(key)` to get display_name, category, tier, units, affects, description.
**When to use:** After computing raw diff, before display.
**Example:**
```rust
let registry = setting_registry();
if let Some(def) = registry.get_by_str(&key) {
    entry.display_name = def.display_name.clone();
    entry.category = Some(def.category);
    entry.tier = Some(def.tier);
    entry.units = def.units.clone();
    entry.affects = def.affects.clone();
    entry.description = def.description.clone();
}
```

### Pattern 3: Exit Code Convention (matching unix diff)
**What:** Process exit codes: 0 = profiles identical, 1 = profiles differ, 2 = error.
**When to use:** Always. The `main()` match arm for `DiffProfiles` should use `process::exit()`.
**Example:**
```rust
Commands::DiffProfiles { .. } => {
    match cmd_diff_profiles(/* args */) {
        Ok(has_differences) => {
            if has_differences { process::exit(1); }
            // else exit 0 naturally
        }
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(2);
        }
    }
}
```

### Pattern 4: Existing CLI Subcommand Pattern
**What:** Top-level subcommand with `clap::Args` struct, matching `ShowProfile`, `SearchProfiles` patterns.
**When to use:** For the `DiffProfiles` variant in `Commands` enum.
**Example:**
```rust
/// Compare two print profiles side by side.
DiffProfiles {
    /// First profile (name or file path)
    left: String,
    /// Second profile (name or file path, omit with --defaults)
    right: Option<String>,
    /// Compare against built-in defaults instead of a second profile
    #[arg(long)]
    defaults: bool,
    /// Show all settings (not just differences)
    #[arg(long)]
    all: bool,
    /// Show impact hints and descriptions
    #[arg(short, long)]
    verbose: bool,
    /// Filter by category (repeatable)
    #[arg(long)]
    category: Vec<String>,
    /// Filter by tier level
    #[arg(long)]
    tier: Option<TierFilter>,  // reuse from schema_command
    /// Output as JSON
    #[arg(long)]
    json: bool,
    /// Suppress output, use exit code only
    #[arg(short, long)]
    quiet: bool,
    /// Color mode
    #[arg(long, default_value = "auto")]
    color: String,
    /// Profile library directory
    #[arg(long)]
    profiles_dir: Option<PathBuf>,
}
```

### Anti-Patterns to Avoid
- **Manual field-by-field comparison:** Do NOT write a match/comparison for each of 400+ PrintConfig fields. Use serde_json serialization to get a generic map.
- **Diff logic in CLI module:** Keep the core diff algorithm in `slicecore-engine::profile_diff`, not in the CLI. The CLI only handles display.
- **Ignoring unknown keys:** Some flattened keys may not appear in SettingRegistry (e.g., deeply nested sub-struct fields). Handle gracefully with fallback display using the raw key.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Config serialization to comparable form | Custom visitor/reflection | `serde_json::to_value()` | PrintConfig already derives Serialize; JSON Value is a universal comparable format |
| Terminal table layout | Manual spacing/padding | `comfy-table` | Already a dependency, handles unicode width, column alignment |
| TTY detection | Manual isatty checks | `std::io::IsTerminal` | Already used in analysis_display.rs |
| Category parsing from string | New parser | Reuse `parse_category()` from schema_command.rs | Exact same logic needed, already handles kebab-case normalization |
| Profile resolution | Custom file lookup | `ProfileResolver` | Handles names, paths, ambiguity errors, suggestions |

## Common Pitfalls

### Pitfall 1: SettingKey Mismatch Between Flattened JSON and Registry
**What goes wrong:** Flattened JSON keys from serde may use different naming than SettingRegistry keys. For example, serde uses the struct field name but SettingRegistry uses dotted-path with prefix.
**Why it happens:** `PrintConfig` has nested structs (e.g., `speed: SpeedConfig`, `retraction: RetractionConfig`). When flattened, JSON produces `speed.travel` but the registry key might be `print.speed.travel`.
**How to avoid:** Check how the `SettingSchema` derive macro generates keys. The `setting_definitions(prefix)` function on `HasSettingSchema` takes a prefix. Look at how the global registry is built in `lib.rs` to understand the prefix used (likely `"print"` for `PrintConfig`). Match that prefix in the flatten logic.
**Warning signs:** All registry lookups return `None` during testing.

### Pitfall 2: Floating-Point Comparison
**What goes wrong:** Two profiles with `layer_height: 0.2` might serialize to slightly different float representations and show as "different" when they are semantically equal.
**Why it happens:** Floating-point serialization can differ based on source (TOML parse vs default construction).
**How to avoid:** Compare `serde_json::Value` directly -- serde_json uses `Number` which preserves exact representation. If both serialize to `0.2`, they compare equal. Only if there is genuine difference (e.g., `0.20000000000000001` vs `0.2`) would they differ, which would indicate a real difference in source data.
**Warning signs:** Many spurious diffs in float-heavy categories (Speed, Quality).

### Pitfall 3: Array/Vec Value Display
**What goes wrong:** Some PrintConfig fields are `Vec<f64>` (e.g., per-extruder values). Displaying them as raw JSON arrays (`[0.4, 0.4]`) is ugly in a table.
**Why it happens:** No special formatting for vector values.
**How to avoid:** Format arrays as comma-separated values in table mode: `0.4, 0.4`. Keep raw JSON array in JSON output mode.
**Warning signs:** Table cells with brackets and quotes.

### Pitfall 4: Missing Category for Unregistered Keys
**What goes wrong:** Some flattened keys won't have entries in SettingRegistry (sub-fields of nested structs that weren't individually registered, or fields added after schema registration).
**Why it happens:** SettingSchema derive may not register every leaf field, especially for deeply nested types.
**How to avoid:** Create an "Uncategorized" fallback group for keys not found in the registry. Still show them in diff output but without enriched metadata.
**Warning signs:** Panics on `unwrap()` of registry lookups.

### Pitfall 5: Profile Type Validation
**What goes wrong:** User compares a machine profile path against a filament profile name.
**Why it happens:** File paths don't carry type information; profile names resolve with type context.
**How to avoid:** For file paths, load the config and let the comparison proceed (PrintConfig is PrintConfig regardless of source type). For library names, the `ProfileResolver` resolves with type context. The "same-type comparison" constraint from CONTEXT.md applies to library profile resolution -- when both are library profiles, they should be same type. For file paths, type checking is less meaningful since all load to PrintConfig.
**Warning signs:** Unclear error messages when types mismatch.

## Code Examples

### Computing a Profile Diff
```rust
// Source: project codebase patterns
use std::collections::BTreeMap;
use serde_json::Value;
use slicecore_config_schema::{SettingCategory, SettingKey, Tier};

#[derive(Debug, Clone, Serialize)]
pub struct DiffEntry {
    pub key: String,
    pub display_name: String,
    pub category: Option<SettingCategory>,
    pub tier: Option<Tier>,
    pub left_value: Value,
    pub right_value: Value,
    pub units: Option<String>,
    pub affects: Vec<SettingKey>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffResult {
    pub left_name: String,
    pub right_name: String,
    pub entries: Vec<DiffEntry>,
    pub total_differences: usize,
    pub category_counts: BTreeMap<String, usize>,
}
```

### Table Output with comfy-table
```rust
// Source: analysis_display.rs patterns
use comfy_table::{Table, ContentArrangement, Cell, Color};

fn display_category_group(
    category_name: &str,
    entries: &[&DiffEntry],
    left_name: &str,
    right_name: &str,
    verbose: bool,
) {
    println!("\n  {category_name}");
    println!("  {}", "-".repeat(category_name.len()));

    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);

    let mut header = vec!["Setting", left_name, right_name];
    if verbose {
        header.push("Affects");
    }
    table.set_header(header);

    for entry in entries {
        let name = format!("{} ({})", entry.display_name, entry.key);
        let left = format_value(&entry.left_value, &entry.units);
        let right = format_value(&entry.right_value, &entry.units);
        let mut row = vec![name, left, right];
        if verbose {
            let affects_str = entry.affects.iter()
                .map(|k| k.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            row.push(affects_str);
        }
        table.add_row(row);
    }
    println!("{table}");
}
```

### Reusing parse_category from schema_command
```rust
// The parse_category function in schema_command.rs handles kebab-case normalization.
// Either make it pub and reuse, or extract to a shared location.
// It maps "line-width" -> LineWidth, "post-process" -> PostProcess, etc.
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual field comparison | serde_json::to_value + flatten | N/A (greenfield) | Automatically handles all 400+ fields without manual enumeration |
| Raw key display | SettingRegistry enrichment | Phase 35 | Display names, units, categories available for every setting |

## Open Questions

1. **Key prefix alignment**
   - What we know: `SettingRegistry` keys use dotted prefixes (e.g., `print.layer_height`). PrintConfig serializes field names without prefix.
   - What's unclear: The exact prefix convention used when building the global registry.
   - Recommendation: Check `slicecore-engine/src/lib.rs` where `GLOBAL_REGISTRY` is built to determine the prefix passed to `setting_definitions()`. The flatten logic must use the same prefix.

2. **Enum value display format**
   - What we know: Enums serialize to snake_case strings via `#[serde(rename_all = "snake_case")]`.
   - What's unclear: Whether to show the display name from `EnumVariant` or the serialized string.
   - Recommendation: Show serialized value (it's what appears in config files). In verbose mode, could additionally show the display name from `ValueType::Enum` variants if available.

3. **parse_category reuse**
   - What we know: `parse_category()` in `schema_command.rs` is not `pub`.
   - What's unclear: Whether to make it pub, duplicate it, or extract to a shared module.
   - Recommendation: Extract to a small shared utility or make `pub(crate)`. Duplication of 20 lines is also acceptable for a CLI crate.

## Sources

### Primary (HIGH confidence)
- `crates/slicecore-cli/src/main.rs` - Commands enum pattern, existing subcommands
- `crates/slicecore-cli/src/schema_command.rs` - Category parsing, tier filtering, registry access patterns
- `crates/slicecore-cli/src/analysis_display.rs` - comfy-table usage, TTY detection, OutputFormat enum
- `crates/slicecore-config-schema/src/types.rs` - SettingDefinition, SettingCategory (16 variants), Tier, SettingKey
- `crates/slicecore-config-schema/src/registry.rs` - SettingRegistry API (get, get_by_str, all)
- `crates/slicecore-engine/src/profile_resolve.rs` - ProfileResolver, ResolvedProfile, ProfileError
- `crates/slicecore-engine/src/config.rs` - PrintConfig with Serialize/Deserialize, from_toml, from_json

### Secondary (MEDIUM confidence)
- `crates/slicecore-cli/Cargo.toml` - Confirmed comfy-table 7.x, clap 4.5 as existing dependencies

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all libraries already in use, no new dependencies
- Architecture: HIGH - follows established patterns in the codebase (schema_command, analysis_display)
- Pitfalls: HIGH - identified from direct code inspection of config serialization and registry lookup patterns

**Research date:** 2026-03-19
**Valid until:** 2026-04-19 (stable domain, internal project patterns)
