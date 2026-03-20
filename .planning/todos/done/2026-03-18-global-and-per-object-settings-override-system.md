---
created: 2026-03-18T19:57:15.106Z
title: Global and per-object settings override system
area: engine
files:
  - crates/slicecore-engine/src/settings.rs
  - crates/slicecore-profiles/src/lib.rs
---

## Problem

Traditional slicers (PrusaSlicer, Cura, OrcaSlicer) allow users to define global print settings (layer height, infill, speed, etc.) and then override specific settings on a per-object basis. For example, a user may want 0.2mm layers globally but 0.1mm for a detailed part on the same plate. This is a fundamental slicer capability that libslic3r-rs currently lacks.

Per-object overrides need to cascade properly: object-level settings take precedence over global defaults, and potentially per-region (modifier mesh) settings override per-object settings. The system must handle inheritance, validation (e.g., a per-object layer height that doesn't divide evenly into the global one), and serialization for project files.

## Solution

Design a layered settings system with:
- Global settings as the base layer (from print profile)
- Per-object overrides that selectively replace specific keys
- Optional per-region/modifier overrides for sub-object control
- A merge/resolve step in the slicing pipeline that produces final effective settings per object
- Serialization support for 3MF project files and CLI config
- Consider a trait-based approach where `EffectiveSettings` is computed by merging layers at slice time
