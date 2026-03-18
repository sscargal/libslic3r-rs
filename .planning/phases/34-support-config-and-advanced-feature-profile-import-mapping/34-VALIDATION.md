---
phase: 34
slug: support-config-and-advanced-feature-profile-import-mapping
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-17
---

# Phase 34 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) |
| **Config file** | Cargo.toml (each crate) |
| **Quick run command** | `cargo test -p slicecore-engine --lib -- support::config` |
| **Full suite command** | `cargo test -p slicecore-engine` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-engine --lib`
- **After every plan wave:** Run `cargo test -p slicecore-engine`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 34-01-01 | 01 | 1 | SUPPORT-MAP | unit | `cargo test -p slicecore-engine -- profile_import::tests::support` | ❌ W0 | ⬜ pending |
| 34-02-01 | 02 | 1 | SCARF-MAP | unit | `cargo test -p slicecore-engine -- profile_import::tests::scarf` | ❌ W0 | ⬜ pending |
| 34-02-02 | 02 | 1 | MULTI-MAP | unit | `cargo test -p slicecore-engine -- profile_import::tests::multi_material` | ❌ W0 | ⬜ pending |
| 34-02-03 | 02 | 1 | GCODE-MAP | unit | `cargo test -p slicecore-engine -- profile_import::tests::custom_gcode` | ❌ W0 | ⬜ pending |
| 34-02-04 | 02 | 1 | POST-MAP | unit | `cargo test -p slicecore-engine -- profile_import::tests::post_process` | ❌ W0 | ⬜ pending |
| 34-03-01 | 03 | 2 | P2-FIELDS | unit | `cargo test -p slicecore-engine -- profile_import::tests::p2` | ❌ W0 | ⬜ pending |
| 34-04-01 | 04 | 2 | GCODE-TRANSLATE | unit | `cargo test -p slicecore-engine -- gcode_template` | ❌ W0 | ⬜ pending |
| 34-05-01 | 05 | 3 | PASSTHROUGH-THRESHOLD | integration | `cargo test -p slicecore-engine -- passthrough_threshold` | ❌ W0 | ⬜ pending |
| 34-05-02 | 05 | 3 | ROUND-TRIP | integration | `cargo test -p slicecore-engine -- round_trip` | ❌ W0 | ⬜ pending |
| 34-06-01 | 06 | 3 | RECONVERT | manual+script | `cargo run -- convert ...` | ❌ WN | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Support mapping test profiles (JSON + INI with all support fields populated)
- [ ] Scarf joint mapping test profiles
- [ ] Multi-material mapping test profiles
- [ ] G-code template translation tests
- [ ] Passthrough threshold integration test
- [ ] Round-trip tests for support-heavy and tree-support profiles

*Existing test infrastructure (cargo test) covers framework needs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Re-conversion of 21k profiles | RECONVERT | Requires profile corpus not in repo | Run `cargo run -- convert` against local profile directory, compare before/after |
| Coverage report accuracy | REPORT | Report formatting is visual | Inspect MAPPING_COVERAGE_REPORT.md for completeness |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
