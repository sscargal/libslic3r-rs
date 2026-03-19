---
phase: 40
slug: adopt-indicatif-for-consistent-cli-progress-display
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
---

# Phase 40 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (standard) |
| **Config file** | workspace Cargo.toml |
| **Quick run command** | `cargo test -p slicecore-cli` |
| **Full suite command** | `cargo test --all-features --workspace` |
| **Estimated runtime** | ~60 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-cli`
- **After every plan wave:** Run `cargo test --all-features --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 40-01-01 | 01 | 1 | N/A-01 | unit | `cargo test -p slicecore-cli --lib -- cli_output::tests` | ❌ W0 | ⬜ pending |
| 40-01-02 | 01 | 1 | N/A-02 | unit | `cargo test -p slicecore-cli --lib -- cli_output::tests` | ❌ W0 | ⬜ pending |
| 40-01-03 | 01 | 1 | N/A-03 | unit | `cargo test -p slicecore-cli --lib -- cli_output::tests` | ❌ W0 | ⬜ pending |
| 40-01-04 | 01 | 1 | N/A-04 | unit | `cargo test -p slicecore-cli --lib -- cli_tests` | ❌ W0 | ⬜ pending |
| 40-01-05 | 01 | 1 | N/A-05 | integration | `cargo test -p slicecore-cli -- slice` | ✅ | ⬜ pending |
| 40-01-06 | 01 | 1 | N/A-06 | unit | `cargo test -p slicecore-cli --lib -- cli_output::tests` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-cli/src/cli_output.rs` — unit tests for CliOutput modes, quiet/json logic, color mode selection
- [ ] Verify existing slice integration tests pass after migration

*Existing test infrastructure covers workspace builds; Wave 0 adds CliOutput-specific unit tests.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Visual spinner animation renders correctly in TTY | N/A | Requires real terminal to verify animation | Run `slicecore slice model.stl` in terminal, observe braille spinner animates |
| Non-TTY plain text fallback is readable | N/A | Requires piped output context | Run `slicecore slice model.stl 2>&1 \| cat` and verify `[step]` format appears |
| `--color never` disables all ANSI codes | N/A | Requires visual inspection | Run `slicecore --color never slice model.stl 2>&1 \| cat -v` and verify no escape codes |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
