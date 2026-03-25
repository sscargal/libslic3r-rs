---
phase: 48-selective-adaptive-z-hop-control
verified: 2026-03-25T23:30:00Z
status: passed
score: 24/24 must-haves verified
gaps:
  - truth: "GCODE-03 claimed by all three plans but that requirement is RepRapFirmware dialect output (Phase 6, already complete) not z-hop control"
    status: resolved
    reason: "All three plans claim requirement GCODE-03 ('RepRapFirmware dialect G-code output') which REQUIREMENTS.md shows as Complete under Phase 6. The actual work done (surface-gated z-hop with distance/Z-range gating and multi-motion-type emission) corresponds to GCODE-07 ('Retraction configuration: distance, speed, z-hop, wipe') which is Pending. REQUIREMENTS.md has no Phase 48 row, and no requirement ID accurately captures this work."
    artifacts:
      - path: ".planning/phases/48-selective-adaptive-z-hop-control/48-01-PLAN.md"
        issue: "requirements: [GCODE-03] — wrong ID, GCODE-03 is RepRapFirmware dialect, already complete"
      - path: ".planning/phases/48-selective-adaptive-z-hop-control/48-02-PLAN.md"
        issue: "requirements: [GCODE-03] — same mismatch"
      - path: ".planning/phases/48-selective-adaptive-z-hop-control/48-03-PLAN.md"
        issue: "requirements: [GCODE-03] — same mismatch"
    missing:
      - "Update all three plan files to reference GCODE-07 instead of GCODE-03"
      - "Update REQUIREMENTS.md tracking table: add Phase 48 row for GCODE-07 (Partial — z-hop portion complete) or mark GCODE-07 Complete if the full retraction scope is satisfied"
---

# Phase 48: Selective Adaptive Z-Hop Control Verification Report

**Phase Goal:** Replace global z-hop with intelligent surface-type-based z-hop that activates only on top solids and ironing surfaces, with layer-position-based rules, height-proportional lift, and distance-gated activation to eliminate unnecessary stringing on interior layers.

**Verified:** 2026-03-25T23:30:00Z
**Status:** gaps_found (one requirements traceability gap; all functional goals achieved)
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | ZHopConfig struct exists with all 12 fields and derives SettingSchema | VERIFIED | `config.rs:909` — `pub struct ZHopConfig` with height, hop_type, height_mode, proportional_multiplier, min_height, max_height, surface_enforce, travel_angle, speed, min_travel, above, below |
| 2 | TopSolidInfill variant exists in FeatureType and is used for top solid surfaces | VERIFIED | `toolpath.rs:34` — `TopSolidInfill,`; `toolpath.rs:494` — `FeatureType::TopSolidInfill` in infill_feature selection with `infill.is_top` condition |
| 3 | All exhaustive match arms on FeatureType handle TopSolidInfill | VERIFIED | gcode_gen:304, statistics:179,202, flow_control:180, preview:128,196 — all 6 match sites updated |
| 4 | Old config format with retraction.z_hop deserializes into ZHopConfig.height | VERIFIED | `config.rs:911` — `#[serde(alias = "z_hop")]` on height field; test `test_zhop_alias_backward_compat` passes |
| 5 | ZHopType enum has Normal, Slope, Spiral, Auto variants | VERIFIED | `config.rs:876-885` |
| 6 | ZHopHeightMode enum has Fixed and Proportional variants | VERIFIED | `config.rs:889-894` |
| 7 | plan_z_hop() returns None when z-hop disabled (height=0.0) | VERIFIED | `planner.rs:80-82`; all gating tests pass |
| 8 | plan_z_hop() returns None when departure feature not TopSolidInfill/Ironing (with TopSolidAndIroning enforce) | VERIFIED | `planner.rs:89-98`; `test_z_hop_surface_gated_skips_non_top` passes |
| 9 | plan_z_hop() returns None when travel distance < min_travel | VERIFIED | `planner.rs:101-103`; covered by 21 planner tests |
| 10 | plan_z_hop() computes proportional height as multiplier * layer_height clamped to min/max | VERIFIED | `planner.rs:114-124` |
| 11 | Auto z-hop type resolves to Spiral on TopSolidInfill/Ironing, Normal elsewhere | VERIFIED | `planner.rs:127-138`; `test_z_hop_auto_resolves_spiral_on_ironing` and `test_z_hop_auto_resolves_normal_on_other_surface` pass |
| 12 | Slope z-hop emits 6 G0 segments with interpolated X/Y/Z | VERIFIED | `gcode_gen.rs:364-392` — `let segments = 6`; `test_z_hop_slope_emits_6_segments` passes |
| 13 | Spiral z-hop emits 6 G0 segments with circular X/Y and linear Z | VERIFIED | `gcode_gen.rs:396-417` — `let segments = 6`; `test_z_hop_spiral_emits_6_segments` passes |
| 14 | G-code generation uses plan_z_hop() instead of direct config.retraction.z_hop | VERIFIED | `gcode_gen.rs:163-173` — `plan_z_hop(departure, seg.length(), seg.z, toolpath.layer_height, &config.z_hop)`; no `retraction.z_hop` references anywhere |
| 15 | OrcaSlicer JSON z_hop maps to z_hop.height | VERIFIED | `profile_import.rs:553` — `"z_hop" => Some("z_hop.height")` |
| 16 | OrcaSlicer z_hop_types maps to z_hop.hop_type | VERIFIED | `profile_import.rs:554` — `"z_hop_types" => Some("z_hop.hop_type")` with enum parsing |
| 17 | OrcaSlicer retract_lift_enforce maps to z_hop.surface_enforce | VERIFIED | `profile_import.rs:555` — `"retract_lift_enforce" => Some("z_hop.surface_enforce")` |
| 18 | OrcaSlicer travel_slope maps to z_hop.travel_angle | VERIFIED | `profile_import.rs:556` — `"travel_slope" => Some("z_hop.travel_angle")` |
| 19 | OrcaSlicer retract_lift_above maps to z_hop.above | VERIFIED | `profile_import.rs:557` — `"retract_lift_above" => Some("z_hop.above")` |
| 20 | OrcaSlicer retract_lift_below maps to z_hop.below | VERIFIED | `profile_import.rs:558` — `"retract_lift_below" => Some("z_hop.below")` |
| 21 | PrusaSlicer INI retract_lift maps to z_hop.height | VERIFIED | `profile_import_ini.rs:344` — `"retract_lift" => Some("z_hop.height")`; `test_ini_retract_lift_sets_z_hop_height` passes |
| 22 | Existing profile import tests still pass | VERIFIED | 932 lib tests pass (0 failed) |
| 23 | is_top propagated from engine through LayerInfill to toolpath feature selection | VERIFIED | `engine.rs:518-525, 1682-1689, 2429-2436` — three construction sites set `is_top: is_top_for_infill && infill_is_solid` |
| 24 | GCODE-03 correctly claimed as the requirement satisfied by this phase | FAILED | GCODE-03 = "RepRapFirmware dialect G-code output", marked Complete in Phase 6. Phase 48 work covers z-hop subsection of GCODE-07 = "Retraction configuration (distance, speed, z-hop, wipe)", still Pending. |

**Score:** 23/24 truths verified (all functional goals achieved; one requirements traceability mismatch)

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/slicecore-engine/src/config.rs` | ZHopConfig, ZHopType, ZHopHeightMode, SurfaceEnforce | VERIFIED | All four types present; ZHopConfig has 12 fields with correct defaults; `pub z_hop: ZHopConfig` in PrintConfig at line 2564 |
| `crates/slicecore-engine/src/toolpath.rs` | TopSolidInfill variant in FeatureType | VERIFIED | Line 34; used in infill_feature assignment at line 494 via `infill.is_top` |
| `crates/slicecore-engine/src/infill/mod.rs` | is_top field on LayerInfill | VERIFIED | Line 53: `pub is_top: bool` |
| `crates/slicecore-engine/src/planner.rs` | ZHopDecision struct, plan_z_hop() function | VERIFIED | Lines 52-153; all 6 gating checks implemented; 21 dedicated tests |
| `crates/slicecore-engine/src/gcode_gen.rs` | Travel arm using plan_z_hop(), emit_z_hop_up, emit_slope_segments, emit_spiral_segments | VERIFIED | Lines 161-173, 324-417; last_extrusion_feature tracking at lines 110, 213 |
| `crates/slicecore-engine/src/profile_import.rs` | 6 OrcaSlicer z-hop field mappings | VERIFIED | Lines 553-558; enum parsing for ZHopType and SurfaceEnforce at lines 1176-1214 |
| `crates/slicecore-engine/src/profile_import_ini.rs` | retract_lift + retract_lift_above/below mappings | VERIFIED | Lines 344-346; apply handlers at lines 964-972 |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `config.rs` | `PrintConfig` | `#[setting(flatten)] pub z_hop: ZHopConfig` | WIRED | `config.rs:2564` — `pub z_hop: ZHopConfig` present in PrintConfig |
| `toolpath.rs` | `assemble_layer_toolpath` | `infill.is_top selects TopSolidInfill vs SolidInfill` | WIRED | `toolpath.rs:492-498` — `if infill.is_top { FeatureType::TopSolidInfill }` |
| `gcode_gen.rs` | `planner.rs` | calls `plan_z_hop()` with departure feature and travel distance | WIRED | `gcode_gen.rs:163-169` — direct call with all 5 required parameters |
| `planner.rs` | `config.rs` | reads `ZHopConfig` for decision parameters | WIRED | `planner.rs:15` — `use crate::config::{..., ZHopConfig, ...}`; `plan_z_hop` parameter `config: &ZHopConfig` |
| `profile_import.rs` | `config.rs` | maps upstream field names to ZHopConfig dotted paths | WIRED | Lines 553-558 for path strings; lines 1175-1214 for direct config.z_hop.* assignments; `use crate::config::{ZHopType, SurfaceEnforce}` present |

---

## Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| GCODE-03 (claimed) | 48-01, 48-02, 48-03 | "RepRapFirmware dialect G-code output" (already Complete, Phase 6) | MISMAPPED | REQUIREMENTS.md shows GCODE-03 as Complete in Phase 6. This phase does not implement RepRapFirmware dialect. The work implements z-hop (GCODE-07 scope). |
| GCODE-07 (actual) | Not claimed in any plan | "Retraction configuration (distance, speed, z-hop, wipe)" (Pending, Phase 3) | ORPHANED | Phase 48 fully implements the z-hop subsection of GCODE-07. The requirement is not claimed in any plan frontmatter. REQUIREMENTS.md tracking table has no Phase 48 row. |

**Orphaned requirement finding:** GCODE-07 is mapped to Phase 3 in the tracking table but no Phase 3 plan claims it, and Phase 48 now delivers the z-hop portion. The tracking table and requirements completion status need updating.

---

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `gcode_gen.rs` | 1532 | `unused import: SurfaceEnforce` in test module | Info | Compiler warning only; no functional impact. A `SurfaceEnforce` import is present in the test module but only `ZHopType` is used by name |

No blocker or warning-severity anti-patterns found. No TODO/FIXME/placeholder comments in z-hop code. No stub implementations. No orphaned artifacts.

---

## Human Verification Required

None — all behavioral correctness can be verified programmatically. The 37 z-hop unit tests and 932 total lib tests cover the full gating logic, motion type emission, and profile import mappings.

---

## Gaps Summary

All functional goals of Phase 48 are achieved. The 932-test suite passes with zero failures. The single gap is a requirements traceability issue:

**The plans reference GCODE-03** ("RepRapFirmware dialect G-code output") which was completed in Phase 6 and has nothing to do with z-hop. The actual work implements the z-hop subsection of **GCODE-07** ("Retraction configuration: distance, speed, z-hop, wipe"), which remains listed as Pending in REQUIREMENTS.md.

To close this gap:
1. Update the `requirements:` field in 48-01-PLAN.md, 48-02-PLAN.md, and 48-03-PLAN.md from `[GCODE-03]` to `[GCODE-07]`
2. Update REQUIREMENTS.md: mark GCODE-07 as Complete (or Partial if wipe/distance-only retraction is not yet done), and add a Phase 48 row to the tracking table

This is a documentation/traceability gap only — the code is correct and complete.

---

_Verified: 2026-03-25T23:30:00Z_
_Verifier: Claude (gsd-verifier)_
