---
phase: 37
slug: ci-benchmark-tracking-with-regression-detection
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
---

# Phase 37 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | GitHub Actions workflow syntax + YAML validation |
| **Config file** | `.github/workflows/ci.yml` |
| **Quick run command** | `yamllint .github/workflows/ci.yml && echo "YAML valid"` |
| **Full suite command** | Create a test PR to trigger the full benchmark workflow |
| **Estimated runtime** | ~5 seconds (YAML lint) / ~15 minutes (full CI run) |

---

## Sampling Rate

- **After every task commit:** Run `yamllint .github/workflows/ci.yml`
- **After every plan wave:** Push to test branch to trigger actual CI run
- **Before `/gsd:verify-work`:** Full PR cycle must be green (bench job runs, comments posted, skip logic works)
- **Max feedback latency:** 5 seconds (local YAML validation)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 37-01-01 | 01 | 1 | N/A-01 | smoke | `grep 'bench:' .github/workflows/ci.yml` | ❌ W0 | ⬜ pending |
| 37-01-02 | 01 | 1 | N/A-02 | smoke | `grep 'paths-filter' .github/workflows/ci.yml` | ❌ W0 | ⬜ pending |
| 37-01-03 | 01 | 1 | N/A-03 | smoke | `grep 'criterion-compare-action' .github/workflows/ci.yml` | ❌ W0 | ⬜ pending |
| 37-01-04 | 01 | 1 | N/A-04 | smoke | `grep 'github-action-benchmark' .github/workflows/ci.yml` | ❌ W0 | ⬜ pending |
| 37-01-05 | 01 | 1 | N/A-05 | smoke | `grep 'bench-ok' .github/workflows/ci.yml` | ❌ W0 | ⬜ pending |
| 37-01-06 | 01 | 1 | N/A-06 | smoke | `grep 'memory' scripts/bench-with-memory.sh` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `scripts/bench-with-memory.sh` — memory tracking wrapper script
- [ ] gh-pages branch created (one-time setup for benchmark data storage)
- [ ] GitHub Pages enabled on the repository
- [ ] `bench-ok` label created in the GitHub repository

*Infrastructure setup needed before CI workflow can function end-to-end.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| PR comment appears with comparison table | N/A-03 | Requires actual GitHub PR context | Create a test PR with a code change, verify bench comment is posted |
| gh-pages updated on main push | N/A-04 | Requires merge to main | Merge a test PR, check gh-pages branch for new JSON data |
| bench-ok label overrides block | N/A-05 | Requires label interaction on a real PR | Create PR with regression, add bench-ok label, verify not blocked |
| Dashboard renders correctly | N/A-04 | Visual verification of gh-pages site | Visit GitHub Pages URL after data is pushed |
| Docs-only PR skips benchmarks | N/A-02 | Requires CI event context | Push a .md-only change, verify bench job is skipped |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 5s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
