---
phase: 27
slug: build-plate-auto-arrangement
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-11
---

# Phase 27 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in (#[test]) + cargo test |
| **Config file** | crates/slicecore-arrange/Cargo.toml |
| **Quick run command** | `cargo test -p slicecore-arrange` |
| **Full suite command** | `cargo test --all-features --workspace` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-arrange`
- **After every plan wave:** Run `cargo test --all-features --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 27-01-01 | 01 | 1 | ADV-02 | unit | `cargo test -p slicecore-arrange -- sequential` | ❌ W0 | ⬜ pending |
| 27-01-02 | 01 | 1 | N/A | unit | `cargo test -p slicecore-arrange -- placer` | ❌ W0 | ⬜ pending |
| 27-01-03 | 01 | 1 | N/A | unit | `cargo test -p slicecore-arrange -- footprint` | ❌ W0 | ⬜ pending |
| 27-01-04 | 01 | 1 | N/A | unit | `cargo test -p slicecore-arrange -- grouper` | ❌ W0 | ⬜ pending |
| 27-01-05 | 01 | 1 | N/A | unit | `cargo test -p slicecore-arrange -- orient` | ❌ W0 | ⬜ pending |
| 27-01-06 | 01 | 1 | N/A | unit | `cargo test -p slicecore-arrange -- bed` | ❌ W0 | ⬜ pending |
| 27-01-07 | 01 | 1 | N/A | unit | `cargo test -p slicecore-arrange -- result` | ❌ W0 | ⬜ pending |
| 27-01-08 | 01 | 1 | N/A | integration | `cargo test -p slicecore-cli -- arrange` | ❌ W0 | ⬜ pending |
| 27-01-09 | 01 | 1 | N/A | integration | `cargo test -p slicecore-arrange -- integration` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-arrange/` — entire crate does not exist yet
- [ ] `crates/slicecore-arrange/Cargo.toml` — workspace member setup
- [ ] All test files — created alongside implementation

*Note: Wave 0 creates the crate scaffold with test stubs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Visual arrangement quality | N/A | Subjective packing density assessment | Arrange 5+ parts, inspect JSON output positions for overlap/gaps |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
