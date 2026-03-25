---
phase: 46
slug: job-output-directories-for-isolated-slice-execution
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-24
---

# Phase 46 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) |
| **Config file** | Cargo.toml [dev-dependencies] |
| **Quick run command** | `cargo test -p slicecore-cli --lib -- job_dir` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-cli --lib -- job_dir`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 46-01-01 | 01 | 1 | API-02 | unit | `cargo test -p slicecore-cli --lib -- job_dir::create` | ❌ W0 | ⬜ pending |
| 46-01-02 | 01 | 1 | API-02 | unit | `cargo test -p slicecore-cli --lib -- job_dir::auto` | ❌ W0 | ⬜ pending |
| 46-01-03 | 01 | 1 | API-02 | unit | `cargo test -p slicecore-cli --lib -- job_dir::nonempty` | ❌ W0 | ⬜ pending |
| 46-01-04 | 01 | 1 | API-02 | unit | `cargo test -p slicecore-cli --lib -- job_dir::lock` | ❌ W0 | ⬜ pending |
| 46-01-05 | 01 | 1 | API-02 | unit | `cargo test -p slicecore-cli --lib -- job_dir::base_priority` | ❌ W0 | ⬜ pending |
| 46-01-06 | 01 | 1 | API-02 | unit | `cargo test -p slicecore-cli --lib -- manifest` | ❌ W0 | ⬜ pending |
| 46-02-01 | 02 | 2 | API-02 | integration | `cargo test -p slicecore-cli -- job_dir::conflicts` | ❌ W0 | ⬜ pending |
| 46-02-02 | 02 | 2 | API-02 | integration | `cargo test -p slicecore-cli -- job_dir::stdout` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-cli/src/job_dir.rs` — new module with unit test stubs for API-02
- [ ] `crates/slicecore-cli/tests/cli_job_dir.rs` — integration test stubs for CLI behavior
- [ ] Add `uuid` (v1.22, features: v4) and `chrono` (v0.4.44, features: serde) to slicecore-cli Cargo.toml

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Thumbnail renders correctly in job dir | API-02 | Visual quality check | Run `slicecore slice model.stl --job-dir /tmp/test-job`, open thumbnail.png |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
