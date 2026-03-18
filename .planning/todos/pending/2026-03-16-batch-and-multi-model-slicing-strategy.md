---
created: 2026-03-16T18:15:00.000Z
title: Batch and multi-model slicing strategy
area: cli
files:
  - crates/slicecore-cli/src/main.rs
---

## Problem

The CLI currently slices one model per invocation. Several use cases need multi-model handling:

1. **Print farms**: Slice dozens of models for different printers with different profiles — each printer may be a different vendor or have different filament loaded
2. **SaaS**: Queue of models from different users needing different configurations
3. **Home users with multiple printers**: Less common but still valid — slice the same model for two printers at once

### Current state and decision

During Phase 30 planning, we chose **single model per invocation** as the CLI paradigm. Batch slicing is handled via shell scripting:

```bash
# Same printer/profile for all models
for f in *.stl; do
  slicecore slice "$f" -m X1C -f PLA
done

# Multi-model on one bed (existing arrange command)
slicecore arrange part1.stl part2.stl --apply
slicecore slice arranged.3mf -m X1C -f PLA
```

This works but doesn't address the harder cases (different printers, different filaments, priority scheduling).

## Solution

### Discussion points

1. **Should the CLI accept multiple models?**
   - Option A: Keep single-model CLI, batch via shell (current recommendation)
   - Option B: `slicecore slice *.stl -m X1C -f PLA` — same profile for all, parallel internally
   - Option C: Manifest file approach: `slicecore batch jobs.toml` where the TOML describes model→profile→printer mappings

2. **Multi-printer / multi-filament routing**
   ```toml
   # Example batch manifest
   [[jobs]]
   model = "bracket.stl"
   printer = "X1C"
   filament = "PLA"
   priority = 1

   [[jobs]]
   model = "gear.stl"
   printer = "Ender3"
   filament = "PETG"
   priority = 2
   ```
   This naturally maps to the daemon job queue (see daemon todo).

3. **Shell script documentation**: Even if we keep single-model CLI, we should document common batch patterns in `--help` or a cookbook:
   - Parallel with `xargs -P`
   - GNU parallel for multi-printer farms
   - Different profiles per model via a simple wrapper script

4. **Relationship to other todos**:
   - **Headless daemon**: Daemon subsumes batch slicing — jobs are submitted individually to a queue with priority. This may be the better long-term answer vs. CLI batch flags.
   - **Job output directories**: Each batch job naturally maps to one job directory.
   - **SaaS**: API accepts individual slice requests; batching is the API client's responsibility, not the slicer's.

### Recommendation

Keep CLI single-model. Document shell batch patterns. Let the daemon handle the real batch/multi-printer use case. Consider a `slicecore batch` command only if there's demand for a manifest-driven approach that shells can't easily replicate.
