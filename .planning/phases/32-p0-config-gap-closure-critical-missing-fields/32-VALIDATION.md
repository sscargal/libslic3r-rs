---
phase: 32
slug: p0-config-gap-closure-critical-missing-fields
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-16
---

# Phase 32 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in (`#[cfg(test)]` + integration tests) |
| **Config file** | `Cargo.toml` workspace config |
| **Quick run command** | `cargo test -p slicecore-engine --lib -- config` |
| **Full suite command** | `cargo test -p slicecore-engine` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-engine --lib -- config`
- **After every plan wave:** Run `cargo test -p slicecore-engine`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 32-01-01 | 01 | 1 | New enums/types | unit | `cargo test -p slicecore-engine --lib -- surface_pattern bed_type internal_bridge` | ✅ | ⬜ pending |
| 32-01-02 | 01 | 1 | DimensionalCompensationConfig | unit | `cargo test -p slicecore-engine --lib -- dimensional_compensation` | ✅ | ⬜ pending |
| 32-02-01 | 02 | 1 | Config struct fields | unit | `cargo test -p slicecore-engine --lib -- config` | ✅ | ⬜ pending |
| 32-03-01 | 03 | 2 | JSON profile mapping | integration | `cargo test -p slicecore-engine -- profile_import` | ✅ | ⬜ pending |
| 32-03-02 | 03 | 2 | INI profile mapping | integration | `cargo test -p slicecore-engine -- profile_import_ini` | ✅ | ⬜ pending |
| 32-04-01 | 04 | 2 | Template vars + validation | integration | `cargo test -p slicecore-engine -- config_validate` | ✅ | ⬜ pending |
| 32-05-01 | 05 | 3 | Profile re-conversion | integration | `cargo test -p slicecore-engine -- integration_profile` | ✅ | ⬜ pending |
| 32-05-02 | 05 | 3 | Golden tests updated | integration | `cargo test -p slicecore-engine -- golden` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| TOML inline comments | Self-documenting configs | Visual inspection | Save a config, verify inline comments present for new fields |
| G-code comment output | New fields in G-code | Visual inspection | Slice a test model, verify new field comments appear in header |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
