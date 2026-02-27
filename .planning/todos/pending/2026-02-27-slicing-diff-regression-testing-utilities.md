---
created: 2026-02-27T17:46:30.000Z
title: Slicing Diff / Regression Testing Utilities
area: testing
files: []
---

## Problem

When making changes to the slicing engine, developers need to compare two slicing runs structurally to detect regressions — not just G-code text diff but semantic comparison of layer counts, feature distributions, extrusion amounts, and timing.

## Solution

The existing G-code analysis/comparison tool (Phase 21) already covers much of this use case via `analyze-gcode` and `compare-gcode` CLI subcommands. Consider whether the existing tooling is sufficient or if a higher-level "slice and compare" convenience wrapper is needed. Low priority — defer until real regression testing workflows surface a gap.
