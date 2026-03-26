---
phase: 50
slug: 3mf-project-output
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-26
---

# Phase 50 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) |
| **Config file** | Cargo.toml per crate |
| **Quick run command** | `cargo test -p slicecore-fileio --lib` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~60 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-fileio --lib`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 50-01-01 | 01 | 1 | P50-01 | unit | `cargo test -p slicecore-fileio export_project -- --nocapture` | ❌ W0 | ⬜ pending |
| 50-01-02 | 01 | 1 | P50-02 | unit | `cargo test -p slicecore-fileio gcode_embedding -- --nocapture` | ❌ W0 | ⬜ pending |
| 50-01-03 | 01 | 1 | P50-03 | unit | `cargo test -p slicecore-fileio settings_config -- --nocapture` | ❌ W0 | ⬜ pending |
| 50-01-04 | 01 | 1 | P50-04 | unit | `cargo test -p slicecore-fileio plate_thumbnails -- --nocapture` | ❌ W0 | ⬜ pending |
| 50-01-05 | 01 | 1 | P50-05 | unit | `cargo test -p slicecore-fileio plate_metadata_json -- --nocapture` | ❌ W0 | ⬜ pending |
| 50-02-01 | 02 | 2 | P50-06 | integration | `cargo test -p slicecore-cli project_output -- --nocapture` | ❌ W0 | ⬜ pending |
| 50-02-02 | 02 | 2 | P50-07 | integration | `cargo test -p slicecore-cli dual_output -- --nocapture` | ❌ W0 | ⬜ pending |
| 50-01-06 | 01 | 1 | P50-08 | unit | `cargo test -p slicecore-fileio project_roundtrip -- --nocapture` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-fileio/src/project_config.rs` — XML config builder tests
- [ ] `crates/slicecore-fileio/src/plate_metadata.rs` — PlateMetadata serialization tests
- [ ] Tests in `crates/slicecore-fileio/src/export.rs` for `export_project_to_3mf()` function

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Bambu printer LCD displays thumbnails | P50-04 | Requires physical printer | Upload .3mf to Bambu printer, verify thumbnail shows on LCD |
| OrcaSlicer opens project correctly | P50-01 | Requires OrcaSlicer GUI | Open generated .3mf in OrcaSlicer, verify all settings load |
| AMS filament mapping works on printer | P50-05 | Requires AMS hardware | Print multi-material project, verify AMS slot selection |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
