# Phase 41: Travel Move Optimization with TSP Algorithms - Research

**Researched:** 2026-03-20
**Domain:** TSP heuristics for toolpath ordering in 3D printing slicers
**Confidence:** HIGH

## Summary

This phase implements travel move optimization using TSP (Travelling Salesman Problem) heuristics to reorder printable elements within each layer, minimizing non-extrusion travel distance. The core algorithms are nearest-neighbor (NN), greedy edge insertion, and 2-opt local improvement. The existing codebase already has a basic nearest-neighbor ordering for infill lines (`nearest_neighbor_order` in `toolpath.rs:431`), which serves as both a starting point and a baseline for benchmarking.

The implementation is self-contained: a new `travel_optimizer.rs` module in the `slicecore-engine` crate. No external TSP crates are needed -- the algorithms are straightforward (NN is O(n^2), greedy edge insertion is O(n^2 log n), 2-opt is O(n^2) per pass) and the node counts per layer are small (typically 5-50 nodes for multi-object plates, rarely exceeding 200). Hand-rolling gives full control over the asymmetric distance matrix model (entry/exit points differ for open vs closed paths) which generic TSP libraries do not handle well.

**Primary recommendation:** Implement as a standalone module with a clean `TspSolver` trait-like interface. Model nodes with entry/exit points. Precompute the full distance matrix for small node counts (< 200), which fits easily in cache. Use the `Auto` algorithm pipeline: compute both NN and greedy initial tours, pick shorter, apply 2-opt refinement.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Full layer ordering: reorder all printable elements within a layer -- contour visit order, infill line ordering, gap fill path ordering
- All feature types included: perimeters, infill, gap fill, support, support interface, ironing, brim, skirt, purge tower
- Keep feature groups: maintain perimeters-first-then-gap-fills-then-infill ordering within each group. Optimize ordering WITHIN each feature group, not across groups
- Individual contours are TSP nodes (not object-level grouping)
- Allow direction reversal on open paths (infill lines)
- Always enter closed paths (perimeters) at seam point
- Preserve wall order (inner-first vs outer-first) within each contour
- Cross-object travel optimized on multi-object plates
- Run on every layer uniformly
- Add print ordering strategy: ByLayer vs ByObject
- Pipeline: compute both NN AND greedy edge insertion, pick shorter, apply 2-opt
- Generalized 2-opt with path direction reversal
- 2-opt has iteration limit (max improvement passes, no time-based budget)
- New `travel_optimizer.rs` module in slicecore-engine crate
- TSP nodes with entry/exit points: closed paths entry=exit=seam, open paths two endpoints
- Parallelizable with rayon via `parallel` feature flag
- Algorithm enum: Auto, NearestNeighbor, GreedyEdgeInsertion, NearestNeighborOnly, GreedyOnly
- Default: Auto
- Extensible enum for future algorithms (`#[non_exhaustive]`)
- `TravelOptConfig` nested struct in `PrintConfig`
- Fields: enabled (bool, default true), algorithm (enum, default Auto), max_iterations (u32), optimize_cross_object (bool, default true)
- CLI override: `--no-travel-opt` flag
- Criterion benchmarks using existing infrastructure
- Integration test assertions: >= 20% travel reduction on multi-object plates
- Travel stats in SliceResult: total_travel_distance, optimized_travel_distance, travel_reduction_percent
- Synthetic test models: 4-object grid, 9-object grid, scattered, varying sizes

### Claude's Discretion
- Exact max_iterations default value (determine through benchmarking)
- Whether to 2-opt both initial tours or only the shorter (may use node count threshold)
- print_order field placement (TravelOptConfig vs top-level PrintConfig)
- Greedy edge insertion implementation details
- Distance matrix computation strategy (precompute full matrix vs lazy evaluation)
- Curated real-world model selection for benchmarks
- 2-opt convergence detection heuristics

### Deferred Ideas (OUT OF SCOPE)
- Combing / avoid-crossing-perimeters
- Wipe-on-retract awareness
- Travel stats in analyze-gcode
- Or-opt / 3-opt / Lin-Kernighan
- Travel path routing
- Simulated annealing / genetic algorithms
- Per-feature-type optimization weights
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| slicecore-engine | (workspace) | Host crate for travel_optimizer.rs | Already contains toolpath.rs, config.rs |
| criterion | (workspace) | Benchmark framework | Already used in Phase 37 benchmarks |
| rayon | (workspace) | Per-layer parallelism | Already gated behind `parallel` feature |
| serde | (workspace) | Config serialization | Already used for all config structs |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| slicecore-config-derive | (workspace) | `#[derive(SettingSchema)]` for config enums | TravelOptAlgorithm enum, TravelOptConfig |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-rolled TSP | ibn-battuta crate | External crate does not handle asymmetric entry/exit point model; adds dependency for simple algorithms |
| Full distance matrix | Lazy evaluation | Full matrix is O(n^2) memory but n < 200 per layer; fits in L1 cache; avoids repeated sqrt calls |
| 2-opt only on best tour | 2-opt on both tours | For n < 30, 2-opting both is negligible cost; for n > 30, only 2-opt the shorter one |

**Installation:** No new dependencies needed. All libraries already in workspace.

## Architecture Patterns

### Recommended Project Structure
```
crates/slicecore-engine/src/
    travel_optimizer.rs    # NEW: TSP algorithms, TspNode, distance matrix
    travel_optimizer/      # Alternative: module directory if file gets large
        mod.rs
        nearest_neighbor.rs
        greedy_edge.rs
        two_opt.rs
    config.rs              # MODIFY: Add TravelOptConfig, TravelOptAlgorithm
    toolpath.rs            # MODIFY: Call optimizer after assembling feature groups
    engine.rs              # MODIFY: Wire optimizer into pipeline, add travel stats
    statistics.rs          # MODIFY: Add travel optimization stats to SliceResult
    lib.rs                 # MODIFY: pub mod travel_optimizer, re-exports
```

### Pattern 1: TSP Node Abstraction
**What:** Model each printable element as a TSP node with entry and exit points.
**When to use:** Always -- this is the core abstraction.
**Example:**
```rust
/// A node in the TSP problem representing a printable element.
#[derive(Debug, Clone)]
pub struct TspNode {
    /// Entry point (where the nozzle arrives to start printing).
    pub entry: Point2,
    /// Exit point (where the nozzle is after printing this element).
    pub exit: Point2,
    /// Whether this is an open path (can be reversed).
    pub reversible: bool,
    /// Index back to the original element for reordering.
    pub original_index: usize,
}
```

For closed paths (perimeters): `entry == exit == seam_point`.
For open paths (infill lines, gap fills): `entry == start`, `exit == end`.
When reversed: swap entry and exit.

### Pattern 2: Distance Matrix
**What:** Precompute all pairwise exit-to-entry distances.
**When to use:** Always for n < 200 nodes (covers all practical cases).
**Example:**
```rust
/// Precomputed distance matrix for TSP.
/// dist[i][j] = distance from exit of node i to entry of node j.
/// For reversible nodes, also consider reversed entry points.
struct DistanceMatrix {
    /// n x n matrix stored as flat Vec for cache efficiency.
    distances: Vec<f64>,
    /// n x n matrix for reversed-entry distances (only meaningful for reversible nodes).
    distances_reversed: Vec<f64>,
    n: usize,
}

impl DistanceMatrix {
    fn dist(&self, from: usize, to: usize) -> f64 {
        self.distances[from * self.n + to]
    }
    fn dist_reversed(&self, from: usize, to: usize) -> f64 {
        self.distances_reversed[from * self.n + to]
    }
}
```

Use `dist_squared` internally and only take sqrt for the final tour length computation. This avoids n^2 sqrt calls during matrix construction.

### Pattern 3: Nearest-Neighbor Construction
**What:** Greedy construction starting from position 0 (or nozzle position), always picking closest unvisited node.
**When to use:** Fast O(n^2) initial solution.
**Key detail:** For reversible nodes, consider both orientations (entry and reversed entry) when computing "closest".
```rust
fn nearest_neighbor(matrix: &DistanceMatrix, nodes: &[TspNode], start_pos: Point2) -> Tour {
    // For each unvisited node, compute min(dist_to_entry, dist_to_reversed_entry)
    // Pick the closest, mark visited, update current position to exit (or reversed exit)
}
```

### Pattern 4: Greedy Edge Insertion
**What:** Sort all possible edges by distance, greedily add shortest edges that don't violate degree constraints (each node has at most one predecessor and one successor) or create premature cycles.
**When to use:** Often produces better initial solutions than NN for clustered nodes.
**Key detail:** Use a union-find (disjoint set) structure to efficiently detect cycle formation. Each node can have degree at most 2 (one in-edge, one out-edge).
```rust
fn greedy_edge_insertion(matrix: &DistanceMatrix, nodes: &[TspNode]) -> Tour {
    // 1. Generate all edges (i, j, distance) including reversed variants
    // 2. Sort edges by distance ascending
    // 3. For each edge: add if node degrees allow and no premature cycle
    // 4. Connect remaining path fragments into a tour
}
```

### Pattern 5: 2-opt Improvement
**What:** Iteratively try reversing sub-sequences of the tour. If reversing a segment reduces total distance, apply the swap.
**When to use:** After initial construction (NN or greedy).
**Key detail:** Generalized 2-opt for this problem must also consider direction reversal of reversible nodes at swap boundaries.
```rust
fn two_opt_improve(tour: &mut Tour, matrix: &DistanceMatrix, max_iterations: u32) -> bool {
    let mut improved = true;
    let mut iterations = 0;
    while improved && iterations < max_iterations {
        improved = false;
        iterations += 1;
        for i in 0..tour.len() - 1 {
            for j in i + 2..tour.len() {
                let delta = compute_swap_delta(tour, matrix, i, j);
                if delta < -EPSILON {
                    tour.reverse_segment(i + 1, j);
                    improved = true;
                }
            }
        }
    }
    improved
}
```

### Pattern 6: Config Struct (follow existing patterns)
**What:** Nested config struct with `#[serde(default)]` and `#[derive(SettingSchema)]`.
**Example:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Travel")]
pub struct TravelOptConfig {
    #[setting(tier = 3, description = "Enable travel move optimization")]
    pub enabled: bool,
    #[setting(tier = 4, description = "TSP algorithm selection")]
    pub algorithm: TravelOptAlgorithm,
    #[setting(tier = 4, description = "Maximum 2-opt improvement iterations")]
    pub max_iterations: u32,
    #[setting(tier = 4, description = "Optimize travel between objects on same layer")]
    pub optimize_cross_object: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, SettingSchema)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum TravelOptAlgorithm {
    #[default]
    Auto,
    NearestNeighbor,
    GreedyEdgeInsertion,
    NearestNeighborOnly,
    GreedyOnly,
}
```

### Pattern 7: Print Ordering Strategy
**What:** Enum controlling whether features are printed by-layer (all objects' perimeters, then all infill) or by-object (all features for object A, then object B per layer).
**Recommendation:** Place `print_order` in `TravelOptConfig` since it directly affects travel optimization behavior.
```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, SettingSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrintOrder {
    #[default]
    ByLayer,
    ByObject,
}
```

### Anti-Patterns to Avoid
- **Computing sqrt in inner loops:** Use squared distances for comparisons; only compute sqrt for final tour length reporting.
- **Allocating per-iteration in 2-opt:** Pre-allocate the tour vector; reverse in-place.
- **Modifying toolpath segment order directly:** Build the optimized ordering as index permutation first, then reorder segments in one pass.
- **Running optimizer on single-node layers:** Short-circuit when n <= 1 (no optimization possible).
- **Breaking seam selection:** The optimizer must NOT change which vertex a perimeter starts at -- it only changes the order in which perimeters are visited.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Union-find for greedy insertion | Manual linked list tracking | Simple `Vec<usize>` union-find with path compression | O(alpha(n)) per operation, well-understood |
| Parallel per-layer dispatch | Manual thread spawning | `maybe_par_iter!` macro + rayon | Already established in codebase |
| Config serialization | Manual TOML parsing | `#[serde(default)]` + `#[derive(SettingSchema)]` | Consistent with all other config structs |
| Benchmark harness | Custom timing code | criterion with `black_box()` | Phase 37 established the pattern |

**Key insight:** The TSP algorithms themselves ARE worth hand-rolling because: (1) node counts are small (5-200), (2) the asymmetric entry/exit point model is non-standard, (3) no external crate handles this specific problem shape well.

## Common Pitfalls

### Pitfall 1: Asymmetric Distance Matrix
**What goes wrong:** Standard TSP assumes symmetric distances (dist(A,B) == dist(B,A)). With entry/exit points, this is NOT true: the distance from node A's exit to node B's entry differs from B's exit to A's entry.
**Why it happens:** Open paths have different start and end points. A perimeter's exit is its seam point, but reaching it from different directions has the same distance -- so perimeters are symmetric but infill lines are not.
**How to avoid:** Always compute dist[i][j] as `distance(nodes[i].exit, nodes[j].entry)`. Never assume symmetry.
**Warning signs:** Tests show different tour lengths depending on starting node when they should be equivalent.

### Pitfall 2: 2-opt Segment Reversal Invalidates Open Path Directions
**What goes wrong:** When 2-opt reverses a segment of the tour, the nodes in that segment are visited in reverse order. For reversible open paths, this means their entry/exit must also be swapped. Forgetting this produces incorrect distance calculations.
**Why it happens:** Standard 2-opt tutorials assume nodes are points, not directed segments.
**How to avoid:** When reversing tour[i+1..=j], also flip the `reversed` flag on each node in that range. Recompute distances using the correct entry/exit points.
**Warning signs:** 2-opt "improvements" that actually increase travel distance.

### Pitfall 3: Greedy Edge Insertion Cycle Detection
**What goes wrong:** Adding an edge that creates a cycle before all nodes are connected produces a disconnected tour.
**Why it happens:** Without proper cycle detection, the greedy algorithm can close a sub-tour prematurely.
**How to avoid:** Use union-find. Only allow an edge (u, v) if: u's out-degree < 1, v's in-degree < 1, and u and v are NOT in the same connected component (unless this is the final edge connecting all nodes).
**Warning signs:** Tour with fewer nodes than expected, or multiple disconnected paths.

### Pitfall 4: Modifying Toolpath Assembly Order
**What goes wrong:** The optimizer should reorder which contours to visit next within a feature group, NOT change the assembly of segments within a contour.
**Why it happens:** Confusion between "reorder the contours" and "reorder the segments within a contour."
**How to avoid:** The optimizer takes a list of feature groups (e.g., list of perimeter contours), returns an ordered index permutation. The assembly function uses this permutation to emit segments in the optimized order.
**Warning signs:** Perimeters with broken seam points or incorrect wall ordering.

### Pitfall 5: Benchmark Baseline Comparison
**What goes wrong:** Comparing optimized vs unoptimized travel without controlling for the same input produces misleading results.
**Why it happens:** Travel distance depends on the exact perimeter/infill layout, which varies with config.
**How to avoid:** Compute baseline travel distance from the SAME assembled toolpath before optimization. Store both values in stats. The integration test asserts on the ratio, not absolute values.
**Warning signs:** Travel reduction percentages that vary wildly between runs.

### Pitfall 6: Single-Node and Two-Node Edge Cases
**What goes wrong:** Algorithms crash or produce incorrect results for degenerate inputs.
**Why it happens:** Most layers have many nodes, but some (e.g., very first/last layers with only brim) may have 0 or 1 nodes per feature group.
**How to avoid:** Short-circuit: n=0 returns empty, n=1 returns identity, n=2 just compares forward vs reverse.
**Warning signs:** Panics on simple test models with few features.

## Code Examples

### Integrating Optimizer into Toolpath Assembly

The optimizer sits between feature group collection and travel insertion:

```rust
// In assemble_layer_toolpath or a wrapper:

// 1. Collect perimeter contours as TspNodes
let perim_nodes: Vec<TspNode> = perimeters.iter().enumerate().map(|(i, cp)| {
    let seam_pt = /* seam point for this contour */;
    TspNode {
        entry: seam_pt,
        exit: seam_pt,  // closed path: entry == exit
        reversible: false,
        original_index: i,
    }
}).collect();

// 2. Optimize ordering
let optimized_order = if config.travel_opt.enabled && perim_nodes.len() > 1 {
    optimize_tour(&perim_nodes, current_pos, &config.travel_opt)
} else {
    (0..perim_nodes.len()).collect()
};

// 3. Emit perimeter segments in optimized order
for &idx in &optimized_order {
    emit_perimeter_segments(&perimeters[idx], ...);
}
```

### Computing Travel Stats

```rust
/// Travel optimization statistics for a slice result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TravelOptStats {
    /// Total travel distance before optimization in mm.
    pub baseline_travel_distance: f64,
    /// Total travel distance after optimization in mm.
    pub optimized_travel_distance: f64,
    /// Percentage reduction: (baseline - optimized) / baseline * 100.
    pub travel_reduction_percent: f64,
}
```

### Union-Find for Greedy Edge Insertion

```rust
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }

    fn union(&mut self, x: usize, y: usize) -> bool {
        let rx = self.find(x);
        let ry = self.find(y);
        if rx == ry { return false; }
        if self.rank[rx] < self.rank[ry] {
            self.parent[rx] = ry;
        } else if self.rank[rx] > self.rank[ry] {
            self.parent[ry] = rx;
        } else {
            self.parent[ry] = rx;
            self.rank[rx] += 1;
        }
        true
    }

    fn connected(&mut self, x: usize, y: usize) -> bool {
        self.find(x) == self.find(y)
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Fixed ordering (as-generated) | Nearest-neighbor ordering for infill only | Phase 3 (current) | Basic optimization, only covers infill |
| Single NN algorithm | Dual construction (NN + greedy) with 2-opt | This phase | 20-35% travel reduction on multi-object plates |
| No travel statistics | Full travel stats in SliceResult | This phase | Enables benchmarking and regression detection |

**Production slicer approaches:**
- PrusaSlicer: Uses nearest-neighbor with some heuristics for perimeter ordering
- OrcaSlicer: Similar to PrusaSlicer, adds some cross-object optimization
- Cura: Uses order-optimization plugin with configurable strategies
- The "try both NN + greedy, pick best, 2-opt" pipeline is state-of-the-art for slicer-grade optimization (more advanced methods like LKH are overkill for n < 200)

## Open Questions

1. **Exact max_iterations default for 2-opt**
   - What we know: For n < 50, 2-opt typically converges in 3-10 passes. For n < 200, 10-50 passes.
   - What's unclear: The optimal default that balances quality vs speed for typical slicing workloads.
   - Recommendation: Start with 100, benchmark, adjust. The convergence check (no improvement) will short-circuit most layers well before hitting the limit.

2. **print_order field placement**
   - What we know: Both TravelOptConfig and top-level PrintConfig are viable.
   - Recommendation: Place in TravelOptConfig. Rationale: print order directly affects travel optimization and is conceptually part of the same feature. Users who care about print ordering also care about travel optimization.

3. **Distance matrix: precompute vs lazy**
   - What we know: For n < 200, a full matrix is ~200*200*8 = 320KB (well within L2 cache). Lazy evaluation saves memory but adds repeated computation.
   - Recommendation: Always precompute for this phase. The node counts are small enough that memory is not a concern, and cache-friendly access patterns make precompute faster.

4. **2-opt one tour vs both**
   - What we know: For n < 30, the cost difference is negligible. For n > 100, 2-opting both doubles optimization time.
   - Recommendation: For n <= 30, 2-opt both initial tours and pick the best result. For n > 30, only 2-opt the shorter initial tour. This threshold can be tuned via benchmarking.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test + criterion 0.5 |
| Config file | crates/slicecore-engine/Cargo.toml (bench entries exist) |
| Quick run command | `cargo test -p slicecore-engine --lib travel_optimizer` |
| Full suite command | `cargo test --workspace && cargo bench -p slicecore-engine` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| (implicit) | NN produces valid tour | unit | `cargo test -p slicecore-engine travel_optimizer::tests::nn_` | Wave 0 |
| (implicit) | Greedy produces valid tour | unit | `cargo test -p slicecore-engine travel_optimizer::tests::greedy_` | Wave 0 |
| (implicit) | 2-opt improves or maintains tour | unit | `cargo test -p slicecore-engine travel_optimizer::tests::two_opt_` | Wave 0 |
| (implicit) | Auto pipeline picks best | unit | `cargo test -p slicecore-engine travel_optimizer::tests::auto_` | Wave 0 |
| (implicit) | >= 20% reduction on 4-object grid | integration | `cargo test -p slicecore-engine travel_reduction_` | Wave 0 |
| (implicit) | >= 20% reduction on 9-object grid | integration | `cargo test -p slicecore-engine travel_reduction_` | Wave 0 |
| (implicit) | Config serialization round-trips | unit | `cargo test -p slicecore-engine config::tests::travel_opt_` | Wave 0 |
| (implicit) | --no-travel-opt disables optimizer | integration | `cargo test -p slicecore-cli no_travel_opt` | Wave 0 |
| (implicit) | TravelOptStats populated in SliceResult | integration | `cargo test -p slicecore-engine travel_stats_` | Wave 0 |
| (implicit) | Criterion benchmark runs | benchmark | `cargo bench -p slicecore-engine -- travel` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-engine --lib travel_optimizer`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green + criterion benchmarks showing >= 20% travel reduction

### Wave 0 Gaps
- [ ] `crates/slicecore-engine/src/travel_optimizer.rs` -- new module with unit tests
- [ ] `crates/slicecore-engine/benches/travel_benchmark.rs` -- criterion benchmark for TSP algorithms
- [ ] `crates/slicecore-engine/tests/travel_reduction.rs` -- integration tests for reduction assertions
- [ ] Bench harness entry in `crates/slicecore-engine/Cargo.toml` for `travel_benchmark`

## Sources

### Primary (HIGH confidence)
- Existing codebase: `toolpath.rs` -- current nearest_neighbor_order implementation, ToolpathSegment/LayerToolpath types
- Existing codebase: `config.rs` -- ScarfJointConfig, SequentialConfig as patterns for TravelOptConfig
- Existing codebase: `statistics.rs` -- PrintStatistics, StatisticsSummary where travel stats integrate
- Existing codebase: `parallel.rs` -- maybe_par_iter! macro for rayon gating
- Existing codebase: `engine.rs` -- SliceResult struct, assemble_layer_toolpath call site
- Existing codebase: `sequential.rs` -- ObjectBounds, print ordering patterns

### Secondary (MEDIUM confidence)
- TSP algorithm complexity and behavior: well-established computer science (Cormen et al. CLRS, standard references)
- Production slicer approaches: based on PrusaSlicer/OrcaSlicer open source code review

### Tertiary (LOW confidence)
- [ibn-battuta crate](https://crates.io/crates/ibn_battuta) -- Rust TSP library (not recommended for use, but validates that NN + 2-opt is standard approach)
- [Rust forum TSP discussion](https://users.rust-lang.org/t/crates-for-solving-the-traveling-salesman-problem/108498) -- confirms no dominant production-ready TSP crate in Rust ecosystem

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all libraries already in workspace, no new dependencies
- Architecture: HIGH - algorithms are well-understood, codebase patterns are clear from existing code
- Pitfalls: HIGH - based on direct code analysis of entry/exit point asymmetry and 2-opt reversal semantics
- Benchmarking: HIGH - criterion infrastructure exists from Phase 37, synthetic model generators exist in slice_benchmark.rs

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable algorithms, stable codebase patterns)
