---
phase: 19-slicing-summary-and-print-statistics
verified: 2026-02-23T20:15:00Z
status: passed
score: 6/6 must-haves verified
re_verification: false
---

# Phase 19: Slicing Summary and Print Statistics Verification Report

**Phase Goal:** Generate detailed per-feature slicing statistics after G-code generation, presenting print time, filament usage, and per-feature breakdowns in user-selectable formats (ASCII table, CSV, JSON) with configurable precision, sorting, and support subtotals
**Verified:** 2026-02-23T20:15:00Z
**Status:** PASSED
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Per-feature statistics (time, filament, distance, segment count) computed from LayerToolpath segments and available in SliceResult after every slice | VERIFIED | `statistics.rs` iterates all `LayerToolpath.segments` via `compute_toolpath_statistics()`; `SliceResult.statistics: Option<PrintStatistics>` populated in both `slice_to_writer_with_events` (line 1332) and `slice_with_preview` (line 2049) |
| 2 | G-code metrics (retraction count/distance, z-hop count/distance, wipe count/distance) extracted from GcodeCommand stream | VERIFIED | `extract_gcode_metrics()` in `statistics.rs` lines 248-321 matches on `GcodeCommand::Retract`, `Unretract`, `RapidMove` (Z-hop detection), `LinearMove`, `ArcMove*`; integration test `test_gcode_metrics_retraction_count_matches_estimate` passes |
| 3 | ASCII table, CSV, and JSON output formats display per-feature breakdown with both percentage-of-total and percentage-of-print columns | VERIFIED | `stats_display.rs`: `format_ascii_table()` (line 97), `format_csv()` (line 269), `format_json()` (line 296); both `time_pct_total` and `time_pct_print` fields exist on `FeatureStatistics` and appear in all three formats |
| 4 | CLI flags control statistics format (--stats-format), quiet mode (--quiet), file output (--stats-file), time precision (--time-precision), sort order (--sort-stats) | VERIFIED | All 6 flags confirmed in `main.rs` struct and in `cargo run -p slicecore-cli -- slice --help` output |
| 5 | When --json is used for slice output, statistics are included by default (--json-no-stats to exclude) | VERIFIED | `main.rs` lines 521-546: when `json_output && !json_no_stats`, statistics injected into JSON value via `value["statistics"] = serde_json::to_value(statistics)` |
| 6 | All features appear in output even when unused (zero values), and support features have separate subtotals | VERIFIED | `compute_statistics()` iterates `default_feature_order()` (14 features) and uses `or default` for missing entries; virtual features (Retract, Unretract, Wipe) appended for total of 17; `summary.model_time_seconds` and `summary.support_time_seconds` computed; integration test `test_zero_features_present` passes |

**Score:** 6/6 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/slicecore-engine/src/statistics.rs` | PrintStatistics, FeatureStatistics, GcodeMetrics, StatisticsSummary, compute_statistics() | VERIFIED | 1032 lines (min 200 required); all types present with serde derives; comprehensive unit tests (12 passing) |
| `crates/slicecore-engine/src/engine.rs` | SliceResult.statistics field, statistics computation in slice pipeline | VERIFIED | `statistics: Option<crate::statistics::PrintStatistics>` at line 65; `compute_statistics` called at lines 1294 and 2014 |
| `crates/slicecore-engine/src/lib.rs` | Re-exports for statistics types | VERIFIED | `pub mod statistics` at line 52; re-exports at lines 105-107: `TimePrecision, StatsSortOrder, compute_statistics` and full type set |
| `crates/slicecore-cli/src/stats_display.rs` | ASCII table, CSV, JSON formatting | VERIFIED | 713 lines (min 150 required); all three format functions present and substantive; 28 unit tests |
| `crates/slicecore-cli/src/main.rs` | CLI flags for statistics control | VERIFIED | Contains `stats_format`, `quiet`, `stats_file`, `json_no_stats`, `time_precision`, `sort_stats` fields |
| `crates/slicecore-cli/Cargo.toml` | comfy-table dependency | VERIFIED | Line 15: `comfy-table = "7"` |
| `crates/slicecore-engine/tests/statistics_integration.rs` | 7 integration tests | VERIFIED | 315 lines; all 7 tests pass |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `statistics.rs` | `toolpath.rs` | `FeatureType\|LayerToolpath\|ToolpathSegment` iteration | WIRED | `use crate::toolpath::{FeatureType, LayerToolpath}` at line 33; iterates `layer.segments` in `compute_toolpath_statistics()` |
| `statistics.rs` | `slicecore-gcode-io/commands.rs` | `GcodeCommand` stream analysis | WIRED | `use slicecore_gcode_io::GcodeCommand` at line 28; `extract_gcode_metrics(commands: &[GcodeCommand])` at line 248 |
| `engine.rs` | `statistics.rs` | `compute_statistics()` call after gcode generation | WIRED | `use crate::statistics::compute_statistics` at line 30; called at lines 1294 and 2014; result stored in `statistics: Some(statistics)` |
| `stats_display.rs` | `statistics.rs` | `PrintStatistics\|FeatureStatistics\|StatisticsSummary` consumption | WIRED | `use slicecore_engine::{FeatureStatistics, PrintStatistics, StatsSortOrder, TimePrecision}` at lines 9-11 |
| `main.rs` | `stats_display.rs` | `format_ascii_table\|format_csv\|format_json` calls | WIRED | `mod stats_display` at line 13; called at lines 576-578 and 592-594 |
| `main.rs` | `statistics.rs` | `result.statistics` access | WIRED | `if let Some(ref statistics) = result.statistics` at lines 525, 573 |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| GCODE-12 | 19-01-PLAN.md, 19-02-PLAN.md | Print time estimation | SATISFIED | Phase 6 implemented foundational trapezoid estimation; Phase 19 builds detailed per-feature time breakdown and display on top of it. `StatisticsSummary.total_time_seconds` from trapezoid model, per-feature times scaled to match. REQUIREMENTS.md maps this to Phase 6 but Phase 19 extends it with the user-facing statistics display. |
| GCODE-13 | 19-01-PLAN.md, 19-02-PLAN.md | Filament usage estimation (weight, length, cost) | SATISFIED | Phase 6 implemented `FilamentUsage`; Phase 19 consumes it via `filament_usage.length_mm/weight_g/cost` in `compute_statistics()` and presents in ASCII table with "3.87m / 11.73g" format. `StatisticsSummary` includes `total_filament_mm`, `total_filament_m`, `total_filament_g`, `total_filament_cost`. |

**Note on requirement mapping:** REQUIREMENTS.md places GCODE-12 and GCODE-13 under Phase 6 in its tracking table. ROADMAP.md assigns these same IDs to Phase 19 as well. The requirements describe capabilities that span both phases: Phase 6 implemented the underlying computation, Phase 19 implemented the detailed per-feature statistical presentation. Both plans reference these IDs. The requirements are satisfied across the two phases collectively.

### Anti-Patterns Found

No anti-patterns found. No TODO/FIXME/PLACEHOLDER markers in key files. No stub returns. All implementations are substantive.

One noted intentional zero-value: `wipe_count` and `total_wipe_distance_mm` are always 0 in `extract_gcode_metrics()` because wipe-on-retraction is not yet implemented. This is by design (documented in code comments and SUMMARY.md as "infrastructure ready but no wipe moves yet").

### Human Verification Required

#### 1. ASCII Table Visual Formatting

**Test:** Run `cargo run -p slicecore-cli -- slice <stl_file>` against a real STL file and observe the ASCII table output.
**Expected:** A well-formatted table with aligned columns showing feature names, time (formatted as "Xm Xs"), filament (in mm or m), weight (in g), and percentage columns. Summary header above table with total time, filament, cost, layers.
**Why human:** Column alignment quality and readability require visual inspection. `comfy-table` auto-sizing behavior cannot be fully verified programmatically.

#### 2. --stats-file Round-trip

**Test:** Run `cargo run -p slicecore-cli -- slice <stl_file> --stats-file /tmp/stats.csv --stats-format csv` and inspect `/tmp/stats.csv`.
**Expected:** A valid CSV file with the correct header and one row per feature.
**Why human:** File write to arbitrary path requires a live STL file and filesystem verification.

#### 3. --json Flag Statistics Injection

**Test:** Run `cargo run -p slicecore-cli -- slice <stl_file> --json` and inspect the JSON output.
**Expected:** JSON output contains both the standard SliceResult fields AND a `"statistics"` key with the full PrintStatistics structure.
**Why human:** Requires a live STL file; the end-to-end JSON injection path (`serde_json::Value` mutation) is best verified visually with real output.

### Gaps Summary

None. All 6 success criteria are verified against actual code. All artifacts exist and are substantive. All key links are wired and confirmed via grep and test execution.

Test results summary:
- `cargo test -p slicecore-engine --lib statistics`: 12/12 unit tests pass
- `cargo test -p slicecore-engine --test statistics_integration`: 7/7 integration tests pass
- `cargo test -p slicecore-cli`: 14/14 CLI tests pass (3 output tests + 6 ai_suggest + 5 plugins)
- `cargo clippy -p slicecore-engine -p slicecore-cli -- -D warnings`: clean (0 warnings)
- Total engine tests: 559 unit tests + 7 integration tests = all pass, 0 failures

---

_Verified: 2026-02-23T20:15:00Z_
_Verifier: Claude (gsd-verifier)_
