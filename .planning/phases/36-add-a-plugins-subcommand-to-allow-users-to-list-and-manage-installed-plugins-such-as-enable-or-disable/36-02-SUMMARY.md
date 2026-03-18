---
phase: 36-add-a-plugins-subcommand
plan: 02
subsystem: cli
tags: [clap, plugins, comfy-table, anyhow, cli-subcommand]

requires:
  - phase: 36-add-a-plugins-subcommand
    plan: 01
    provides: "PluginStatus, DiscoveredPlugin, discover_all_with_status, write_status"
provides:
  - "PluginsCommand enum with list/enable/disable/info/validate subcommands"
  - "run_plugins() CLI dispatcher"
  - "Global --plugin-dir flag on Cli struct"
  - "Plugins variant in Commands enum"
affects: [36-03, cli-tests, plugin-management]

tech-stack:
  added: [anyhow]
  patterns: [global-cli-flag, subcommand-dispatch-with-error-handling]

key-files:
  created:
    - crates/slicecore-cli/src/plugins_command.rs
  modified:
    - crates/slicecore-cli/src/main.rs
    - crates/slicecore-cli/Cargo.toml
    - Cargo.toml

key-decisions:
  - "Added anyhow to workspace for CLI error handling (plan specified anyhow::Error return type)"
  - "Plugin enable validation uses discover_plugins on parent dir to check manifest + version"
  - "Plugin disable requires manifest.is_some() before allowing disable (identity validation)"

patterns-established:
  - "Subcommand module pattern: PluginsCommand enum + run_plugins() dispatcher"
  - "Global flag extraction: cli.plugin_dir extracted before match cli.command"

requirements-completed: [PLG-CLI-LIST, PLG-CLI-ENABLE, PLG-CLI-DISABLE, PLG-CLI-INFO, PLG-CLI-VALIDATE, PLG-GLOBAL-PLUGINDIR]

duration: 6min
completed: 2026-03-18
---

# Phase 36 Plan 02: CLI Plugins Subcommand Summary

**Full plugins CLI with list/enable/disable/info/validate subcommands, table and JSON output, filtering, and global --plugin-dir flag**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-18T19:33:39Z
- **Completed:** 2026-03-18T19:39:33Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Created plugins_command.rs with all 5 subcommands (list, enable, disable, info, validate)
- List command supports table (comfy_table UTF8_FULL) and JSON output with category/status filtering
- Promoted --plugin-dir to global CLI flag available on all subcommands
- Enable validates manifest + version before enabling; disable validates manifest identity

## Task Commits

Each task was committed atomically:

1. **Task 1: Create plugins_command.rs with all subcommands** - `79f1f47` (feat)
2. **Task 2: Promote --plugin-dir to global flag and register Plugins subcommand** - `690d405` (feat)

## Files Created/Modified
- `crates/slicecore-cli/src/plugins_command.rs` - PluginsCommand enum and run_plugins() with list/enable/disable/info/validate handlers
- `crates/slicecore-cli/src/main.rs` - Global --plugin-dir, Plugins variant, dispatch, updated after_help
- `crates/slicecore-cli/Cargo.toml` - Added anyhow dependency
- `Cargo.toml` - Added anyhow to workspace dependencies

## Decisions Made
- Added anyhow (v1) to workspace dependencies since the plan specified `anyhow::Error` as the return type for `run_plugins` and its handlers, and the existing codebase used `Box<dyn Error>` for other commands
- Enable validation uses `discover_plugins()` on the parent directory to validate manifest parsing and version compatibility, rather than attempting a full library load (which would require actual .so/.wasm files)
- Disable validates plugin identity by checking `manifest.is_some()` per the user decision documented in the plan

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added anyhow dependency to workspace**
- **Found during:** Task 1
- **Issue:** Plan specified `anyhow::Error` return type but anyhow was not in workspace dependencies
- **Fix:** Added `anyhow = "1"` to workspace Cargo.toml and `anyhow = { workspace = true }` to CLI Cargo.toml
- **Files modified:** Cargo.toml, crates/slicecore-cli/Cargo.toml
- **Verification:** cargo build succeeds
- **Committed in:** 79f1f47

**2. [Rule 1 - Bug] Fixed clippy redundant closure warning**
- **Found during:** Task 2 (verification)
- **Issue:** `.map(|p| to_list_entry(p))` flagged as redundant closure by clippy
- **Fix:** Changed to `.map(to_list_entry)`
- **Files modified:** crates/slicecore-cli/src/plugins_command.rs
- **Verification:** `cargo clippy -p slicecore-cli` clean
- **Committed in:** 690d405

**3. [Rule 1 - Bug] Adapted enable/validate to work with discover_plugins API**
- **Found during:** Task 1
- **Issue:** Plan suggested `discover_and_load(&plugin.dir)` for validation, but `discover_and_load` scans subdirectories of the given path, so passing a single plugin dir would find nothing
- **Fix:** Used `discover_plugins(plugin_dir)` on the parent directory to validate the plugin can be discovered (manifest parsed + version validated)
- **Files modified:** crates/slicecore-cli/src/plugins_command.rs
- **Verification:** cargo build and cargo test pass
- **Committed in:** 79f1f47

---

**Total deviations:** 3 auto-fixed (1 blocking, 2 bugs)
**Impact on plan:** All auto-fixes necessary for correctness. No scope creep.

## Issues Encountered
None beyond the auto-fixed deviations above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Plugins CLI is complete and ready for integration testing in Plan 03
- All 5 subcommands functional: list, enable, disable, info, validate
- Global --plugin-dir flag works across all subcommands

---
*Phase: 36-add-a-plugins-subcommand*
*Completed: 2026-03-18*
