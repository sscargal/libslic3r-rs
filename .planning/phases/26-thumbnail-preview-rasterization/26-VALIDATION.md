---
phase: 26
slug: thumbnail-preview-rasterization
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-10
---

# Phase 26 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) |
| **Config file** | workspace Cargo.toml |
| **Quick run command** | `cargo test -p slicecore-render` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-render`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 26-01-01 | 01 | 1 | RENDER-01 | unit | `cargo test -p slicecore-render framebuffer -x` | ❌ W0 | ⬜ pending |
| 26-01-02 | 01 | 1 | RENDER-02 | unit | `cargo test -p slicecore-render rasterizer -x` | ❌ W0 | ⬜ pending |
| 26-01-03 | 01 | 1 | RENDER-03 | unit | `cargo test -p slicecore-render camera -x` | ❌ W0 | ⬜ pending |
| 26-01-04 | 01 | 1 | RENDER-04 | unit | `cargo test -p slicecore-render shading -x` | ❌ W0 | ⬜ pending |
| 26-02-01 | 02 | 2 | RENDER-05 | unit | `cargo test -p slicecore-render png -x` | ❌ W0 | ⬜ pending |
| 26-02-02 | 02 | 2 | RENDER-06 | integration | `cargo test -p slicecore-fileio thumbnail -x` | ❌ W0 | ⬜ pending |
| 26-02-03 | 02 | 2 | RENDER-07 | integration | `cargo test -p slicecore-gcode-io thumbnail -x` | ❌ W0 | ⬜ pending |
| 26-03-01 | 03 | 3 | RENDER-08 | integration | `cargo test -p slicecore-cli thumbnail -x` | ❌ W0 | ⬜ pending |
| 26-03-02 | 03 | 3 | RENDER-09 | smoke | `cargo build -p slicecore-render --target wasm32-unknown-unknown` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/slicecore-render/` — entire crate is new
- [ ] `crates/slicecore-render/Cargo.toml` — new crate manifest
- [ ] `crates/slicecore-render/src/lib.rs` — public API entry point
- [ ] PNG and base64 dependencies in workspace Cargo.toml

*Wave 0 creates the crate scaffold and dependency declarations.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Visual quality of rendered thumbnails | RENDER-04 | Subjective shading appearance | Render test mesh, visually inspect PNG output for correct lighting/shading |
| 3MF thumbnail visible in slicer software | RENDER-06 | Requires external slicer to verify | Open generated 3MF in Bambu Studio/PrusaSlicer, confirm thumbnail displays |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
