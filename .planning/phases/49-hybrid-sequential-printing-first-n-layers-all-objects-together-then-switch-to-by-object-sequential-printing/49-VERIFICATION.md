---
phase: 49-hybrid-sequential-printing
verified: 2026-03-26T02:30:00Z
status: passed
score: 16/16 must-haves verified
re_verification: false
---

# Phase 49: Hybrid Sequential Printing Verification Report

**Phase Goal:** Implement hybrid print mode where Phase 1 prints first N layers of all objects together for adhesion verification, then Phase 2 switches to sequential by-object printing for quality, with early failure detection and conditional object skipping.
**Verified:** 2026-03-26T02:30:00Z
**Status:** passed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Hybrid config fields exist on SequentialConfig and parse from TOML | VERIFIED | `config.rs:4012` — `pub hybrid_enabled: bool`, `transition_layers: u32`, `transition_height: f64`; TOML test `hybrid_toml_parsing` passes |
| 2 | HybridPlan struct captures transition point, object order, and safe Z | VERIFIED | `sequential.rs:54-66` — all five fields present: `shared_layer_count`, `transition_z`, `object_order`, `safe_z`, `objects` |
| 3 | plan_hybrid_print() computes correct transition layer and ordering | VERIFIED | `sequential.rs:278` — function exists; `plan_hybrid_print_three_objects` test verifies shortest-first ordering and correct transition Z |
| 4 | compute_transition_layer() handles layer-count and height-based thresholds | VERIFIED | `sequential.rs:244` — three-tier logic implemented; tests `compute_transition_layer_by_count`, `_by_height`, and `_fallback` all pass |
| 5 | ObjectProgress event variant exists for per-object progress | VERIFIED | `event.rs:145` — variant with all 6 required fields; emitted at `engine.rs:2119` during sequential phase |
| 6 | Single object with hybrid enabled degrades gracefully | VERIFIED | `engine.rs:1262-1268` — emits warning "Hybrid sequential enabled but only one object found. Falling through to normal slicing." |
| 7 | Hybrid mode slices shared layers as combined mesh and sequential layers per-component | VERIFIED | `engine.rs:2048-2139` — shared layers via `generate_full_gcode(&layer_toolpaths[..shared_count])`, per-object via sub-mesh extraction and independent `Engine::new` instances |
| 8 | G-code contains OBJECT_START/OBJECT_END comment markers around each object in sequential phase | VERIFIED | `gcode_gen.rs:656-668` — `emit_object_start` / `emit_object_end`; called at `engine.rs:2073` and `2134`; format `OBJECT_START id=N name="..."` confirmed by test |
| 9 | Transition includes retract and safe-Z travel | VERIFIED | `gcode_gen.rs:674-705` — `emit_hybrid_transition` emits retract extrude + RapidMove; called at `engine.rs:2058`; safe-Z travel between objects at `engine.rs:2138` |
| 10 | No gap or overlap at transition layer boundary | VERIFIED | `engine.rs:2049` — `shared_count = plan.shared_layer_count.min(layer_toolpaths.len())`; per-object sub-mesh uses independent slice so no double-printing |
| 11 | Per-object progress events emitted during sequential phase | VERIFIED | `engine.rs:2112-2128` — ObjectProgress emitted for each `obj_layer` in `0..obj_total_layers` |
| 12 | CLI has --hybrid-dry-run flag that shows hybrid plan without slicing | VERIFIED | `main.rs:331` — `hybrid_dry_run: bool` with `#[arg(long)]`; `cargo run -p slicecore-cli -- slice --help` shows flag |
| 13 | Dry-run output shows object order, transition point, and phase breakdown | VERIFIED | `main.rs:1686-1716` — prints "=== Hybrid Sequential Print Plan ===", "Transition: after layer N", "Phase 1 (Shared Layers):", "Phase 2 (Sequential):" |
| 14 | Profile import maps complete_objects/print_sequence to sequential.enabled | VERIFIED | `profile_import.rs:782-784` and `profile_import_ini.rs:302-303` — both map `complete_objects` and `print_sequence` to `sequential.enabled` |
| 15 | No invented hybrid field mappings in profile import | VERIFIED | grep for `hybrid_enabled`, `transition_layers`, `transition_height` in both import files returns only test assertions confirming they are NOT mapped |
| 16 | All tests pass (26 sequential, 38 gcode_gen, 90 profile_import) | VERIFIED | All test suites exit 0 with 0 failures |

**Score:** 16/16 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/slicecore-engine/src/config.rs` | hybrid_enabled, transition_layers, transition_height on SequentialConfig | VERIFIED | Lines 4005-4054; tier=3, depends_on="sequential.enabled", correct defaults (false/5/0.0) |
| `crates/slicecore-engine/src/sequential.rs` | HybridPlan, HybridObjectInfo, plan_hybrid_print(), compute_transition_layer() | VERIFIED | Lines 54-330; all structs and functions public; 10 hybrid tests passing |
| `crates/slicecore-engine/src/event.rs` | ObjectProgress variant on SliceEvent | VERIFIED | Lines 141-158; all 6 fields present |
| `crates/slicecore-engine/src/lib.rs` | Re-exports for HybridPlan, HybridObjectInfo, plan_hybrid_print, compute_transition_layer | VERIFIED | Lines 135-136; all four items re-exported |
| `crates/slicecore-engine/src/engine.rs` | Two-phase hybrid slicing in slice_to_writer_with_events | VERIFIED | Lines 1255-1362 (plan phase), 2044-2139 (gcode phase); plan_hybrid_print called, ObjectProgress emitted |
| `crates/slicecore-engine/src/gcode_gen.rs` | emit_object_start, emit_object_end, emit_hybrid_transition, emit_safe_z_travel | VERIFIED | Lines 656-715; 5 tests passing in gcode_gen::tests |
| `crates/slicecore-cli/src/main.rs` | --hybrid-dry-run flag and dry-run output | VERIFIED | Lines 328-1719; flag defined, wired through call chain, output displays all required sections |
| `crates/slicecore-engine/src/profile_import.rs` | complete_objects -> sequential.enabled mapping | VERIFIED | Lines 782-784, 1971-1975; 3 tests verifying mapping and absence of hybrid field mappings |
| `crates/slicecore-engine/src/profile_import_ini.rs` | complete_objects -> sequential.enabled mapping for INI | VERIFIED | Lines 302-303, 822-823 |

Note: Plan 03 artifact listed as `slice_workflow.rs`, but the implementation landed in `main.rs`. The goal is fully achieved — this is a location deviation from the plan spec, not a gap.

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `sequential.rs` | `config.rs` | `plan_hybrid_print` takes PrintConfig with hybrid fields | VERIFIED | `sequential.rs:290` calls `compute_transition_layer(&config.sequential, ...)` which reads `config.sequential.hybrid_enabled`, `transition_layers`, `transition_height` |
| `engine.rs` | `sequential.rs` | `plan_hybrid_print()` call | VERIFIED | `engine.rs:1338` — `crate::sequential::plan_hybrid_print(&object_bounds, &object_names, &self.config, &approx_heights)` |
| `engine.rs` | `gcode_gen.rs` | generate_full_gcode for shared + per-object generation | VERIFIED | `engine.rs:2052` — `generate_full_gcode(&layer_toolpaths[..shared_count], &self.config)`; gcode helpers called at 2058, 2073, 2134, 2138 |
| `engine.rs` | `event.rs` | ObjectProgress event emission | VERIFIED | `engine.rs:2119` — `SliceEvent::ObjectProgress { ... }` emitted inside sequential loop |
| `main.rs` | `sequential.rs` | plan_hybrid_print() for dry-run preview | VERIFIED | `main.rs:1672` — `slicecore_engine::sequential::plan_hybrid_print(...)` called in dry-run path |

---

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| ADV-02 | 49-01, 49-02, 49-03 | Sequential printing (object-by-object with collision detection) | SATISFIED | Full hybrid sequential implementation: config fields, planning structs, engine pipeline, G-code markers, CLI flag, profile import. All tests passing. REQUIREMENTS.md already marks ADV-02 as `[x]` complete. |

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| No anti-patterns found | — | — | — | — |

No TODO/FIXME/placeholder/stub patterns found in modified files. No empty handler implementations. Engine sub-mesh extraction and per-object slicing use real `Engine::new` instances, not placeholders.

---

### Human Verification Required

None — all automated checks passed. The hybrid mode is a slicing pipeline concern with no visual/UI components to verify.

Items that would require human verification in a production context:
- Visual inspection of output G-code with `--hybrid-dry-run` against a real multi-object STL/3MF
- Confirming no gap/overlap artifacts at the transition boundary in actual printed output

These are outside scope for this phase (foundation types + planning logic).

---

### Gaps Summary

No gaps. All must-haves from all three plans verified against the actual codebase.

**Plan 01 (Foundation Types):** All config fields, structs, functions, events, and 10 tests confirmed present and substantive.

**Plan 02 (Engine Pipeline):** Two-phase slicing confirmed in `engine.rs`; G-code marker helpers confirmed in `gcode_gen.rs`; 5 marker tests passing; per-object progress events wired.

**Plan 03 (CLI + Profile Import):** `--hybrid-dry-run` flag confirmed in CLI with full display output; `complete_objects` and `print_sequence` mappings confirmed in both JSON and INI importers; no hybrid field mappings present; 3 tests pass.

One structural deviation from plan specs: Plan 03 expected dry-run logic in `slice_workflow.rs`, but it was implemented in `main.rs`. This is not a gap — the observable behavior (flag exists, shows plan preview) is fully present and wired.

---

_Verified: 2026-03-26T02:30:00Z_
_Verifier: Claude (gsd-verifier)_
