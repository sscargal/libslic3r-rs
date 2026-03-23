---
phase: 43
slug: enable-disable-printer-and-filament-profiles
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 43 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) |
| **Config file** | workspace Cargo.toml |
| **Quick run command** | `cargo test -p slicecore-engine --lib enabled_profiles` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~60 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-engine --lib enabled_profiles && cargo test -p slicecore-cli cli_profile_enable`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 43-01-01 | 01 | 1 | API-02 | unit | `cargo test -p slicecore-engine enabled_profiles` | ❌ W0 | ⬜ pending |
| 43-01-02 | 01 | 1 | API-02 | unit | `cargo test -p slicecore-engine enabled_profiles` | ❌ W0 | ⬜ pending |
| 43-01-03 | 01 | 1 | API-02 | unit | `cargo test -p slicecore-engine profile_resolve::tests` | ❌ W0 | ⬜ pending |
| 43-02-01 | 02 | 1 | API-02 | integration | `cargo test -p slicecore-cli cli_profile_enable` | ❌ W0 | ⬜ pending |
| 43-02-02 | 02 | 1 | API-02 | integration | `cargo test -p slicecore-cli cli_profile_enable` | ❌ W0 | ⬜ pending |
| 43-02-03 | 02 | 1 | API-02 | integration | `cargo test -p slicecore-cli cli_profile_enable` | ❌ W0 | ⬜ pending |
| 43-03-01 | 03 | 2 | API-02 | integration | `cargo test -p slicecore-cli cli_profile_enable` | ❌ W0 | ⬜ pending |
| 43-03-02 | 03 | 2 | API-02 | unit | `cargo test -p slicecore-engine enabled_profiles::compatibility` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-engine/src/enabled_profiles.rs` — new module with unit tests for load/save/enable/disable
- [ ] `crates/slicecore-cli/tests/cli_profile_enable.rs` — integration tests for enable/disable/setup/status commands
- [ ] Test fixture: sample `enabled-profiles.toml` files (empty, partial, full)
- [ ] Test fixture: sample machine profile with `[compatibility]` section

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Interactive wizard renders correctly in terminal | API-02 | Requires live terminal with TTY for dialoguer rendering | Run `slicecore profile setup` in a terminal, verify vendor → model → filament flow renders with proper multi-select |
| Wizard re-run shows current state pre-selected | API-02 | Requires interactive terminal session | Enable some profiles, run `profile setup` again, verify previously enabled items are pre-checked |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
