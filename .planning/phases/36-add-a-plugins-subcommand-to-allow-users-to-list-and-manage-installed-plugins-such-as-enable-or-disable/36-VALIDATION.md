---
phase: 36
slug: add-a-plugins-subcommand-to-allow-users-to-list-and-manage-installed-plugins-such-as-enable-or-disable
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-18
---

# Phase 36 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test + scripts/qa_tests (bash) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p slicecore-plugin --lib` |
| **Full suite command** | `cargo test --workspace && scripts/qa_tests --group plugin` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-plugin -p slicecore-cli --lib`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green + `scripts/qa_tests --group plugin,errors`
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 36-01-01 | 01 | 1 | N/A-01 | unit | `cargo test -p slicecore-plugin status` | ❌ W0 | ⬜ pending |
| 36-01-02 | 01 | 1 | N/A-02 | unit | `cargo test -p slicecore-plugin discovery` | ✅ Extend | ⬜ pending |
| 36-01-03 | 01 | 1 | N/A-03 | unit | `cargo test -p slicecore-plugin registry` | ✅ Extend | ⬜ pending |
| 36-02-01 | 02 | 1 | N/A-04 | integration | `scripts/qa_tests --group plugin` | ❌ W0 | ⬜ pending |
| 36-02-02 | 02 | 1 | N/A-05 | integration | `scripts/qa_tests --group plugin` | ❌ W0 | ⬜ pending |
| 36-02-03 | 02 | 1 | N/A-06 | integration | `scripts/qa_tests --group plugin` | ❌ W0 | ⬜ pending |
| 36-02-04 | 02 | 1 | N/A-07 | integration | `scripts/qa_tests --group plugin` | ❌ W0 | ⬜ pending |
| 36-02-05 | 02 | 1 | N/A-08 | integration | `scripts/qa_tests --group plugin` | ❌ W0 | ⬜ pending |
| 36-03-01 | 03 | 2 | N/A-09 | unit | `cargo test -p slicecore-plugin discovery` | ❌ W0 | ⬜ pending |
| 36-03-02 | 03 | 2 | N/A-10 | integration | `scripts/qa_tests --group plugin` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-plugin/src/status.rs` — new module with PluginStatus read/write + tests
- [ ] `crates/slicecore-cli/src/plugins_command.rs` — new CLI module
- [ ] Expand `scripts/qa_tests` with `group_plugin()` test cases using fixture plugin dirs

*Existing test infrastructure covers framework setup — no new framework install needed.*

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
