---
created: 2026-03-16T19:20:00.000Z
title: Hybrid sequential printing — first layer all, then by object
area: engine
files:
  - crates/slicecore-engine/src/sequential.rs
  - crates/slicecore-engine/src/engine.rs
  - crates/slicecore-engine/src/planner.rs
  - crates/slicecore-engine/src/gcode_gen.rs
---

## Problem

Two existing print modes have complementary strengths and weaknesses:

**By Layer (default):**
- Prints one layer across ALL objects before moving to the next layer
- Reliable: every object gets its first layer on a warm, freshly-printed bed
- Poor quality: excessive travel/stringing between distant objects, retraction storms

**By Object (sequential):**
- Completes one object entirely before starting the next
- Great quality: minimal travel, no stringing between objects, each object is "clean"
- Risky: if object #2 fails adhesion on layer 1, you've already wasted hours completing object #1. The failed object may also knock over and damage other in-progress objects.

**The gap**: No slicer offers a hybrid mode that gets the reliability of by-layer first layers with the quality of by-object for the remaining layers.

## Solution: Hybrid Print Sequencing

### Concept

```
Phase 1: By-Layer (first N layers)     Phase 2: By-Object (remaining layers)
┌─────────────────────────────┐        ┌─────────────────────────────┐
│  Obj A    Obj B    Obj C    │        │  Complete A → Complete B →  │
│  Layer 1  Layer 1  Layer 1  │        │  Complete C                 │
│  Layer 2  Layer 2  Layer 2  │        │                             │
│  Layer 3  Layer 3  Layer 3  │        │  (each object from layer    │
│  (all objects together)     │        │   N+1 to top)               │
└─────────────────────────────┘        └─────────────────────────────┘
```

**Phase 1 (By-Layer)**: Print the first N layers (configurable, default 3-5) across all objects simultaneously. This ensures:
- Every object gets adhesion verified before committing time to full prints
- User can spot first-layer failures early (all objects' first layers are visible within minutes)
- Bed adhesion is uniform (fresh heated bed for all objects)

**Phase 2 (By-Object)**: After all objects have a solid foundation, switch to sequential printing. Complete each object fully before starting the next. Benefits:
- Minimal travel between objects (no cross-plate moves)
- Reduced stringing (no long travel moves)
- If an object fails after the foundation phase, other objects are unaffected
- Completed objects cool down cleanly without being revisited

### Configurable parameters

```toml
[sequential]
mode = "hybrid"              # "by_layer" | "by_object" | "hybrid"
hybrid_shared_layers = 3     # Number of layers printed by-layer before switching
hybrid_shared_mm = 0.6       # Alternative: switch after this Z height
object_order = "nearest"     # Order for by-object phase: "nearest" | "tallest_first" | "shortest_first" | "manual"
```

### Implementation in slicecore-engine

The existing `sequential.rs` handles by-object printing with collision avoidance. Hybrid mode extends this:

1. **Planner phase**: Separate the layer list into two groups:
   - Shared layers (0..N): standard by-layer processing, all objects interleaved
   - Object layers (N+1..end): grouped per object, processed sequentially

2. **Collision check**: The by-object phase needs clearance validation (existing logic). Key difference: when starting object #2, objects #1's full height is already printed, so the gantry must clear it. This is the same constraint as current sequential mode.

3. **G-code generation**:
   - Shared layers: normal by-layer G-code (all objects per layer)
   - Transition: after last shared layer, insert a "switching to sequential" comment
   - Object layers: complete G-code for object A (layers N+1 to top), then object B, then C
   - Between objects: travel to safe Z, move to next object's position

4. **Edge case: objects with different heights**
   - Short objects complete first during by-object phase
   - Remaining objects continue without interference
   - Object ordering matters: shortest-first completes quickly and frees bed space; tallest-first minimizes gantry clearance risk

### Advanced: Failure recovery

With hybrid mode, the slicer can support a "checkpoint" at the transition point:

```gcode
; === HYBRID TRANSITION: Layer 3/120 ===
; All objects have solid foundation. Switching to by-object mode.
; Objects will complete in order: A → B → C
; If an object fails, remaining objects are independent.
M0 ; Optional pause for user inspection (configurable)
```

User can inspect all first layers, remove any object that didn't adhere, and resume. The slicer could even generate alternative G-code files:
- `model_all.gcode` — normal hybrid (all 3 objects)
- `model_skip_B.gcode` — hybrid excluding object B (if it failed first layer)

### Why this matters for each audience

**Home users**: "I'm printing 4 gifts overnight. I want to know they all stuck to the bed before I go to sleep. If one fails, I don't want it to ruin the other 3."

**Print farms**: "We run plates of 8-12 parts. Hybrid mode lets us verify adhesion in the first 2 minutes. If any part fails, we cancel immediately instead of wasting 4 hours."

**SaaS**: Reliable batch printing reduces waste and support tickets. "Why did my print fail?" → "Because another object on the plate knocked yours over." Hybrid eliminates this entire failure class.

## Dependencies

- **Phase 6 (Sequential printing)**: ✓ Already implemented — hybrid extends this
- **Phase 27 (Auto-arrangement)**: ✓ Arrangement must respect sequential clearance constraints
- **Travel optimization** (todo): By-layer phase benefits from TSP ordering
- **Job output directories** (todo): Each object could get its own sub-directory in hybrid mode
