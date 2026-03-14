# Phase 30: CLI Profile Composition and Slice Workflow - Context

**Gathered:** 2026-03-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Build the CLI workflow for composing multiple profile layers (machine + filament + process) into a final PrintConfig with provenance tracking, and enhance the `slice` command to support this real-world multi-profile workflow. Includes profile resolution from library and user directories, config validation, logging, progress bar, and migration of existing profile commands to the shared resolver.

**Not in scope:** Multi-extruder support, 3MF project output, job directory system, profile management CLI commands (create/edit/delete), custom metadata via --set, stdin pipe support for slice, batch slicing.

</domain>

<decisions>
## Implementation Decisions

### Profile Composition Model
- **5-layer merge with provenance tracking:** Defaults → Machine → Filament → Process → User overrides → --set CLI flags
- Merge order determines priority: later layers win on conflicts
- Machine + filament are **required** (safety-critical). Process defaults to built-in "Standard" quality (0.20mm, 20% infill, moderate speeds)
- `--unsafe-defaults` escape hatch allows slicing without profiles (dev/testing only, with warning)
- All profiles are partial — only set the fields they care about, rest falls through to defaults
- Profiles declare their type via `profile_type` field (machine/filament/process), validated against the flag used
- Single-level `inherits` field supported for user profiles extending library profiles (depth limit: Claude's discretion)

### Provenance Tracking
- Parallel provenance map: `HashMap<String, FieldSource>` alongside PrintConfig
- `FieldSource` records: source type (Machine/Filament/Process/User/CLI/Default), file path, what it overrode
- PrintConfig itself stays clean — no generics or wrappers per field
- Conflict detection: warn on stderr when same field set by multiple profile layers
- Profile checksums (SHA256) included in G-code header and saved configs for change detection

### Merge Implementation
- TOML partial deserialization + field-level tracking
- Deserialize each profile into toml::Value tree, deep-merge layers, record provenance at each step, then deserialize final table into PrintConfig
- `--set` values auto-coerced from string as TOML literals (float, int, bool, string)
- `--set` validates keys against known PrintConfig field paths — error on unknown keys with "did you mean?" suggestions
- Start/end G-code template variables (e.g., `{nozzle_temp}`) resolved during merge using final merged values

### CLI Flag Design
- **New flags:** `-m/--machine`, `-f/--filament`, `-p/--process` for typed profile selection
- **`--overrides`** file flag: TOML or JSON file as 5th merge layer (after process, before --set). Auto-detect format.
- **`--set key=value`** (repeatable): inline overrides, wins everything. Dotted paths for nested fields (e.g., `speeds.perimeter=40.0`)
- **`--config`** kept as replay/power-user path: loads a single complete PrintConfig (file path only). Mutually exclusive with -m/-f/-p
- **`--save-config [path]`**: writes merged config to TOML file with provenance as comments. Defaults to `model.config.toml` alongside G-code if no path given. Works with --dry-run.
- **`--show-config`**: prints final merged config with source annotations (human-readable + --json for programmatic)
- **`--dry-run`**: resolves profiles, merges config, shows warnings/provenance, skips actual slice
- **`--no-log`**: suppress log file creation
- **`--log-file <path>`**: custom log file path
- **`--force`**: override safety validation errors (dangerous config values)
- Name-or-path auto-detection: values containing `/` or ending in `.toml` treated as file paths; otherwise searched in profile library

### Profile Resolution
- **ProfileResolver** as shared module in slicecore-engine (reused by slice, list-profiles, search-profiles, show-profile)
- Search order: user profiles (`~/.slicecore/profiles/`) → library index (index.json) → library filesystem scan (fallback)
- Case-insensitive substring matching. Exact ID match prioritized over substring
- Flag constrains type: -m only searches machine profiles, -f only filament, -p only process
- Ambiguity: error with list of matching profiles and hint to use more specific name
- User profiles win over library profiles for same short name
- Library profiles accessed by full ID for disambiguation (e.g., `orcaslicer/BBL/filament/PLA_Basic`)
- Inheritance resolution: auto-resolve parent by ID through same resolution chain
- Library directory auto-detection order: $SLICECORE_PROFILES_DIR → --profiles-dir → ./profiles/ → <binary-dir>/profiles/ → ~/.slicecore/library/

### User Profile Storage
- User profiles stored in `~/.slicecore/profiles/` organized by type (machine/, filament/, process/)
- Library profiles stay immutable — users never edit them
- User profiles shadow library profiles (user dir searched first)

### Slice Workflow UX
- **Output:** G-code only (model.stl → model.gcode). Input auto-detected from STL, 3MF, OBJ
- **Embedded config in G-code:** Full merged config as comments in G-code header, including reproduce command (copy-paste CLI invocation), profile IDs, checksums, SliceCore version, timestamp
- **Log file:** Always created by default (model.log alongside model.gcode). Contains full stderr output with timestamps. --no-log to suppress, --log-file for custom path
- **Progress bar:** indicatif crate for interactive terminals, text fallback for non-TTY/pipes/log files. Auto-detect via terminal detection
- **Default output verbosity:** Progress summary to stderr — profile resolution, mesh info, repair status, slicing progress, summary stats (filament, time estimate, line count)
- **Statistics:** One-line summary in default output. Full detailed stats via --stats-format and --json
- **Config validation:** Warn on suspicious values (layer height > nozzle diameter, extreme speeds). Error on dangerous values (temp exceeds machine max, bed size mismatch). --force to override safety errors
- **Structured exit codes:** 0=success, 1=general error, 2=config/profile error, 3=mesh error, 4=safety validation error
- **Error reporting:** Categorized errors vs warnings. Fatal issues stop slicing (non-zero exit). Non-fatal issues are warnings, slice proceeds. All logged to .log file
- **Single model per invocation.** Batch via shell loops or xargs

### Existing Command Migration
- list-profiles, search-profiles, show-profile updated to use ProfileResolver
- These commands now discover and display user profiles alongside library profiles
- Source column added to output (user vs library)

### Profile Library Bootstrapping
- Ship minimal built-in profiles (Generic PLA, Generic PETG, common printers) for first-time users
- Full library via import-profiles from slicer resources
- Error with setup instructions when no profiles found and no built-ins match

### WASM Compatibility
- Config merge logic (TOML value merging, provenance tracking) is WASM-safe — no filesystem access needed
- ProfileResolver is CLI/server only (requires filesystem)
- WASM consumers pass pre-resolved config values to merge function

### Backwards Compatibility
- Existing --config flag stays, works as before (single complete PrintConfig)
- --config and -m/-f/-p are mutually exclusive (error if combined)
- Existing CLI tests using --config continue to work
- Plugin config (--plugin-dir) stays separate from profile merge system

### Testing
- **Unit tests:** Merge logic, provenance tracking, conflict detection, --set parsing, template variable resolution
- **Integration tests:** ProfileResolver name resolution, ambiguity handling, type-constrained search, inheritance resolution, missing profile errors
- **CLI E2E tests:** Slice with -m/-f/-p, --dry-run, --save-config, --config replay, --show-config, exit codes, log file creation

### Claude's Discretion
- Inheritance depth limit enforcement approach
- Exact built-in profile set (which printers/filaments to ship)
- indicatif progress bar styling and layout details
- Internal ProfileResolver data structures and caching strategy
- Performance optimization (defer unless profiling shows issues)
- --overrides file format auto-detection implementation
- Help text and documentation wording

</decisions>

<specifics>
## Specific Ideas

- "--config becomes the replay path" — first slice with -m/-f/-p, --save-config writes the merged result, later slices use --config to replay. Clean separation of compose vs reuse.
- G-code header should include a copy-paste reproduce command so users can re-slice without remembering flags
- Error messages should include "did you mean?" suggestions for typos in --set keys and profile names
- Profile resolution should hint when the wrong type is used (e.g., `-m PLA` → "did you mean --filament PLA?")
- Log file contains everything stderr shows, plus timestamps, for post-mortem debugging
- The merge system operates on toml::Value trees, not PrintConfig structs — keeps the merge generic and avoids a 150+ field parallel Option struct

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `PrintConfig::from_file()` (config.rs:1085): already auto-detects TOML vs JSON format — reuse for --overrides format detection
- `SettingOverrides::merge_into()` (config.rs:1316): existing partial merge pattern for 7 fields — the new TOML-level merge replaces this approach for profile composition
- `profile_convert::merge_import_results()`: merges multiple imported profile results — similar concept to profile composition
- `load_index()` and `ProfileIndexEntry`: existing profile index system for searching 21k+ profiles
- `search-profiles` command: existing substring search logic that can inform ProfileResolver
- `slicecore_gcode_io`: G-code writer infrastructure for embedding config comments

### Established Patterns
- clap for CLI parsing with derive macros and subcommands
- `process::exit(1)` for fatal errors in CLI command handlers
- stderr for progress/warnings, stdout for output data (JSON, G-code paths)
- TOML as native config format, JSON as imported format

### Integration Points
- `cmd_slice()` in main.rs: main integration point — needs profile resolution + merge before engine invocation
- `Engine::new(config)`: engine takes a single PrintConfig — merge happens before this call
- `cmd_list_profiles()`, `cmd_search_profiles()`, `cmd_show_profile()`: need migration to ProfileResolver
- G-code generator: needs template variable resolution and config embedding in output

</code_context>

<deferred>
## Deferred Ideas

- **Job output directory system** — per-slice working directory with G-code + log + config + thumbnails. Essential for SaaS (parallel request isolation). Deferred to SaaS/API phase.
- **Slice manifest file** — `.slicecore.toml` with `--manifest` flag for re-slicing. Natural extension of --save-config.
- **3MF project output** — 3MF with model + settings + G-code embedded. Bigger feature, own phase.
- **Multi-extruder CLI support** — multiple -f flags for dual/multi-extruder. Affects entire pipeline.
- **Profile management commands** — `slicecore create-profile`, edit, delete. Interactive profile creation.
- **Custom metadata via --set** — allowing arbitrary user metadata through --set. Revisit if use case emerges.
- **Stdin pipe support** — `slicecore slice -` reading model from stdin. Defer until requested.
- **Profile library distribution/versioning** — dedicated crate or downloadable package for profile library. Must be version-controlled with update detection. Needs to support all major slicer profiles and be maintained. Consider: dedicated crate, downloadable zip, versioned release artifacts.
- **Batch slicing** — multiple models in one invocation.

</deferred>

---

*Phase: 30-cli-profile-composition-and-slice-workflow*
*Context gathered: 2026-03-13*
