---
phase: 39
slug: jpeg-thumbnail-export
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
---

# Phase 39 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) |
| **Config file** | Workspace Cargo.toml |
| **Quick run command** | `cargo test -p slicecore-render -p slicecore-cli -p slicecore-gcode-io -p slicecore-fileio` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p slicecore-render -p slicecore-cli`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 39-01-01 | 01 | 1 | PNG encoding after migration | unit | `cargo test -p slicecore-render -- encode` | ❌ W0 | ⬜ pending |
| 39-01-02 | 01 | 1 | JPEG encoding valid JFIF | unit | `cargo test -p slicecore-render -- encode_jpeg` | ❌ W0 | ⬜ pending |
| 39-01-03 | 01 | 1 | JPEG alpha compositing white bg | unit | `cargo test -p slicecore-render -- jpeg_white_background` | ❌ W0 | ⬜ pending |
| 39-02-01 | 02 | 1 | CLI --format jpeg .jpg file | integration | `cargo test -p slicecore-cli --test cli_thumbnail` | ❌ W0 | ⬜ pending |
| 39-02-02 | 02 | 1 | CLI auto-detect .jpg extension | integration | `cargo test -p slicecore-cli --test cli_thumbnail` | ❌ W0 | ⬜ pending |
| 39-02-03 | 02 | 1 | Quality validation out-of-range | unit | `cargo test -p slicecore-cli` | ❌ W0 | ⬜ pending |
| 39-01-04 | 01 | 1 | gcode_embed encoded_data field | unit | `cargo test -p slicecore-render -- gcode_embed` | ✅ needs update | ⬜ pending |
| 39-02-04 | 02 | 1 | 3MF always PNG regardless | integration | `cargo test -p slicecore-fileio` | ✅ existing | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Update `png_encode.rs` tests for new module name and `image` crate API
- [ ] Add JPEG encoding unit tests (magic bytes `FF D8 FF`, valid output)
- [ ] Add JPEG alpha compositing test (white background verification)
- [ ] Add CLI integration tests for `--format jpeg` and `--quality`
- [ ] Update `gcode_embed.rs` tests for field rename (`png_data` -> `encoded_data`)

*Existing infrastructure covers base test patterns; new tests needed for JPEG-specific behaviors.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Visual quality of JPEG thumbnails | Quality control | Subjective image quality assessment | Generate thumbnails at quality 85, 50, 10; visually compare |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
