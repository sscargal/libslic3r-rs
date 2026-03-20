---
created: 2026-03-16T18:10:00.000Z
title: Job output directories for isolated slice execution
area: cli
files:
  - crates/slicecore-cli/src/main.rs
  - crates/slicecore-engine/src/engine.rs
---

## Problem

Currently the CLI writes a single G-code file wherever `-o` points (or next to the input file). No job directory concept exists. All artifacts from a slice (G-code, logs, thumbnails, merged config) are scattered or lost.

This matters for two audiences:
- **Print farms / SaaS**: Parallel requests MUST be isolated. Each job needs its own directory with all artifacts, metadata, and logs contained together. Without this, concurrent slicing risks file collisions and makes job tracking impossible.
- **CLI power users**: Organized output (G-code + log + saved config in one folder) is cleaner than files scattered next to input models.

### Decision already made

During Phase 30 planning, we decided: **simple logging now (log file alongside G-code), job directory deferred**. This todo captures the deferred job directory work.

## Solution

### Proposed job directory structure

```
jobs/
├── job-abc123/
│   ├── model.gcode          # Sliced output
│   ├── model.log            # STDOUT/STDERR captured
│   ├── config.toml          # Merged config snapshot (reproducibility)
│   ├── thumbnail.png        # Preview render
│   └── manifest.json        # Job metadata (input file, profile, timestamps, hash)
```

### Implementation approach

1. **`--job-dir` flag** on `slice` command: opt-in for CLI users, always-on for daemon/SaaS mode
2. **Job ID generation**: UUID or content-hash based for uniqueness
3. **Manifest format**: JSON with input file path, profile used, start/end timestamps, slicing duration, output hash for integrity verification
4. **Config snapshot**: Dump the fully-merged `PrintConfig` used for the slice — enables exact reproduction
5. **Log capture**: Tee STDOUT/STDERR to `{jobname}.log` inside the job directory
6. **Thumbnail**: If render crate available, auto-generate preview
7. **Cleanup policy**: Optional `--max-jobs` or `--retention` for disk management

### Ties to other todos

- **Headless daemon slicer**: Daemon would use job directories as its native output model
- **Network printer discovery**: Job directory could include printer assignment and send status
- **SaaS**: Job directories become the unit of work for cloud slicing API responses
