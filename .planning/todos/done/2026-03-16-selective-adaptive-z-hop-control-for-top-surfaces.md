---
created: 2026-03-16T19:32:48.250Z
title: Selective adaptive z-hop control for top surfaces
area: engine
files:
  - crates/slicecore-engine/src/config.rs:217-218
  - crates/slicecore-engine/src/gcode_gen.rs
  - crates/slicecore-engine/src/toolpath.rs
  - crates/slicecore-engine/src/surface.rs
---

## Problem

Global Z-hop (retract_lift_z) is a blunt tool. When enabled, every retraction lifts the nozzle by the same height on every layer. This causes:

1. **Increased stringing on lower/interior layers**: Z-hop gives the nozzle time to ooze during the lift+travel+lower cycle. On internal layers where the nozzle wouldn't drag across anything visible anyway, Z-hop adds string-inducing travel time for zero benefit.
2. **Slower print times**: Every Z-hop adds two Z-axis moves (up + down) per retraction. With hundreds of retractions per print, this accumulates significantly.
3. **No Z-hop where it matters most**: Top surfaces (the visible final layers) need Z-hop to prevent nozzle-dragging scars across finished surfaces. But users who disable Z-hop globally to reduce stringing lose this protection.

Users are specifically requesting "Top Surface Only Z-Hop" — Z-hop only when traveling across surfaces that will be visible in the final print.

### Current state

`config.rs:217-218` defines a single global `z_hop: f64` in `RetractionConfig`. No per-layer or per-region control exists.

## Solution

### v1: Algorithmic adaptive Z-hop

#### Strategy 1: Surface-type-based Z-hop

Classify each travel move's context and apply Z-hop selectively:

| Travel context | Z-hop? | Rationale |
|---------------|--------|-----------|
| Crossing top solid surface | Yes (full) | Prevent nozzle scars on visible surface |
| Crossing ironed surface | Yes (full) | Ironed surfaces are highest quality — protect them |
| Crossing bottom surface | No | Hidden against bed |
| Crossing infill | No | Not visible |
| Crossing perimeter (not top layer) | Optional (reduced) | Somewhat visible but less critical |
| Crossing support material | No | Support is removed anyway |
| Travel within same island | No | Short travel, low risk |
| Travel between objects (sequential) | Yes (full) | Risk of hitting completed objects |

**Implementation**: During G-code generation, each travel move already knows (or can determine) what surface types it crosses. Tag each retraction with the crossing context and apply Z-hop accordingly.

```rust
/// Z-hop decision for a travel move
fn should_z_hop(travel: &TravelMove, layer: &Layer) -> Option<f64> {
    if travel.crosses_top_surface(layer) {
        Some(config.retraction.z_hop)          // Full Z-hop
    } else if travel.crosses_external_perimeter(layer) && !layer.is_bottom() {
        Some(config.retraction.z_hop * 0.5)    // Reduced Z-hop
    } else {
        None                                    // No Z-hop
    }
}
```

#### Strategy 2: Layer-position-based Z-hop

Simpler heuristic — enable Z-hop only on layers that contain top surfaces:

```rust
fn layer_z_hop(layer: &Layer, config: &RetractionConfig) -> f64 {
    if layer.has_top_surfaces() {
        config.z_hop
    } else if layer.index >= total_layers - config.top_solid_layers {
        config.z_hop  // Always Z-hop in the last N layers
    } else {
        0.0
    }
}
```

- Pros: Very simple, no per-travel analysis needed
- Cons: Entire layer either has Z-hop or doesn't — less granular

#### Strategy 3: Height-proportional Z-hop

Scale Z-hop height based on how much damage a nozzle drag would cause:

```rust
fn adaptive_z_hop_height(travel: &TravelMove, layer: &Layer) -> f64 {
    let base = config.retraction.z_hop;
    match travel.worst_crossing_type(layer) {
        SurfaceType::TopSolid => base,              // Full height
        SurfaceType::ExternalPerimeter => base * 0.6, // Moderate
        SurfaceType::InternalPerimeter => base * 0.3, // Minimal
        SurfaceType::Infill | SurfaceType::Support => 0.0, // None
    }
}
```

#### Strategy 4: Distance-gated Z-hop

Only Z-hop for travel moves longer than a threshold — short hops don't need Z-hop because ooze is minimal and the nozzle barely touches the surface:

```rust
fn distance_gated_z_hop(travel: &TravelMove, config: &Config) -> f64 {
    if travel.distance_mm() > config.z_hop_min_travel_mm {
        compute_z_hop(travel)
    } else {
        0.0
    }
}
```

#### Recommended v1: Combine strategies 1 + 4

Surface-type-based decisions with a minimum travel distance gate. This eliminates the most stringing (skip Z-hop on infill/interior) while avoiding unnecessary Z-hops on short moves.

### Configuration

```toml
[retraction]
z_hop = 0.4                    # Z-hop height in mm (when applied)
z_hop_mode = "adaptive"        # "always" | "top_surface" | "adaptive" | "never"
z_hop_min_travel_mm = 2.0      # Skip Z-hop for travels shorter than this
z_hop_top_only = true          # Only Z-hop when crossing top solid surfaces (simple mode)

# Fine-grained control (advanced)
z_hop_on_top_surface = true
z_hop_on_external_perimeter = false
z_hop_on_ironing = true
z_hop_on_infill = false
z_hop_on_support = false
```

### v2: AI-driven Z-hop optimization

An AI that understands the entire plate can make smarter Z-hop decisions:

#### What AI brings

1. **Semantic surface importance**: AI knows that the face of a figurine needs more protection than the base, even though both are "top surfaces." It can assign Z-hop priority per region.

2. **Cross-object awareness**: When printing multiple objects, AI understands which travel paths risk colliding with completed features on other objects — and only Z-hops those specific travels.

3. **Material-specific behavior**: AI knows that PETG strings more than PLA, so it might recommend more aggressive Z-hop avoidance for PETG (faster travels instead) while using Z-hop more freely with PLA.

4. **Stringing prediction**: Given material, temperature, retraction settings, and travel distance, AI can predict the probability of visible stringing and only enable Z-hop when the predicted stringing would be worse than the nozzle-drag risk.

5. **Print history learning**: After feedback ("this print had nozzle scars on the top" or "too much stringing"), AI adjusts Z-hop strategy for future prints with similar geometry.

#### AI integration

```bash
# v1: algorithmic (no AI)
slicecore slice model.stl --z-hop-mode adaptive

# v2: AI-optimized
slicecore slice model.stl --z-hop-mode ai
# AI analyzes model geometry → classifies surface importance →
# generates per-travel Z-hop decisions → embeds in G-code
```

The AI produces a `ZHopPriorityMap` — per-region importance scores that feed into the algorithmic Z-hop decision:

```rust
struct ZHopPriorityMap {
    /// Per-region Z-hop priority (0.0 = never Z-hop, 1.0 = always Z-hop)
    regions: Vec<(BoundingBox, f64)>,
}
```

#### AI prompt sketch

```
Analyze this 3D model for Z-hop optimization:
- Model: [geometry summary, dimensions, feature description]
- Material: [type, known stringing behavior]
- Objects on plate: [count, layout, heights]

For each region of the model, classify:
1. How visible is this surface in the final print? (0-1)
2. How likely is nozzle-drag damage here? (0-1)
3. How likely is stringing from Z-hop here? (0-1)

Return a Z-hop priority map.
```

## Dependencies

- **Surface classification** (existing): `surface.rs` classifies top/bottom/internal surfaces
- **G-code generation** (existing): `gcode_gen.rs` handles retraction/travel moves
- **Travel path analysis**: Need to determine what surfaces a travel move crosses
- **Phase 8 (AI)**: For v2 AI-driven optimization

## Phased implementation

1. **Phase A**: `z_hop_mode = "top_surface"` — simple layer-level Z-hop only on layers with top surfaces
2. **Phase B**: Per-travel surface crossing analysis — Z-hop only when crossing top/ironed surfaces
3. **Phase C**: Distance-gated Z-hop and proportional heights
4. **Phase D**: AI-driven Z-hop priority maps and per-region optimization
