# Phase 38: Profile Diff Command - Context

**Gathered:** 2026-03-19
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement a `slicecore diff-profiles` CLI subcommand that compares two print profiles side by side. Shows settings that differ, grouped by SettingCategory, with optional impact hints from the dependency graph, and supports table (human) and JSON (programmatic) output formats. Does NOT include multi-way diff (3+ profiles), profile merging, or profile editing.

</domain>

<decisions>
## Implementation Decisions

### Input & Profile Resolution
- Accept both profile library names (e.g., `BBL/PLA_Basic`) and file paths (e.g., `config.toml`) — reuse existing `ProfileResolver` logic
- Same-type comparison only — error if comparing mismatched types (machine vs filament)
- `--defaults` flag compares a single profile against built-in `PrintConfig::default()`
- CLI command is top-level `diff-profiles` (matches existing `list-profiles`, `search-profiles`, `show-profile` pattern)
- Labels use actual resolved profile names as column headers (not A/B)

### Diff Display & Grouping
- Default: show only settings that differ between the two profiles (`--all` flag to show everything)
- Group by `SettingCategory` from the schema registry (15 variants: Quality, Speed, Cooling, etc.)
- Categories with no differences are hidden by default
- Show both display name AND raw key: `First Layer Height (first_layer_height)`
- Units from SettingRegistry shown next to values: `45.0 mm/s`
- Summary header: total differences + per-category breakdown
  ```
  Comparing: BBL/PLA_Basic vs BBL/ABS_Basic
  12 differences across 4 categories:
    Speed: 5  |  Quality: 3  |  Retraction: 2  |  Cooling: 2
  ```

### Impact Hints & Annotations
- Impact hints are opt-in via `--verbose` / `-v` flag
- Default output is clean values-only table
- Verbose mode adds: `affects` list (from dependency graph) + setting description
- `--category <name>` flag filters diff to specific category (repeatable for multiple categories)
- `--tier <level>` flag filters to specific tier level (simple, intermediate, advanced, developer)

### Output Formats
- Table format (default): aligned columns with category headers, terminal colors with auto-detection
- JSON format (`--json`): full metadata per entry (key, display_name, category, tier, left/right values, affects, description)
- Terminal colors: green/red for changed values, auto-detect TTY, `--color always/never/auto` override
- `--quiet` / `-q` flag: suppress output, exit code 0 = identical, 1 = different (for scripting)
- JSON summary includes total count and per-category breakdown

### Claude's Discretion
- Internal diff algorithm (field-by-field comparison via serde_json or reflection)
- Color scheme and exact ANSI codes
- How to handle nested struct comparison (flatten to dotted keys vs recursive)
- Table column width calculation and alignment logic
- Error messages for profile resolution failures
- How to compare enum values (display name vs variant name)
- Test strategy and integration test fixtures

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Profile system
- `crates/slicecore-engine/src/config.rs` — PrintConfig and all sub-structs (~400+ fields with defaults)
- `crates/slicecore-engine/src/profile_resolve.rs` — ProfileResolver for resolving names/paths to configs
- `crates/slicecore-engine/src/profile_library.rs` — ProfileIndexEntry, batch conversion, profile index

### Schema system (Phase 35)
- `crates/slicecore-config-schema/src/types.rs` — SettingDefinition, SettingCategory enum, SettingKey, Tier, affects/affected_by
- `crates/slicecore-config-schema/src/lib.rs` — SettingRegistry with search, global singleton

### CLI patterns
- `crates/slicecore-cli/src/main.rs` — Existing Commands enum, clap Subcommand pattern
- `crates/slicecore-cli/src/schema_command.rs` — Schema subcommand as reference for registry-aware CLI
- `crates/slicecore-cli/src/plugins_command.rs` — Subcommand group pattern

### Prior config decisions
- `.planning/phases/35-configschema-system-with-setting-metadata-and-json-schema-generation/35-CONTEXT.md` — SettingCategory, tier system, affects graph, JSON output patterns

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `SettingRegistry::global()` — lazy singleton with all ~400+ setting definitions, categories, tiers, affects graph
- `SettingCategory` enum (15 variants) — ready to use for grouping
- `ProfileResolver` — resolves profile names and file paths to `PrintConfig`
- `ProfileSource` — tracks where each setting value came from
- `PrintConfig::default()` — built-in defaults for `--defaults` mode
- `SettingDefinition.affects` / `affected_by` — full dependency graph for impact hints
- `SettingDefinition.units` — units strings (mm/s, mm, °C, etc.) for annotated output
- `analysis_display.rs` — existing terminal display helpers (may have table formatting)

### Established Patterns
- Clap `#[derive(Subcommand)]` for top-level commands
- `--json` flag pattern used on multiple existing commands (slice, analyze-gcode)
- `--format` pattern used on schema and stats commands
- Serde serialization for all config types (JSON round-trip possible)
- `PrintConfig::from_toml()` / `from_json()` for loading profiles

### Integration Points
- `crates/slicecore-cli/src/main.rs` — add `DiffProfiles` variant to `Commands` enum
- `crates/slicecore-engine/` — diff logic lives here (new module, likely `profile_diff.rs`)
- `SettingRegistry` — query for category, display name, affects, tier, units per setting key
- Profile library index — for resolving profile names to file paths

</code_context>

<specifics>
## Specific Ideas

- Exit code behavior matches `diff` convention: 0 = identical, 1 = different, 2 = error
- Summary + category-grouped output is the core UX — scannable at a glance, detailed when needed
- The `--verbose` flag should be the go-to for understanding WHY profiles differ (not just WHAT differs)
- JSON output should be rich enough that a frontend could render its own diff view without additional API calls

</specifics>

<deferred>
## Deferred Ideas

- **Multi-way diff** — comparing 3+ profiles with matrix display (significant UX complexity)
- **Markdown output format** — formatted tables for docs/PRs/chat
- **CSV output format** — flat export for spreadsheets
- **--only-keys filter** — show specific settings by key name
- **Profile merge command** — combine two profiles, resolving conflicts
- **Interactive diff** — TUI with expandable categories and setting details
- **Diff against upstream** — detect what changed since profile was imported from OrcaSlicer/PrusaSlicer

</deferred>

---

*Phase: 38-profile-diff-command-to-compare-presets-side-by-side-implement-slicecore-profile-diff-cli-subcommand-with-settings-comparison-category-grouping-impact-hints-and-multiple-output-formats*
*Context gathered: 2026-03-19*
