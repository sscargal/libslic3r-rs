---
phase: 48
slug: selective-adaptive-z-hop-control
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-25
---

# Phase 48 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p slicecore-engine --lib` |
| **Full suite command** | `cargo test --all-features --workspace` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-engine --lib`
- **After every plan wave:** Run `cargo test --all-features --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 48-01-01 | 01 | 1 | GCODE-03 | unit | `cargo test -p slicecore-engine zhop_config` | ❌ W0 | ⬜ pending |
| 48-01-02 | 01 | 1 | GCODE-03 | unit | `cargo test -p slicecore-engine z_hop_backward_compat` | ❌ W0 | ⬜ pending |
| 48-02-01 | 02 | 1 | GCODE-03 | unit | `cargo test -p slicecore-engine z_hop_surface_gate` | ❌ W0 | ⬜ pending |
| 48-02-02 | 02 | 1 | GCODE-03 | unit | `cargo test -p slicecore-engine z_hop_distance_gate` | ❌ W0 | ⬜ pending |
| 48-02-03 | 02 | 1 | GCODE-03 | unit | `cargo test -p slicecore-engine z_hop_z_range` | ❌ W0 | ⬜ pending |
| 48-02-04 | 02 | 1 | GCODE-03 | unit | `cargo test -p slicecore-engine z_hop_proportional` | ❌ W0 | ⬜ pending |
| 48-03-01 | 03 | 2 | GCODE-03 | unit | `cargo test -p slicecore-engine z_hop_slope` | ❌ W0 | ⬜ pending |
| 48-03-02 | 03 | 2 | GCODE-03 | unit | `cargo test -p slicecore-engine z_hop_spiral` | ❌ W0 | ⬜ pending |
| 48-03-03 | 03 | 2 | GCODE-03 | unit | `cargo test -p slicecore-engine z_hop_auto` | ❌ W0 | ⬜ pending |
| 48-04-01 | 04 | 2 | GCODE-03 | unit | `cargo test -p slicecore-engine profile_import` | ✅ existing | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-engine/src/config.rs` — ZHopConfig struct tests (deserialization, defaults, backward compat)
- [ ] `crates/slicecore-engine/src/planner.rs` — plan_z_hop() tests (surface gate, distance gate, Z-range, proportional)
- [ ] `crates/slicecore-engine/src/gcode_gen.rs` — z-hop motion type emission tests (Slope segments, Spiral segments, Auto resolution)
- [ ] Update existing `z_hop_during_retraction` test to work with new ZHopConfig

*Existing infrastructure covers framework needs — only test stubs required.*

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
