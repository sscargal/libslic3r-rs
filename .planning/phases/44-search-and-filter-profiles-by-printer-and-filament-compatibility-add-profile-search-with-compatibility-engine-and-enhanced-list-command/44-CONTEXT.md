# Phase 44: Search and Filter Profiles by Printer and Filament Compatibility - Context

**Gathered:** 2026-03-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Add `slicecore profile search <query>` with filter flags (--material, --vendor, --nozzle, --type), extend the Phase 43 compatibility engine with nozzle diameter matching and temperature range validation, enhance `profile list` with the same filter flags, add profile sets (named machine+filament+process combos) with default set support, integrate search-to-enable workflow, and surface compatibility warnings during slice execution.

**Not in scope:** Hardware requirement checks (enclosure, direct drive — needs richer profile metadata), community-based profile recommendations (needs external data), scored fuzzy matching (typo-tolerant search), profile creation from scratch.

</domain>

<decisions>
## Implementation Decisions

### Search Query Design
- **Fuzzy substring matching:** Case-insensitive substring match across name, vendor, material, and ID fields
- Free-text positional query is required for `profile search`; filter flags are optional refinements
- **AND logic:** Query + all filter flags must match simultaneously
- **Filter flags:** `--material / -m`, `--vendor / -v`, `--nozzle / -n`, `--type / -t` — shared between search and list commands
- **Compatible by default:** Search results filtered to only profiles compatible with enabled printers. `--include-incompatible` flag shows all profiles, with incompatible ones displaying inline warnings (nozzle mismatch, temp concerns)
- **Enabled-only default:** Consistent with Phase 43 — shows only enabled profiles by default, `--all` bypasses activation filter
- **Search + enable combo:** `profile search <query> --enable` allows searching, picking results, and enabling them in one flow

### Compatibility Engine (extends Phase 43)
- **Nozzle diameter matching:** Exact match — if filament profile specifies nozzle_size=0.4, it only matches printers with 0.4mm nozzle. Profiles without nozzle_size are compatible with all nozzles
- **Temperature range validation:** Compare filament min temp against printer max nozzle temp. Warn if printer can't reach filament's minimum. Non-blocking warning only (stderr)
- **Hardware requirements deferred:** Enclosure, direct drive, heated bed checks need profile metadata that doesn't exist yet — noted for future phase
- **Compatibility display:** Both inline warnings in search/list results AND dedicated `profile compat <id>` command for detailed breakdown of compatibility with enabled printers

### Compatibility Warnings in Slice
- When running `slicecore slice`, show compatibility warnings (nozzle mismatch, temp range concerns) in the pre-slice summary
- Non-blocking warnings on stderr — don't prevent slicing, just inform the user
- Leverages the same compatibility engine used by search/list

### Enhanced List Command
- `profile list` gains same filter flags as search: `--material`, `--vendor`, `--nozzle`, `--type`
- **Distinction from search:** `profile list` dumps/filters without a text query; `profile search <query>` requires a search term. Both accept filter flags
- Compatibility column is opt-in via `--compat` flag, not shown by default
- Retains Phase 43's `--enabled` (default) / `--disabled` / `--all` flags

### Profile Sets (Favorites)
- **Named combo:** A profile set is a named machine + filament + process triple (e.g., 'my-x1c-pla' = X1C + PLA_Basic + 0.20mm_Standard)
- **Storage:** `[sets]` section in `~/.slicecore/enabled-profiles.toml` — keeps all profile state in one file
- **Default set:** One set can be marked as default — `slicecore slice model.stl` (no -m/-f/-p flags) uses the default set instead of requiring explicit profile selection
- **Slice integration:** `slicecore slice model.stl --set my-x1c-pla` expands to -m/-f/-p flags from the set
- **CLI commands under `profile set`:**
  - `profile set create <name> --machine X --filament Y --process Z` — create a named combo
  - `profile set delete <name>` — remove a saved set
  - `profile set list` — show all saved sets in a table
  - `profile set show <name>` — detailed view of a single set's profiles
  - `profile set default <name>` — mark a set as default for slice
  - Additional subcommands (e.g., `set search`) at Claude's discretion based on complexity

### Claude's Discretion
- Search result ranking/ordering within substring matches
- Table column layout and formatting details for search/list output
- `profile compat <id>` output format and level of detail
- Whether `profile set search` is warranted given typical set counts
- How `--set` flag integrates with existing -m/-f/-p flag validation in slice command
- Error handling when a set references profiles that no longer exist or are disabled
- Test strategy and fixtures for compatibility engine extensions

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Profile system
- `crates/slicecore-engine/src/profile_resolve.rs` — ProfileResolver, resolution order, ProfileSource enum, ResolvedProfile. Extended in Phase 43 with enabled_only mode
- `crates/slicecore-engine/src/enabled_profiles.rs` — EnabledProfiles data model, CompatibilityInfo engine (must be extended with nozzle + temp checks), ProfileSection
- `crates/slicecore-engine/src/config.rs` — PrintConfig, from_file(), profile loading
- `crates/slicecore-engine/src/profile_library.rs` — ProfileIndexEntry (has material, nozzle_size, printer_model fields), ProfileIndex, load_index()

### CLI patterns
- `crates/slicecore-cli/src/profile_command.rs` — ProfileCommand enum with existing subcommands. Add search, compat, set subcommands here
- `crates/slicecore-cli/src/main.rs` — Commands enum, profile command routing, slice command handler
- `crates/slicecore-cli/src/slice_workflow.rs` — Slice execution workflow, add compatibility warnings and --set flag here

### Prior phase context
- `.planning/phases/43-enable-disable-printer-and-filament-profiles-to-narrow-search-scope-add-profile-activation-system-with-first-run-wizard-and-per-printer-filament-visibility/43-CONTEXT.md` — EnabledProfiles design, CompatibilityInfo, activation model, --all flag, per-printer filament visibility
- `.planning/phases/42-clone-and-customize-profiles-from-defaults-add-profile-clone-command-for-creating-custom-profiles-from-existing-presets-with-edit-and-validate-workflow/42-CONTEXT.md` — Profile subcommand group structure, library immutability, user profile storage
- `.planning/phases/30-cli-profile-composition-and-slice-workflow/30-CONTEXT.md` — ProfileResolver design, -m/-f/-p flags, merge system, --set CLI flag interaction

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `CompatibilityInfo` (enabled_profiles.rs): Already has `is_compatible()` and `from_index_entries()` — must be extended with nozzle diameter and temperature range checks
- `ProfileResolver` (profile_resolve.rs): Resolves names to file paths, supports enabled_only filtering — used by both search and list
- `ProfileIndexEntry` (profile_library.rs): Has `material`, `nozzle_size`, `printer_model`, `vendor` fields — exactly the fields search needs to match against
- `EnabledProfiles` (enabled_profiles.rs): TOML serialization with typed sections — extend with `[sets]` section for profile sets
- `ProfileCommand` enum (profile_command.rs): Existing subcommand group — add Search, Compat, Set variants

### Established Patterns
- Clap `#[derive(Subcommand)]` for command groups (profile_command.rs, plugins_command.rs)
- `--json` flag for programmatic output on all profile commands
- `--all` flag to bypass activation filter (Phase 43 pattern)
- stderr for warnings, stdout for output data
- AND-logic filter combining (consistent with unix tool conventions)

### Integration Points
- `ProfileCommand` enum: add `Search`, `Compat`, `Set(SetCommand)` variants
- `CompatibilityInfo`: add `check_nozzle()` and `check_temperature()` methods
- `EnabledProfiles`: add `sets: HashMap<String, ProfileSet>` field with `default_set: Option<String>`
- `cmd_slice()` in main.rs/slice_workflow.rs: add `--set` flag expansion and pre-slice compatibility warnings
- `profile list` handler: add filter flag processing reusing search filter logic

</code_context>

<specifics>
## Specific Ideas

- Search compatible by default feels natural — when you search for filaments, you want ones that work with your printers. `--include-incompatible` is the escape hatch
- Inline compatibility warnings (nozzle mismatch icon, temp warning) in search/list results give quick visual feedback without cluttering output
- `profile compat <id>` detailed command is for when you want to understand WHY something is flagged
- Profile sets solve the repetitive `-m X -f Y -p Z` typing — default set means zero flags for common use case
- `search --enable` bridges the gap between discovery and activation in one workflow step
- Compatibility warnings during slice catch issues before wasting filament — surface them in pre-slice summary

</specifics>

<deferred>
## Deferred Ideas

- **Hardware requirement checks** — Enclosure, direct drive, heated bed compatibility checks. Needs profile metadata that doesn't exist yet (printer capabilities section)
- **Profile recommendations** — "Users with X1C also use these filaments" or vendor-curated suggestions. Needs external data source or enriched profile metadata
- **Scored fuzzy matching** — Typo-tolerant search with edit distance scoring. Current substring matching covers most cases; fuzzy adds complexity
- **Profile set search** — Search within saved profile sets. Likely unnecessary given typical set counts, but worth revisiting if usage grows
- **Batch set operations** — Create sets from currently enabled profile combinations, import/export sets

</deferred>

---

*Phase: 44-search-and-filter-profiles*
*Context gathered: 2026-03-23*
