---
phase: 41-travel-move-optimization
verified: 2026-03-20T18:00:00Z
status: passed
score: 20/20 must-haves verified
re_verification: false
---

# Phase 41: Travel Move Optimization Verification Report

**Phase Goal:** Optimize toolpath ordering within layers using TSP heuristics (NN, greedy edge insertion, 2-opt) to reduce non-extrusion travel distance by 20-35% on multi-object plates, with criterion benchmarks and CI-enforcing integration tests.
**Verified:** 2026-03-20T18:00:00Z
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | TspNode models entry/exit points correctly for closed and open paths | VERIFIED | `pub struct TspNode { pub entry: Point2, pub exit: Point2, pub reversible: bool, ... }` in travel_optimizer.rs:27 |
| 2 | DistanceMatrix precomputes asymmetric exit-to-entry distances | VERIFIED | `struct DistanceMatrix` at travel_optimizer.rs:133, test_distance_matrix_asymmetric passes |
| 3 | Nearest-neighbor construction produces valid tour visiting all nodes | VERIFIED | `fn nearest_neighbor` at travel_optimizer.rs:183, test_nn_four_points passes |
| 4 | Greedy edge insertion produces valid tour with union-find cycle detection | VERIFIED | `fn greedy_edge_insertion` at travel_optimizer.rs:302, `struct UnionFind` at travel_optimizer.rs:245, test_greedy_four_points passes |
| 5 | 2-opt improvement reduces or maintains tour length | VERIFIED | `fn two_opt_improve` at travel_optimizer.rs:419, test_two_opt_improves passes |
| 6 | Auto pipeline picks shorter of NN and greedy initial tours before 2-opt | VERIFIED | `pub fn optimize_tour` at travel_optimizer.rs:557 dispatches Auto mode picking shorter of NN/greedy |
| 7 | TravelOptConfig serializes/deserializes with serde defaults | VERIFIED | config.rs:1745 with `#[serde(default)]`, travel_opt_config_toml_roundtrip test passes |
| 8 | TravelOptAlgorithm enum is non_exhaustive with 5 variants | VERIFIED | `#[non_exhaustive]` at config.rs:1702, 5 variants (Auto, NearestNeighbor, GreedyEdgeInsertion, NearestNeighborOnly, GreedyOnly) |
| 9 | Perimeters, gap fills, and infill within each layer are reordered by TSP optimizer | VERIFIED | toolpath.rs:285, 414, 529 — three separate `optimize_tour` calls for each feature group |
| 10 | Optimizer respects seam points for closed paths (entry=exit=seam) | VERIFIED | toolpath.rs:267 constructs TspNode with entry=seam_pt, exit=seam_pt, reversible=false |
| 11 | Feature group ordering preserved (perimeters then gap fill then infill) | VERIFIED | toolpath.rs structure preserves sequential feature group emission order |
| 12 | Travel stats populated in SliceResult | VERIFIED | engine.rs:1910-1920 constructs `Some(TravelOptStats {...})` when enabled and baseline > 0 |
| 13 | Optimizer disabled via config.travel_opt.enabled=false falls back to original ordering | VERIFIED | toolpath.rs:583 fallback `nearest_neighbor_order` when optimizer disabled |
| 14 | Per-layer optimization parallelizes via rayon when parallel feature is enabled | VERIFIED | engine.rs:1267-1290 — `maybe_par_iter!(layers).map(...).collect()` with optimize_tour inside |
| 15 | Travel stat accumulation is parallel-safe | VERIFIED | engine.rs:1319-1328 — sequential summation after parallel collect, no shared mutable state during parallel execution |
| 16 | --no-travel-opt CLI flag disables travel optimization | VERIFIED | main.rs:268 `no_travel_opt: bool`, main.rs:1194-1195 sets `print_config.travel_opt.enabled = false` |
| 17 | Criterion benchmarks run for NN, greedy, 2-opt, and Auto on synthetic multi-object plates | VERIFIED | travel_benchmark.rs:219 lines — 4 benchmark groups (4-obj, 9-obj, 25-obj, scattered), all 5 algorithm variants, benchmarks run successfully with `--test` flag |
| 18 | Integration tests assert >= 20% travel reduction on 4-object and 9-object grids | VERIFIED | travel_reduction.rs:118 `assert_reduction(&nodes, &config, 20.0)` (4-obj), line 149 (9-obj), line 177 (scattered) — all 6 tests pass |
| 19 | Benchmarks use black_box for inputs | VERIFIED | travel_benchmark.rs:6 `use criterion::{black_box, ...}`, line 122 `optimize_tour(black_box(nodes), black_box(start), black_box(&config))` |
| 20 | Benchmark harness entry exists in Cargo.toml | VERIFIED | Cargo.toml line 61 `name = "travel_benchmark"` with `harness = false` |

**Score:** 20/20 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/slicecore-engine/src/travel_optimizer.rs` | TSP algorithms: TspNode, DistanceMatrix, Tour, optimize_tour, NN, greedy, 2-opt | VERIFIED | 943 lines (min 300), all required symbols present, 10 unit tests pass |
| `crates/slicecore-engine/src/config.rs` | TravelOptConfig, TravelOptAlgorithm, PrintOrder enums | VERIFIED | All three types present with correct derives, defaults, serde attrs |
| `crates/slicecore-engine/src/lib.rs` | Module declaration and re-exports | VERIFIED | `pub mod travel_optimizer` at line 67; TravelOptConfig, TravelOptAlgorithm, PrintOrder, TravelOptStats, optimize_tour, Tour, TspNode all re-exported |
| `crates/slicecore-engine/src/toolpath.rs` | Modified assemble_layer_toolpath calling optimize_tour for each feature group | VERIFIED | Three optimize_tour call sites (perimeters, gap fills, infill), fallback nearest_neighbor_order preserved |
| `crates/slicecore-engine/src/engine.rs` | TravelOptStats in SliceResult, parallel-compatible stat accumulation | VERIFIED | travel_opt_stats field at line 136, Some(...) construction at line 1910, sequential accumulation after parallel collect |
| `crates/slicecore-engine/src/statistics.rs` | TravelOptStats struct with baseline/optimized/reduction fields | VERIFIED | struct at line 40 with all three f64 fields |
| `crates/slicecore-cli/src/main.rs` | --no-travel-opt flag on slice command | VERIFIED | `no_travel_opt: bool` at line 268, applied at line 1194 |
| `crates/slicecore-engine/benches/travel_benchmark.rs` | Criterion benchmarks for TSP algorithms | VERIFIED | 219 lines (min 60), 4 benchmark groups, black_box usage, criterion_group/criterion_main macros |
| `crates/slicecore-engine/tests/travel_reduction.rs` | Integration tests asserting >= 20% travel reduction | VERIFIED | 326 lines (min 40), 6 tests passing including 3 at >= 20% threshold |
| `crates/slicecore-engine/Cargo.toml` | [[bench]] entry for travel_benchmark | VERIFIED | `name = "travel_benchmark"` with `harness = false` |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| travel_optimizer.rs | config.rs | `use crate::config::TravelOpt` | WIRED | `use crate::config::TravelOptConfig` imported and used in `optimize_tour` signature |
| lib.rs | travel_optimizer.rs | `pub mod travel_optimizer` | WIRED | line 67 declares module, line 133 re-exports `optimize_tour, Tour, TspNode` |
| toolpath.rs | travel_optimizer.rs | `travel_optimizer::optimize_tour` | WIRED | `use crate::travel_optimizer::{optimize_tour, TspNode}` at line 22, called at lines 285, 414, 529 |
| engine.rs | statistics.rs | `TravelOptStats` | WIRED | `crate::statistics::TravelOptStats` constructed at engine.rs:1911, field at line 136 |
| engine.rs | parallel.rs | `maybe_par_iter!` | WIRED | `maybe_par_iter!(layers)` at engine.rs:1267 wrapping layer processing |
| travel_benchmark.rs | travel_optimizer.rs | `slicecore_engine::TspNode` | WIRED | `use slicecore_engine::{TspNode, optimize_tour, ...}` |
| travel_reduction.rs | travel_optimizer.rs | `slicecore_engine::optimize_tour` | WIRED | `use slicecore_engine::{TspNode, optimize_tour, ...}` |
| main.rs | config.rs | `config.travel_opt.enabled = false` | WIRED | main.rs:1195 `print_config.travel_opt.enabled = false` |

---

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| GCODE-05 | 41-01, 41-02, 41-03, 41-04 | "Speed planning (per-feature speed control)" per REQUIREMENTS.md; travel optimization per ROADMAP.md Phase 41 | SATISFIED (with note) | All phase 41 implementation is complete. The GCODE-05 label in REQUIREMENTS.md describes speed planning (completed in Phase 3), but ROADMAP.md Phase 41 explicitly assigns GCODE-05 to this phase. This is a planning doc label mismatch — the codebase implementation fully satisfies the phase goal regardless of the ID description discrepancy. |

**Note on GCODE-05 label:** REQUIREMENTS.md line 82 shows GCODE-05 as "Speed planning (per-feature speed control)" already marked complete in Phase 3. ROADMAP.md Phase 41 reuses this ID to track travel optimization. This is a documentation inconsistency in the planning system but does not indicate any implementation failure.

---

### Anti-Patterns Found

No blocking or warning-level anti-patterns found.

- No TODO/FIXME/HACK comments in any phase 41 files
- No stub implementations (return null, unimplemented!(), empty bodies)
- No orphaned code (all new modules declared and re-exported)
- `cargo clippy -p slicecore-engine -- -D warnings` exits 0

---

### Human Verification Required

None required. All goal-critical behaviors are verified programmatically:

- TSP algorithm correctness: unit tests in travel_optimizer.rs cover all edge cases
- Travel reduction target (20-35%): integration tests assert >= 20% on 4-obj and 9-obj grids, all pass
- Benchmark compilation and execution: `--test` mode confirms benchmarks run
- CLI flag functionality: help text confirmed, flag wired to config mutation

---

### Test Execution Summary

| Test Suite | Command | Result |
|------------|---------|--------|
| travel_optimizer unit tests | `cargo test -p slicecore-engine --lib` (travel_optimizer module) | 10/10 pass |
| Full lib test suite | `cargo test -p slicecore-engine --lib` | 797/797 pass (0 failed) |
| travel_reduction integration tests | `cargo test -p slicecore-engine --test travel_reduction` | 6/6 pass |
| benchmark smoke test | `cargo bench -p slicecore-engine --bench travel_benchmark -- --test` | All pass |
| clippy | `cargo clippy -p slicecore-engine -- -D warnings` | 0 warnings |
| CLI build | `cargo build -p slicecore-cli` | Success |

---

## Verification Summary

Phase 41 fully achieves its goal. The TSP travel optimization implementation is complete across all four plans:

- **Plan 01:** Core algorithms (NN, greedy edge insertion, 2-opt, Auto mode) implemented in travel_optimizer.rs with all types and config structures. Ten unit tests covering edge cases and algorithm correctness all pass.
- **Plan 02:** Optimizer wired into assemble_layer_toolpath for all three feature groups (perimeters, gap fills, infill). TravelOptStats tracked per-layer and accumulated in a parallel-safe manner after rayon collect. SliceResult carries optimization statistics.
- **Plan 03:** `--no-travel-opt` CLI flag correctly disables optimization and appears in CLI help.
- **Plan 04:** Criterion benchmarks cover 4 plate configurations across all 5 algorithm variants. Integration tests assert >= 20% reduction on multi-object plates (4-obj, 9-obj, scattered) — all pass.

The 20-35% travel reduction target is validated by CI-enforcing integration tests asserting >= 20% on deliberate worst-case initial orderings.

---

_Verified: 2026-03-20T18:00:00Z_
_Verifier: Claude Sonnet 4.6 (gsd-verifier)_
