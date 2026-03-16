---
created: 2026-03-16T18:25:00.000Z
title: Future feature brainstorm for libslic3r-rs library and CLI
area: general
files: []
---

## Problem

With 35 phases complete or planned and the core slicing pipeline mature, we need to identify what's missing for a competitive slicer library. This brainstorm focuses strictly on libslic3r-rs (library + CLI) — not SaaS, not web UI.

**Scope**: slicecore-engine, slicecore-cli, and supporting crates. Features that benefit home users, print farms (via CLI/library API), or SaaS (as the backend executor).

**Already covered** by existing phases (32-35) or todos: config gaps, support config mapping, ConfigSchema, network printer discovery, slicing diff testing, 3MF project output, indicatif progress, batch slicing, calibration catalog, TUI evaluation, headless daemon, job directories.

## Feature Brainstorm

### A. Mesh Analysis & Preparation (slicecore-mesh, slicecore-slicer)

| # | Feature | Who benefits | Description |
|---|---------|-------------|-------------|
| A1 | **Printability analysis & scoring** | All | Pre-slice mesh analysis: detect thin walls, overhangs, unsupported features, non-manifold edges. Output a printability score (0-100) with specific warnings. Helps SaaS reject unprintable models early. |
| A2 | **Mesh simplification / decimation** | SaaS, farms | Reduce triangle count for faster slicing while preserving print-relevant features. High-poly sculpts (1M+ triangles) are common on Printables. |
| A3 | **Adaptive layer height from curvature** | Home users | Analyze mesh surface curvature to auto-generate variable layer heights — thick layers on flat areas, thin layers on curves. PrusaSlicer has this; we should too. |
| A4 | **Multi-color mesh segmentation** | Home users | Paint/assign colors to mesh faces for MMU/AMS printing. Input: per-face color map or texture-to-color conversion. Output: multi-extruder toolpath. |
| A5 | **Seam map input** | Home users | Accept a seam preference file (per-layer position hints or mesh-face annotations) for precise seam placement on visible surfaces. |
| A6 | **Custom support painting / blockers** | Home users | Per-face support enforcement/blocking via modifier meshes or annotation files, beyond the current auto-detect. |

### B. Slicing Engine Improvements (slicecore-engine)

| # | Feature | Who benefits | Description |
|---|---------|-------------|-------------|
| B1 | **Variable infill by region** | All | Different infill density/pattern per region — dense near load-bearing surfaces, sparse in cosmetic areas. Can be auto-detected from stress analysis or user-annotated. |
| B2 | **Print failure prediction** | SaaS, farms | Analyze generated toolpath for likely failure modes: unsupported cantilevers, insufficient cooling time, excessive retraction count, nozzle collisions with curled layers. |
| B3 | **Advanced ironing patterns** | Home users | Monotonic ironing, cross-hatch ironing, ironing only on top surfaces above certain area threshold. Improve on the current implementation. |
| B4 | **Arachne perimeter refinement** | Home users | Improve variable-width perimeter generation for better gap fill, smoother transitions, and handling of acute corners. |
| B5 | **Fuzzy skin / texture generation** | Home users | Programmatic surface texture (fuzzy skin, wood grain, stipple) by perturbing outer perimeter paths. |
| B6 | **Organic supports** | All | Tree supports with organic/smooth branching (like OrcaSlicer's organic supports), not just the angular tree supports currently implemented. |
| B7 | **Paint-on variable settings** | Home users | Per-region overrides for speed, temperature, fan — applied via modifier meshes or face annotations, extending the existing modifier system. |

### C. G-code & Firmware (slicecore-gcode-io, slicecore-engine)

| # | Feature | Who benefits | Description |
|---|---------|-------------|-------------|
| C1 | **Arc fitting / arc welder** | All | Convert sequential linear moves into G2/G3 arcs where curvature permits. Reduces G-code size 30-60%, improves print quality on arc-capable firmware. |
| C2 | **G-code simulation** | SaaS, farms | Simulate toolpath execution: detect nozzle-mesh collisions (curling), calculate true print time accounting for acceleration, validate firmware compatibility. |
| C3 | **Firmware retraction (G10/G11)** | Power users | Option to use firmware-managed retraction instead of explicit E-moves. Some firmwares handle this better. |
| C4 | **Pressure advance tuning in G-code** | Power users | Embed PA/LA value changes per feature type (e.g., lower PA for infill, higher for external perimeters). |
| C5 | **Speed profile optimization** | Farms, SaaS | Given a target print time, optimize speed/acceleration per feature to meet the target while maintaining quality. Inverse of "how long will this take?" |

### D. Profile & Material System (slicecore-engine)

| # | Feature | Who benefits | Description |
|---|---------|-------------|-------------|
| D1 | **Material properties database** | All | Structured data per filament: density, shrinkage %, glass transition temp, recommended temp ranges, mechanical properties (tensile, impact), drying requirements. Powers AI suggestions and auto-config. |
| D2 | **Profile inheritance / layering** | All | Formal parent→child profile system: "PETG-Generic" → "PETG-Polymaker" → "PETG-Polymaker-HighSpeed". Only overridden fields stored in child. |
| D3 | **Profile diffing** | Power users | `slicecore profile diff profile-a.toml profile-b.toml` — shows exactly which settings differ, with human-readable descriptions. |
| D4 | **Shrinkage compensation per material** | Home users | Auto-scale model dimensions based on known material shrinkage factors. Critical for functional parts in ABS, ASA, Nylon. |
| D5 | **Filament spool tracking** | Farms | Track remaining filament per spool (by weight or length). Warn when a slice would exceed remaining spool. Library provides the data model; CLI provides the commands. |

### E. Platform & Integration (cross-crate)

| # | Feature | Who benefits | Description |
|---|---------|-------------|-------------|
| E1 | **C FFI / shared library export** | Ecosystem | `cdylib` target exposing core slicing API via C ABI. Enables plugins for other slicers, bindings for Python/Node/Swift, and integration into non-Rust codebases. |
| E2 | **WASM optimization & browser demo** | SaaS | Optimize WASM build size and performance. Create a standalone browser demo that loads STL, slices in-browser, and renders toolpath preview — proving the WASM story. |
| E3 | **Python bindings (PyO3)** | Data scientists, farms | Python wrapper for the slicing library. Enables scripted batch operations, ML training on slicing data, Jupyter notebook workflows. |
| E4 | **Benchmarking suite** | All devs | Standardized performance benchmarks: slice time, memory usage, G-code size across a curated set of models (calibration cube → complex organic). Track regressions per commit. |
| E5 | **Structured error catalog** | SaaS, farms | Every error has a unique code (SC-1001), severity, recovery hint. Enables SaaS to map errors to user-friendly messages and automated retry strategies. |

### F. CLI Enhancements (slicecore-cli)

| # | Feature | Who benefits | Description |
|---|---------|-------------|-------------|
| F1 | **Config validation command** | All | `slicecore validate config.toml` — check config for errors, warnings, and suggestions without slicing. Report missing required fields, out-of-range values, conflicting settings. |
| F2 | **Slice preview (ASCII/SVG)** | Power users | `slicecore preview model.stl --layer 50` — render a single layer as ASCII art or SVG for quick inspection without a GUI. |
| F3 | **G-code diff** | Testing, devs | `slicecore diff file1.gcode file2.gcode` — semantic diff that ignores comments/whitespace, groups changes by layer, highlights meaningful differences (speed changes, path changes, retraction changes). |
| F4 | **Shell completions** | Power users | Generate bash/zsh/fish completions via clap's derive. Low effort, high UX impact. |
| F5 | **Recipe files** | Farms | `slicecore slice --recipe my-workflow.toml model.stl` — a recipe bundles machine + filament + process profiles + post-processors + output format in one reusable file. |
| F6 | **Watch mode** | Devs | `slicecore watch model.stl -m X1C -f PLA` — re-slice automatically when the input file changes. Useful during CAD iteration. |

## Prioritization Suggestion

**Highest impact for broadest audience:**
1. A1 (printability analysis) — differentiator, helps everyone
2. C1 (arc fitting) — universal quality improvement
3. D1 (material database) — powers AI suggestions, auto-config
4. E1 (C FFI) — unlocks ecosystem integration
5. A3 (adaptive layer height) — expected feature in modern slicers

**Highest impact for farms/SaaS specifically:**
1. B2 (failure prediction) — reduces waste, enables automated QA
2. C2 (G-code simulation) — validates before printing
3. E3 (Python bindings) — scripting and automation
4. E5 (structured errors) — programmatic error handling
5. A2 (mesh decimation) — handles user-submitted high-poly models

**Quick wins (low effort, high value):**
1. F4 (shell completions) — one-liner with clap
2. D3 (profile diff) — straightforward CLI command
3. F1 (config validation) — reuse existing config parsing with better error reporting
4. C3 (firmware retraction) — small G-code gen change
