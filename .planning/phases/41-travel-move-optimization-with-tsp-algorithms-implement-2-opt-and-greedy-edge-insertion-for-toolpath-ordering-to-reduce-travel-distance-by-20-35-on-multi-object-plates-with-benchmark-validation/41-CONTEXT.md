# Phase 41: Travel Move Optimization with TSP Algorithms - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Optimize toolpath ordering within layers to minimize non-extrusion travel distance using TSP heuristics (2-opt and greedy edge insertion). Add a print ordering strategy (by-layer vs by-object-by-layer). Validate with criterion benchmarks and integration test assertions showing >= 20% travel reduction on multi-object plates. Does NOT include travel path routing (combing/avoid-crossing-perimeters), wipe-on-retract, or G-code analysis display changes.

</domain>

<decisions>
## Implementation Decisions

### Optimization Scope
- Full layer ordering: reorder all printable elements within a layer — contour visit order, infill line ordering (replacing current nearest-neighbor), gap fill path ordering
- All feature types included: perimeters, infill, gap fill, support, support interface, ironing, brim, skirt, purge tower
- Keep feature groups: maintain perimeters-first-then-gap-fills-then-infill ordering within each group. Optimize ordering WITHIN each feature group, not across groups
- Individual contours are TSP nodes (not object-level grouping) — on a plate with 5 objects each having 3 contours, that's 15 nodes to optimize
- Allow direction reversal on open paths (infill lines) — consider both endpoints when computing travel
- Always enter closed paths (perimeters) at seam point — optimizer respects seam selection, only optimizes which contour to visit next
- Preserve wall order (inner-first vs outer-first) within each contour — optimizer reorders which contour to visit, not wall sequence within a contour
- Cross-object travel optimized: on multi-object plates, optimize which object's contours to visit next per layer
- Run on every layer uniformly (trivial single-contour layers will be fast no-ops)

### Print Ordering Strategy
- Add a print ordering strategy: ByLayer (current behavior — all perimeters across all objects, then all infill) vs ByObject (complete each object's features per layer before moving to next)
- TSP optimizer optimizes within whichever ordering mode is selected
- This is a new capability complementary to TSP optimization

### Algorithm Design
- Pipeline: compute both initial solutions (nearest-neighbor AND greedy edge insertion), pick the shorter one, then apply 2-opt refinement
- Generalized 2-opt: when evaluating swaps, also consider path direction reversal at the swap point
- 2-opt has iteration limit (max improvement passes, converge or stop — no time-based budget)
- Claude's discretion on whether to 2-opt both initial tours or only the shorter one (may vary by node count)
- New `travel_optimizer.rs` module in slicecore-engine crate
- TSP nodes modeled with entry/exit points: closed paths have entry=exit=seam, open paths have two distinct endpoints. Distance matrix computed from exit-to-entry distances
- Parallelizable with rayon: each layer's optimization runs independently, gated behind existing `parallel` feature flag

### Algorithm Enum
- Full menu of values: Auto (try both NN + greedy, 2-opt best), NearestNeighbor (NN + 2-opt), GreedyEdgeInsertion (greedy + 2-opt), NearestNeighborOnly (NN, no 2-opt), GreedyOnly (greedy, no 2-opt)
- Default: Auto
- Extensible enum for future algorithms (or-opt, 3-opt, Lin-Kernighan)

### Config
- `TravelOptConfig` nested struct in `PrintConfig` (consistent with ScarfJointConfig, RetractConfig pattern)
- Fields: `enabled` (bool, default true), `algorithm` (enum, default Auto), `max_iterations` (u32), `optimize_cross_object` (bool, default true)
- `print_order` field placement: Claude's discretion (inside TravelOptConfig or top-level PrintConfig)
- CLI override: `--no-travel-opt` flag to disable for debugging, in addition to config file

### Benchmarking
- Baseline: current nearest-neighbor ordering in toolpath.rs
- Test models: both synthetic multi-object plates (programmatic: 4-object grid, 9-object grid, scattered, varying sizes) + curated real-world models
- Use criterion benchmarks (existing infrastructure from Phase 37)
- Integration test assertions: assert >= 20% travel reduction on multi-object plates — fails CI if not met
- Travel stats added to SliceResult: total_travel_distance, optimized_travel_distance, travel_reduction_percent

### Claude's Discretion
- Exact max_iterations default value (determine through benchmarking)
- Whether to 2-opt both initial tours or only the shorter (may use node count threshold)
- print_order field placement (TravelOptConfig vs top-level PrintConfig)
- Greedy edge insertion implementation details
- Distance matrix computation strategy (precompute full matrix vs lazy evaluation)
- Curated real-world model selection for benchmarks
- 2-opt convergence detection heuristics

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing toolpath code
- `crates/slicecore-engine/src/toolpath.rs` — Current `assemble_layer_toolpath` function, `nearest_neighbor_order` for infill, `ToolpathSegment` and `LayerToolpath` types, `FeatureType` enum
- `crates/slicecore-engine/src/planner.rs` — `plan_retraction` uses travel distance, skirt/brim generation
- `crates/slicecore-engine/src/gcode_gen.rs` — Converts LayerToolpath to GcodeCommand sequences

### Multi-object and sequential
- `crates/slicecore-engine/src/sequential.rs` — Object ordering for sequential printing (shortest-first), ObjectBounds type
- `crates/slicecore-engine/src/engine.rs` lines 1079-1120 — Multi-object component detection and sequential validation

### Config patterns
- `crates/slicecore-engine/src/config.rs` — PrintConfig with nested structs (ScarfJointConfig, RetractConfig as patterns for TravelOptConfig)

### Statistics
- `crates/slicecore-engine/src/statistics.rs` — Existing SliceStatistics struct where travel stats should be added

### Architecture
- `.planning/codebase/ARCHITECTURE.md` — Workspace structure, crate dependency layering, feature flags

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `nearest_neighbor_order()` in `toolpath.rs:431`: Already implements NN for infill lines with reversal — can be generalized into the TSP module as one algorithm variant
- `distance()` helper in `toolpath.rs:419`: Euclidean distance between Point2 values — reusable for distance matrix computation
- `FeatureType::Travel` segments: Already inserted by `assemble_layer_toolpath` — optimizer replaces the ordering logic that precedes travel insertion
- `rayon` feature flag and `maybe_par_iter` pattern: Existing parallelism infrastructure to gate per-layer optimization

### Established Patterns
- Nested config structs with `#[serde(default)]` and `#[derive(ConfigSchema)]`: Follow for TravelOptConfig
- `thiserror` error enums at crate boundary: Add optimization-specific errors if needed
- Criterion benchmarks in `benches/` directory: Follow Phase 37 patterns for benchmark setup

### Integration Points
- `assemble_layer_toolpath()`: The main function to modify — currently assembles in fixed order (perimeters → gap fill → infill). Optimizer reorders within each group before travel insertion
- `Engine::slice()` / `slice_with_events()`: Where per-layer optimization would be called (after toolpath assembly, before G-code generation)
- `PrintConfig`: Add `TravelOptConfig` struct, `#[derive(ConfigSchema)]` for setting registry
- `SliceStatistics`: Add travel distance metrics
- CLI `main.rs`: Add `--no-travel-opt` global flag

</code_context>

<specifics>
## Specific Ideas

- Algorithm enum should be extensible for future additions (or-opt, 3-opt, Lin-Kernighan) — design with `#[non_exhaustive]` or similar
- "By-object-by-layer" print ordering combines the best of by-object and by-layer approaches from existing slicers — complete all features for each object within a layer before moving to the next
- The "try both initial solutions" approach mirrors how production slicers handle this — greedy NN is fast but greedy edge insertion often produces shorter initial tours
- Future TODO for research: investigate other TSP algorithms and their suitability for toolpath optimization

</specifics>

<deferred>
## Deferred Ideas

- **Combing / avoid-crossing-perimeters** — Travel moves that stay inside printed perimeters to avoid surface defects. Major feature in PrusaSlicer/OrcaSlicer. Significant scope — own phase.
- **Wipe-on-retract awareness** — Optimizer prefers travel paths that allow wiping during retraction. Quality improvement but adds complexity.
- **Travel stats in analyze-gcode** — Extend analyze-gcode command to report travel distance/percentage from existing G-code files.
- **Or-opt / 3-opt / Lin-Kernighan** — More advanced TSP improvement heuristics for future algorithm enum values.
- **Travel path routing** — Not just ordering but actual path planning — routing travel moves around obstacles, shortest path through geometry.
- **Simulated annealing / genetic algorithms** — Alternative metaheuristic approaches for toolpath optimization.
- **Per-feature-type optimization weights** — Prioritize reducing travel between perimeters over infill for quality reasons.

</deferred>

---

*Phase: 41-travel-move-optimization-with-tsp-algorithms*
*Context gathered: 2026-03-20*
