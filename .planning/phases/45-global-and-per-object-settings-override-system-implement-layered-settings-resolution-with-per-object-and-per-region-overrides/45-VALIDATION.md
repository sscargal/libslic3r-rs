---
phase: 45
slug: global-and-per-object-settings-override-system
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-24
---

# Phase 45 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) + proptest 1.x + criterion 0.5 |
| **Config file** | Cargo.toml [dev-dependencies] in each crate |
| **Quick run command** | `cargo test -p slicecore-engine --lib` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-engine --lib && cargo test -p slicecore-cli --lib`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 45 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 45-01-01 | 01 | 1 | ADV-03-a | unit | `cargo test -p slicecore-engine cascade` | ❌ W0 | ⬜ pending |
| 45-01-02 | 01 | 1 | ADV-03-b | unit | `cargo test -p slicecore-engine z_schedule` | ❌ W0 | ⬜ pending |
| 45-01-03 | 01 | 1 | ADV-03-c | unit | `cargo test -p slicecore-engine plate_config` | ❌ W0 | ⬜ pending |
| 45-01-04 | 01 | 1 | ADV-03-d | unit | `cargo test -p slicecore-engine override_safety_complete` | ❌ W0 | ⬜ pending |
| 45-01-05 | 01 | 1 | ADV-03-e | unit | `cargo test -p slicecore-engine modifier` | Partial | ⬜ pending |
| 45-02-01 | 02 | 2 | ADV-03-f | integration | `cargo test -p slicecore-cli override_set` | ❌ W0 | ⬜ pending |
| 45-02-02 | 02 | 2 | ADV-03-g | integration | `cargo test -p slicecore-cli plate_cmd` | ❌ W0 | ⬜ pending |
| 45-02-03 | 02 | 2 | ADV-03-h | unit | `cargo test -p slicecore-engine engine_plate` | ❌ W0 | ⬜ pending |
| 45-02-04 | 02 | 2 | ADV-03-i | unit | `cargo test -p slicecore-engine provenance` | ❌ W0 | ⬜ pending |
| 45-02-05 | 02 | 2 | ADV-03-j | unit | `cargo test -p slicecore-engine cascade_proptest` | ❌ W0 | ⬜ pending |
| 45-02-06 | 02 | 2 | ADV-03-k | integration | `cargo test -p slicecore-cli per_object_stats` | ❌ W0 | ⬜ pending |
| 45-02-07 | 02 | 2 | ADV-03-l | integration | `cargo test -p slicecore-fileio threemf_overrides` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-engine/src/plate_config.rs` — PlateConfig struct + parsing tests
- [ ] `crates/slicecore-engine/src/cascade.rs` — 10-layer cascade resolution + tests
- [ ] `crates/slicecore-engine/src/z_schedule.rs` — Z-schedule computation + tests
- [ ] `tests/fixtures/plate-configs/` — Reusable plate config TOML fixtures
- [ ] `tests/fixtures/override-sets/` — Reusable override set TOML fixtures
- [ ] proptest dependency added to slicecore-engine Cargo.toml [dev-dependencies]

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| OVERRIDE_SAFETY_MAP.md review | ADV-03-d | Requires human domain judgment | Review all ~385 field classifications for correctness |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 45s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
