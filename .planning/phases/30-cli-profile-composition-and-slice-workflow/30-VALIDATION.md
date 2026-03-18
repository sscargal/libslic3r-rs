---
phase: 30
slug: cli-profile-composition-and-slice-workflow
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-14
---

# Phase 30 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test + cargo test |
| **Config file** | Cargo.toml [dev-dependencies] |
| **Quick run command** | `cargo test -p slicecore-engine --lib profile_compose` |
| **Full suite command** | `cargo test -p slicecore-engine -p slicecore-cli` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-engine --lib`
- **After every plan wave:** Run `cargo test -p slicecore-engine -p slicecore-cli`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 30-01-01 | 01 | 1 | N/A-01 | unit | `cargo test -p slicecore-engine profile_compose` | ❌ W0 | ⬜ pending |
| 30-01-02 | 01 | 1 | N/A-02 | unit | `cargo test -p slicecore-engine profile_compose::provenance` | ❌ W0 | ⬜ pending |
| 30-01-03 | 01 | 1 | N/A-03 | unit | `cargo test -p slicecore-engine profile_compose::set_parsing` | ❌ W0 | ⬜ pending |
| 30-01-04 | 01 | 1 | N/A-06 | unit | `cargo test -p slicecore-engine profile_compose::validation` | ❌ W0 | ⬜ pending |
| 30-02-01 | 02 | 1 | N/A-04 | integration | `cargo test -p slicecore-engine profile_resolve` | ❌ W0 | ⬜ pending |
| 30-02-02 | 02 | 1 | N/A-05 | integration | `cargo test -p slicecore-engine profile_resolve::type_constraint` | ❌ W0 | ⬜ pending |
| 30-03-01 | 03 | 2 | N/A-07 | integration | `cargo test -p slicecore-cli cli_slice_profiles` | ❌ W0 | ⬜ pending |
| 30-03-02 | 03 | 2 | N/A-08 | integration | `cargo test -p slicecore-cli cli_slice_dry_run` | ❌ W0 | ⬜ pending |
| 30-03-03 | 03 | 2 | N/A-09 | integration | `cargo test -p slicecore-cli cli_save_config` | ❌ W0 | ⬜ pending |
| 30-03-04 | 03 | 2 | N/A-10 | integration | `cargo test -p slicecore-cli cli_exit_codes` | ❌ W0 | ⬜ pending |
| 30-03-05 | 03 | 2 | N/A-11 | unit | `cargo test -p slicecore-cli cli_mutex` | ❌ W0 | ⬜ pending |
| 30-03-06 | 03 | 2 | N/A-12 | integration | `cargo test -p slicecore-cli cli_log_file` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-engine/src/profile_compose.rs` — new module with unit tests for merge, provenance, set-parsing, validation
- [ ] `crates/slicecore-engine/src/profile_resolve.rs` — new module with unit tests for resolution and type-constrained search
- [ ] `crates/slicecore-cli/tests/cli_slice_profiles.rs` — E2E tests for new slice workflow
- [ ] `crates/slicecore-cli/src/slice_workflow.rs` — orchestrator module
- [ ] `crates/slicecore-cli/src/progress.rs` — progress bar wrapper
- [ ] Dependencies: `cargo add -p slicecore-engine sha2@0.10 dirs@5 strsim@0.11` and `cargo add -p slicecore-cli indicatif@0.17`

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Progress bar visual rendering | N/A | Terminal-dependent visual output | Run `slicecore slice model.stl -m generic_printer -f generic_pla` in interactive terminal, verify progress bar animates |
| TTY vs non-TTY detection | N/A | Requires testing both terminal modes | Pipe output through `cat` to verify text fallback |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
