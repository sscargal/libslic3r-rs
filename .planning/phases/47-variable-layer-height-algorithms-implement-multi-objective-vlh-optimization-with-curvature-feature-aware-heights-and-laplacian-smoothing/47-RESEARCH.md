# Phase 47: Variable Layer Height Algorithms - Research

**Researched:** 2026-03-25
**Domain:** Multi-objective VLH optimization, feature-aware height selection, Laplacian smoothing
**Confidence:** HIGH

## Summary

Phase 47 extends the existing single-objective curvature-based adaptive layer height system (`adaptive.rs`) into a multi-objective optimizer with four objectives (quality, speed, strength, material), feature-aware height selection via a pre-pass feature map, Laplacian smoothing for transition continuity, and per-layer diagnostic events. The existing `compute_adaptive_layer_heights` function becomes a thin wrapper calling the new system with quality-only weights.

The codebase already has strong foundations: curvature sampling, forward/backward ratio clamping, overhang detection, bridge detection, hole detection (`is_circular_hole`), a Z-schedule with per-object membership, a `PrintConfig` with `#[derive(SettingSchema)]`, and a pub/sub event bus. The implementation work is primarily algorithmic (new optimizer, feature map, Laplacian smoother) rather than infrastructural.

**Primary recommendation:** Structure as five modules within `slicecore-slicer`: `vlh/objectives.rs` (objective functions), `vlh/features.rs` (feature map pre-pass), `vlh/optimizer.rs` (greedy+DP optimizer), `vlh/smooth.rs` (Laplacian + ratio clamping), and `vlh/mod.rs` (public API + diagnostics). Add new VLH config fields to `PrintConfig` following the existing `adaptive_*` pattern.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Four objectives: visual quality, print speed, mechanical strength, material savings
- Users control balance via weighted sliders (quality, speed, strength, material weights normalized internally)
- Material savings is a soft weight only -- no hard filament budget constraint
- Uniform per-Z height -- each layer has one height applied to all features
- Per-object weights supported on multi-object plates (ZSchedule tracks per-object membership)
- When per-object weights conflict at the same Z, most conservative (thinnest) wins
- Max layer height hard-limited by nozzle diameter (~75% of nozzle diameter)
- Existing `compute_adaptive_layer_heights` refactored as wrapper calling new multi-objective system with quality-only weights
- Two optimization modes: greedy with lookahead (default, fast) and dynamic programming (optional flag)
- Deterministic by default (SLICE-05 compliance); stochastic exploration opt-in via flag
- Perceptual model: weight curvature by surface visibility (external surfaces weighted equally, internal/infill get no quality weight)
- Geometric heuristics for stress zone identification (holes, thin walls, sharp angles, overhangs) -- no FEA
- Pre-pass feature map before VLH optimization, producing per-Z map of detected features
- Four feature types: overhangs near threshold, bridges, thin walls/small features, holes/cylindrical features
- Per-feature configurable influence weights in PrintConfig
- Feature influence extends 2-3 layers above and below detection zone
- Overhang detection uses continuous angle (0-90deg) with configurable sensitivity range
- Thin wall threshold configurable (default: 2x nozzle diameter)
- Hole/cylinder detection reuses `is_circular_hole()` from ADV-07
- Bridge detection queries existing support module's bridge detection functions
- Laplacian smoothing first, then forward/backward ratio clamping as safety net
- Laplacian smoothing strength configurable (0.0-1.0, default ~0.5)
- Default 3-5 Laplacian iterations
- Anchor points preserved during smoothing: first layer height and feature-demanded minimums are pinned
- Per-layer diagnostic output via structured events through existing pub/sub event system
- Diagnostics enabled in debug/verbose mode, off by default

### Claude's Discretion
- Exact DP state space discretization (number of candidate heights per Z)
- Greedy lookahead window size
- Laplacian smoothing kernel shape (uniform vs Gaussian neighbors)
- Curvature sampling resolution improvements
- Internal data structures for the feature map
- Stochastic exploration algorithm choice (simulated annealing, genetic, etc.)

### Deferred Ideas (OUT OF SCOPE)
- Simple FEA-lite stress estimation
- User-painted VLH regions
- Time-budgeted VLH mode
- VLH preview visualization
- Layer height quantization to motor microstep multiples
- Multi-mesh VLH coherence
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SLICE-05 | Deterministic output (same input + config = identical G-code) | Greedy and DP modes must be deterministic by default. No floating-point non-determinism from parallel reduction. Stochastic exploration is opt-in only with a fixed seed. All sorting must be stable. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| slicecore-slicer | (workspace) | Host crate for VLH optimizer | Existing adaptive.rs lives here |
| slicecore-engine | (workspace) | PrintConfig + event system | Config fields and diagnostic events |
| slicecore-mesh | (workspace) | TriangleMesh, normals, AABB | Curvature sampling and feature detection input |
| ordered-float | (workspace dep) | Deterministic float comparisons | Already used in z_schedule.rs for BTreeSet keying |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| slicecore-geo | (workspace) | Polygon operations, offsets | Feature detection (thin wall measurement) |
| slicecore-math | (workspace) | Point3, Vec3, coordinate conversions | Geometric computations |
| serde | (workspace) | Config serialization | New VLH config fields |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom DP optimizer | `nalgebra` for matrix-based DP | Overkill -- DP is 1D array walking, no matrix ops needed |
| Custom Laplacian | Signal processing crate | Overkill -- 1D Laplacian is trivial (weighted neighbor average) |
| Feature map as HashMap | Sorted Vec of (z, features) | Vec with binary search is faster for sequential Z access and more cache-friendly |

No new external dependencies needed. All required infrastructure exists in the workspace.

## Architecture Patterns

### Recommended Project Structure
```
crates/slicecore-slicer/src/
  adaptive.rs           # Existing -- refactored to delegate to vlh/
  vlh/
    mod.rs              # Public API: compute_vlh_heights(), VlhConfig, VlhResult
    objectives.rs       # Objective functions (quality, speed, strength, material)
    features.rs         # Feature map pre-pass (overhang, bridge, thin wall, hole detection)
    optimizer.rs        # Greedy+lookahead and DP optimizer implementations
    smooth.rs           # Laplacian smoothing + ratio clamping safety net
```

### Pattern 1: Multi-Objective Scoring
**What:** Each objective produces a "desired height" at each Z, then a weighted sum combines them.
**When to use:** At every candidate Z position during optimization.

```rust
/// Per-Z objective scores, each mapping curvature/features to a desired height.
pub struct ObjectiveScores {
    /// Quality: thin where curvature is high on external surfaces
    pub quality_height: f64,
    /// Speed: thick everywhere (max_height)
    pub speed_height: f64,
    /// Strength: thin near stress features (holes, thin walls, sharp angles)
    pub strength_height: f64,
    /// Material: thick everywhere (max_height)
    pub material_height: f64,
}

impl ObjectiveScores {
    /// Weighted combination of objectives.
    /// Weights are pre-normalized to sum to 1.0.
    pub fn combine(&self, weights: &VlhWeights) -> f64 {
        weights.quality * self.quality_height
            + weights.speed * self.speed_height
            + weights.strength * self.strength_height
            + weights.material * self.material_height
    }
}
```

### Pattern 2: Feature Map Pre-Pass
**What:** Before optimization, scan the mesh at fine Z intervals to build a map of detected features and their influence on desired layer height.
**When to use:** Once per mesh, before the optimizer runs.

```rust
/// A detected feature at a specific Z range that influences layer height.
pub struct FeatureDetection {
    pub feature_type: FeatureType,
    /// Z range where the feature is detected (before margin extension).
    pub z_min: f64,
    pub z_max: f64,
    /// The desired minimum layer height this feature demands.
    pub demanded_height: f64,
}

pub enum FeatureType {
    Overhang { angle_deg: f64 },
    Bridge,
    ThinWall { width_mm: f64 },
    Hole { diameter_mm: f64 },
}

/// Pre-pass: build feature map from mesh geometry.
/// Returns sorted by z_min for efficient lookup.
pub fn build_feature_map(
    mesh: &TriangleMesh,
    config: &VlhConfig,
) -> Vec<FeatureDetection> { ... }
```

### Pattern 3: Laplacian Smoothing on 1D Height Array
**What:** After the optimizer produces raw heights, apply 1D Laplacian smoothing to ensure smooth transitions, with anchor points (first layer, feature minimums) pinned.
**When to use:** Post-optimization, before ratio clamping safety net.

```rust
/// 1D Laplacian smoothing on layer heights.
/// Pinned indices are not moved during smoothing.
pub fn laplacian_smooth(
    heights: &mut [(f64, f64)],  // (z, height) pairs
    pinned: &[bool],             // true = anchor point, don't move
    lambda: f64,                 // smoothing strength 0.0-1.0
    iterations: usize,           // number of passes
) {
    for _ in 0..iterations {
        let prev: Vec<f64> = heights.iter().map(|h| h.1).collect();
        for i in 1..heights.len() - 1 {
            if pinned[i] {
                continue;
            }
            let avg = (prev[i - 1] + prev[i + 1]) / 2.0;
            heights[i].1 = prev[i] + lambda * (avg - prev[i]);
        }
    }
}
```

### Pattern 4: DP Optimizer
**What:** Discretize candidate heights into N levels per Z, then find the minimum-cost path through the lattice with transition constraints.
**When to use:** When `vlh_optimizer_mode == OptimizerMode::DynamicProgramming`.

```rust
/// DP state: for each Z level, for each candidate height index,
/// store the minimum accumulated cost and the predecessor height index.
/// State space: O(num_z_levels * num_candidates)
/// Time: O(num_z_levels * num_candidates^2)
///
/// Recommended: ~10-20 candidate heights per Z (e.g., linearly spaced
/// between min_height and max_height). This gives manageable O(n * 200-400)
/// per Z level.
```

### Anti-Patterns to Avoid
- **Non-deterministic float sorting:** Never use `f64::partial_cmp` without a tie-breaking strategy. Use `ordered_float::OrderedFloat` or `total_cmp` for all comparisons to satisfy SLICE-05.
- **Parallel reduction of floats:** Floating-point addition is not associative. Serial accumulation (or deterministic parallel reduction order) is required for SLICE-05.
- **Modifying existing adaptive.rs API signature:** The refactored wrapper must keep the same `pub fn compute_adaptive_layer_heights(mesh, min, max, quality, first_layer) -> Vec<(f64, f64)>` signature for backward compatibility.
- **Coupling feature detection to optimizer internals:** Feature map is a pre-pass that produces a simple data structure consumed by the optimizer. Don't make the optimizer call detection functions directly.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Overhang detection | Custom angle calculation | `support::detect::detect_overhangs_layer` | Already implements layer-diff + raycast validation with configurable angle |
| Bridge detection | Custom unsupported-span finder | `support::bridge::detect_bridges` / `is_bridge_candidate` | Handles span direction, width measurement, below-contour analysis |
| Hole detection | Custom circularity heuristic | `polyhole::is_circular_hole` | Already handles winding check, centroid computation, radius variance |
| Polygon offsetting | Custom offset for thin wall detection | `slicecore_geo::offset_polygons` | Handles join types, degenerate cases |
| Float-safe sets/maps | Custom hash for f64 | `ordered_float::OrderedFloat` with `BTreeSet`/`BTreeMap` | Already used in z_schedule.rs; provides total ordering |
| Event emission | Custom diagnostic output | `event::EventBus::emit` with new `SliceEvent::VlhDiagnostic` variant | Existing pub/sub infrastructure with NDJSON support |

**Key insight:** The existing codebase has all the geometric primitives needed. Phase 47 is about orchestrating them into a multi-objective optimization pipeline, not about building new geometric operations.

## Common Pitfalls

### Pitfall 1: Floating-Point Non-Determinism (SLICE-05 Violation)
**What goes wrong:** Different layer heights on different runs with identical input.
**Why it happens:** Non-associative float addition in parallel reductions, unstable sorts on floats, or platform-dependent FMA instructions.
**How to avoid:** Use serial accumulation for objective score summation. Use `OrderedFloat` for all sorting. Use `f64::total_cmp` for comparisons. Pin iteration order everywhere.
**Warning signs:** Flaky golden-file tests, different G-code across runs.

### Pitfall 2: Laplacian Smoothing Destroys Feature-Demanded Heights
**What goes wrong:** Smoothing averages away the thin layers demanded by features (holes, overhangs), defeating the purpose of feature detection.
**Why it happens:** Naive Laplacian smoothing moves all vertices toward their neighbors' average.
**How to avoid:** Pin feature-demanded heights as anchor points. The `pinned` array marks these indices as immovable. Laplacian only smooths the transitions between anchored regions.
**Warning signs:** Feature-detected thin zones being smoothed back to thick heights.

### Pitfall 3: Laplacian Smoothing Shrinkage
**What goes wrong:** After many iterations, all heights converge toward the global average, losing the variation that VLH is supposed to provide.
**Why it happens:** Classic Laplacian smoothing shrinkage problem (well-documented in mesh processing literature).
**How to avoid:** Limit iterations to 3-5. Use lambda < 1.0 (default 0.5). Optionally use Taubin smoothing (alternating positive/negative lambda) to counteract shrinkage, but with only 3-5 iterations on a 1D signal this is likely unnecessary.
**Warning signs:** Height profile becoming nearly flat after smoothing.

### Pitfall 4: DP State Space Explosion
**What goes wrong:** DP optimizer is too slow for large models with thousands of Z levels.
**Why it happens:** O(num_z * candidates^2) with too many candidates or too fine Z sampling.
**How to avoid:** Limit candidates to 10-20 per Z level. For models with >2000 layers, consider adaptive candidate reduction or switching to greedy mode with a warning.
**Warning signs:** DP mode taking >10 seconds on typical models.

### Pitfall 5: Feature Detection at Wrong Resolution
**What goes wrong:** Feature map misses features that fall between sampling points.
**Why it happens:** Feature pre-pass samples at too coarse a Z resolution.
**How to avoid:** Sample at min_height/2 resolution (same as existing curvature sampling). Feature influence margin (2-3 layers) provides additional safety net.
**Warning signs:** Known overhangs or holes not triggering feature detection.

### Pitfall 6: Breaking Backward Compatibility
**What goes wrong:** Existing users of `compute_adaptive_layer_heights` get different results.
**Why it happens:** Refactored wrapper doesn't exactly reproduce old behavior.
**How to avoid:** The wrapper must map `quality` parameter to quality-only weights and preserve the exact same curvature-to-height mapping. Add a golden-file regression test comparing old vs new wrapper output on the sphere and cube test meshes.
**Warning signs:** Existing adaptive layer height tests failing after refactor.

## Code Examples

### Example 1: VLH Config Fields for PrintConfig

```rust
// In slicecore-engine/src/config.rs, alongside existing adaptive_* fields

/// Enable multi-objective variable layer height optimization.
#[setting(tier = 2, description = "Enable multi-objective VLH")]
pub vlh_enabled: bool,

/// VLH quality weight (0.0-1.0).
#[setting(tier = 2, description = "Quality weight for VLH")]
pub vlh_quality_weight: f64,

/// VLH speed weight (0.0-1.0).
pub vlh_speed_weight: f64,

/// VLH strength weight (0.0-1.0).
pub vlh_strength_weight: f64,

/// VLH material weight (0.0-1.0).
pub vlh_material_weight: f64,

/// VLH optimizer mode.
pub vlh_optimizer_mode: VlhOptimizerMode,

/// Laplacian smoothing strength (0.0 = no smoothing, 1.0 = full).
pub vlh_smoothing_strength: f64,

/// Laplacian smoothing iterations.
pub vlh_smoothing_iterations: u32,

/// Enable VLH diagnostic events.
pub vlh_diagnostics: bool,

// Feature weights
pub vlh_feature_overhang_weight: f64,
pub vlh_feature_bridge_weight: f64,
pub vlh_feature_thin_wall_weight: f64,
pub vlh_feature_hole_weight: f64,

/// Overhang sensitivity range lower bound (degrees).
pub vlh_overhang_angle_min: f64,
/// Overhang sensitivity range upper bound (degrees).
pub vlh_overhang_angle_max: f64,

/// Thin wall threshold (mm). Walls thinner than this trigger thin layers.
pub vlh_thin_wall_threshold: f64,

/// Feature influence margin in number of layers above/below detection.
pub vlh_feature_margin_layers: u32,

/// Enable stochastic exploration (non-deterministic).
pub vlh_stochastic: bool,
```

### Example 2: VLhDiagnostic Event Variant

```rust
// New variant for SliceEvent enum in event.rs

/// VLH diagnostic data for a single layer.
VlhDiagnostic {
    /// Layer index.
    layer: usize,
    /// Z height in mm.
    z: f64,
    /// Final computed layer height in mm.
    height: f64,
    /// Per-objective scores before weighting.
    quality_score: f64,
    speed_score: f64,
    strength_score: f64,
    material_score: f64,
    /// Which objective or feature most influenced the final height.
    dominant_factor: String,
    /// Features detected at this Z.
    features: Vec<String>,
},
```

### Example 3: Greedy Optimizer with Lookahead

```rust
/// Greedy optimizer: at each Z, evaluate next `lookahead` positions
/// and pick the height that minimizes accumulated cost over the window.
pub fn optimize_greedy(
    z_samples: &[(f64, ObjectiveScores)],
    feature_map: &FeatureMap,
    weights: &VlhWeights,
    config: &VlhConfig,
) -> Vec<(f64, f64)> {
    let lookahead = 5; // Claude's discretion: 5 layers
    let mut result = Vec::with_capacity(z_samples.len());
    // ... greedy selection with lookahead window
    result
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single curvature-based VLH (PrusaSlicer-style) | Multi-objective optimization with feature awareness | This phase | Enables quality/speed/strength tradeoff control |
| Hard ratio clamping only | Laplacian smoothing + ratio clamping | This phase | Smoother transitions, less staircase effect |
| No feature awareness in VLH | Feature map pre-pass (overhangs, bridges, holes, thin walls) | This phase | Layer height responds to specific geometric features |

**Industry context:** PrusaSlicer uses curvature-only adaptive layers with Gaussian smoothing. Bambu Studio adds automatic mode with similar curvature analysis. No major open-source slicer implements multi-objective VLH with feature awareness and strength objectives. This is a differentiating feature.

## Open Questions

1. **DP Candidate Count**
   - What we know: More candidates = better optimization but O(n^2) cost per Z level.
   - What's unclear: Optimal balance between quality and speed for typical models.
   - Recommendation: Start with 15 candidates (linearly spaced min to max). Benchmark on test models. Claude's discretion area.

2. **Greedy Lookahead Window**
   - What we know: Larger window = better global optimization but diminishing returns.
   - What's unclear: How many layers of lookahead are needed for visually equivalent results to DP.
   - Recommendation: Start with 5 layers lookahead. Benchmark against DP results. Claude's discretion area.

3. **Laplacian Kernel Shape**
   - What we know: Uniform (equal neighbor weights) is simplest. Gaussian gives smoother results but adds complexity.
   - What's unclear: Whether the difference matters for a 1D height signal with only 3-5 iterations.
   - Recommendation: Start with uniform weights. The difference is negligible at low iteration counts. Claude's discretion area.

4. **Stochastic Exploration Algorithm**
   - What we know: Simulated annealing is well-understood and deterministic with fixed seed.
   - What's unclear: Whether any stochastic approach meaningfully improves over DP for 1D optimization.
   - Recommendation: Implement simulated annealing with configurable seed. Low priority -- greedy and DP cover the main use cases. Claude's discretion area.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) + proptest |
| Config file | Cargo.toml [dev-dependencies] |
| Quick run command | `cargo test -p slicecore-slicer --lib vlh` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SLICE-05 | Deterministic: same input = same output | unit + integration | `cargo test -p slicecore-slicer vlh_deterministic` | No -- Wave 0 |
| SLICE-05 | Deterministic: greedy mode reproducible | unit | `cargo test -p slicecore-slicer greedy_deterministic` | No -- Wave 0 |
| SLICE-05 | Deterministic: DP mode reproducible | unit | `cargo test -p slicecore-slicer dp_deterministic` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-slicer --lib vlh`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/slicecore-slicer/src/vlh/mod.rs` -- VLH module root with public API
- [ ] Test infrastructure for VLH: determinism tests, regression tests vs old adaptive.rs
- [ ] Golden file: sphere adaptive heights from old system (regression baseline)

## Sources

### Primary (HIGH confidence)
- Codebase inspection: `adaptive.rs` (604 lines) -- full existing VLH implementation read
- Codebase inspection: `z_schedule.rs` -- per-object Z-schedule with membership tracking
- Codebase inspection: `event.rs` -- SliceEvent enum and EventBus pub/sub system
- Codebase inspection: `config.rs` -- PrintConfig with SettingSchema derive, existing adaptive_* fields
- Codebase inspection: `support/detect.rs` -- overhang detection with layer-diff + raycast
- Codebase inspection: `support/bridge.rs` -- bridge detection with span direction/width
- Codebase inspection: `polyhole.rs` -- `is_circular_hole()` function signature and behavior
- Codebase inspection: `designDocs/04-IMPLEMENTATION-GUIDE.md` lines 308-326 -- DP algorithm spec

### Secondary (MEDIUM confidence)
- [Laplacian smoothing - Wikipedia](https://en.wikipedia.org/wiki/Laplacian_smoothing) -- Algorithm description, shrinkage problem
- [Stanford CS468 Mesh Smoothing lecture](https://graphics.stanford.edu/courses/cs468-12-spring/LectureSlides/06_smoothing.pdf) -- Laplacian operator, Taubin smoothing
- [PrusaSlicer adaptive layers](https://help.prusa3d.com/article/variable-layer-height-function_1750) -- Industry reference implementation
- [Bambu Studio VLH](https://wiki.bambulab.com/en/software/bambu-studio/adaptive-layer-height) -- Industry approach (27% time savings claim)

### Tertiary (LOW confidence)
- None -- all findings verified against codebase or established algorithms

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All dependencies already in workspace, no new external crates needed
- Architecture: HIGH - Module structure follows established crate patterns, all integration points verified in code
- Pitfalls: HIGH - Determinism (SLICE-05) pitfalls verified against existing code patterns; Laplacian smoothing pitfalls from established literature
- Feature detection: HIGH - All four detection functions exist and have been read (overhang, bridge, hole, thin wall via polygon offset)

**Research date:** 2026-03-25
**Valid until:** 2026-04-25 (stable domain -- algorithms are well-established, codebase unlikely to change significantly)
