---
created: 2026-03-16T19:05:00.000Z
title: Non-planar slicing and generative design structural optimization
area: engine
files:
  - crates/slicecore-slicer/src/layer.rs
  - crates/slicecore-slicer/src/contour.rs
  - crates/slicecore-engine/src/engine.rs
  - crates/slicecore-engine/src/gcode_gen.rs
  - crates/slicecore-mesh/src/bvh.rs
---

## Problem

Standard planar slicing has two fundamental limitations:

1. **Staircase effect**: Flat layers on curved surfaces create visible stepping. Ironing and small layer heights mitigate but never eliminate this.
2. **Z-axis weakness**: Interlayer adhesion is always the weakest bond. Parts break along layer lines under load. This makes FDM unsuitable for many functional applications.

Non-planar slicing and generative design address both limitations at the algorithm level, producing parts with superior surface finish and mechanical properties.

## Part 1: Non-Planar (NP) and Curved Layer Slicing

### Concept

Instead of flat horizontal layers, the print head follows 3D contoured surfaces. Filament is deposited along the curvature of the part, resulting in:
- Smooth surfaces without staircase artifacts
- Continuous fiber paths across curved geometry (better strength)
- Reduced layer count on gentle slopes

### Technical Approach

**3-Axis NP (feasible with standard printers):**
The nozzle moves in Z during XY travel, following a height map. Limited by nozzle/heater block collision with already-printed geometry.

```
Standard planar:          Non-planar:
  ___________              ___________
 |___________|            /           \
 |___________|           /    curved   \
 |___________|          /    layers     \
 |___________|         /                 \
```

**Algorithm pipeline:**
1. **Surface analysis**: Identify top surfaces with slope angle < threshold (typically < 45° from horizontal for 3-axis)
2. **Height field generation**: Compute Z displacement field across each layer's XY extent
3. **Collision detection**: Using the mesh BVH (Phase 29), verify nozzle clearance at every point:
   - Nozzle cone geometry (length, taper angle)
   - Heater block dimensions
   - Previously printed layer geometry
   - Build Minkowski sum of nozzle geometry with toolpath for swept volume check
4. **Toolpath generation**: Use Dijkstra's or A* on a grid/graph to find paths that:
   - Follow the height field smoothly
   - Minimize fiber cutting (directional changes)
   - Maintain printable slope angles
   - Avoid collisions
5. **G-code output**: Standard G1 moves with varying Z per move. No firmware changes needed for 3-axis NP.

**5-Axis NP (future, requires specialized hardware):**
Print head tilts to remain perpendicular to the deposition surface. Requires either a tilting print head or a tilting build plate. Eliminates most collision constraints but needs specialized kinematics.

### Collision Avoidance — The Hard Problem

The primary engineering challenge. Must account for:

| Component | Typical Dimensions | Constraint |
|-----------|-------------------|-----------|
| Nozzle tip | 0.4mm diameter | Must not contact printed material |
| Nozzle cone | ~15mm tall, ~10° taper | Clearance above nozzle tip |
| Heater block | ~20mm × 16mm × 12mm | Widest component, limits NP angle |
| Part cooling fan/duct | Variable, ~30mm | Often the actual limiting factor |
| Bowden tube (if present) | ~4mm diameter, flexible | Minor constraint |

**Safe NP zone**: Only the topmost few layers of a print can use NP paths. Below that, previously printed geometry creates collision risk. The algorithm must determine the maximum safe NP depth per region.

**Approaches:**
- Conservative: NP only on the very top surface (1-3 layers). Simple, safe, still eliminates staircase on top.
- Aggressive: NP on any surface meeting slope/clearance criteria. Requires full swept-volume collision checking.
- Hybrid: Planar for bulk, NP for final top layers. Best of both worlds.

### Research References

- Ahlers et al., "3D Printing of Nonplanar Layers" (2019) — 3-axis NP on standard Prusa
- Bi et al., "Continuous Contour-Zigzag Hybrid Toolpath" (2020) — path optimization
- Chakraborty et al., "Curved Layer FDM" (2008) — original curved layer concept
- PrusaSlicer experimental NP branch (never merged to stable)

## Part 2: Generative Design and Load-Path Optimization

### Concept

The slicer doesn't just slice what it's given — it participates in design by optimizing internal structure based on load conditions. User provides:
- The mesh (outer shell geometry)
- Load conditions (where forces are applied, where it's fixed)
- Constraints (max weight, min safety factor)

The slicer generates:
- Optimized infill density distribution (dense where stressed, sparse where not)
- Optimized infill orientation (aligned with principal stress directions)
- Wall thickness recommendations
- Optional topology-optimized internal geometry (lattice/truss structures)

### Technical Approach

**Level 1: Heuristic stress-aware infill (no FEA)**
- Analyze geometry to infer likely load paths (thin sections = high stress, thick sections = distributed)
- Use distance from outer shell + section thickness as proxy for stress
- Apply gradient infill density (e.g., 50% near thin walls, 15% in thick centers)
- Low complexity, no external solver needed

**Level 2: Simplified FEA integration**
- Voxelize the mesh at infill resolution
- Apply boundary conditions (user-specified or AI-inferred)
- Run simplified linear FEA (element-free Galerkin or lattice-spring model)
- Map von Mises stress field to infill density: `density = min_density + (max_density - min_density) * stress / max_stress`
- Map principal stress directions to infill line angles per voxel region
- Medium complexity, could use a lightweight Rust FEA crate

**Level 3: Full topology optimization**
- SIMP (Solid Isotropic Material with Penalization) or level-set topology optimization
- Generate internal truss/lattice structures optimized for the specific load case
- Output as modified mesh that the standard slicer processes
- High complexity, requires serious numerical solver

**Level 4: AI-guided generative design**
- LLM analyzes the model + load description in natural language
- Suggests structural modifications: "Add a 2mm fillet at this stress concentration", "This section should be solid, not infilled"
- Uses the FEA results as context for recommendations
- Generates modified mesh or per-region override instructions

### Comparative Strategy Table

| Strategy | Primary Benefit | Target Application | Complexity |
|----------|----------------|-------------------|-----------|
| Planar slicing | Simple, reliable, fast | General purpose, blocks | Low |
| Non-planar (NP) | Superior finish, isotropic strength | Curves, thin shells, wings | High (collision detection) |
| Variable layer height | Speed vs. quality balance | Sloped surfaces, organic shapes | Moderate (already implemented) |
| Generative infill | Weight reduction, load handling | Functional mechanical parts | High (FEA required) |
| Arc generation (G2/G3) | Smoother circular features | Holes, gears, cylinders | Low |
| Stress-aware orientation | Directional strength | Loaded functional parts | High (FEA + AI) |

## Implementation Phasing

### Phase A: NP top surface only (3-axis, conservative)
- NP only on the topmost 1-3 layers
- Simple height-field following, no complex collision avoidance
- **Impact**: Eliminates staircase on top surfaces — the most visible quality issue
- **Effort**: Medium. Modifies G-code gen to add Z-varying moves on top layers.

### Phase B: Full 3-axis NP with collision avoidance
- NP on any qualifying surface with swept-volume collision checking
- Uses BVH for efficient collision queries
- **Impact**: Smooth surfaces on all gentle slopes
- **Effort**: High. The collision detection is the bulk of the work.

### Phase C: Heuristic stress-aware infill
- No FEA solver needed — geometry-based stress estimation
- Gradient infill density from shell distance
- **Impact**: Better strength-to-weight without user effort
- **Effort**: Medium. Modifies infill generation to accept density field.

### Phase D: FEA-integrated generative infill
- Lightweight FEA solver, stress-to-density mapping
- CLI: `slicecore slice model.stl --optimize-for-load load-case.json`
- **Impact**: Engineered parts competitive with injection molding
- **Effort**: Very high. Requires FEA solver integration.

## Dependencies

- **Phase 25 (Parallel slicing)**: ✓ NP benefits from parallel layer processing
- **Phase 29 (CSG/BVH)**: ✓ BVH needed for collision avoidance
- **Adaptive layer height** (slicecore-slicer): ✓ Already implemented — NP extends this concept
- **Arc fitting** (todo): Complements NP for circular features
- **AI integration** (Phase 8): ✓ Needed for Level 4 generative design
