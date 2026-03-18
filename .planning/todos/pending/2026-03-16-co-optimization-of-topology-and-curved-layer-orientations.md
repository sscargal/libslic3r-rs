---
created: 2026-03-16T19:32:48.250Z
title: Co-optimization of topology and curved layer orientations
area: engine
files:
  - crates/slicecore-slicer/src/layer.rs
  - crates/slicecore-engine/src/engine.rs
---

## Problem

Traditional structural optimization for 3D printing is sequential: first optimize topology (what material goes where), then figure out how to slice it (layer orientations). This two-step approach leaves performance on the table because:

- Topology optimization doesn't account for anisotropic FDM material properties (layer adhesion is the weak axis)
- Slicing doesn't feed back into structural design (layers are imposed after the fact)
- The result is parts that are structurally suboptimal for the actual print orientation

ArXiv research on neural network-based inverse design demonstrates that simultaneously optimizing structural topology AND curved layer orientations achieves a 33.1% improvement in failure loads compared to traditional sequential optimization.

## Solution

### Concept

Instead of "design part → slice part", co-optimization treats topology and layer paths as a single optimization problem:

```
Traditional:  Topology Opt → Fixed Design → Planar Slicing → Print
Co-Optimized: Topology + Layer Paths → Simultaneously Optimized → Non-Planar Print
```

The optimizer considers:
- **Where material should be** (topology) — structural efficiency
- **How layers should be oriented** (curved paths) — maximizes interlayer adhesion along load paths
- **Printability constraints** — overhang angles, nozzle clearance, collision avoidance

### Neural network inverse design approach

1. **Input**: Load conditions, boundary conditions, build volume, material properties (including anisotropic FDM properties)
2. **Neural network**: Trained on FEM simulations to predict optimal topology + layer orientations jointly
3. **Output**: Optimized material distribution + non-planar toolpath orientation field
4. **Constraint enforcement**: Printability constraints (max overhang, min feature size) enforced during optimization

### Integration with slicecore

This feature bridges non-planar slicing (existing todo) with structural optimization:

- **New module**: `slicecore-engine/src/topology_opt.rs` — interfaces with the optimization model
- **Non-planar slicer**: Consumes the layer orientation field to generate curved toolpaths
- **Validation**: FEM verification that the optimized design meets load requirements
- **Export**: Optimized design as mesh + layer orientation field metadata

### Practical use cases

- **Functional brackets/mounts**: Load-bearing parts where orientation matters
- **Lightweight structures**: Minimum material for required strength
- **Lattice/infill optimization**: Internal structure co-designed with layer paths for maximum strength-to-weight

## Dependencies

- **Non-planar slicing** (todo): Required to print curved layer orientations
- **PFEM/FEM simulation** (todo): Structural analysis for optimization feedback
- **Neural network runtime**: ONNX or similar for inference in Rust
- **Research reference**: ArXiv paper on neural network-based inverse design for AM

## Phased implementation

1. **Phase A**: Research — implement basic topology optimization with FDM anisotropy awareness
2. **Phase B**: Integrate with non-planar slicer to consume orientation fields
3. **Phase C**: Neural network-based co-optimization model (training + inference)
4. **Phase D**: Interactive workflow — user specifies loads, system co-optimizes and slices
