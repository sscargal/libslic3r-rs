---
phase: 22-migrate-from-lib3mf-to-lib3mf-core-ecosystem
plan: 01
subsystem: fileio
tags: [lib3mf-core, 3mf, wasm, pure-rust, zip, mesh-parsing]

# Dependency graph
requires:
  - phase: 02-file-io-and-gcode
    provides: "slicecore-fileio crate with lib3mf-based 3MF parser"
provides:
  - "lib3mf-core 0.2.0 replaces lib3mf 0.1.3 in slicecore-fileio"
  - "Unconditional 3MF parsing on all targets including WASM"
  - "Pure Rust 3MF dependency chain (no C/zstd-sys)"
affects: [wasm-compilation, ci-pipeline, future-3mf-features]

# Tech tracking
tech-stack:
  added: [lib3mf-core 0.2.0, glam 0.31 (dev)]
  patterns: [ZipArchiver+parse_model pipeline for 3MF reading, Geometry::Mesh enum-based object geometry access]

key-files:
  created: []
  modified:
    - crates/slicecore-fileio/Cargo.toml
    - crates/slicecore-fileio/src/threemf.rs
    - crates/slicecore-fileio/src/lib.rs

key-decisions:
  - "ZipArchiver+find_model_path+parse_model pipeline for in-memory 3MF parsing (no Model::from_reader convenience method in lib3mf-core)"
  - "Geometry::Mesh enum match instead of object.mesh Option check (lib3mf-core uses Geometry enum)"
  - "Object struct literal construction (lib3mf-core Object has no new() constructor, requires explicit field initialization)"
  - "Sorted index assertions in multi-object test (HashMap iteration order nondeterministic)"
  - "glam 0.31 added as dev-dependency for BuildItem transform field in tests (not re-exported by lib3mf-core)"

patterns-established:
  - "lib3mf-core archive pipeline: ZipArchiver::new(cursor) -> find_model_path -> read_entry -> parse_model"
  - "f32->f64 lossless vertex conversion for lib3mf-core Vertex to Point3"
  - "Object construction with Geometry::Mesh wrapper and explicit ResourceId"

requirements-completed: [MESH-02, FOUND-01]

# Metrics
duration: 4min
completed: 2026-02-25
---

# Phase 22 Plan 01: lib3mf to lib3mf-core Migration Summary

**Replaced lib3mf 0.1.3 with lib3mf-core 0.2.0 for pure Rust WASM-compatible 3MF parsing, removing all WASM cfg gates**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-25T21:45:27Z
- **Completed:** 2026-02-25T21:49:47Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Replaced lib3mf with lib3mf-core 0.2.0 (pure Rust, no C/zstd-sys dependency)
- Rewrote 3MF parser to use ZipArchiver + parse_model pipeline with f32->f64 vertex conversion
- Removed all WASM cfg gates -- 3MF now available on all targets unconditionally
- All 39 unit tests + 7 integration tests pass with zero clippy warnings
- Multi-object test adapted for HashMap iteration order independence

## Task Commits

Each task was committed atomically:

1. **Task 1: Swap lib3mf dependency to lib3mf-core and rewrite threemf.rs parser + tests** - `a0f9ee1` (feat)
2. **Task 2: Remove WASM cfg gates from lib.rs and update dispatch + tests** - `1f4ea65` (feat)

## Files Created/Modified
- `crates/slicecore-fileio/Cargo.toml` - Replaced lib3mf with unconditional lib3mf-core dependency, added glam dev-dep
- `crates/slicecore-fileio/src/threemf.rs` - Rewrote parse() and all tests for lib3mf-core API
- `crates/slicecore-fileio/src/lib.rs` - Removed WASM cfg gates, unified dispatch, rewrote 3MF test

## Decisions Made
- Used ZipArchiver+find_model_path+parse_model multi-step pipeline (Model::from_reader not available in lib3mf-core)
- lib3mf-core Object uses `geometry: Geometry::Mesh(mesh)` enum pattern instead of `mesh: Option<Mesh>` -- required match arm change in parser
- Object has no `new()` constructor -- used struct literal with all fields explicit
- BuildItem has no Default derive -- required explicit `transform: glam::Mat4::IDENTITY` and glam as dev-dependency
- Multi-object test uses sorted index comparison for HashMap iteration order independence (behavioral equivalence per user decision)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Object API different from research expectation**
- **Found during:** Task 1 (threemf.rs rewrite)
- **Issue:** Research expected `object.mesh: Option<Mesh>` but lib3mf-core uses `object.geometry: Geometry` enum. Also expected `add_object(ResourceId, Object)` but actual API is `add_object(Object)` where Object contains its own `id: ResourceId`.
- **Fix:** Used `Geometry::Mesh(mesh)` pattern matching in parser, struct literal construction with all required fields for Object
- **Files modified:** crates/slicecore-fileio/src/threemf.rs
- **Verification:** All 4 threemf tests pass
- **Committed in:** a0f9ee1 (Task 1 commit)

**2. [Rule 3 - Blocking] iter_objects() returns &Object not (&ResourceId, &Object)**
- **Found during:** Task 1 (threemf.rs rewrite)
- **Issue:** Research expected `iter_objects()` to yield `(&ResourceId, &Object)` tuples but it yields `&Object` values only
- **Fix:** Changed iteration from `for (_id, object) in ...` to `for object in ...`
- **Files modified:** crates/slicecore-fileio/src/threemf.rs
- **Verification:** All tests pass
- **Committed in:** a0f9ee1 (Task 1 commit)

**3. [Rule 3 - Blocking] glam not re-exported by lib3mf-core**
- **Found during:** Task 1 (threemf.rs tests)
- **Issue:** BuildItem requires `transform: glam::Mat4` field but glam is not re-exported by lib3mf-core and BuildItem doesn't derive Default
- **Fix:** Added `glam = "0.31"` as dev-dependency (matches lib3mf-core's glam version)
- **Files modified:** crates/slicecore-fileio/Cargo.toml
- **Verification:** Tests compile and pass
- **Committed in:** a0f9ee1 (Task 1 commit)

---

**Total deviations:** 3 auto-fixed (3 blocking - API differences from research)
**Impact on plan:** All deviations were API shape differences between research expectations and actual lib3mf-core 0.2.0 API. The research correctly identified MEDIUM confidence on Object/write APIs. All fixes were straightforward type-level changes with no architectural impact.

## Issues Encountered
None beyond the API deviations documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- lib3mf-core migration complete, 3MF parsing available on all targets
- Ready for WASM CI test addition (Plan 02) to prove WASM 3MF parsing works
- Future lib3mf-core features (validation, streaming) available for future phases

## Self-Check: PASSED

- [x] crates/slicecore-fileio/Cargo.toml exists
- [x] crates/slicecore-fileio/src/threemf.rs exists
- [x] crates/slicecore-fileio/src/lib.rs exists
- [x] Commit a0f9ee1 exists (Task 1)
- [x] Commit 1f4ea65 exists (Task 2)
- [x] All 39 unit tests + 7 integration tests pass
- [x] Zero clippy warnings
- [x] No old lib3mf references in source
- [x] No WASM cfg gates in slicecore-fileio

---
*Phase: 22-migrate-from-lib3mf-to-lib3mf-core-ecosystem*
*Completed: 2026-02-25*
