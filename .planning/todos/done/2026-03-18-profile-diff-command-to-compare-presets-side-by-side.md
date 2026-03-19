---
created: 2026-03-18T19:57:15.106Z
title: Profile diff command to compare presets side by side
area: cli
files:
  - crates/slicecore-cli/src/main.rs
  - crates/slicecore-engine/src/config.rs
---

## Problem

Users frequently need to compare two print/filament presets to understand how they differ and what effect those differences will have on slicing results. For example, comparing "0.4mm Standard @BBL H2S" with "Bambu PLA Basic @BBL H2S" — these are different preset types (print vs filament) but a user may want to see all parameter differences that affect the final output.

Currently there's a `--compare` flag that does side-by-side cost/time estimation with different filament profiles, but no way to diff the raw settings of two presets to see exactly which parameters differ, what the values are, and what those differences mean for print quality/speed/strength.

This was noted in the future feature brainstorm (item D3) but needs dedicated implementation.

## Solution

Implement `slicecore profile diff <preset-a> <preset-b>`:

1. **Load and normalize**: Parse both presets into a common settings structure, resolving inheritance chains (presets often inherit from a base)
2. **Diff engine**: Compare all keys, categorize differences:
   - Only in A / Only in B / Different values
   - Group by category (speed, temperature, retraction, infill, etc.)
3. **Human-readable output**: Table format showing setting name, value A, value B, and a brief description of what the setting controls
4. **Impact hints**: Where possible, annotate whether a difference is likely to affect print time, quality, strength, or material usage
5. **Output formats**: Table (default), JSON, and potentially a summary mode that highlights only the most impactful differences
6. **Cross-type comparison**: Handle comparing presets of different types (print profile vs filament profile) by showing the union of all settings
