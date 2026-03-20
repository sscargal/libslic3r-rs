---
phase: 42
slug: clone-and-customize-profiles-from-defaults
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 42 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) |
| **Config file** | Cargo.toml [dev-dependencies] |
| **Quick run command** | `cargo test -p slicecore-cli --lib` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-cli --lib`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 42-01-01 | 01 | 1 | API-02 | unit | `cargo test -p slicecore-cli profile_command` | ❌ W0 | ⬜ pending |
| 42-01-02 | 01 | 1 | API-02 | unit | `cargo test -p slicecore-cli profile_command` | ❌ W0 | ⬜ pending |
| 42-01-03 | 01 | 1 | API-02 | unit | `cargo test -p slicecore-cli profile_command` | ❌ W0 | ⬜ pending |
| 42-01-04 | 01 | 1 | API-02 | unit | `cargo test -p slicecore-cli profile_command` | ❌ W0 | ⬜ pending |
| 42-01-05 | 01 | 1 | API-02 | integration | `cargo test -p slicecore-cli --test cli_profile` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-cli/src/profile_command.rs` — stubs with inline #[cfg(test)] tests for API-02
- [ ] `crates/slicecore-cli/tests/cli_profile.rs` — integration test stubs for profile subcommands
- [ ] Test fixtures: sample TOML profiles for clone/set/validate scenarios (use tempfile crate)

*Existing infrastructure covers test framework — no new framework install needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `profile edit` opens $EDITOR | API-02 | Requires interactive terminal and real editor process | 1. Set EDITOR=nano 2. Run `slicecore profile edit <name>` 3. Verify editor opens with TOML content 4. Save and close 5. Verify changes persisted |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
