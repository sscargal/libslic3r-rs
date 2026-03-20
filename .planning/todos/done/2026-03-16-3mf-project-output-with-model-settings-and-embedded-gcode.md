---
created: 2026-03-16T18:20:00.000Z
title: 3MF project output with model settings and embedded G-code
area: fileio
files:
  - crates/slicecore-fileio/src/threemf.rs
  - crates/slicecore-fileio/src/export.rs
---

## Problem

The slicer can already write 3MF mesh files (Phase 24), but a full 3MF **project** file is different — it bundles the model geometry, print settings, thumbnails, and optionally the sliced G-code into a single archive. This is the format PrusaSlicer, BambuStudio, and OrcaSlicer use for their native project files.

Without project output, users can't:
- Save a complete "slice session" as a single shareable file
- Re-open a project with all settings intact (model + profile + plate layout)
- Send a ready-to-print 3MF to Bambu printers (which expect G-code embedded in 3MF)
- Share reproducible print setups with others

## Solution

### What's new vs. existing 3MF export

| Feature | Current 3MF export | 3MF project |
|---------|-------------------|-------------|
| Mesh geometry | Yes | Yes |
| Print settings (slicer metadata) | No | Yes — custom XML extensions |
| Plate/build layout | No | Yes — multi-object positioning |
| Embedded G-code | No | Yes — as attachment in ZIP |
| Thumbnail(s) | No | Yes — PNG in `/Metadata/` |
| Filament/material info | No | Yes — material metadata |
| Printer profile reference | No | Yes — machine metadata |

### Discussion points

1. **Metadata schema**: Follow PrusaSlicer/BambuStudio conventions for compatibility, or define our own? Compatibility is probably more valuable for ecosystem interop.
2. **G-code embedding**: Bambu printers require G-code inside the 3MF. This is the primary motivation for some users. Format: `plate_1.gcode` (or `.gcode.gz`) in the archive root or `/Metadata/`.
3. **Round-trip fidelity**: Can we read a PrusaSlicer 3MF project, modify settings, and re-export without losing data? This requires preserving unknown XML extensions.
4. **Thumbnail generation**: Ties into the render crate (Phase 26). Auto-generate plate thumbnails for the project file.
5. **CLI integration**: `slicecore slice model.stl -o project.3mf --project` — writes full project instead of just G-code. Or `slicecore project create` as a separate command.
6. **Scope**: This is a significant feature. Consider splitting into:
   - Phase A: Settings metadata in 3MF (read + write PrusaSlicer/Bambu format)
   - Phase B: Embedded G-code (Bambu printer compat)
   - Phase C: Full project round-trip (open, modify, re-export)

### Ties to other todos

- **Job output directories**: A 3MF project file is essentially a job directory in a single ZIP
- **Batch slicing**: Could output batch results as multi-plate 3MF projects
