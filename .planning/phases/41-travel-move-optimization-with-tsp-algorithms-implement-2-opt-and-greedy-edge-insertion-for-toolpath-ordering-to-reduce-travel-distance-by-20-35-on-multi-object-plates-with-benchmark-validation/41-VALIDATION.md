---
phase: 41
slug: travel-move-optimization-with-tsp-algorithms
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 41 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test + criterion 0.5 |
| **Config file** | crates/slicecore-engine/Cargo.toml |
| **Quick run command** | `cargo test -p slicecore-engine --lib travel_optimizer` |
| **Full suite command** | `cargo test --workspace && cargo bench -p slicecore-engine` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-engine --lib travel_optimizer`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 41-01-01 | 01 | 1 | TSP node abstraction | unit | `cargo test -p slicecore-engine travel_optimizer::tests::nn_` | ❌ W0 | ⬜ pending |
| 41-01-02 | 01 | 1 | Greedy edge insertion | unit | `cargo test -p slicecore-engine travel_optimizer::tests::greedy_` | ❌ W0 | ⬜ pending |
| 41-01-03 | 01 | 1 | 2-opt improvement | unit | `cargo test -p slicecore-engine travel_optimizer::tests::two_opt_` | ❌ W0 | ⬜ pending |
| 41-01-04 | 01 | 1 | Auto pipeline | unit | `cargo test -p slicecore-engine travel_optimizer::tests::auto_` | ❌ W0 | ⬜ pending |
| 41-02-01 | 02 | 1 | Config round-trip | unit | `cargo test -p slicecore-engine config::tests::travel_opt_` | ❌ W0 | ⬜ pending |
| 41-03-01 | 03 | 2 | >= 20% reduction 4-object | integration | `cargo test -p slicecore-engine travel_reduction_` | ❌ W0 | ⬜ pending |
| 41-03-02 | 03 | 2 | >= 20% reduction 9-object | integration | `cargo test -p slicecore-engine travel_reduction_` | ❌ W0 | ⬜ pending |
| 41-03-03 | 03 | 2 | TravelOptStats populated | integration | `cargo test -p slicecore-engine travel_stats_` | ❌ W0 | ⬜ pending |
| 41-04-01 | 04 | 2 | --no-travel-opt flag | integration | `cargo test -p slicecore-cli no_travel_opt` | ❌ W0 | ⬜ pending |
| 41-05-01 | 05 | 3 | Criterion benchmarks | benchmark | `cargo bench -p slicecore-engine -- travel` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-engine/src/travel_optimizer.rs` — new module with unit test stubs
- [ ] `crates/slicecore-engine/benches/travel_benchmark.rs` — criterion benchmark harness
- [ ] `crates/slicecore-engine/tests/travel_reduction.rs` — integration tests for reduction assertions
- [ ] Bench harness entry in `crates/slicecore-engine/Cargo.toml` for `travel_benchmark`

*Existing test infrastructure (cargo test, criterion) covers framework needs.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
