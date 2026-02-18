---
phase: 10-cli-feature-integration
plan: 02
subsystem: cli
tags: [clap, ai, plugin, ollama, serde_json]

# Dependency graph
requires:
  - phase: 10-01
    provides: "CLI feature flag wiring (slicecore-ai and slicecore-plugin dependencies)"
  - phase: 08-ai-integration
    provides: "AiConfig, ProfileSuggestion, Engine::suggest_profile()"
  - phase: 07-plugin-system
    provides: "PluginRegistry, discover_and_load, Engine::with_plugin_registry()"
provides:
  - "ai-suggest CLI subcommand with text and JSON output"
  - "--plugin-dir flag on slice subcommand"
  - "Plugin discovery and loading wired into slice pipeline"
  - "Friendly error messages for unreachable AI providers and missing plugin dirs"
affects: [10-03-integration-tests]

# Tech tracking
tech-stack:
  added: [serde_json (runtime dep for CLI)]
  patterns: [CLI-flag-overrides-config for plugin_dir, connection-error-friendly-messaging]

key-files:
  modified:
    - crates/slicecore-cli/src/main.rs
    - crates/slicecore-cli/Cargo.toml

key-decisions:
  - "serde_json promoted from dev-dependencies to runtime dependencies for --json output in ai-suggest"
  - "Plugin dir resolution: CLI --plugin-dir overrides config plugin_dir (consistent with existing CLI override patterns)"
  - "Connection error detection via string matching on error messages (Connection refused, ConnectError, error sending request)"

patterns-established:
  - "CLI override pattern: CLI flag > config file > default (applied to plugin_dir)"
  - "Friendly error messages: detect provider unreachable and suggest 'ollama serve' for default config"

# Metrics
duration: 2min
completed: 2026-02-18
---

# Phase 10 Plan 02: AI Suggest Subcommand and Plugin Loading Summary

**ai-suggest subcommand wired to Engine::suggest_profile() with text/JSON output, and plugin loading integrated into slice via --plugin-dir flag**

## Performance

- **Duration:** 2 min
- **Started:** 2026-02-18T17:50:27Z
- **Completed:** 2026-02-18T17:52:41Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- Added `ai-suggest` subcommand with `--ai-config` and `--format` flags for AI-powered print profile suggestions
- Added `--plugin-dir` flag to `slice` subcommand that overrides config `plugin_dir`
- Wired `PluginRegistry::discover_and_load()` into the slice pipeline with informative plugin loading output
- Added helpful error when plugin infill is requested without a configured plugin directory
- Connection errors to AI providers produce user-friendly messages suggesting `ollama serve`

## Task Commits

Each task was committed atomically:

1. **Task 1: Add AiSuggest subcommand and plugin loading in cmd_slice** - `c6fb3f7` (feat)

## Files Created/Modified
- `crates/slicecore-cli/src/main.rs` - Added AiSuggest subcommand, --plugin-dir flag, cmd_ai_suggest function, plugin loading in cmd_slice
- `crates/slicecore-cli/Cargo.toml` - Added serde_json to runtime dependencies

## Decisions Made
- Promoted serde_json from dev-dependencies to runtime dependencies (needed for `--json` output in ai-suggest)
- Plugin directory resolution follows CLI-flag-overrides-config pattern (consistent with existing patterns)
- Connection error detection uses string matching on error messages for broad provider compatibility

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- AI suggest subcommand and plugin loading are wired into CLI
- Ready for Plan 10-03 integration testing
- Phase 10 SC2 (ai-suggest exists) and SC3 (plugin infill via CLI) plumbing is in place

## Self-Check: PASSED

All files and commits verified:
- crates/slicecore-cli/src/main.rs: FOUND
- crates/slicecore-cli/Cargo.toml: FOUND
- 10-02-SUMMARY.md: FOUND
- Commit c6fb3f7: FOUND

---
*Phase: 10-cli-feature-integration*
*Completed: 2026-02-18*
