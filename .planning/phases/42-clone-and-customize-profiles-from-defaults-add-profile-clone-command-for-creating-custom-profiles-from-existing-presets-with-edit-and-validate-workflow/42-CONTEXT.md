# Phase 42: Clone and Customize Profiles from Defaults - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Enable users to create custom profiles by cloning existing presets via `slicecore profile clone <source> <new-name>`, with subsequent editing via `slicecore profile set`, `slicecore profile edit`, and schema-based validation via `slicecore profile validate`. Includes a new `slicecore profile` subcommand group with clone, set, get, reset, edit, validate, delete, and rename operations. Existing top-level profile commands (list-profiles, show-profile, etc.) remain for backwards compatibility with aliases under the new `profile` group.

**Not in scope:** Profile creation from scratch (guided/template), profile import/export, profile versioning/changelog, profile groups/tags, batch operations, enable/disable system (Phase 43).

</domain>

<decisions>
## Implementation Decisions

### Command Structure
- New `slicecore profile` subcommand group (follows `calibrate`, `schema` pattern)
- Subcommands: `clone`, `set`, `get`, `reset`, `edit`, `validate`, `delete`, `rename`
- Existing top-level commands (list-profiles, show-profile, search-profiles, diff-profiles) remain as-is for backwards compatibility
- Add aliases under `profile` group: `profile list`, `profile show`, `profile diff`, `profile search`
- Implementation in dedicated `profile_command.rs` (follows plugins_command.rs, schema_command.rs pattern)
- Add `Profile(ProfileCommand)` variant to `Commands` enum in main.rs

### Profile Type Scope
- All operations work uniformly on all profile types (machine, filament, process)
- Profile type is preserved from source during clone
- No type-specific restrictions on any operation

### User Profile Storage
- User profiles stored in `~/.slicecore/profiles/` organized by type (machine/, filament/, process/) — consistent with Phase 30
- Library profiles are always immutable — all modification commands (set, edit, delete, rename) refuse to operate on library profiles with clear error message suggesting clone first

### Clone Behavior
- Creates a full standalone TOML copy with ALL settings from source
- Sets `inherits` metadata field pointing back to source for tracking
- Sets `is_custom = true` and records clone source in `[metadata]` section
- Profile names restricted to alphanumeric + hyphens + underscores (names become filenames directly: `my-pla` -> `my-pla.toml`)
- Name conflicts error with message suggesting `--force` to overwrite or different name
- Post-clone output: success message with file path + hint showing next steps (set, edit, show)
- Source resolution uses existing `ProfileResolver` (searches user profiles first, then library)

### Edit Workflow
- `profile set <name> <key> <value>`: single key-value per call, validated against SettingRegistry (type, range, constraints)
- Unknown keys error with "did you mean?" suggestions
- Out-of-range values error with valid range and current value shown
- `profile get <name> <key>`: read a single setting value from a profile
- `profile reset <name> <key>`: reset a setting back to the source profile's value (uses `inherits` metadata to look up original)
- `profile edit <name>`: opens TOML file in $EDITOR/$VISUAL, validates after editor closes, reports warnings/errors but saves regardless (user can fix with `set` or re-edit)
- `profile validate <name>`: on-demand schema validation, reports all errors and warnings

### Safety & Deletion
- `profile delete <name>`: user profiles only, requires `--yes` flag or interactive confirmation, shows file path before deleting
- Library profile deletion refused with clear error
- `profile rename <old> <new>`: atomic rename (moves file + updates metadata.name), validates new name, user profiles only
- All modification commands (set, edit, delete, rename) refuse to operate on library profiles with error suggesting clone first

### Claude's Discretion
- Internal data structures for profile metadata parsing
- TOML manipulation approach (toml_edit for in-place edits vs full deserialize-modify-serialize)
- Error message wording and formatting details
- Test strategy and fixtures
- $EDITOR detection fallback chain
- How `reset` resolves the original value from the inherits chain
- Whether aliases use clap aliases or separate handler routing

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Profile system
- `crates/slicecore-engine/src/profile_resolve.rs` -- ProfileResolver for resolving names/paths to configs, user profile search
- `crates/slicecore-engine/src/config.rs` -- PrintConfig struct, from_file(), TOML/JSON loading
- `crates/slicecore-engine/src/profile_library.rs` -- ProfileIndexEntry, library profile index
- `crates/slicecore-engine/src/builtin_profiles.rs` -- Built-in profile definitions

### Schema & validation
- `crates/slicecore-config-schema/src/types.rs` -- SettingDefinition, SettingCategory, SettingKey, Tier, affects/affected_by
- `crates/slicecore-config-schema/src/lib.rs` -- SettingRegistry with search, global singleton
- `crates/slicecore-config-schema/src/validate.rs` -- Schema validation logic
- `crates/slicecore-engine/src/config_validate.rs` -- ConfigSchemaValidator

### CLI patterns
- `crates/slicecore-cli/src/main.rs` -- Commands enum, existing profile command handlers
- `crates/slicecore-cli/src/plugins_command.rs` -- Subcommand group pattern (reference implementation)
- `crates/slicecore-cli/src/schema_command.rs` -- Another subcommand group pattern
- `crates/slicecore-cli/src/diff_profiles_command.rs` -- Profile-aware CLI command pattern

### Prior decisions
- `.planning/phases/30-cli-profile-composition-and-slice-workflow/30-CONTEXT.md` -- Profile resolution, user profile storage, merge system, CLI flag design
- `.planning/phases/35-configschema-system-with-setting-metadata-and-json-schema-generation/35-CONTEXT.md` -- SettingRegistry, validation, categories, tiers
- `.planning/phases/38-profile-diff-command-to-compare-presets-side-by-side-implement-slicecore-profile-diff-cli-subcommand-with-settings-comparison-category-grouping-impact-hints-and-multiple-output-formats/38-CONTEXT.md` -- Profile diff patterns, ProfileResolver usage in CLI

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ProfileResolver` (profile_resolve.rs): resolves profile names to file paths, searches user then library, supports type-constrained search -- core dependency for all profile commands
- `SettingRegistry::global()` (config-schema): lazy singleton with ~400+ setting definitions, categories, tiers, affects graph -- powers validation for set/edit/validate
- `ConfigSchemaValidator` (config_validate.rs): existing schema validation -- reuse for validate command
- `PrintConfig::from_file()` (config.rs): auto-detects TOML vs JSON, loads full config -- reuse for clone source loading
- `ProfileSource` enum (profile_resolve.rs): tracks Library vs User source -- use to enforce immutability rules

### Established Patterns
- Clap `#[derive(Subcommand)]` for command groups (plugins_command.rs, schema_command.rs)
- `--json` flag for programmatic output on profile commands
- ProfileResolver used consistently across list/search/show/diff commands
- stderr for warnings, stdout for output data
- `process::exit(1)` for fatal errors in CLI handlers

### Integration Points
- `Commands` enum in main.rs: add `Profile(ProfileCommand)` variant
- `ProfileResolver::new()`: construct with same config as existing commands
- `SettingRegistry::global()`: query for validation during set/edit
- `~/.slicecore/profiles/{type}/` directory: read/write user profile TOML files

</code_context>

<specifics>
## Specific Ideas

- Error messages for library profile modification should suggest clone first: `Cannot modify library profile "BBL/PLA_Basic". Clone it first: slicecore profile clone BBL/PLA_Basic my-pla`
- Post-clone output should show next steps: `set`, `edit`, `show` commands with the new profile name
- `profile set` should show "did you mean?" for typos in both key names and profile names
- `profile validate` output should distinguish errors (would block slicing) from warnings (suspicious but allowed)
- `profile delete` confirmation should show the full file path so user knows exactly what's being deleted

</specifics>

<deferred>
## Deferred Ideas

- **Profile create from scratch** -- guided/template-based creation for new printers not in library. Needs more UX design, separate phase.
- **Profile import/export** -- import external TOML/JSON into user profiles, export for sharing. Useful for profile distribution.
- **Profile versioning/changelog** -- track what changed since clone, diff against source. Phase 38's diff-profiles already handles comparison.
- **Profile groups/tags** -- organize user profiles by project or printer setup
- **profile list --custom filter** -- filter list-profiles to show only user-created custom profiles

</deferred>

---

*Phase: 42-clone-and-customize-profiles-from-defaults*
*Context gathered: 2026-03-20*
