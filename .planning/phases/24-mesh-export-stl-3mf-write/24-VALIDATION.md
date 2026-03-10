---
phase: 24
slug: mesh-export-stl-3mf-write
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-10
---

# Phase 24 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test (cargo test) |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `cargo test -p slicecore-fileio` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-fileio`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 24-01-01 | 01 | 1 | N/A-06 | build | `cargo check -p slicecore-fileio` | Existing | pending |
| 24-01-02 | 01 | 1 | N/A-01 | unit | `cargo test -p slicecore-fileio save_mesh_3mf` | W0 | pending |
| 24-01-03 | 01 | 1 | N/A-02 | unit | `cargo test -p slicecore-fileio save_mesh_stl` | W0 | pending |
| 24-01-04 | 01 | 1 | N/A-03 | unit | `cargo test -p slicecore-fileio save_mesh_obj` | W0 | pending |
| 24-01-05 | 01 | 1 | N/A-04 | integration | `cargo test -p slicecore-fileio round_trip` | W0 | pending |
| 24-01-06 | 01 | 1 | N/A-07 | unit | `cargo test -p slicecore-fileio format_from_ext` | W0 | pending |
| 24-02-01 | 02 | 2 | N/A-05 | integration | `cargo test -p slicecore-cli convert` | W0 | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-fileio/src/export.rs` — export module with conversion + format dispatch
- [ ] Round-trip tests (export then reimport) for STL, 3MF, OBJ
- [ ] Format-from-extension unit tests
- [ ] CLI convert subcommand integration test

*Existing infrastructure covers build verification (cargo check).*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
