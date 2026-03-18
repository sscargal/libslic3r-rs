---
created: 2026-03-16T18:50:00.000Z
title: Arachne slicer and advanced slicing algorithm brainstorm
area: engine
files:
  - crates/slicecore-engine/src/arachne.rs
  - crates/slicecore-engine/src/perimeter.rs
  - crates/slicecore-slicer/src/contour.rs
  - crates/slicecore-slicer/src/adaptive.rs
---

## Current State

We have a working Arachne implementation (627 lines, `arachne.rs`) using `boostvoronoi` for medial axis computation. It handles thin walls by generating variable-width extrusion paths. Standard regions fall back to fixed-width perimeters.

## Problem

The current Arachne is a solid v1 but there's significant room for improvement, and several alternative/complementary slicing algorithms could dramatically improve print quality. This brainstorm covers both Arachne refinement and broader algorithmic innovations.

## Arachne Improvements

### A1: Smooth width transitions

**Current**: Width changes abruptly at segment boundaries.
**Improvement**: Interpolate width along the medial axis using cubic splines or Gaussian smoothing. Prevents sudden extrusion width jumps that cause pressure artifacts.
**Impact**: Eliminates visible seams at thin→thick transitions.

### A2: Multi-pass thin wall filling

**Current**: Single variable-width path along medial axis for thin regions.
**Improvement**: For walls between 2x and 3x nozzle width, generate two variable-width paths that share the space optimally, rather than one wide path + gap or falling back to fixed-width.
**Impact**: Better fill quality in the "awkward width" range (1.0-1.5mm for 0.4mm nozzle).

### A3: Junction handling at intersections

**Current**: Medial axis branches at polygon junctions are handled independently.
**Improvement**: At T-junctions and Y-junctions, smoothly merge variable-width paths to avoid over-extrusion blobs. Use flow-rate ramping on approach to junctions.
**Impact**: Eliminates blobs at wall intersections — one of the most visible Arachne artifacts.

### A4: Outer-wall-first with variable width

**Current**: Variable-width paths are generated inside-out.
**Improvement**: Support outer-wall-first mode for Arachne paths, where the outermost variable-width perimeter prints first for better dimensional accuracy.
**Impact**: Better surface finish on thin-wall regions.

### A5: Voronoi robustness improvements

**Current**: Uses `boostvoronoi` which can produce degenerate edges on certain inputs.
**Improvement**: Add pre-processing (polygon simplification, tiny-edge removal) and post-processing (degenerate edge filtering, topology validation) to handle adversarial geometry.
**Impact**: Fewer crashes/artifacts on complex real-world models from Printables/Thingiverse.

## Alternative Slicing Algorithms

### B1: Fermat spiral continuous paths

**Paper**: "Connected Fermat Spirals for Layered Fabrication" (Zhao et al., 2016)
**Concept**: Generate toolpaths as continuous Fermat spirals that fill the entire layer in one unbroken path — no seams, no retractions within a layer.
**How**: Compute medial axis → generate spiral paths that wind in and out along the medial axis → connect into a single continuous curve.
**Impact**: Eliminates seams entirely, reduces retractions by 90%+, improves surface quality. Revolutionary for vase/cosmetic prints.
**Complexity**: High. Requires solving the path-planning problem globally per layer.

### B2: Stress-aware toolpath alignment

**Concept**: Orient infill and perimeter extrusion directions to align with predicted stress fields, similar to how carbon fiber is laid up in composites.
**How**: Run simplified FEA on the model → compute principal stress directions per layer → align rectilinear infill with tension direction, cross-hatch in shear zones.
**Impact**: 2-5x strength improvement in specific load directions without increasing material usage. Game-changer for functional parts.
**Complexity**: Very high. Requires integrated FEA solver or pre-computed stress field input.

### B3: Non-planar slicing

**Concept**: Instead of flat horizontal layers, slice along curved surfaces that follow the model geometry. Eliminates staircase artifacts on low-angle surfaces.
**How**: Generate non-planar layers as NURBS surfaces → project toolpaths onto these surfaces → output 5-axis G-code (or approximate with 3-axis by tilting the print head within safe limits).
**Impact**: Dramatically better surface finish on top surfaces of organic models. Eliminates the need for ironing on gentle slopes.
**Complexity**: Very high. Requires collision detection (nozzle vs. already-printed geometry), 3-axis height-map approach is feasible but limited.

### B4: Adaptive resolution contour slicing

**Concept**: Vary the XY resolution of contour extraction per-region. Fine resolution (0.01mm) on detailed features, coarse resolution (0.1mm) on large flat walls.
**How**: Analyze contour curvature → adaptive sampling during mesh-plane intersection → fewer points on straight segments, more on curves.
**Impact**: Faster slicing (fewer points to process) with equal or better quality. Currently our contour extraction uses uniform resolution.

### B5: Convex decomposition for optimal perimeter ordering

**Concept**: Decompose each layer's polygon into convex regions → generate perimeters per convex region → stitch together with minimal travel. Avoids the problem of standard offsetting creating self-intersecting perimeters on concave geometry.
**How**: Hertel-Mehlhorn or DCEL-based convex decomposition → per-region perimeter generation → traveling-salesman ordering.
**Impact**: Better perimeter quality on complex concave geometry (interlocking parts, text cutouts, lattice structures).

### B6: Continuous toolpath planning (TSP-based)

**Concept**: Instead of generating perimeters/infill independently per region and connecting with travel moves, solve the entire layer as a single optimization problem: minimize non-printing travel while maintaining print order constraints.
**How**: Model as an asymmetric TSP with precedence constraints (outer before inner, or vice versa) → use LKH or similar heuristic solver.
**Impact**: 10-30% reduction in travel moves and retractions → faster prints, fewer stringing artifacts.

### B7: Gradient infill

**Concept**: Smoothly vary infill density from the shell inward, rather than a single uniform density. Dense near walls (good bonding), sparse in center (material savings).
**How**: Distance field from outer shell → map distance to density → modify infill line spacing continuously.
**Impact**: Better strength-to-weight ratio than uniform infill. Cura has a plugin for this; native support would be better.

### B8: Coasting and wipe optimization

**Concept**: AI/ML-trained coasting distances and wipe patterns that adapt to extrusion system characteristics (bowden vs. direct drive, filament type, speed).
**How**: Train a small model on pressure advance behavior → predict optimal coast distance per speed/material/extrusion-system combination.
**Impact**: Eliminates the need for manual coasting tuning — different for every printer/filament combo.

## Implementation Priority

**High value, buildable now (extends existing code):**
1. A1 (smooth width transitions) — refine existing arachne.rs
2. A3 (junction handling) — biggest visible artifact fix
3. B4 (adaptive contour resolution) — speed win in slicecore-slicer
4. B7 (gradient infill) — extends existing infill system

**High value, significant research needed:**
5. B1 (Fermat spirals) — revolutionary but complex
6. B6 (TSP-based toolpath) — measurable improvement
7. B2 (stress-aware alignment) — functional parts differentiator

**Moonshot:**
8. B3 (non-planar slicing) — hardware-dependent, but visually stunning results
