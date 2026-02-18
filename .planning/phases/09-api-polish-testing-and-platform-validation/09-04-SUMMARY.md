---
phase: 09-api-polish-testing-and-platform-validation
plan: 04
subsystem: api
tags: [events, pub-sub, json, msgpack, serialization, cli, structured-output]

# Dependency graph
requires:
  - phase: 09-03
    provides: "Serde Serialize/Deserialize on SliceResult, PrintTimeEstimate, FilamentUsage"
provides:
  - "SliceEvent enum with tagged JSON serialization"
  - "EventBus pub/sub system with CallbackSubscriber and NdjsonSubscriber"
  - "Engine::slice_with_events for progress monitoring"
  - "SliceMetadata with JSON and MessagePack serialization"
  - "CLI --json and --msgpack structured output flags"
affects: [09-06, 09-07, 09-08]

# Tech tracking
tech-stack:
  added: [rmp-serde]
  patterns: [pub-sub event bus, structured output serialization, CLI format flags]

key-files:
  created:
    - crates/slicecore-engine/src/event.rs
    - crates/slicecore-engine/src/output.rs
    - crates/slicecore-cli/tests/cli_output.rs
  modified:
    - crates/slicecore-engine/src/lib.rs
    - crates/slicecore-engine/src/engine.rs
    - crates/slicecore-engine/Cargo.toml
    - crates/slicecore-cli/src/main.rs
    - crates/slicecore-cli/Cargo.toml
    - Cargo.toml

key-decisions:
  - "rmp-serde 1.x for MessagePack (workspace dependency)"
  - "EventBus uses Vec<Box<dyn EventSubscriber>> with Send+Sync bounds"
  - "slice_to_writer refactored to internal slice_to_writer_with_events with Option<&EventBus>"
  - "CLI structured output moves human summary to stderr"
  - "ConfigSummary captures 10 key PrintConfig fields for output reproducibility"
  - "SliceEvent uses serde tag='type' for clean JSON"

patterns-established:
  - "EventBus pattern: subscribe(Box<dyn EventSubscriber>) + emit(&SliceEvent)"
  - "Structured output pattern: to_json/to_msgpack functions on SliceMetadata"
  - "CLI flag pattern: --json/--msgpack with stderr fallback for human output"

# Metrics
duration: 8min
completed: 2026-02-18
---

# Phase 09 Plan 04: Event System & Structured Output Summary

**EventBus pub/sub with SliceEvent progress monitoring, JSON/MessagePack structured output, and CLI --json/--msgpack flags**

## Performance

- **Duration:** 8 min
- **Started:** 2026-02-18T00:25:43Z
- **Completed:** 2026-02-18T00:33:40Z
- **Tasks:** 2
- **Files modified:** 10

## Accomplishments
- SliceEvent enum with 6 variants (StageChanged, LayerComplete, Warning, Error, PerformanceMetric, Complete) and tagged JSON serialization
- EventBus with subscribe/emit pattern, CallbackSubscriber closure wrapper, NdjsonSubscriber for streaming JSON lines
- Engine::slice_with_events emitting stage transitions and per-layer progress events
- SliceMetadata struct with ConfigSummary providing JSON and MessagePack structured output
- CLI --json and --msgpack flags producing parseable structured output on stdout
- 14 unit tests + 2 doc tests + 3 CLI integration tests

## Task Commits

Each task was committed atomically:

1. **Task 1: Event system and structured output module** - `b7bac6a` (feat)
2. **Task 2: CLI --json and --msgpack flags** - `baf73d0` (feat)

## Files Created/Modified
- `crates/slicecore-engine/src/event.rs` - SliceEvent enum, EventSubscriber trait, EventBus, CallbackSubscriber, NdjsonSubscriber
- `crates/slicecore-engine/src/output.rs` - SliceMetadata, ConfigSummary, to_json, to_msgpack, from_msgpack
- `crates/slicecore-engine/src/lib.rs` - Module registration and re-exports for event and output
- `crates/slicecore-engine/src/engine.rs` - slice_with_events method, event emissions in pipeline
- `crates/slicecore-engine/Cargo.toml` - rmp-serde dependency
- `crates/slicecore-cli/src/main.rs` - --json and --msgpack flags on slice subcommand
- `crates/slicecore-cli/Cargo.toml` - tempfile, serde_json, rmp-serde dev-dependencies
- `crates/slicecore-cli/tests/cli_output.rs` - 3 CLI integration tests for structured output
- `Cargo.toml` - rmp-serde workspace dependency

## Decisions Made
- rmp-serde 1.x added as workspace dependency for MessagePack serialization
- EventBus uses Vec<Box<dyn EventSubscriber>> with Send+Sync bounds for thread safety
- slice_to_writer refactored to delegate to internal slice_to_writer_with_events(Option<&EventBus>) avoiding code duplication
- CLI moves human-readable summary to stderr when --json or --msgpack is active (clean stdout)
- ConfigSummary captures 10 key fields (layer_height, nozzle_diameter, infill_density, infill_pattern, wall_count, etc.)
- SliceEvent uses #[serde(tag = "type")] for clean JSON output format

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed PrintTimeEstimate test struct fields**
- **Found during:** Task 1 (output module tests)
- **Issue:** Test constructed PrintTimeEstimate with fields (retraction_time_seconds, layer_change_count, layer_change_time_seconds) that don't exist on the actual struct
- **Fix:** Removed nonexistent fields from test helper, matching actual struct definition (total_seconds, move_time_seconds, travel_time_seconds, retraction_count)
- **Files modified:** crates/slicecore-engine/src/output.rs
- **Verification:** cargo test passes
- **Committed in:** b7bac6a (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Trivial test data fix. No scope creep.

## Issues Encountered
None -- plan executed smoothly.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Event system ready for use in integration tests and monitoring
- Structured output enables external tool integration and CI artifact capture
- Ready for remaining phase 09 plans (benchmarks, documentation, release)

---
*Phase: 09-api-polish-testing-and-platform-validation*
*Completed: 2026-02-18*
