---
phase: 44
slug: search-and-filter-profiles-by-printer-and-filament-compatibility
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-23
---

# Phase 44 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) |
| **Config file** | Cargo.toml workspace `[workspace.lints]` |
| **Quick run command** | `cargo test -p slicecore-engine --lib enabled_profiles` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-engine --lib enabled_profiles && cargo test -p slicecore-cli --lib profile_command`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 44-01-01 | 01 | 1 | API-02a | unit | `cargo test -p slicecore-engine --lib enabled_profiles::tests::nozzle` | ❌ W0 | ⬜ pending |
| 44-01-02 | 01 | 1 | API-02b | unit | `cargo test -p slicecore-engine --lib enabled_profiles::tests::temperature` | ❌ W0 | ⬜ pending |
| 44-01-03 | 01 | 1 | API-02c | unit | `cargo test -p slicecore-engine --lib -- search_filter` | ❌ W0 | ⬜ pending |
| 44-01-04 | 01 | 1 | API-02d | unit | `cargo test -p slicecore-engine --lib enabled_profiles::tests::set` | ❌ W0 | ⬜ pending |
| 44-01-05 | 01 | 1 | API-02e | unit | `cargo test -p slicecore-engine --lib enabled_profiles::tests::set_roundtrip` | ❌ W0 | ⬜ pending |
| 44-02-01 | 02 | 2 | API-02f | integration | `cargo test -p slicecore-cli --test cli_profile_search` | ❌ W0 | ⬜ pending |
| 44-02-02 | 02 | 2 | API-02g | integration | `cargo test -p slicecore-cli --test cli_profile_compat` | ❌ W0 | ⬜ pending |
| 44-02-03 | 02 | 2 | API-02h | integration | `cargo test -p slicecore-cli --test cli_profile_set` | ❌ W0 | ⬜ pending |
| 44-02-04 | 02 | 2 | API-02i | integration | `cargo test -p slicecore-cli --test cli_slice_profiles -- set` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-engine/src/enabled_profiles.rs` — test stubs for nozzle check, temp check, ProfileSet CRUD, sets round-trip
- [ ] `crates/slicecore-cli/tests/cli_profile_search.rs` — CLI integration tests for search with filters
- [ ] `crates/slicecore-cli/tests/cli_profile_compat.rs` — CLI integration tests for compat command
- [ ] `crates/slicecore-cli/tests/cli_profile_set.rs` — CLI integration tests for set subcommands

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
