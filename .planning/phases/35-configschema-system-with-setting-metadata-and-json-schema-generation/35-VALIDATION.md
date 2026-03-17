---
phase: 35
slug: configschema-system-with-setting-metadata-and-json-schema-generation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-17
---

# Phase 35 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | `Cargo.toml` workspace |
| **Quick run command** | `cargo test -p slicecore-config-schema -p slicecore-config-derive` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-config-schema -p slicecore-config-derive`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 35-01-01 | 01 | 1 | Core types | unit | `cargo test -p slicecore-config-schema` | ❌ W0 | ⬜ pending |
| 35-01-02 | 01 | 1 | Derive macro | unit+integration | `cargo test -p slicecore-config-derive` | ❌ W0 | ⬜ pending |
| 35-02-01 | 02 | 1 | Registry singleton | unit | `cargo test -p slicecore-config-schema` | ❌ W0 | ⬜ pending |
| 35-03-01 | 03 | 2 | Field annotations | integration | `cargo test -p slicecore-engine --test schema_integration` | ❌ W0 | ⬜ pending |
| 35-04-01 | 04 | 2 | Schema validation | integration | `cargo test -p slicecore-engine --test config_validation` | ❌ W0 | ⬜ pending |
| 35-05-01 | 05 | 3 | JSON Schema output | integration | `cargo test -p slicecore-config-schema --test json_schema` | ❌ W0 | ⬜ pending |
| 35-06-01 | 06 | 3 | CLI subcommand | integration | `cargo test -p slicecore-cli` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-config-schema/tests/` — unit tests for core types (SettingDefinition, ValueType, SettingRegistry)
- [ ] `crates/slicecore-config-derive/tests/` — proc-macro integration tests (derive on test structs)
- [ ] Test fixtures: sample config structs with known field counts and types

*Existing cargo test infrastructure covers framework needs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| TIER_MAP.md accuracy | Tier assignments | Domain knowledge review | Compare tier assignments against OrcaSlicer Simple/Advanced/Expert tabs |

*All other behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
