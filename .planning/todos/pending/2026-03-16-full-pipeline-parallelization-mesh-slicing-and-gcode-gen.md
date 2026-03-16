---
created: 2026-03-16T18:30:00.000Z
title: Full pipeline parallelization — mesh slicing and G-code gen
area: engine
files:
  - crates/slicecore-slicer/src/layer.rs
  - crates/slicecore-slicer/src/contour.rs
  - crates/slicecore-engine/src/gcode_gen.rs
  - crates/slicecore-engine/src/parallel.rs
  - crates/slicecore-mesh/src/bvh.rs
---

## Problem

Phase 25 parallelized the **per-layer processing** stage (perimeter generation, surface classification, infill, toolpath assembly) using rayon. However, two other computationally expensive stages remain sequential:

1. **Mesh slicing (triangle→contour)**: Currently iterates layers sequentially, intersecting the mesh plane-by-plane. Each plane intersection is independent — perfect for parallel execution. For high-poly meshes (1M+ triangles), this is a significant bottleneck.

2. **G-code generation**: Currently encodes layers sequentially into G-code text. While layer ordering must be preserved in the output, the per-layer encoding (move calculations, extrusion math, comment generation) can be parallelized with a final ordered concatenation.

Together these two stages can represent 30-50% of total slice time on complex models, meaning Phase 25's parallelism leaves significant performance on the table.

## Solution

### Stage 1: Parallel mesh slicing

- Split the Z-height list across rayon threads
- Each thread intersects the mesh BVH at its assigned Z heights independently
- BVH is read-only after construction — safe to share across threads
- Collect results into a Vec<Layer> ordered by Z
- **Complexity**: Low-medium. Main concern is BVH cache contention, but since each thread reads different triangle subsets at different Z heights, cache behavior should be reasonable.

### Stage 2: Parallel G-code encoding

- Use rayon `par_iter` over layers to encode each layer's G-code independently into a `String` or `Vec<u8>`
- Maintain layer ordering via indexed collection (`par_iter().enumerate()`)
- Final pass: concatenate encoded layers in order, insert layer-change commands
- **Complexity**: Medium. G-code state (current position, E accumulator, retraction state) is per-layer-boundary. Need to either: (a) compute initial state per layer in a sequential prefix-scan, then encode in parallel, or (b) encode relative moves and resolve absolute positions in the concatenation pass.

### Stage 3: Pipeline parallelism (advanced)

- Overlap stages: while mesh slicing produces layer N+K, layer processing works on layer N, and G-code gen encodes layer N-K
- Requires a bounded channel/pipeline architecture (crossbeam or tokio channels)
- **Complexity**: High. Requires rearchitecting from batch-stage to streaming-pipeline. Biggest win for very large models where memory pressure from holding all layers is a concern.

### Benchmarking approach

- Use the existing calibration cube + a high-poly organic model (1M+ triangles)
- Measure: wall time, CPU utilization, peak memory, G-code output determinism
- Compare: sequential baseline → Phase 25 (layer parallel) → this todo (full pipeline)
- Target: 2-4x additional speedup on 8+ core machines over Phase 25 baseline
