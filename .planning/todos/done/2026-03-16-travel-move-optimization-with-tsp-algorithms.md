---
created: 2026-03-16T19:10:00.000Z
title: Travel move optimization with TSP algorithms
area: engine
files:
  - crates/slicecore-engine/src/toolpath.rs
  - crates/slicecore-engine/src/planner.rs
  - crates/slicecore-engine/src/gcode_gen.rs
  - crates/slicecore-engine/src/seam.rs
---

## Problem

Travel moves (non-printing repositioning) can account for 10-30% of total print time, especially with:
- Multiple objects on the plate (travel between objects every layer)
- Complex perimeter/infill ordering (many short hops)
- Retraction overhead on every travel (retract → travel → unretract → prime)

Phase 27 implemented auto-arrangement for good spatial packing, but the **toolpath ordering** within and between objects per layer is where the real travel optimization happens. This is fundamentally a Traveling Salesman Problem (TSP) variant.

### Current state

The engine currently uses simple heuristics for toolpath ordering:
- Nearest-neighbor for island ordering within a layer
- Basic perimeter ordering (inner→outer or outer→inner)
- No global optimization across all features in a layer

## TSP Algorithm Candidates

### Tier 1: Fast heuristics (microseconds per layer)

| Algorithm | Quality | Speed | Description |
|-----------|---------|-------|-------------|
| **Nearest Neighbor (NN)** | ~75% optimal | O(n²) | Current approach. Pick closest unvisited island. Simple but gets stuck in bad local optima. |
| **Greedy Edge Insertion** | ~80% optimal | O(n² log n) | Build tour by adding shortest edges that don't create cycles. Better than NN for clustered geometry. |
| **Christofides-like** | ~85% optimal | O(n³) | Minimum spanning tree + minimum weight matching. Guaranteed within 1.5x optimal for metric TSP. |

### Tier 2: Local search improvement (milliseconds per layer)

| Algorithm | Quality | Speed | Description |
|-----------|---------|-------|-------------|
| **2-opt** | ~90% optimal | O(n²) per pass | Iteratively reverse segments to uncross edges. Standard post-processing for any initial tour. |
| **3-opt** | ~93% optimal | O(n³) per pass | More powerful segment reversal. Diminishing returns vs. 2-opt cost. |
| **Or-opt** | ~91% optimal | O(n²) per pass | Move segments of 1-3 nodes to better positions. Good complement to 2-opt. |
| **Lin-Kernighan (LK)** | ~95% optimal | Variable | Variable-depth search. The gold standard for TSP heuristics. Complex to implement but excellent results. |

### Tier 3: Metaheuristics (milliseconds-seconds per layer)

| Algorithm | Quality | Speed | Description |
|-----------|---------|-------|-------------|
| **LKH (Lin-Kernighan-Helsgott)** | ~98% optimal | Seconds | Best known TSP heuristic. Available as C library. Would need FFI or Rust port. |
| **Simulated Annealing** | ~92% optimal | Configurable | Random perturbations with decreasing acceptance of worse solutions. Easy to implement, tunable time budget. |
| **Genetic Algorithm** | ~90% optimal | Configurable | Population-based search. Good for exploring diverse solutions but slower convergence. |

## The Slicer-Specific TSP Variant

Our problem is not pure TSP — it has constraints that make it an **Asymmetric TSP with Precedence Constraints (ATSP-PC)**:

### Constraints

1. **Precedence**: Outer perimeters before/after inner (configurable). Infill before/after perimeters. These are hard constraints.
2. **Asymmetric costs**: Travel from A→B may differ from B→A because:
   - Retraction state differs (some moves need retraction, others don't)
   - Combing (travel within printed area) has different path lengths depending on direction
3. **Start/end points matter**: Each extrusion path has a start and end. The travel cost depends on which end we approach from — effectively doubling the node count.
4. **Seam alignment**: The start point of outer perimeters is constrained by seam placement (Phase 4). This further constrains the TSP.
5. **Retraction cost is non-linear**: First retraction in a sequence is ~100ms, but if we can chain non-retract travels (combing), we save significantly.

### Multi-level optimization

```
Level 1: Inter-object ordering (which object next?)
  └── Level 2: Per-object feature ordering (which perimeter/infill next?)
        └── Level 3: Per-feature start point selection (which end to approach?)
              └── Level 4: Travel path routing (combing vs. retract-travel)
```

## Recommended Implementation

### Phase 1: 2-opt improvement on current NN (quick win)

**What**: After nearest-neighbor ordering, run 2-opt local search to uncross obviously bad travel paths.
**Expected improvement**: 10-20% travel reduction on multi-object plates.
**Effort**: Low — 50-100 lines added to toolpath.rs.
**When to run**: Per-layer, after all features are generated but before G-code encoding.

### Phase 2: Greedy + 2-opt + Or-opt (solid improvement)

**What**: Replace NN with greedy edge insertion, then polish with 2-opt + Or-opt.
**Expected improvement**: 20-35% travel reduction.
**Effort**: Medium — new module for TSP solvers.
**Key insight**: Run optimization on feature groups (all perimeters of one island, then all infill of that island) rather than individual moves. Reduces n from thousands to tens/hundreds per layer.

### Phase 3: Start-point optimization with seam awareness

**What**: For each extrusion path, choose the start point (and thus travel endpoint) that minimizes travel from the previous feature while respecting seam constraints.
**Expected improvement**: Additional 5-15% on top of Phase 2.
**Effort**: Medium — requires integrating seam logic with TSP solver.
**Key insight**: The seam and TSP problems are coupled. The optimal seam position depends on where we're coming from, and the optimal tour depends on seam positions.

### Phase 4: LK-style variable-depth search (advanced)

**What**: Full Lin-Kernighan implementation or Rust port.
**Expected improvement**: Additional 5-10% on top of Phase 3, approaching optimal.
**Effort**: High — LK is complex to implement correctly.
**Alternative**: Use `lkh` C library via FFI if pure-Rust proves too costly.

### Phase 5: Combing-aware cost model

**What**: Instead of Euclidean distance, use actual combing path length as the travel cost in the TSP. Short Euclidean distances across a gap may actually require retraction, while longer routes through printed area may be faster.
**Expected improvement**: Better retraction decisions, smoother prints, less stringing.
**Effort**: Medium — requires computing combing paths during TSP evaluation.

## Benchmarking Plan

Test suite for measuring travel optimization:

| Test case | Objects | Islands/layer | Expected travel % |
|-----------|---------|---------------|-------------------|
| Single calibration cube | 1 | 1-2 | 3-5% (baseline) |
| 4 calibration cubes | 4 | 4-8 | 15-25% |
| Plate of 20 small parts | 20 | 20-40 | 25-35% |
| Single complex model (lots of islands) | 1 | 50+ | 10-20% |
| Benchy fleet (6 boats) | 6 | 6-12 | 20-30% |

**Metrics**: Total travel distance (mm), total travel time (s), retraction count, % of print time that is travel.

Compare: NN baseline → each optimization phase → theoretical optimal (for small instances, solve exact with branch-and-bound).

## Ties to other todos

- **Full pipeline parallelization**: TSP solving per layer is independent — trivially parallelizable with rayon
- **Arc fitting (G2/G3)**: Reduces G-code size but doesn't affect travel ordering
- **Sequential printing**: Travel optimization is even more critical for sequential mode where the head must clear objects entirely
