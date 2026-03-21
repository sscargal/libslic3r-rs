# Phase 43: Enable/Disable Printer and Filament Profiles - Research

**Researched:** 2026-03-21
**Domain:** Profile activation system, interactive CLI wizard, TOML config management
**Confidence:** HIGH

## Summary

This phase adds a profile activation layer on top of the existing `ProfileResolver` and `ProfileCommand` infrastructure. The core work involves: (1) a new `enabled-profiles.toml` config file at `~/.slicecore/enabled-profiles.toml` with typed sections, (2) four new CLI subcommands under `slicecore profile` (enable, disable, setup, status), (3) an interactive first-run wizard using `dialoguer` for multi-select prompts, and (4) per-printer filament compatibility filtering.

The existing codebase already has strong patterns to follow: `ProfileCommand` enum with clap derive for subcommands, `ProfileResolver` with search/resolve methods, `ProfileIndexEntry` with vendor/type/printer_model metadata, and `console` 0.15 already in dependencies. The `dialoguer` crate (0.12) is from the same `console-rs` organization and pairs naturally with the existing `console` dependency.

**Primary recommendation:** Use `dialoguer` 0.12 for interactive prompts. Add `EnabledProfiles` struct in slicecore-engine with TOML serde. Extend `ProfileResolver` with an `enabled_filter` method rather than modifying `resolve()` signature. Wire wizard trigger into `cmd_slice` early-exit path.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **Hybrid activation model:** Nothing enabled by default. Built-in profiles NOT auto-enabled. Wizard guides selection for all users
- **Config file:** `~/.slicecore/enabled-profiles.toml` with typed sections `[machine]`, `[filament]`, `[process]` each with `enabled = [...]` array of profile IDs
- **Individual profile granularity** (not vendor-level)
- **First-run wizard triggers** on first `slicecore slice` when no `enabled-profiles.toml` exists. Non-TTY skips with warning
- **Wizard flow:** Vendor selection -> Printer model selection -> Compatible filaments (Enter for all compatible). Process profiles auto-enabled for selected printers
- **Import-aware:** If no profile library found, wizard detects and offers to run `import-profiles`
- **Re-runnable setup:** Shows current state, allows add/remove modifications
- **CLI commands:** `profile enable <id>...`, `profile disable <id>...`, `profile setup`, `profile setup --reset`, `profile setup --machine <id> --filament <id>`, `profile status`
- **`--all` flag** on profile commands bypasses activation filter
- **`--json` flag** on all commands for programmatic output
- **Compatibility source:** Machine profiles declare compatible filament types/vendors in `[compatibility]` section
- **Per-printer filament filtering:** `profile list --type filament` shows only filaments compatible with enabled printers
- **Incompatible slice:** Warn on stderr but proceed (not blocking)
- **Auto-detect profile type** from metadata; `--type` flag as optional override

### Claude's Discretion
- Interactive picker implementation (dialoguer, inquire, or custom)
- Vendor list extraction from profile library index
- Compatibility section schema details in machine profiles
- TOML file read/write approach for enabled-profiles.toml
- Error message wording and formatting
- How wizard detects installed slicers for import suggestion
- Test strategy and fixtures
- Profile status output formatting details

### Deferred Ideas (OUT OF SCOPE)
- Network printer discovery (mDNS/SSDP)
- Compatibility scores/ratings
- Community-based recommendations
- `profile enable --search`
- Vendor-level enable/disable (`--vendor` glob)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| API-02 | Full-featured CLI interface (slice, validate, analyze commands) | Extended with profile enable/disable/setup/status subcommands. Existing `ProfileCommand` enum pattern provides direct integration point. `dialoguer` for interactive wizard. `--json` flag pattern already established on all profile commands |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| dialoguer | 0.12.0 | Interactive multi-select prompts, confirm dialogs | Same org as `console` (already in deps). Provides `MultiSelect`, `Select`, `Confirm` widgets. Most popular Rust CLI prompt library |
| toml | 0.8 (workspace) | Read/write `enabled-profiles.toml` | Already in workspace dependencies, used throughout project |
| serde | 1 (workspace) | Serialize/deserialize `EnabledProfiles` struct | Already in workspace dependencies |
| clap | 4.5 (existing) | CLI argument parsing for new subcommands | Already in use for all CLI commands |
| console | 0.15 (existing) | Terminal styling, TTY detection | Already in CLI dependencies |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde_json | 1 (workspace) | `--json` output serialization | All commands with `--json` flag |
| home | 0.5 (existing) | Home directory resolution for config path | Finding `~/.slicecore/` |
| comfy-table | 7 (existing) | Formatted table output | `profile status` and list display |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| dialoguer | inquire 0.9 | inquire has richer UX but different ecosystem from console-rs. dialoguer pairs with existing `console` dep |
| dialoguer | custom prompts | More control but significant effort for multi-select, pagination, filtering. Not worth hand-rolling |

**Installation:**
```bash
cargo add dialoguer@0.12 -p slicecore-cli
```

No other new dependencies needed -- everything else is already in the workspace.

## Architecture Patterns

### Recommended Project Structure
```
crates/slicecore-engine/src/
  enabled_profiles.rs    # EnabledProfiles struct, load/save, filtering logic
  profile_resolve.rs     # Extended with enabled_only filtering methods
  profile_library.rs     # ProfileIndexEntry already has vendor/type/printer_model

crates/slicecore-cli/src/
  profile_command.rs     # Add Enable, Disable, Setup, Status variants
  profile_wizard.rs      # NEW: Interactive wizard logic (separated for testability)
```

### Pattern 1: EnabledProfiles Config Struct
**What:** A serde-derivable struct that maps directly to the TOML file format
**When to use:** All read/write operations on `enabled-profiles.toml`
**Example:**
```rust
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

/// Tracks which profiles are enabled for the current user.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnabledProfiles {
    /// Enabled machine profiles.
    #[serde(default)]
    pub machine: ProfileSection,
    /// Enabled filament profiles.
    #[serde(default)]
    pub filament: ProfileSection,
    /// Enabled process profiles.
    #[serde(default)]
    pub process: ProfileSection,
}

/// A typed section within the enabled-profiles config.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileSection {
    /// Profile IDs that are enabled.
    #[serde(default)]
    pub enabled: Vec<String>,
}

impl EnabledProfiles {
    /// Default config path: `~/.slicecore/enabled-profiles.toml`
    pub fn default_path() -> Option<PathBuf> {
        home::home_dir().map(|h| h.join(".slicecore").join("enabled-profiles.toml"))
    }

    /// Load from file. Returns `None` if file doesn't exist.
    pub fn load(path: &Path) -> Result<Option<Self>, anyhow::Error> {
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(path)?;
        let profiles: Self = toml::from_str(&content)?;
        Ok(Some(profiles))
    }

    /// Save to file, creating parent directories.
    pub fn save(&self, path: &Path) -> Result<(), anyhow::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Check if a profile ID is enabled for a given type.
    pub fn is_enabled(&self, profile_type: &str, id: &str) -> bool {
        let section = match profile_type {
            "machine" => &self.machine,
            "filament" => &self.filament,
            "process" => &self.process,
            _ => return false,
        };
        section.enabled.iter().any(|e| e == id)
    }
}
```

### Pattern 2: ProfileResolver Extension (Filter, Don't Modify Resolve)
**What:** Add filtering methods to ProfileResolver rather than changing `resolve()` signature
**When to use:** When list/search commands need to filter by enabled status
**Example:**
```rust
impl ProfileResolver {
    /// Filter a list of resolved profiles to only those that are enabled.
    pub fn filter_enabled(
        &self,
        profiles: Vec<ResolvedProfile>,
        enabled: &EnabledProfiles,
    ) -> Vec<ResolvedProfile> {
        profiles
            .into_iter()
            .filter(|p| enabled.is_enabled(&p.profile_type, &p.name))
            .collect()
    }

    /// Get the index for vendor/model extraction.
    pub fn index(&self) -> Option<&ProfileIndex> {
        self.index.as_ref()
    }
}
```

### Pattern 3: Wizard as Separate Module
**What:** Extract wizard logic into `profile_wizard.rs` for testability
**When to use:** All interactive setup logic
**Rationale:** The wizard has complex flow (vendor -> model -> filaments) that benefits from separation. The module can accept trait-based prompt interfaces for testing.

### Pattern 4: Compatibility Section in Machine Profiles
**What:** Add a `[compatibility]` section to machine profile TOML files
**When to use:** Per-printer filament visibility filtering
**Example:**
```toml
# In a machine profile TOML
[compatibility]
# Compatible filament types for this printer
filament_types = ["PLA", "PETG", "ABS", "TPU", "ASA"]
# Compatible filament vendors (empty = all vendors)
filament_vendors = []
# Specific compatible filament IDs (if fine-grained control needed)
filament_ids = []
```

### Anti-Patterns to Avoid
- **Modifying `ProfileResolver::resolve()` signature:** Adding an `enabled_only` bool parameter creates a viral change across all callers. Use separate filter methods instead
- **Hardcoding vendor lists:** Extract vendors dynamically from `ProfileIndex` entries
- **Blocking on compatibility data:** Not all machine profiles will have `[compatibility]` sections. Default to "all filaments compatible" when section is missing
- **Mixing wizard UI with business logic:** Keep prompt rendering separate from profile selection logic for testability

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Multi-select terminal prompts | Custom stdin reader with arrow keys | `dialoguer::MultiSelect` | Handles terminal raw mode, scrolling, filtering, cross-platform |
| Single-select prompts | Custom menu | `dialoguer::Select` | Same reasons as above |
| Confirmation prompts | `print!("y/n: "); read_line()` | `dialoguer::Confirm` | Handles defaults, validation, non-TTY fallback |
| TOML serialization | Manual string building | `toml::to_string_pretty` with serde derive | Handles escaping, formatting, edge cases |
| TTY detection | Manual `/dev/tty` checks | `std::io::IsTerminal` (already used in codebase) | Standard library, cross-platform |
| Home directory | `$HOME` env var parsing | `home::home_dir()` (already in deps) | Cross-platform (Windows, macOS, Linux) |

**Key insight:** The `console-rs` ecosystem (`console` + `dialoguer`) is already half-present in this project. Adding `dialoguer` completes the interactive prompt toolkit without introducing a new ecosystem.

## Common Pitfalls

### Pitfall 1: Non-TTY Wizard Trigger in CI/Docker
**What goes wrong:** `slicecore slice` in a CI pipeline triggers the wizard, which hangs waiting for input
**Why it happens:** stdin is not a terminal in CI environments
**How to avoid:** Check `std::io::stdin().is_terminal()` before triggering wizard. In non-TTY, print a warning to stderr with instructions: `"No enabled profiles. Run: slicecore profile setup --machine <id> --filament <id>"` then exit with non-zero status
**Warning signs:** Tests passing locally but hanging in CI

### Pitfall 2: Profile ID Format Mismatch
**What goes wrong:** User enables `"BBL/PLA_Basic"` but library index stores `"orcaslicer/BBL/filament/Bambu_PLA_Basic_BBL_A1"`
**Why it happens:** Multiple ID formats exist: short names, full index IDs, file paths
**How to avoid:** Decide on ONE canonical ID format for `enabled-profiles.toml` and document it. Use the `ProfileIndexEntry.id` field as the canonical form. Provide normalization when enabling by short name
**Warning signs:** Enable command succeeds but profiles don't show in filtered list

### Pitfall 3: Empty Compatibility Section Blocks All Filaments
**What goes wrong:** Machine profiles without `[compatibility]` show zero compatible filaments
**Why it happens:** Treating missing section as "compatible with nothing" instead of "compatible with everything"
**How to avoid:** `Option<CompatibilitySection>` where `None` means "all compatible". Only restrict when explicit data exists
**Warning signs:** New printers or user-imported profiles show no filaments

### Pitfall 4: TOML Write Clobbers Manual Edits
**What goes wrong:** User manually adds comments to `enabled-profiles.toml`, `profile enable` overwrites them
**Why it happens:** `toml::to_string_pretty` serializes from struct, losing comments
**How to avoid:** This is acceptable for v1. Document that the file is managed by CLI. For future: consider `toml_edit` crate for round-trip preservation. Not needed now since the file is simple and machine-managed
**Warning signs:** User complaints about lost comments

### Pitfall 5: Wizard Re-run Replaces Instead of Modifies
**What goes wrong:** Running `profile setup` again wipes previously enabled profiles
**Why it happens:** Wizard writes fresh `EnabledProfiles` instead of merging
**How to avoid:** Load existing `enabled-profiles.toml` first, pre-select currently enabled items in multi-select, save merged result
**Warning signs:** User runs setup to add a printer and loses all filament selections

### Pitfall 6: dialoguer Panics on Non-TTY
**What goes wrong:** `dialoguer::Select::interact()` panics when no TTY is available
**Why it happens:** dialoguer requires a terminal for rendering
**How to avoid:** Always guard interactive prompts with `std::io::stdin().is_terminal()` check BEFORE calling any dialoguer functions. Fall back to non-interactive error path
**Warning signs:** Test failures when run with stdin redirected

## Code Examples

### Adding New Subcommands to ProfileCommand
```rust
// In profile_command.rs - add to the ProfileCommand enum
/// Enable one or more profiles by ID.
Enable {
    /// Profile IDs to enable (omit for interactive picker)
    ids: Vec<String>,

    /// Profile type filter for interactive picker
    #[arg(long)]
    r#type: Option<String>,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Override profiles directory
    #[arg(long)]
    profiles_dir: Option<PathBuf>,
},

/// Disable one or more profiles.
Disable {
    /// Profile IDs to disable (omit for interactive picker)
    ids: Vec<String>,

    /// Profile type filter for interactive picker
    #[arg(long)]
    r#type: Option<String>,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Override profiles directory
    #[arg(long)]
    profiles_dir: Option<PathBuf>,
},

/// Interactive first-run setup wizard.
Setup {
    /// Clear all enabled profiles and start fresh
    #[arg(long)]
    reset: bool,

    /// Machine profile ID (non-interactive)
    #[arg(long)]
    machine: Vec<String>,

    /// Filament profile ID (non-interactive)
    #[arg(long)]
    filament: Vec<String>,

    /// Process profile ID (non-interactive)
    #[arg(long)]
    process: Vec<String>,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Override profiles directory
    #[arg(long)]
    profiles_dir: Option<PathBuf>,
},

/// Show enabled profile summary.
Status {
    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Override profiles directory
    #[arg(long)]
    profiles_dir: Option<PathBuf>,
},
```

### Wizard Vendor Extraction from Index
```rust
/// Extract unique vendors from the profile index, sorted alphabetically.
fn extract_vendors(index: &ProfileIndex) -> Vec<String> {
    let mut vendors: Vec<String> = index
        .profiles
        .iter()
        .filter(|p| p.profile_type == "machine")
        .map(|p| p.vendor.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    vendors.sort();
    vendors
}

/// Extract machine profiles for a given vendor.
fn machines_for_vendor(index: &ProfileIndex, vendor: &str) -> Vec<&ProfileIndexEntry> {
    index
        .profiles
        .iter()
        .filter(|p| p.profile_type == "machine" && p.vendor == vendor)
        .collect()
}
```

### Wizard Trigger in cmd_slice
```rust
// Early in cmd_slice, before profile resolution:
let enabled_path = EnabledProfiles::default_path();
if let Some(ref path) = enabled_path {
    if !path.exists() && !force {
        if std::io::stdin().is_terminal() {
            eprintln!("No profiles enabled yet. Starting setup wizard...");
            eprintln!("(Use --force to skip, or run 'slicecore profile setup' manually)\n");
            // Launch wizard, then continue
            run_setup_wizard(profiles_dir)?;
        } else {
            eprintln!("Error: No enabled profiles found.");
            eprintln!("Run: slicecore profile setup --machine <id> --filament <id>");
            eprintln!("Or use --force to proceed with defaults.");
            process::exit(1);
        }
    }
}
```

### Slicer Detection for Import Suggestion
```rust
/// Check common slicer installation paths for import suggestion.
fn detect_installed_slicers() -> Vec<(&'static str, PathBuf)> {
    let mut found = Vec::new();

    let candidates = [
        ("OrcaSlicer", dirs_for_orcaslicer()),
        ("PrusaSlicer", dirs_for_prusaslicer()),
        ("BambuStudio", dirs_for_bambustudio()),
    ];

    for (name, paths) in &candidates {
        for path in paths {
            if path.is_dir() {
                found.push((*name, path.clone()));
                break; // One per slicer is enough
            }
        }
    }
    found
}

fn dirs_for_orcaslicer() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = home::home_dir() {
        // Linux
        dirs.push(home.join(".config/OrcaSlicer/system"));
        // macOS
        dirs.push(home.join("Library/Application Support/OrcaSlicer/system"));
    }
    dirs
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `atty` crate for TTY detection | `std::io::IsTerminal` (stable since Rust 1.70) | 2023 | No external dependency needed; already used in this codebase |
| `dialoguer` 0.10 with `console` 0.15 | `dialoguer` 0.12 with `console` 0.15 | 2024 | API stable, improved fuzzy search in Select |
| Manual TOML string building | `toml` 0.8 with serde derive | 2023 | Type-safe serialization |

**Deprecated/outdated:**
- `atty` crate: Unmaintained, use `std::io::IsTerminal` instead (already done in this project)

## Open Questions

1. **Canonical Profile ID format for enabled-profiles.toml**
   - What we know: `ProfileIndexEntry.id` is like `"orcaslicer/BBL/filament/Bambu_PLA_Basic_BBL_A1"`. CONTEXT.md examples use shorter `"BBL/PLA_Basic"` format
   - What's unclear: Which format to store in the enabled list. Full ID is unambiguous but verbose. Short ID needs resolution logic
   - Recommendation: Use `ProfileIndexEntry.id` as canonical form. Support short names in `profile enable` CLI args by resolving to full ID before storing. Display short form in wizard for readability

2. **Compatibility section population**
   - What we know: Machine profiles currently have no `[compatibility]` section. `ProfileIndexEntry` has `printer_model` field. Filament entries have `printer_model` field (the `@BBL X1C` suffix)
   - What's unclear: Whether to parse existing `printer_model` fields for compatibility or require explicit `[compatibility]` sections
   - Recommendation: Use existing `printer_model` field on filament entries as implicit compatibility (filament `"Bambu PLA @BBL X1C"` is compatible with machine whose model matches `"BBL X1C"`). Add `[compatibility]` section parsing as an enhancement for explicit overrides. Default to "all compatible" when no data exists

3. **Profile list `--enabled`/`--disabled` flag defaults**
   - What we know: CONTEXT says `profile list --enabled` (default) / `--disabled` / `--all`
   - What's unclear: Whether existing `profile list` without flags should default to `--enabled` (breaking change) or `--all` (backward compatible)
   - Recommendation: Default to `--enabled` when `enabled-profiles.toml` exists, `--all` when it doesn't. This matches the progressive disclosure model

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | workspace Cargo.toml |
| Quick run command | `cargo test -p slicecore-engine --lib enabled_profiles` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| API-02-a | EnabledProfiles load/save round-trip | unit | `cargo test -p slicecore-engine enabled_profiles` | No - Wave 0 |
| API-02-b | Enable/disable modifies TOML correctly | unit | `cargo test -p slicecore-engine enabled_profiles` | No - Wave 0 |
| API-02-c | ProfileResolver filters by enabled status | unit | `cargo test -p slicecore-engine profile_resolve::tests` | No - Wave 0 |
| API-02-d | Wizard non-TTY exits gracefully | integration | `cargo test -p slicecore-cli cli_profile_enable` | No - Wave 0 |
| API-02-e | `profile status` shows correct counts | integration | `cargo test -p slicecore-cli cli_profile_enable` | No - Wave 0 |
| API-02-f | Per-printer filament filtering | unit | `cargo test -p slicecore-engine enabled_profiles::compatibility` | No - Wave 0 |
| API-02-g | `--all` flag bypasses activation filter | integration | `cargo test -p slicecore-cli cli_profile_enable` | No - Wave 0 |
| API-02-h | `--json` output on all new commands | integration | `cargo test -p slicecore-cli cli_profile_enable` | No - Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-engine --lib enabled_profiles && cargo test -p slicecore-cli cli_profile_enable`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/slicecore-engine/src/enabled_profiles.rs` -- new module with unit tests
- [ ] `crates/slicecore-cli/tests/cli_profile_enable.rs` -- integration tests for enable/disable/setup/status commands
- [ ] Test fixture: sample `enabled-profiles.toml` files (empty, partial, full)
- [ ] Test fixture: sample machine profile with `[compatibility]` section

## Sources

### Primary (HIGH confidence)
- Existing codebase: `profile_resolve.rs`, `profile_command.rs`, `profile_library.rs`, `plugins_command.rs`, `main.rs` -- direct code inspection
- [dialoguer crate docs](https://docs.rs/dialoguer/latest/dialoguer/struct.MultiSelect.html) -- MultiSelect API
- [dialoguer on crates.io](https://crates.io/crates/dialoguer) -- version 0.12.0, console-rs ecosystem

### Secondary (MEDIUM confidence)
- [dialoguer GitHub](https://github.com/console-rs/dialoguer) -- same organization as `console` crate already in use

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all libraries already in project or from same ecosystem. Only new dependency is `dialoguer` 0.12
- Architecture: HIGH - patterns directly derived from existing `ProfileCommand`, `ProfileResolver`, and `PluginsCommand` code
- Pitfalls: HIGH - based on direct code inspection of TTY handling patterns already in codebase and known dialoguer behavior

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable domain, no fast-moving dependencies)
