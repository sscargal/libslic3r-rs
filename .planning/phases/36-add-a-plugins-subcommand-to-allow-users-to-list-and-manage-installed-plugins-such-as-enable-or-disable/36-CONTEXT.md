# Phase 36: Plugins Subcommand - Context

**Gathered:** 2026-03-18
**Status:** Ready for planning

<domain>
## Phase Boundary

Add a `slicecore plugins` CLI subcommand for listing, inspecting, enabling, disabling, and validating installed plugins. Update the plugin discovery pipeline to respect per-plugin state files. This phase does NOT include install/uninstall/upgrade or marketplace features — those are reserved for future phases.

</domain>

<decisions>
## Implementation Decisions

### Subcommand structure
- Nested subcommands under `slicecore plugins`: `list`, `enable <name>`, `disable <name>`, `info <name>`, `validate <name>`
- Consistent with existing CLI patterns (`slicecore csg`, `slicecore calibrate`)
- Reserve namespace for future `install`, `uninstall`, `upgrade`, `search` commands (don't implement, just ensure clap structure accommodates them)

### Output format
- `plugins list` defaults to human-readable table with columns: Name, Version, Type, Category, Status
- `--json` flag outputs flat array of plugin objects (consistent with `schema --format json`)
- `--category infill|postprocessor` filter by plugin category
- `--status enabled|disabled|error` filter by status
- `plugins info <name>` shows full manifest fields, status, filesystem path, capabilities, API version

### Plugin discovery
- Config-first with CLI override: read `plugin_dir` from config.toml by default, `--plugin-dir` flag overrides
- Error with clear message if neither is set
- `--plugin-dir` promoted to global CLI flag (shared by `slice`, `plugins`, and future commands)

### Enable/disable mechanism
- Per-plugin `.status` file inside each plugin's directory: `{"enabled": true}` or `{"enabled": false}`
- `.status` file always present — auto-created with `{"enabled": true}` during discovery if missing
- `plugin.toml` remains the single source of truth for plugin metadata (name, version, description, author)
- `.status` is host-side state only — starts with `enabled` field, can grow later (last_used, installed_date)
- Install = drop directory (auto-enabled), Uninstall = delete directory (state goes with it)

### Validation behavior
- `plugins enable` validates by attempting to load the plugin; if load fails, remains disabled with error message
- `plugins disable` also validates (confirms plugin identity before disabling)
- `plugins validate <name>` is a standalone health check — load test, API version check, sandbox test
- When a disabled plugin's infill pattern is referenced in a slice config: hard error with actionable message ("Plugin 'X' is disabled. Enable with `slicecore plugins enable X`")

### Broken plugins in list
- Broken/invalid plugins shown in `plugins list` with status "Error: <brief reason>" and ??? for unknown fields
- `plugins info <name>` on a broken plugin shows whatever could be parsed + the error

### Slicing pipeline integration
- Update `discover_and_load()` to read `.status` files and skip disabled plugins
- This makes enable/disable actually work end-to-end during slicing

### QA test coverage
- Add `plugins` group to `scripts/qa_tests` exercising list, enable, disable, info, validate commands
- Include error cases (nonexistent plugin, no plugin dir configured)

### Claude's Discretion
- Exact table formatting and column widths
- `.status` file JSON structure beyond `enabled` field
- Error message wording details
- clap argument parsing implementation details

</decisions>

<canonical_refs>
## Canonical References

No external specs — requirements fully captured in decisions above.

### Existing plugin system
- `crates/slicecore-plugin/src/lib.rs` — Plugin system architecture overview, feature flags, module listing
- `crates/slicecore-plugin/src/registry.rs` — PluginRegistry with discover_and_load, list_infill_plugins, get_infill_plugin, PluginInfo struct
- `crates/slicecore-plugin/src/discovery.rs` — Plugin directory scanning and plugin.toml manifest parsing
- `crates/slicecore-plugin/src/postprocess.rs` — PostProcessorPluginAdapter (second plugin category)
- `crates/slicecore-plugin-api/src/lib.rs` — Shared FFI-safe types and traits (PluginManifest, InfillRequest, InfillResult)

### CLI integration points
- `crates/slicecore-cli/src/main.rs` — Current plugin handling (--plugin-dir on slice command, PluginRegistry usage)
- `scripts/qa_tests` — QA test script to add plugins test group

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `PluginRegistry` — already has `discover_and_load()`, `list_infill_plugins()`, `get_infill_plugin()` — extend with status awareness
- `PluginInfo` struct — has name, description, version, plugin_kind — add enabled/status field
- `PostProcessorPluginAdapter` — second plugin type to include in list/management
- `discovery` module — plugin directory scanning logic to extend with .status file handling

### Established Patterns
- Nested subcommands via clap (see `csg`, `calibrate` modules) — follow same pattern
- `--json` flag pattern used across CLI (analyze-gcode, csg info, schema, arrange)
- Table output format used by calibrate list, list-profiles
- Config-first with CLI override pattern used by slice command's --plugin-dir

### Integration Points
- `main.rs` slice command — currently has `--plugin-dir` flag, needs to move to global
- `PluginRegistry::discover_and_load()` — needs .status file reading and filtering
- `scripts/qa_tests` — add plugins group, update CRATE_MAP if needed, update fixture needs

</code_context>

<specifics>
## Specific Ideas

- User explicitly wants `.status` file to always exist (both enabled and disabled states) for consistency
- Per-plugin state chosen over global state to avoid sync issues with install/uninstall lifecycle
- Validation on both enable AND disable operations (not just enable)
- Future marketplace commands (install/uninstall/upgrade/search) should slot naturally into the subcommand structure

</specifics>

<deferred>
## Deferred Ideas

- Plugin marketplace / registry — install, uninstall, upgrade, search commands (future phase)
- Plugin signing and verification — security for marketplace plugins
- Plugin dependency resolution — plugin A requires plugin B
- Per-plugin configuration — settings beyond enable/disable
- Plugin update checking — compare installed version to latest available
- Plugin sandboxing controls per-plugin — memory limits, CPU fuel overrides
- Auto-update notifications

</deferred>

---

*Phase: 36-add-a-plugins-subcommand*
*Context gathered: 2026-03-18*
