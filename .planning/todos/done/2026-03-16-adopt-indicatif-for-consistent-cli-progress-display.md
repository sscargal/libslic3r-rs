---
created: 2026-03-16T17:59:31.978Z
title: Adopt indicatif for consistent CLI progress display
area: cli
files:
  - crates/slicecore-cli/src/main.rs
  - crates/slicecore-cli/src/calibrate/mod.rs
  - crates/slicecore-cli/src/analysis_display.rs
  - crates/slicecore-cli/src/stats_display.rs
---

## Problem

The CLI currently uses ad-hoc println!/eprintln! for progress indication across commands (slicing, calibration, G-code analysis, etc.). There is no consistent progress bar, spinner, or step-tracking UX. The `indicatif` crate (https://crates.io/crates/indicatif) is the Rust ecosystem standard for terminal progress display and would unify the experience.

## Solution

1. Add `indicatif` as a dependency to `slicecore-cli`
2. Audit all CLI commands for places that should show progress:
   - Slicing pipeline (layer progress, toolpath generation)
   - Calibration commands (mesh generation, slicing, G-code post-processing steps)
   - G-code analysis (parsing progress for large files)
   - File conversion/export
   - Profile import
3. Replace ad-hoc output with `indicatif` widgets:
   - `ProgressBar` for known-length operations (layer slicing, file parsing)
   - `ProgressBar::new_spinner()` for indeterminate operations
   - `MultiProgress` for parallel operations
   - Styled step indicators for sequential pipeline stages
4. Ensure consistent styling (colors, templates, tick characters) across all commands
5. Respect `--quiet` / `--json` flags by suppressing progress display when structured output is requested
