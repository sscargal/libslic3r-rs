---
phase: 29
slug: mesh-boolean-operations-csg
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-12
---

# Phase 29 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test + proptest + criterion |
| **Config file** | Cargo.toml [dev-dependencies] |
| **Quick run command** | `cargo test -p slicecore-mesh --lib csg` |
| **Full suite command** | `cargo test --all-features --workspace` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-mesh --lib`
- **After every plan wave:** Run `cargo test --all-features --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 29-01-01 | 01 | 1 | CSG-01 | integration | `cargo test -p slicecore-mesh csg_union` | ❌ W0 | ⬜ pending |
| 29-01-02 | 01 | 1 | CSG-02 | integration | `cargo test -p slicecore-mesh csg_difference` | ❌ W0 | ⬜ pending |
| 29-01-03 | 01 | 1 | CSG-03 | integration | `cargo test -p slicecore-mesh csg_intersection` | ❌ W0 | ⬜ pending |
| 29-01-04 | 01 | 1 | CSG-04 | integration | `cargo test -p slicecore-mesh csg_xor` | ❌ W0 | ⬜ pending |
| 29-01-05 | 01 | 1 | CSG-05 | integration | `cargo test -p slicecore-mesh csg_union_many` | ❌ W0 | ⬜ pending |
| 29-01-06 | 01 | 1 | CSG-06 | integration | `cargo test -p slicecore-mesh csg_split` | ❌ W0 | ⬜ pending |
| 29-01-07 | 01 | 1 | CSG-07 | integration | `cargo test -p slicecore-mesh csg_hollow` | ❌ W0 | ⬜ pending |
| 29-01-08 | 01 | 1 | CSG-08 | unit | `cargo test -p slicecore-mesh csg_primitives` | ❌ W0 | ⬜ pending |
| 29-01-09 | 01 | 1 | CSG-09 | unit | `cargo test -p slicecore-mesh csg_attributes` | ❌ W0 | ⬜ pending |
| 29-01-10 | 01 | 1 | CSG-10 | integration | `cargo test -p slicecore-mesh csg_coplanar` | ❌ W0 | ⬜ pending |
| 29-01-11 | 01 | 1 | CSG-11 | integration | `cargo test -p slicecore-cli csg_cli` | ❌ W0 | ⬜ pending |
| 29-01-12 | 01 | 1 | CSG-12 | unit | `cargo test -p slicecore-mesh csg_report_serde` | ❌ W0 | ⬜ pending |
| 29-01-13 | 01 | 1 | CSG-13 | integration | `cargo test -p slicecore-mesh csg_cancellation` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-mesh/src/csg/mod.rs` — CSG module root
- [ ] `crates/slicecore-mesh/src/csg/error.rs` — CsgError type
- [ ] `crates/slicecore-mesh/src/csg/report.rs` — CsgReport type
- [ ] `crates/slicecore-mesh/benches/csg_bench.rs` — Criterion benchmarks
- [ ] `fuzz/fuzz_targets/fuzz_csg.rs` — Fuzz target
- [ ] Add `robust = "1.2"` to slicecore-mesh Cargo.toml

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Visual mesh quality after CSG | CSG-01..04 | Geometric artifacts need visual inspection | Export result meshes, open in MeshLab/3D viewer, check for holes/artifacts |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
