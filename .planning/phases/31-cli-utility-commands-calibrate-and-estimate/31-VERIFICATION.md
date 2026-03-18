---
phase: 31-cli-utility-commands-calibrate-and-estimate
verified: 2026-03-16T18:00:00Z
status: passed
score: 13/13 must-haves verified
re_verification: false
---

# Phase 31: CLI Utility Commands — Calibrate and Estimate Verification Report

**Phase Goal:** CLI users can generate printer-specific calibration G-code (temperature tower, retraction test, flow rate, first layer) and get cost estimation breakdowns from G-code analysis, with multi-config comparison and volume-based rough estimation for model files

**Verified:** 2026-03-16T18:00:00Z
**Status:** passed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Calibrate subcommand group exists with temp-tower, retraction, flow, first-layer, list sub-subcommands | VERIFIED | `CalibrateCommand` enum in `calibrate/mod.rs` with all 5 variants; `Commands::Calibrate` wired in `main.rs` line 568, dispatched at line 818 |
| 2 | temp-tower command generates valid G-code with temperature changes at correct Z heights | VERIFIED | `cmd_temp_tower` in `calibrate/temp_tower.rs`: full pipeline (resolve config -> params -> mesh -> validate -> slice -> inject M104 via `inject_temp_changes_text` -> write gcode + instructions); test `test_calibrate_temp_tower_generates_gcode` passes |
| 3 | retraction command generates valid G-code using profile retraction settings, with Z-boundary comments documenting the retraction range | VERIFIED | `cmd_retraction` in `calibrate/retraction.rs`: slices with profile's retraction, injects `RETRACTION SECTION` comments; test `test_calibrate_retraction_generates_gcode` passes |
| 4 | flow command generates calibration G-code for extrusion multiplier tuning with M221 overrides | VERIFIED | `cmd_flow` in `calibrate/flow.rs`: calls `inject_flow_changes_text` which emits `M221 S{pct}` at Z boundaries; test `test_calibrate_flow_generates_gcode` passes |
| 5 | first-layer command generates single-layer calibration G-code covering the bed | VERIFIED | `cmd_first_layer` in `calibrate/first_layer.rs`: generates 0.3mm flat plate at 80% bed coverage, overrides config for solid infill; test `test_calibrate_first_layer_generates_gcode` passes |
| 6 | CostEstimate computes filament, electricity, depreciation, labor costs with progressive disclosure | VERIFIED | `compute_cost` in `cost_model.rs`: independently computes 4 components, produces `missing_hints` for absent inputs; 7 unit tests all pass |
| 7 | analyze-gcode shows cost breakdown when cost flags are provided | VERIFIED | `AnalyzeGcode` variant in `main.rs` has `filament_price`, `printer_watts`, `electricity_rate`, `printer_cost`, `expected_hours`, `labor_rate`, `setup_time` flags; wired to `compute_cost` at line 2512 |
| 8 | analyze-gcode with --model flag accepts STL/3MF for rough volume-based estimation | VERIFIED | `if model` branch at line 2481 loads mesh via `slicecore_fileio::load_mesh`, calls `volume_estimate`, builds `CostInputs`, renders output; test `test_analyze_gcode_model_rough_estimate` passes |
| 9 | Missing cost inputs show N/A with helpful hints | VERIFIED | `compute_cost` pushes to `missing_hints` vec; `display_cost_table` renders None cells as "N/A" with hint text |
| 10 | Multi-config comparison shows side-by-side table of time, filament, cost across multiple configs | VERIFIED | `--compare-filament` flag (line 448), multi-config comparison block at line 2624; `display_comparison_table` in `analysis_display.rs` line 575 |
| 11 | --dry-run on any calibrate command shows model dimensions and parameter range without slicing | VERIFIED | All 4 calibrate commands check `args.dry_run` early; call `display_dry_run` from `common.rs` then return; test `test_calibrate_temp_tower_dry_run` passes |
| 12 | All output formats (table, json, csv, markdown) work for cost data | VERIFIED | `display_cost_table`, `display_cost_json`, `display_cost_csv`, `display_cost_markdown` all present in `analysis_display.rs`; tests `test_analyze_gcode_cost_all_formats`, `test_analyze_gcode_cost_json`, `test_analyze_gcode_cost_csv`, `test_analyze_gcode_cost_markdown` all pass |
| 13 | Bed size validation rejects models that exceed printer bed dimensions | VERIFIED | `validate_bed_fit` in `calibrate.rs`: checks width/depth with 10mm margin, returns descriptive error; test `test_temp_tower_mesh_fails_small_bed` in `calibration_tests.rs` passes |

**Score:** 13/13 truths verified

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/slicecore-engine/src/calibrate.rs` | Core calibration types, mesh generation, bed validation, temp injection | VERIFIED | 972 lines; exports `TempTowerParams`, `RetractionParams`, `FlowParams`, `FirstLayerParams`, `validate_bed_fit`, `inject_temp_changes`, `generate_temp_tower_mesh`, `generate_retraction_mesh`, `generate_flow_mesh`, `generate_first_layer_mesh`, `inject_flow_changes_text`, `retraction_schedule`, `flow_schedule` |
| `crates/slicecore-engine/src/cost_model.rs` | CostInputs, CostEstimate, compute_cost(), volume_estimate() | VERIFIED | 387 lines; all 4 types exported, progressive disclosure implemented, `filament_mm_to_grams` imported and used for weight computation |
| `crates/slicecore-cli/src/calibrate/mod.rs` | CalibrateCommand enum with subcommand dispatch | VERIFIED | `pub enum CalibrateCommand` with 5 variants, `run_calibrate` dispatches to each submodule's command function |
| `crates/slicecore-cli/src/calibrate/common.rs` | Shared calibration CLI utilities | VERIFIED | `resolve_calibration_config`, `write_instructions`, `format_calibration_header`, `display_dry_run`, `save_calibration_model` all present and implemented |
| `crates/slicecore-cli/src/calibrate/temp_tower.rs` | Temperature tower command | VERIFIED | `pub fn cmd_temp_tower` with full pipeline including temp injection, instructions, summary output |
| `crates/slicecore-cli/src/calibrate/retraction.rs` | Retraction test command | VERIFIED | `pub fn cmd_retraction` with Z-boundary comment injection and manual workflow instructions |
| `crates/slicecore-cli/src/calibrate/flow.rs` | Flow rate calibration command | VERIFIED | `pub fn cmd_flow` with M221 injection and caliper measurement instructions |
| `crates/slicecore-cli/src/calibrate/first_layer.rs` | First layer calibration command | VERIFIED | `pub fn cmd_first_layer` with solid infill config override and Z-offset tuning instructions |
| `crates/slicecore-cli/src/analysis_display.rs` | Cost display formatting | VERIFIED | `display_cost_table`, `display_cost_json`, `display_cost_csv`, `display_cost_markdown`, `display_volume_estimate`, `display_comparison_table` all present |
| `crates/slicecore-cli/tests/cli_calibrate.rs` | Integration tests | VERIFIED | 790 lines, 19 tests, all passing |
| `crates/slicecore-engine/tests/calibration_tests.rs` | Engine-level calibration tests | VERIFIED | 346 lines, 14 tests, all passing |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `crates/slicecore-cli/src/main.rs` | `crates/slicecore-cli/src/calibrate/mod.rs` | `Commands::Calibrate` dispatching to `run_calibrate` | WIRED | Line 568: variant declared; line 818: dispatched |
| `crates/slicecore-engine/src/cost_model.rs` | `crates/slicecore-engine/src/statistics.rs` | `filament_mm_to_grams` for weight computation | WIRED | Line 31 of `cost_model.rs`: `use crate::statistics::filament_mm_to_grams;`; used in `volume_estimate` |
| `crates/slicecore-cli/src/main.rs` | `crates/slicecore-engine/src/cost_model.rs` | `compute_cost` called with CLI flag values | WIRED | Lines 2512, 2584, 2641, 2696 all call `cost_model::compute_cost` |
| `crates/slicecore-cli/src/analysis_display.rs` | `crates/slicecore-engine/src/cost_model.rs` | `display_cost_*` renders `CostEstimate` | WIRED | Line 15 of `analysis_display.rs`: `use slicecore_engine::cost_model::{CostEstimate, VolumeEstimate};` |
| `crates/slicecore-cli/src/calibrate/temp_tower.rs` | `crates/slicecore-engine/src/calibrate.rs` | Calls `generate_temp_tower_mesh` and temp schedule | WIRED | Imports `generate_temp_tower_mesh`, `temp_schedule`, `validate_bed_fit`, `TempTowerParams` |
| `crates/slicecore-cli/src/calibrate/temp_tower.rs` | `crates/slicecore-engine/src/engine.rs` | Slices generated mesh through Engine | WIRED | `Engine::new(config)` then `engine.slice(&mesh, None)` |
| `crates/slicecore-cli/src/calibrate/flow.rs` | `crates/slicecore-engine/src/calibrate.rs` | Uses `generate_flow_mesh` and `inject_flow_changes_text` | WIRED | Both imported and called in pipeline |
| `crates/slicecore-cli/src/calibrate/first_layer.rs` | `crates/slicecore-engine/src/calibrate.rs` | Uses `generate_first_layer_mesh` | WIRED | Imported and called with `(params, bed_x, bed_y)` |

---

## Requirements Coverage

No requirement IDs were specified for this phase. Phase is self-contained.

---

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/slicecore-cli/src/analysis_display.rs` | 1088 | `display_cost_json` is never called (compiler warning) | Info | Non-blocking; function exists and is correct, just unused by the current code path which uses JSON via serde_json directly in `main.rs` |

No blockers or stubs found. No placeholder implementations. All command pipelines are fully implemented.

---

## Human Verification Required

### 1. Temperature Tower G-code Quality

**Test:** Run `slicecore calibrate temp-tower --start-temp 190 --end-temp 220 --step 10 -o /tmp/tower.gcode` and inspect the output G-code in a slicer preview or viewer.
**Expected:** G-code contains `M104 S190`, `M104 S200`, `M104 S210`, `M104 S220` commands at appropriate Z heights; the mesh geometry looks like 4 stacked blocks.
**Why human:** Cannot verify G-code visual correctness or that the Z boundaries align with actual block boundaries without running the slicer and inspecting output programmatically against a real sliced geometry.

### 2. Cost Estimation Display Clarity

**Test:** Run `slicecore analyze-gcode test.gcode --filament-price 25 --printer-watts 200 --electricity-rate 0.12` and observe the table output.
**Expected:** Table shows 4 cost rows with currency-formatted values, a total row, and no "N/A" rows for the provided inputs.
**Why human:** Display formatting quality (column alignment, color, readability) cannot be verified programmatically.

### 3. Retraction Instructions Clarity

**Test:** Review the `retraction_test.instructions.md` file generated by `slicecore calibrate retraction`.
**Expected:** Instructions clearly explain the manual per-section reprint workflow, including what to look for (stringing vs. blobs), how to binary search, and how to apply the result.
**Why human:** Instruction quality and clarity is a UX concern that requires human judgment.

---

## Gaps Summary

No gaps found. All 13 observable truths are verified against the actual codebase.

- The calibrate subcommand group is fully wired and all 4 G-code generation commands are implemented with proper mesh generation, engine slicing, post-processing, and companion instructions.
- The cost model computes all 4 cost components with progressive disclosure, correctly handles missing inputs, and is fully integrated into `analyze-gcode`.
- Volume-based rough estimation (`--model` flag) loads mesh files, computes volume, and produces estimates with the disclaimer.
- Multi-config comparison (`--compare-filament`) is implemented with side-by-side display supporting table/json/csv/markdown.
- All 4 calibrate commands support `--dry-run` and `--save-model`.
- 33 integration tests (19 CLI + 14 engine) cover all major features and all pass.

---

_Verified: 2026-03-16T18:00:00Z_
_Verifier: Claude (gsd-verifier)_
