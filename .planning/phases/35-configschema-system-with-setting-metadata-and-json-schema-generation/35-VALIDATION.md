---
phase: 35
slug: configschema-system-with-setting-metadata-and-json-schema-generation
status: draft
nyquist_compliant: true
wave_0_complete: true
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

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Nyquist | Status |
|---------|------|------|-------------|-----------|-------------------|---------|--------|
| 35-01-01 | 01 | 1 | Core types | unit (inline) | `cargo check -p slicecore-config-schema` | OK | pending |
| 35-01-02 | 01 | 1 | Registry | unit (inline `#[cfg(test)]`) | `cargo test -p slicecore-config-schema` | OK | pending |
| 35-02-01 | 02 | 1 | Derive macro | unit+integration | `cargo test -p slicecore-config-derive` | OK | pending |
| 35-03-01 | 03 | 2 | TIER_MAP review | checkpoint | N/A (human review) | OK | pending |
| 35-04-01 | 04 | 3 | Field annotations (enums + first half) | compile check | `cargo check -p slicecore-engine` | OK | pending |
| 35-04-02 | 04 | 3 | Field annotations (remaining + PrintConfig) | compile check | `cargo check -p slicecore-engine` | OK | pending |
| 35-05-01 | 05 | 3 | Schema validation upgrade | unit+integration | `cargo test -p slicecore-config-schema -p slicecore-engine` | OK | pending |
| 35-06-01 | 06 | 4 | JSON Schema + metadata JSON | unit (inline) | `cargo test -p slicecore-config-schema` | OK | pending |
| 35-06-02 | 06 | 4 | Search + global registry | unit+compile | `cargo test -p slicecore-config-schema -p slicecore-engine` | OK | pending |
| 35-07-01 | 07 | 5 | CLI subcommand | integration | `cargo test -p slicecore-cli` | OK | pending |

*Status: pending / green / red / flaky*

---

## Nyquist Rationale

- **Plans 35-01, 35-02, 35-05, 35-06:** Create inline `#[cfg(test)] mod tests` blocks within source files. `cargo test` runs these directly -- no separate test files needed (Wave 0 not required).
- **Plans 35-04 (annotation tasks):** These add `#[derive(SettingSchema)]` and `#[setting()]` attributes to existing structs. `cargo check` is the correct verification -- annotations are compile-time only; if they compile, they are correct. Runtime behavior is tested in plans 35-05 and 35-06 which exercise the generated code.
- **Plan 35-03:** Human checkpoint (TIER_MAP review) -- no automated test applicable.
- **Plan 35-07:** Uses existing `cargo test -p slicecore-cli` infrastructure.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| TIER_MAP.md accuracy | Tier assignments | Domain knowledge review | Compare tier assignments against OrcaSlicer Simple/Advanced/Expert tabs |

*All other behaviors have automated verification.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or justified exemption
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Inline tests cover Wave 0 needs (no separate test scaffold required)
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
