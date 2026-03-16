---
phase: 31
slug: cli-utility-commands-calibrate-and-estimate
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-03-16
---

# Phase 31 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml per crate |
| **Quick run command** | `cargo test -p slicecore-cli -p slicecore-engine --lib` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~60 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-cli -p slicecore-engine --lib`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 31-01-01 | 01 | 1 | — | integration | `cargo test -p slicecore-cli --test cli_calibrate` | ❌ W0 | ⬜ pending |
| 31-01-02 | 01 | 1 | — | unit | `cargo test -p slicecore-engine calibrate` | ❌ W0 | ⬜ pending |
| 31-01-03 | 01 | 1 | — | unit | `cargo test -p slicecore-engine cost_model` | ❌ W0 | ⬜ pending |
| 31-02-01 | 02 | 1 | — | integration | `cargo test -p slicecore-cli --test cli_calibrate temp_tower` | ❌ W0 | ⬜ pending |
| 31-02-02 | 02 | 1 | — | integration | `cargo test -p slicecore-cli --test cli_calibrate retraction` | ❌ W0 | ⬜ pending |
| 31-02-03 | 02 | 1 | — | integration | `cargo test -p slicecore-cli --test cli_calibrate flow` | ❌ W0 | ⬜ pending |
| 31-02-04 | 02 | 1 | — | integration | `cargo test -p slicecore-cli --test cli_calibrate first_layer` | ❌ W0 | ⬜ pending |
| 31-03-01 | 03 | 2 | — | unit | `cargo test -p slicecore-engine rough_estimate` | ❌ W0 | ⬜ pending |
| 31-03-02 | 03 | 2 | — | integration | `cargo test -p slicecore-cli --test cli_calibrate multi_config` | ❌ W0 | ⬜ pending |
| 31-03-03 | 03 | 2 | — | integration | `cargo test -p slicecore-cli --test cli_calibrate companion` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-cli/tests/cli_calibrate.rs` — integration tests for calibrate subcommands
- [ ] `crates/slicecore-engine/src/calibrate.rs` — core calibration module with unit tests
- [ ] `crates/slicecore-engine/src/cost_model.rs` — cost model module with unit tests

*Existing infrastructure (cargo test, clap, comfy-table) covers framework needs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Generated G-code prints correctly | — | Physical print required | Slice temp tower, print, verify temperature changes at correct heights |
| Calibration instructions are understandable | — | Human readability | Read companion .instructions.md file, verify clarity |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
