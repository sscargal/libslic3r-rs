---
phase: 28
slug: g-code-post-processing-plugin-point
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-12
---

# Phase 28 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) |
| **Config file** | Cargo.toml workspace test settings |
| **Quick run command** | `cargo test -p slicecore-plugin-api -p slicecore-plugin -p slicecore-engine --lib` |
| **Full suite command** | `cargo test --all-features --workspace` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-plugin-api -p slicecore-plugin -p slicecore-engine --lib`
- **After every plan wave:** Run `cargo test --all-features --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 28-01-01 | 01 | 1 | PLUGIN-01 | unit | `cargo test -p slicecore-plugin-api postprocess` | ❌ W0 | ⬜ pending |
| 28-01-02 | 01 | 1 | PLUGIN-02 | unit | `cargo test -p slicecore-plugin registry::tests` | ❌ W0 | ⬜ pending |
| 28-02-01 | 02 | 2 | ADV-04 | integration | `cargo test -p slicecore-engine --test post_process_integration` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-plugin-api/src/postprocess_types.rs` — FfiGcodeCommand, PostProcessRequest/Result types
- [ ] `crates/slicecore-plugin-api/src/postprocess_traits.rs` — GcodePostProcessorPlugin trait
- [ ] `crates/slicecore-plugin/src/postprocess_convert.rs` — GcodeCommand <-> FfiGcodeCommand conversion
- [ ] `crates/slicecore-plugin/src/postprocess.rs` — PostProcessorPluginAdapter, pipeline runner
- [ ] `crates/slicecore-engine/src/postprocess_builtin.rs` — 4 built-in post-processors
- [ ] `crates/slicecore-engine/tests/post_process_integration.rs` — integration tests

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| CLI `post-process` subcommand UX | ADV-04 | Requires actual G-code file I/O | Run `cargo run -- post-process --help` and verify output |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
