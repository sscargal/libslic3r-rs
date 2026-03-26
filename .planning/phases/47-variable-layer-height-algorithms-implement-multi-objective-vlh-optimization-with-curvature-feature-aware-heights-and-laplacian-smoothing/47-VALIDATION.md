---
phase: 47
slug: variable-layer-height-algorithms-implement-multi-objective-vlh-optimization-with-curvature-feature-aware-heights-and-laplacian-smoothing
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-25
---

# Phase 47 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) + proptest |
| **Config file** | Cargo.toml [dev-dependencies] |
| **Quick run command** | `cargo test -p slicecore-slicer --lib vlh` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-slicer --lib vlh`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 47-01-01 | 01 | 1 | SLICE-05 | unit + integration | `cargo test -p slicecore-slicer vlh_deterministic` | ❌ W0 | ⬜ pending |
| 47-01-02 | 01 | 1 | SLICE-05 | unit | `cargo test -p slicecore-slicer greedy_deterministic` | ❌ W0 | ⬜ pending |
| 47-01-03 | 01 | 1 | SLICE-05 | unit | `cargo test -p slicecore-slicer dp_deterministic` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-slicer/src/vlh/mod.rs` — VLH module root with public API
- [ ] Test infrastructure for VLH: determinism tests, regression tests vs old adaptive.rs
- [ ] Golden file: sphere adaptive heights from old system (regression baseline)

*Existing test infrastructure (cargo test) covers framework needs — Wave 0 focuses on VLH-specific test scaffolding.*

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
