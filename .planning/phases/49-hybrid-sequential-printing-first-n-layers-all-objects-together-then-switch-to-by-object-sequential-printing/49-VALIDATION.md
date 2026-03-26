---
phase: 49
slug: hybrid-sequential-printing
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-26
---

# Phase 49 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `#[test]` + cargo test |
| **Config file** | `Cargo.toml` workspace |
| **Quick run command** | `cargo test -p slicecore-engine --lib` |
| **Full suite command** | `cargo test -p slicecore-engine -p slicecore-arrange` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-engine --lib`
- **After every plan wave:** Run `cargo test -p slicecore-engine -p slicecore-arrange`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 49-01-01 | 01 | 1 | ADV-02 | unit | `cargo test -p slicecore-engine hybrid` | ❌ W0 | ⬜ pending |
| 49-01-02 | 01 | 1 | ADV-02 | unit | `cargo test -p slicecore-engine hybrid_config` | ❌ W0 | ⬜ pending |
| 49-02-01 | 02 | 1 | ADV-02 | unit | `cargo test -p slicecore-engine hybrid_slice` | ❌ W0 | ⬜ pending |
| 49-03-01 | 03 | 2 | ADV-02 | unit | `cargo test -p slicecore-engine object_markers` | ❌ W0 | ⬜ pending |
| 49-04-01 | 04 | 2 | ADV-02 | unit | `cargo test -p slicecore-engine hybrid_progress` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Test stubs for hybrid config validation
- [ ] Test stubs for two-phase slicing logic
- [ ] Test stubs for object marker generation
- [ ] Test stubs for per-object progress events
- [ ] Test fixtures: multi-object mesh for hybrid validation

*Existing test infrastructure (cargo test) covers framework needs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Dry-run CLI output format | ADV-02 | Visual output verification | Run `--hybrid-dry-run` on multi-object STL, verify readable output |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
