# Phase 44: Search and Filter Profiles by Printer and Filament Compatibility - Research

**Researched:** 2026-03-23
**Domain:** CLI profile search, compatibility engine, profile sets (Rust/clap)
**Confidence:** HIGH

## Summary

Phase 44 extends the existing profile system (Phases 42-43) with search/filter capabilities, an enhanced compatibility engine, and profile sets. The codebase already has a solid foundation: `ProfileResolver::search()` does case-insensitive substring matching, `CompatibilityInfo` checks filament type/vendor/ID compatibility, `EnabledProfiles` manages TOML-serialized activation state, and `ProfileCommand` is a well-structured clap Subcommand enum.

The primary work involves: (1) extending `CompatibilityInfo` with nozzle diameter and temperature range checks, (2) adding filter flags (`--material`, `--vendor`, `--nozzle`, `--type`) to both `search` and `list` commands, (3) adding `profile compat <id>` and `profile set` subcommand groups, (4) adding `--set` flag to the slice command with default set support, and (5) surfacing compatibility warnings during slice execution.

**Primary recommendation:** Build the compatibility engine extensions first in `slicecore-engine`, then layer CLI commands on top. Reuse the existing `ProfileIndexEntry` fields (material, nozzle_size, vendor, profile_type) for filter matching -- they already contain exactly the metadata needed.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **Search Query Design:** Fuzzy substring matching, case-insensitive, across name/vendor/material/ID. AND logic for query + filters. Filter flags: `--material / -m`, `--vendor / -v`, `--nozzle / -n`, `--type / -t`. Compatible-by-default with `--include-incompatible` escape hatch. Enabled-only default with `--all` bypass. `search --enable` combo workflow.
- **Compatibility Engine:** Nozzle diameter exact match (profiles without nozzle_size compatible with all). Temperature range validation (warn if printer can't reach filament min). Non-blocking stderr warnings. Both inline warnings and dedicated `profile compat <id>` command.
- **Compatibility Warnings in Slice:** Show in pre-slice summary on stderr. Non-blocking. Uses same compatibility engine.
- **Enhanced List Command:** Gains same filter flags as search. Distinction: list dumps/filters without text query; search requires query. Compatibility column opt-in via `--compat`. Retains Phase 43 `--enabled`/`--disabled`/`--all`.
- **Profile Sets:** Named machine + filament + process triple. Stored in `[sets]` section of `~/.slicecore/enabled-profiles.toml`. Default set support. `--set` flag in slice command. CLI under `profile set` with create/delete/list/show/default subcommands.

### Claude's Discretion
- Search result ranking/ordering within substring matches
- Table column layout and formatting details for search/list output
- `profile compat <id>` output format and level of detail
- Whether `profile set search` is warranted given typical set counts
- How `--set` flag integrates with existing -m/-f/-p flag validation in slice command
- Error handling when a set references profiles that no longer exist or are disabled
- Test strategy and fixtures for compatibility engine extensions

### Deferred Ideas (OUT OF SCOPE)
- Hardware requirement checks (enclosure, direct drive, heated bed)
- Profile recommendations ("users with X1C also use...")
- Scored fuzzy matching (typo-tolerant with edit distance)
- Profile set search
- Batch set operations (import/export sets)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| API-02 | Full-featured CLI interface (slice, validate, analyze commands) | This phase extends the CLI with search, compat, and set subcommands plus filter flags on list. All new commands follow established clap derive patterns in `profile_command.rs`. The `--set` flag on slice and pre-slice compatibility warnings enhance the slice command. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | (workspace) | CLI argument parsing with derive macros | Already used throughout; `ProfileCommand` enum uses `#[derive(Subcommand)]` |
| serde + toml | (workspace) | TOML serialization for EnabledProfiles + ProfileSet | Already used for enabled-profiles.toml; extend with `[sets]` section |
| serde_json | (workspace) | JSON output via `--json` flag | Already used on all profile commands |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| anyhow | (workspace) | Error handling in CLI layer | All `cmd_*` functions return `Result<(), anyhow::Error>` |
| thiserror | (workspace) | Error types in engine layer | For new error variants in CompatibilityInfo |
| tempfile | (workspace) | Temporary directories for tests | Test fixtures for enabled-profiles.toml with sets |

No new dependencies are needed. Everything builds on existing workspace crates.

## Architecture Patterns

### Recommended Project Structure
```
crates/slicecore-engine/src/
  enabled_profiles.rs     # Extend EnabledProfiles with ProfileSet, default_set
                          # Extend CompatibilityInfo with check_nozzle(), check_temperature()
  profile_library.rs      # ProfileIndexEntry already has all needed fields
  profile_resolve.rs      # ProfileResolver::search() already works; may need index-based search

crates/slicecore-cli/src/
  profile_command.rs      # Add Compat, Set(SetCommand) variants to ProfileCommand enum
                          # Add filter flags to Search and List variants
                          # Add cmd_compat(), cmd_set_*() handler functions
  slice_workflow.rs       # Add --set flag expansion and pre-slice compatibility warnings
  main.rs                 # Add --set flag to Slice command struct
```

### Pattern 1: Shared Filter Struct
**What:** Extract filter flags into a shared struct used by both `search` and `list` commands.
**When to use:** When multiple commands share the same filter parameters.
**Example:**
```rust
/// Shared filter flags for profile search and list commands.
#[derive(clap::Args, Default)]
pub struct ProfileFilters {
    /// Filter by material type (PLA, ABS, PETG, etc.)
    #[arg(short = 'm', long)]
    pub material: Option<String>,

    /// Filter by vendor name
    #[arg(short = 'v', long)]
    pub vendor: Option<String>,

    /// Filter by nozzle diameter (mm)
    #[arg(short = 'n', long)]
    pub nozzle: Option<f64>,

    /// Filter by profile type (machine, filament, process)
    #[arg(short = 't', long = "type")]
    pub profile_type: Option<String>,
}
```

Then flatten into both Search and List variants:
```rust
Search {
    query: String,
    #[command(flatten)]
    filters: ProfileFilters,
    // ...
},
List {
    #[command(flatten)]
    filters: ProfileFilters,
    // ...
},
```

### Pattern 2: Compatibility Check Result Enum
**What:** Return structured compatibility results instead of just bool.
**When to use:** For `profile compat <id>` detailed output and inline warnings.
**Example:**
```rust
#[derive(Debug, Clone)]
pub enum CompatCheck {
    Compatible,
    NozzleMismatch { profile_nozzle: f64, printer_nozzles: Vec<f64> },
    TemperatureWarning { filament_min: f64, printer_max: f64 },
}

#[derive(Debug, Clone)]
pub struct CompatReport {
    pub checks: Vec<CompatCheck>,
}

impl CompatReport {
    pub fn is_compatible(&self) -> bool {
        self.checks.iter().all(|c| matches!(c, CompatCheck::Compatible))
    }
    pub fn warnings(&self) -> Vec<&CompatCheck> {
        self.checks.iter().filter(|c| !matches!(c, CompatCheck::Compatible)).collect()
    }
}
```

### Pattern 3: Profile Set in EnabledProfiles TOML
**What:** Extend the existing `EnabledProfiles` struct with a `sets` HashMap and `default_set` field.
**When to use:** For profile set storage alongside existing enabled profiles.
**Example TOML format:**
```toml
[machine]
enabled = ["BBL/Bambu_X1C"]

[filament]
enabled = ["Bambu_PLA_Basic"]

[process]
enabled = ["0.20mm_Standard"]

[sets.my-x1c-pla]
machine = "BBL/Bambu_X1C"
filament = "Bambu_PLA_Basic"
process = "0.20mm_Standard"

[sets.my-petg-draft]
machine = "BBL/Bambu_X1C"
filament = "Bambu_PETG_Basic"
process = "0.12mm_Draft"

[defaults]
set = "my-x1c-pla"
```

### Pattern 4: Nested Subcommand Group for profile set
**What:** Use a nested enum for set subcommands under `profile set`.
**When to use:** The established pattern from `ProfileCommand` and `CsgCommand`.
**Example:**
```rust
/// Profile set management subcommands.
#[derive(Subcommand)]
pub enum SetCommand {
    Create { name: String, #[arg(long)] machine: String, #[arg(long)] filament: String, #[arg(long)] process: String },
    Delete { name: String, #[arg(long)] yes: bool },
    List { #[arg(long)] json: bool },
    Show { name: String },
    Default { name: String },
}
```

### Anti-Patterns to Avoid
- **Duplicating filter logic:** Don't implement filtering separately in search and list. Extract shared `apply_filters()` function.
- **Blocking on compatibility warnings:** The CONTEXT.md is explicit -- compatibility warnings are non-blocking stderr. Never prevent slicing due to nozzle mismatch or temp concerns.
- **Loading full profiles for search:** Use `ProfileIndexEntry` metadata (already has material, nozzle_size, vendor, printer_model) for search/filter. Don't load and parse TOML files just to filter.
- **Conflicting short flags:** The existing `Search` variant uses `-l` for `--limit`. The `List` variant's existing `--material` flag has no short form. Use `-m`/`-v`/`-n`/`-t` as specified in CONTEXT.md, but be careful: `-m` conflicts with the top-level `slice --machine` short flag. Since these are subcommand-scoped, no actual conflict.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CLI argument flattening | Manual arg forwarding | `#[command(flatten)]` with shared `ProfileFilters` struct | Clap handles validation, help text, and conflict detection |
| TOML serialization for sets | Manual string formatting | `serde::Serialize/Deserialize` on `ProfileSet` struct | Serde handles nested tables, escaping, round-trip correctness |
| Substring matching | Custom search algorithm | Existing `ProfileResolver::search()` + `ProfileIndexEntry` field checks | Already case-insensitive, handles user + library sources |

**Key insight:** The existing `ProfileIndexEntry` struct already has every field needed for filtering (material, nozzle_size, vendor, profile_type, printer_model). The `CompatibilityInfo::from_index_entries()` already builds compatibility from index data. The work is extending these, not replacing them.

## Common Pitfalls

### Pitfall 1: Nozzle Size Comparison with Floating Point
**What goes wrong:** `0.4 != 0.40000000000000002` in f64 comparison.
**Why it happens:** Nozzle sizes from TOML parsing may have floating-point precision issues.
**How to avoid:** Use epsilon comparison (`(a - b).abs() < 0.001`) for nozzle diameter matching, or round to 2 decimal places before comparing.
**Warning signs:** Tests pass with hardcoded values but fail with parsed TOML values.

### Pitfall 2: Short Flag Conflicts Between Subcommands
**What goes wrong:** `-m` means `--material` in search/list but `--machine` in slice context.
**Why it happens:** Clap short flags are scoped to the subcommand, but users may be confused.
**How to avoid:** Since these are different subcommands (`profile search -m PLA` vs `slice -m X1C`), there's no technical conflict. But ensure help text is clear about what `-m` means in each context.
**Warning signs:** User confusion in help output.

### Pitfall 3: EnabledProfiles TOML Backward Compatibility
**What goes wrong:** Adding `[sets]` and `[defaults]` sections to the TOML file breaks loading on older versions.
**Why it happens:** `#[serde(default)]` is needed on new fields to handle files without the new sections.
**How to avoid:** Always use `#[serde(default)]` on new `EnabledProfiles` fields. Use `HashMap<String, ProfileSet>` which defaults to empty. Use `Option<String>` for `default_set` which defaults to `None`.
**Warning signs:** `EnabledProfiles::load()` fails on files created before Phase 44.

### Pitfall 4: Temperature Data Not Available in ProfileIndexEntry
**What goes wrong:** Temperature range validation needs filament's min temp and printer's max nozzle temp, but `ProfileIndexEntry` doesn't have temperature fields.
**Why it happens:** The index was designed for search metadata, not full config data.
**How to avoid:** For temperature checks, load the actual profile TOML to extract temp values. Only do this for the `profile compat <id>` detailed command and pre-slice warnings -- not during search/list filtering (too expensive). The compatibility engine should have two tiers: fast (index-based: nozzle, material, vendor) and detailed (config-based: temperature ranges).
**Warning signs:** Trying to add temp fields to ProfileIndexEntry bloats the index unnecessarily.

### Pitfall 5: Set References to Deleted/Disabled Profiles
**What goes wrong:** User deletes a profile that's referenced in a set. Set becomes invalid.
**Why it happens:** Sets store profile IDs as strings, no referential integrity.
**How to avoid:** Validate set references lazily (at use time, not save time). When expanding `--set`, check that each referenced profile exists and is enabled. Provide clear error message: "Set 'my-x1c-pla' references profile 'PLA_Basic' which is no longer enabled. Run 'profile set show my-x1c-pla' for details."

## Code Examples

### Extending CompatibilityInfo with Nozzle Check
```rust
// In enabled_profiles.rs
impl CompatibilityInfo {
    /// Checks nozzle diameter compatibility between a filament profile and enabled printers.
    ///
    /// Returns `None` if compatible (or no nozzle data), `Some(mismatch)` if incompatible.
    pub fn check_nozzle(
        entry: &ProfileIndexEntry,
        machine_entries: &[&ProfileIndexEntry],
    ) -> Option<CompatCheck> {
        let Some(filament_nozzle) = entry.nozzle_size else {
            return None; // No nozzle constraint = compatible with all
        };
        let printer_nozzles: Vec<f64> = machine_entries
            .iter()
            .filter_map(|m| m.nozzle_size)
            .collect();
        if printer_nozzles.is_empty() {
            return None; // No printer nozzle data = compatible
        }
        let matches = printer_nozzles
            .iter()
            .any(|n| (n - filament_nozzle).abs() < 0.001);
        if matches {
            None
        } else {
            Some(CompatCheck::NozzleMismatch {
                profile_nozzle: filament_nozzle,
                printer_nozzles,
            })
        }
    }
}
```

### Profile Set Data Model
```rust
// In enabled_profiles.rs
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProfileSet {
    pub machine: String,
    pub filament: String,
    pub process: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DefaultsSection {
    #[serde(default)]
    pub set: Option<String>,
}

// Extend EnabledProfiles:
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct EnabledProfiles {
    #[serde(default)]
    pub machine: ProfileSection,
    #[serde(default)]
    pub filament: ProfileSection,
    #[serde(default)]
    pub process: ProfileSection,
    #[serde(default)]
    pub sets: HashMap<String, ProfileSet>,
    #[serde(default)]
    pub defaults: DefaultsSection,
}
```

### Filter Application on ProfileIndexEntry
```rust
/// Applies filter flags to a profile index entry.
fn matches_filters(entry: &ProfileIndexEntry, filters: &ProfileFilters) -> bool {
    if let Some(ref mat) = filters.material {
        match &entry.material {
            Some(m) if m.to_lowercase().contains(&mat.to_lowercase()) => {}
            _ => return false,
        }
    }
    if let Some(ref vendor) = filters.vendor {
        if !entry.vendor.to_lowercase().contains(&vendor.to_lowercase()) {
            return false;
        }
    }
    if let Some(nozzle) = filters.nozzle {
        match entry.nozzle_size {
            Some(n) if (n - nozzle).abs() < 0.001 => {}
            _ => return false,
        }
    }
    if let Some(ref ptype) = filters.profile_type {
        if entry.profile_type != *ptype {
            return false;
        }
    }
    true
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Top-level `search-profiles` / `list-profiles` commands | `profile search` / `profile list` subcommands (Phase 43) | Phase 43 | Both still exist; subcommands are the canonical path |
| No activation filtering | Enabled-only default with `--all` bypass (Phase 43) | Phase 43 | Search and list must inherit this pattern |
| No compatibility checks | `CompatibilityInfo` with filament type/vendor/ID checks | Phase 43 | Must extend, not replace |

## Open Questions

1. **Temperature Data Source for Compatibility**
   - What we know: `ProfileIndexEntry` has `nozzle_size` but NOT temperature fields. `FilamentConfig` has `nozzle_temperature_range_low/high`. `MachineConfig` has `nozzle_diameters` but NO max nozzle temperature field.
   - What's unclear: Where does "printer max nozzle temp" come from? MachineConfig doesn't have this field. The safety limit in `config_validate.rs` is a hardcoded 500C constant, not per-printer.
   - Recommendation: For Phase 44, temperature validation can only check filament temp ranges against the hardcoded safety limit (already done in config_validate.rs). Per-printer max temp would require adding a field to MachineConfig and re-converting profiles. Recommend implementing temperature check as: warn if filament `nozzle_temperature_range_high` exceeds a reasonable threshold (e.g., 300C for standard printers). Document this as a known limitation -- full printer-specific temp validation needs profile metadata enrichment.

2. **Index-based vs Config-based Search**
   - What we know: `ProfileResolver::search()` returns `ResolvedProfile` (name, path, source, type) but NOT `ProfileIndexEntry` data (material, nozzle_size, vendor). The search currently only matches on name.
   - What's unclear: Should we refactor search to return index entries, or do a two-phase approach (search by name, then filter by index metadata)?
   - Recommendation: Add a new `search_index_entries()` method to `ProfileResolver` that returns `Vec<ProfileIndexEntry>` with full metadata for filtering. The existing `search()` can remain for backward compatibility. Alternatively, since `cmd_list()` already uses `resolver.search("", ...)` followed by manual vendor filtering, the pattern of search-then-filter is established.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml workspace `[workspace.lints]` |
| Quick run command | `cargo test -p slicecore-engine --lib enabled_profiles` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| API-02a | Nozzle diameter compatibility check | unit | `cargo test -p slicecore-engine --lib enabled_profiles::tests::nozzle` | Wave 0 |
| API-02b | Temperature range compatibility check | unit | `cargo test -p slicecore-engine --lib enabled_profiles::tests::temperature` | Wave 0 |
| API-02c | Profile search with filter flags | unit | `cargo test -p slicecore-engine --lib -- search_filter` | Wave 0 |
| API-02d | Profile set CRUD operations | unit | `cargo test -p slicecore-engine --lib enabled_profiles::tests::set` | Wave 0 |
| API-02e | Profile set TOML round-trip | unit | `cargo test -p slicecore-engine --lib enabled_profiles::tests::set_roundtrip` | Wave 0 |
| API-02f | CLI search command with filters | integration | `cargo test -p slicecore-cli --test cli_profile_search` | Wave 0 |
| API-02g | CLI compat command output | integration | `cargo test -p slicecore-cli --test cli_profile_compat` | Wave 0 |
| API-02h | CLI set subcommands | integration | `cargo test -p slicecore-cli --test cli_profile_set` | Wave 0 |
| API-02i | Slice --set flag expansion | integration | `cargo test -p slicecore-cli --test cli_slice_profiles -- set` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-engine --lib enabled_profiles && cargo test -p slicecore-cli --lib profile_command`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/slicecore-engine/src/enabled_profiles.rs` -- tests for nozzle check, temp check, ProfileSet CRUD, sets round-trip
- [ ] `crates/slicecore-cli/tests/cli_profile_search.rs` -- CLI integration tests for search with filters
- [ ] `crates/slicecore-cli/tests/cli_profile_compat.rs` -- CLI integration tests for compat command
- [ ] `crates/slicecore-cli/tests/cli_profile_set.rs` -- CLI integration tests for set subcommands

## Sources

### Primary (HIGH confidence)
- Source code: `crates/slicecore-engine/src/enabled_profiles.rs` -- CompatibilityInfo, EnabledProfiles, ProfileSection
- Source code: `crates/slicecore-engine/src/profile_library.rs` -- ProfileIndexEntry with material, nozzle_size, vendor, printer_model fields
- Source code: `crates/slicecore-cli/src/profile_command.rs` -- ProfileCommand enum, cmd_search, cmd_list implementations
- Source code: `crates/slicecore-engine/src/config.rs` -- MachineConfig (nozzle_diameters, no max_temp), FilamentConfig (nozzle_temperature_range_low/high)
- Source code: `crates/slicecore-cli/src/slice_workflow.rs` -- SliceWorkflowOptions, run_slice_workflow
- Source code: `crates/slicecore-engine/src/profile_resolve.rs` -- ProfileResolver::search(), ResolvedProfile

### Secondary (MEDIUM confidence)
- CONTEXT.md Phase 44 -- user decisions and implementation constraints
- CONTEXT.md Phase 43 -- EnabledProfiles design, CompatibilityInfo, activation model

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, all patterns established in prior phases
- Architecture: HIGH -- direct extension of existing code with clear integration points
- Pitfalls: HIGH -- identified from actual code inspection (floating point, missing temp fields, TOML backward compat)

**Research date:** 2026-03-23
**Valid until:** 2026-04-23 (stable internal architecture, no external dependency changes)
