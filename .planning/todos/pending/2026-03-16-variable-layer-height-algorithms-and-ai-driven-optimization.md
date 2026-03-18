---
created: 2026-03-16T19:32:48.250Z
title: Variable layer height algorithms and AI-driven optimization
area: engine
files:
  - crates/slicecore-slicer/src/adaptive.rs
  - crates/slicecore-slicer/src/layer.rs
  - crates/slicecore-engine/src/engine.rs
---

## Problem

Variable layer height (VLH) is a must-have quality feature — every major slicer supports it. slicecore already has a curvature-based adaptive layer height implementation in `adaptive.rs` that maps surface curvature to layer thickness. However:

1. **Current implementation is curvature-only**: It considers surface steepness/curvature but not other factors like structural importance, thermal behavior, or semantic meaning of geometry.
2. **No user editing**: PrusaSlicer and Bambu Studio allow users to paint/modify the height profile with a slider UI. slicecore has no equivalent workflow for manual adjustment.
3. **No AI assistance**: Current algorithms are purely geometric. An AI that understands the *entire* model could make smarter per-region decisions than local curvature analysis alone.

## Current state (what we have)

`crates/slicecore-slicer/src/adaptive.rs`:
- Samples curvature at fine Z intervals from triangle normals
- Maps high curvature → thin layers, low curvature → thick layers
- Enforces max 50% height change between adjacent layers (smoothing)
- Returns `(z_position, layer_height)` pairs
- Controlled by `quality` factor (0.0=speed, 1.0=quality), `min_height`, `max_height`

## Algorithms used by existing slicers

### PrusaSlicer / Bambu Studio (curvature-based)
- Same general approach: analyze surface angle changes → map to layer heights
- User can manually edit the height profile via a Z-slider UI tool
- The "smooth" profile attempts to minimize visible layer line artifacts

### Cura (adaptive layers)
- Variation threshold: user sets max allowed overshoot from target layer height
- Simple: if surface angle changes significantly, reduce layer height
- Less sophisticated than PrusaSlicer's approach

### IdeaMaker
- Allows per-region layer height overrides (paint regions on the model)
- Combines automatic adaptation with manual control

### Common limitations across all slicers
- **Z-axis only**: All algorithms look at vertical curvature. Horizontal features (fine text, logos) get no special treatment.
- **Global quality slider**: One quality setting for the whole model — can't say "high quality face, fast body"
- **No print-time awareness**: Algorithms don't consider that thinner layers dramatically increase print time
- **No structural awareness**: A load-bearing wall gets the same treatment as a cosmetic surface

## New/innovative algorithm ideas

### 1. Multi-objective optimization
Instead of curvature-only, optimize a weighted combination:
- **Visual quality** (curvature-based — current approach)
- **Print time** (prefer thicker layers where quality permits)
- **Structural integrity** (thinner layers where interlayer adhesion matters for strength)
- **Thermal behavior** (thinner layers on small features to allow cooling)

```
layer_height = optimize(
    w_quality * curvature_cost +
    w_speed * thickness_benefit +
    w_strength * adhesion_requirement +
    w_thermal * cooling_need
)
```

### 2. Perceptual quality model
Human eye sensitivity varies by viewing angle and surface type:
- Near-horizontal surfaces: layer lines barely visible → can use thick layers
- Near-vertical with gentle curves: layer lines very visible → thin layers critical
- Internal/hidden surfaces: invisible → maximum thickness always
- Use a perceptual model (visual saliency) rather than raw curvature

### 3. Feature-aware layer height
Detect geometric features and assign layer heights semantically:
| Feature | Layer height strategy |
|---------|---------------------|
| Fine text/embossing | Minimum layer height for legibility |
| Flat horizontal surface | Thick layers (no visible stepping) |
| Gentle dome/sphere | Thin layers (staircase most visible) |
| Vertical wall | Default layers (layer lines visible but uniform) |
| Overhangs/bridges | May need specific heights for support strategy |
| Thin walls | Layer height ≤ wall thickness for proper bonding |

### 4. Gradient-based smoothing (LaPlacian)
Current: max 50% change between adjacent layers.
Better: treat the height profile as a 1D signal and apply Laplacian smoothing with constraints, producing the smoothest possible transition that satisfies the curvature requirements. This eliminates visible "steps" in the layer height profile itself.

### 5. Print-time-budgeted optimization
User specifies a time budget: "I want the best quality possible within 4 hours."
Algorithm solves: minimize visual quality cost subject to total_time ≤ budget.
This is a constrained optimization problem (dynamic programming or LP).

## AI-assisted variable layer height

### Why AI helps
An AI (LLM with vision) can understand the *entire model semantically* — something curvature-only algorithms cannot do:
- "This is a face — prioritize quality on the front, less on the back"
- "This is a gear — uniform layers for dimensional accuracy"
- "This is a decorative vase — prioritize the visible exterior curves"
- "This text says 'FRAGILE' — use minimum layers for readability"

### AI-driven VLH workflow

```bash
# Basic: curvature-only (current)
slicecore slice model.stl --adaptive-layers

# AI-enhanced: semantic understanding
slicecore slice model.stl --adaptive-layers --ai-optimize
# AI analyzes model → identifies regions → assigns quality priorities → generates height profile
```

### AI integration points

1. **Region classification**: AI identifies functional vs. cosmetic vs. hidden regions of the model
2. **Quality priority map**: AI assigns per-region quality priorities (0-1) that feed into the VLH algorithm
3. **User intent**: "I want this to look good for display" vs. "I need this to be strong and fast" → AI adjusts the multi-objective weights
4. **Feature detection**: AI spots fine details (text, logos, decorative elements) that curvature analysis alone might miss
5. **Cross-model learning**: AI remembers that "this type of model" benefited from a particular height profile

### Implementation sketch

```rust
/// AI-generated quality priority map
struct QualityPriorityMap {
    /// Per-Z-height quality priority (0.0 = speed, 1.0 = max quality)
    priorities: Vec<(f64, f64)>,
    /// Per-region overrides (from AI semantic analysis)
    region_overrides: Vec<RegionPriority>,
}

/// Enhanced adaptive layer computation
fn compute_ai_adaptive_layers(
    mesh: &TriangleMesh,
    min_height: f64,
    max_height: f64,
    curvature_profile: &[f64],
    quality_map: &QualityPriorityMap,  // From AI
    time_budget: Option<Duration>,      // Optional constraint
) -> Vec<(f64, f64)> {
    // Combine curvature-based heights with AI quality priorities
    // Solve constrained optimization if time budget is given
}
```

## Dependencies

- **Existing adaptive.rs**: ✓ Foundation curvature-based algorithm
- **Phase 8 (AI integration)**: ✓ LLM provider for semantic analysis
- **Non-planar slicing** (todo): VLH and non-planar are complementary — both vary Z behavior
- **AI innovation brainstorm** (todo): Item #10 covers AI-driven layer height adaptation

## Phased implementation

1. **Phase A**: Improve current algorithm — add Laplacian smoothing, perceptual quality model
2. **Phase B**: Feature-aware detection (text, overhangs, thin walls) using geometric analysis
3. **Phase C**: Multi-objective optimization with time budget constraint
4. **Phase D**: AI semantic region classification feeding into VLH weights
5. **Phase E**: Manual editing API (height profile export/import for future UI tools)
