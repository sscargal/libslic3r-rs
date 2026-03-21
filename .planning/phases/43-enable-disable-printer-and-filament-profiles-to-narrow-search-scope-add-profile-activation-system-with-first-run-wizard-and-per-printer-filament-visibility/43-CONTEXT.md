# Phase 43: Enable/Disable Printer and Filament Profiles - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement an enable/disable system for printer and filament profiles using `~/.slicecore/enabled-profiles.toml`, with CLI commands (enable/disable/list/setup/status), interactive first-run wizard with vendor-to-model-to-filament flow, per-printer filament visibility filtering, and import-aware setup detection. Extends the existing `slicecore profile` subcommand group from Phase 42.

**Not in scope:** Network printer discovery (mDNS/SSDP), compatibility scores/ratings, community-based recommendations, profile enable with search, batch enable/disable by vendor glob.

</domain>

<decisions>
## Implementation Decisions

### Activation Model
- **Hybrid model:** Nothing enabled by default. Built-in profiles are NOT auto-enabled — wizard guides selection for all users regardless of profile source
- First-run wizard triggers on first `slicecore slice` attempt when no `enabled-profiles.toml` exists
- Other commands (list-profiles, search-profiles) work without setup via `--all` flag
- All slicer wizards (PrusaSlicer, OrcaSlicer, etc.) assume nothing and ask what printer(s) you have — we follow the same approach

### Config File
- Location: `~/.slicecore/enabled-profiles.toml`
- Typed sections: `[machine]`, `[filament]`, `[process]` each with `enabled = [...]` array of profile IDs
- Individual profile granularity (not vendor-level)
- Example:
  ```toml
  [machine]
  enabled = ["BBL/Bambu_X1C", "BBL/Bambu_A1"]

  [filament]
  enabled = ["BBL/PLA_Basic", "BBL/PETG_Basic", "Generic/PLA"]

  [process]
  enabled = ["BBL/0.20mm_Standard"]
  ```

### ProfileResolver Integration
- ProfileResolver gains `enabled_only` mode — default for list/search/slice commands
- `--all` flag on profile commands bypasses activation filter and shows everything
- When no `enabled-profiles.toml` exists and `--all` is not set, show hint to run `profile setup`

### CLI Commands (under `slicecore profile`)
- **`profile enable <id>...`** — Enable one or more profiles by ID. Bare `profile enable` (no args) launches interactive picker
- **`profile disable <id>...`** — Disable one or more profiles. Bare `profile disable` launches picker showing enabled profiles
- **`profile setup`** — Interactive first-run wizard (vendor → model → filaments flow)
- **`profile setup --reset`** — Clear all enabled profiles and start fresh
- **`profile setup --machine <id> --filament <id>`** — Non-interactive setup for CI/scripts
- **`profile status`** — Quick overview: "2 printers, 15 filaments, 4 process profiles enabled"
- **`profile list --enabled`** (default) / `--disabled` / `--all`** — Activation-aware listing
- Auto-detect profile type from profile metadata; `--type` flag as optional override/filter for interactive picker
- `--json` flag on all commands for programmatic output

### First-Run Wizard
- **Trigger:** First `slicecore slice` when no `enabled-profiles.toml` exists. Non-TTY contexts skip wizard with warning + setup instructions
- **Flow:** Vendor selection → Printer model selection → Compatible filaments shown (Enter for all compatible)
- **Process profiles:** Auto-enabled for selected printers (quality presets like 0.08mm Detail, 0.20mm Standard apply to all)
- **Import-aware:** If no profile library is found, wizard detects this and offers to run `import-profiles` from an installed slicer first
- **Re-runnable:** Running `profile setup` again allows adding or removing profiles (not replace-only). Shows current state and allows modifications
- **Non-interactive:** `profile setup --machine BBL/Bambu_X1C --filament BBL/PLA_Basic` for scripted environments. Skip wizard + warn for auto-trigger in non-TTY

### Per-Printer Filament Visibility
- **Compatibility source:** Machine profiles declare compatible filament types/vendors in a `[compatibility]` section. Data extracted from imported profiles
- **Filtering behavior:** `profile list --type filament` shows only filaments compatible with enabled printers. `--all` bypasses. Multiple enabled printers → union of compatible filaments
- **User-created filaments:** Inherit compatibility from clone source. User can override via `profile set my-pla compatibility.printers [...]`
- **Incompatible slice:** Warn on stderr ("Filament X may not be compatible with printer Y") but proceed. Not a blocking error

### Claude's Discretion
- Interactive picker implementation (dialoguer, inquire, or custom)
- Vendor list extraction from profile library index
- Compatibility section schema details in machine profiles
- TOML file read/write approach for enabled-profiles.toml
- Error message wording and formatting
- How wizard detects installed slicers for import suggestion
- Test strategy and fixtures
- Profile status output formatting details

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Profile system
- `crates/slicecore-engine/src/profile_resolve.rs` — ProfileResolver, resolution order, ProfileSource enum, ResolvedProfile. Must be extended with `enabled_only` mode
- `crates/slicecore-engine/src/config.rs` — PrintConfig, from_file(), profile loading
- `crates/slicecore-engine/src/profile_library.rs` — ProfileIndexEntry, library profile index, vendor/type metadata
- `crates/slicecore-engine/src/builtin_profiles.rs` — Built-in profile definitions

### CLI patterns
- `crates/slicecore-cli/src/profile_command.rs` — Existing profile subcommand group (clone/set/get/reset/edit/validate/delete/rename). Add enable/disable/setup/status here
- `crates/slicecore-cli/src/main.rs` — Commands enum, profile command routing
- `crates/slicecore-cli/src/plugins_command.rs` — Subcommand group reference pattern

### Schema & validation
- `crates/slicecore-config-schema/src/types.rs` — SettingDefinition, SettingKey for compatibility fields
- `crates/slicecore-config-schema/src/validate.rs` — Schema validation logic

### Prior phase context
- `.planning/phases/30-cli-profile-composition-and-slice-workflow/30-CONTEXT.md` — ProfileResolver design, user profile storage, slice workflow, `--all` flag pattern
- `.planning/phases/42-clone-and-customize-profiles-from-defaults-add-profile-clone-command-for-creating-custom-profiles-from-existing-presets-with-edit-and-validate-workflow/42-CONTEXT.md` — Profile subcommand group, clone metadata, library immutability

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ProfileResolver` (profile_resolve.rs): Core resolution engine — searches user → library → filesystem. Must be extended with enabled_only filtering mode
- `ProfileCommand` enum (profile_command.rs): Existing subcommand group with 8 commands — add Enable, Disable, Setup, Status variants
- `ProfileSource` enum (profile_resolve.rs): Tracks Library vs User vs BuiltIn source — useful for display in status/list
- `load_index()` and `ProfileIndexEntry` (profile_library.rs): Library index with vendor/type metadata — vendor list for wizard derived from this
- `PrintConfig::from_file()` (config.rs): TOML/JSON auto-detect loading — reuse for reading compatibility sections

### Established Patterns
- Clap `#[derive(Subcommand)]` for command groups (profile_command.rs, plugins_command.rs)
- `--json` flag for programmatic output on all profile commands
- ProfileResolver used consistently across list/search/show/diff/clone commands
- stderr for warnings/progress, stdout for output data
- `process::exit(1)` for fatal errors in CLI handlers

### Integration Points
- `ProfileCommand` enum in profile_command.rs: add Enable, Disable, Setup, Status variants
- `ProfileResolver::resolve()`: add enabled_only parameter or separate method
- `cmd_slice()` in main.rs: add wizard trigger check before profile resolution
- `~/.slicecore/enabled-profiles.toml`: new file, read/write from ProfileResolver and CLI commands
- Machine profile `[compatibility]` section: new metadata parsed during profile loading

</code_context>

<specifics>
## Specific Ideas

- Wizard should feel like PrusaSlicer/OrcaSlicer first-run experience — ask what printer(s) you have, what filament(s) you use, assume nothing
- "Enter for all compatible" on filament selection — sensible default that most users will accept
- `profile status` as a quick sanity check after setup: "2 printers, 15 filaments, 4 process profiles enabled"
- Import-aware: if wizard can't find profiles, guide user to import from installed slicer before continuing
- Re-running setup should show current enabled state and allow add/remove, not force a full redo
- Future improvement TODO: scan local network for printers (mDNS/SSDP) with user permission to auto-suggest printer selection in wizard

</specifics>

<deferred>
## Deferred Ideas

- **Network printer discovery** — Scan local network (mDNS/SSDP) to auto-detect printers for wizard. Needs network APIs and user permission flow. Future wizard enhancement
- **Compatibility scores/ratings** — Replace binary compatible/incompatible with confidence levels ("tested", "likely compatible", "unknown"). Needs richer data model
- **Community-based recommendations** — "Most popular filaments for your printer" suggestions. Needs external data source
- **`profile enable --search`** — Combine search + enable in one step. Convenience shortcut, not essential
- **Vendor-level enable/disable** — `profile enable --vendor BBL` to bulk-enable all profiles from a vendor. Useful but individual granularity covers the need

</deferred>

---

*Phase: 43-enable-disable-printer-and-filament-profiles*
*Context gathered: 2026-03-21*
