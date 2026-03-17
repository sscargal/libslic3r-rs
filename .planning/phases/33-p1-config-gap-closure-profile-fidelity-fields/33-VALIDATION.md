---
phase: 33
slug: p1-config-gap-closure-profile-fidelity-fields
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-17
---

# Phase 33 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p slicecore-engine --lib -- p1_` |
| **Full suite command** | `cargo test -p slicecore-engine` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-engine --lib -- p1_`
- **After every plan wave:** Run `cargo test -p slicecore-engine`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 33-01-01 | 01 | 1 | P1-01 | unit | `cargo test -p slicecore-engine -- p1_fuzzy_skin` | ❌ W0 | ⬜ pending |
| 33-01-02 | 01 | 1 | P1-02 | unit | `cargo test -p slicecore-engine -- p1_brim` | ❌ W0 | ⬜ pending |
| 33-01-03 | 01 | 1 | P1-03 | unit | `cargo test -p slicecore-engine -- p1_input_shaping` | ❌ W0 | ⬜ pending |
| 33-01-04 | 01 | 1 | P1-04 | unit | `cargo test -p slicecore-engine -- p1_tool_change` | ❌ W0 | ⬜ pending |
| 33-02-01 | 02 | 1 | P1-05 | unit | `cargo test -p slicecore-engine -- p1_accel` | ❌ W0 | ⬜ pending |
| 33-02-02 | 02 | 1 | P1-06 | unit | `cargo test -p slicecore-engine -- p1_cooling` | ❌ W0 | ⬜ pending |
| 33-02-03 | 02 | 1 | P1-07 | unit | `cargo test -p slicecore-engine -- p1_multi_material` | ❌ W0 | ⬜ pending |
| 33-03-01 | 03 | 2 | P1-08 | integration | `cargo test -p slicecore-engine -- p1_json_import` | ❌ W0 | ⬜ pending |
| 33-03-02 | 03 | 2 | P1-09 | integration | `cargo test -p slicecore-engine -- p1_ini_import` | ❌ W0 | ⬜ pending |
| 33-03-03 | 03 | 2 | P1-10 | unit | `cargo test -p slicecore-engine -- p1_template` | ❌ W0 | ⬜ pending |
| 33-03-04 | 03 | 2 | P1-11 | unit | `cargo test -p slicecore-engine -- p1_validation` | ❌ W0 | ⬜ pending |
| 33-04-01 | 04 | 3 | P1-12 | integration | `cargo test -p slicecore-engine -- profile_reconversion` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

None — existing test infrastructure covers all phase requirements. Tests will be added alongside implementation (same pattern as Phase 32).

---

## Manual-Only Verifications

All phase behaviors have automated verification.

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
