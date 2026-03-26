# Phase 47: Variable Layer Height Algorithms - Context

**Gathered:** 2026-03-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Extend variable layer height beyond basic curvature analysis with multi-objective optimization (quality + speed + strength + material), feature-aware height selection, perceptual surface quality models, Laplacian smoothing for transition continuity, and per-layer diagnostic output. The existing `compute_adaptive_layer_heights` function in `adaptive.rs` is refactored into the new multi-objective system as a convenience wrapper.

</domain>

<decisions>
## Implementation Decisions

### Optimization Objectives
- Four objectives: visual quality, print speed, mechanical strength, material savings
- Users control balance via weighted sliders (quality, speed, strength, material weights normalized internally)
- Material savings is a soft weight only — no hard filament budget constraint
- Uniform per-Z height — each layer has one height applied to all features (no internal-vs-external splitting within a layer, since the speed benefit of thicker-on-internals is negligible in practice)
- Per-object weights supported on multi-object plates (ZSchedule already tracks per-object membership)
- When per-object weights conflict at the same Z, most conservative (thinnest) wins
- Max layer height hard-limited by nozzle diameter (~75% of nozzle diameter)
- Existing `compute_adaptive_layer_heights` refactored as wrapper calling new multi-objective system with quality-only weights

### Optimizer Strategy
- Two optimization modes: greedy with lookahead (default, fast) and dynamic programming (optional flag for high-quality optimization)
- Deterministic by default (SLICE-05 compliance); stochastic exploration opt-in via flag
- Design docs spec DP approach: minimize total deviation from true surface subject to min/max height and max adjacent change constraints

### Visual Quality Model
- Perceptual model: weight curvature by surface visibility
- All external surfaces weighted equally (no differentiation between top surfaces, outward-facing walls, overhangs)
- Internal/infill surfaces get no quality weight

### Strength Objective
- Geometric heuristics for stress zone identification: detect holes, thin walls, sharp angle transitions, overhangs from mesh geometry alone
- No FEA required — WASM-compatible, low resource usage

### Feature-Aware Heights
- Pre-pass feature map: run feature detection before VLH optimization, producing a per-Z map of detected features
- Four feature types trigger thinner layers: overhangs near threshold, bridges, thin walls/small features, holes/cylindrical features
- Per-feature configurable influence weights in PrintConfig (each feature type has its own weight, defaults ship with profiles)
- When multiple features overlap at the same Z, most demanding (thinnest) wins
- Feature influence extends beyond detection zone: configurable margin (default 2-3 layers) above and below for gradual transition
- Overhang detection uses continuous angle (0-90deg) with configurable sensitivity range (e.g., "thin layers between 40-60deg"), not the 4-tier support system
- Thin wall threshold configurable (default: 2x nozzle diameter)
- Hole/cylinder detection reuses existing `is_circular_hole()` from ADV-07 implementation
- Bridge detection queries existing support module's bridge detection functions
- Feature detections included in VLH diagnostic event stream

### Smoothing Strategy
- Laplacian smoothing first, then forward/backward ratio clamping as safety net
- Laplacian smoothing strength configurable (0.0-1.0 in PrintConfig, default ~0.5)
- Default 3-5 Laplacian iterations
- Anchor points preserved during smoothing: first layer height and feature-demanded minimums are pinned; Laplacian only smooths transitions between anchored points

### Diagnostics
- Per-layer diagnostic output showing objective scores, feature detections, and which objective/feature influenced the final height
- Output via structured events through existing pub/sub event system (API-05)
- Enabled in debug/verbose mode, off by default

### Claude's Discretion
- Exact DP state space discretization (number of candidate heights per Z)
- Greedy lookahead window size
- Laplacian smoothing kernel shape (uniform vs Gaussian neighbors)
- Curvature sampling resolution improvements
- Internal data structures for the feature map
- Stochastic exploration algorithm choice (simulated annealing, genetic, etc.)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing VLH Implementation
- `crates/slicecore-slicer/src/adaptive.rs` — Current single-objective curvature-based VLH (to be refactored into new system)
- `crates/slicecore-slicer/src/lib.rs` — Slicer crate public API including `compute_adaptive_layer_heights`, `slice_mesh_adaptive`
- `crates/slicecore-slicer/src/layer.rs` — Layer height computation and mesh slicing functions

### Z-Schedule and Engine Integration
- `crates/slicecore-engine/src/z_schedule.rs` — Per-object Z-schedule with object membership tracking
- `crates/slicecore-engine/src/engine.rs` — Engine orchestrator (integration point for VLH in pipeline)
- `crates/slicecore-engine/src/config.rs` — PrintConfig (where new VLH weights and settings go)

### Feature Detection Dependencies
- `crates/slicecore-engine/src/support/detect.rs` — Overhang and bridge detection
- `crates/slicecore-engine/src/surface.rs` — Surface type classification
- `crates/slicecore-mesh/src/lib.rs` — TriangleMesh with normals, AABB, vertices, indices

### Design Documents
- `designDocs/04-IMPLEMENTATION-GUIDE.md` lines 308-326 — Adaptive layer height algorithm spec including DP approach
- `designDocs/06-NOVEL-IDEAS.md` — Related ideas (topology-aware infill, seam visibility scoring)

### Event System
- Event system (API-05) — For VLH diagnostic output

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `adaptive.rs::sample_curvature_profile()` — Curvature sampling from triangle normals, usable as one input to multi-objective optimizer
- `adaptive.rs::smooth_heights()` — Forward/backward ratio clamping, becomes the safety-net post-Laplacian step
- `adaptive.rs::lookup_desired_height()` — Linear interpolation between height samples, reusable for feature map lookups
- `z_schedule.rs::ZSchedule` — Per-object Z membership tracking, enables per-object weight application
- `support/detect.rs` — Bridge and overhang detection, queried by feature map pre-pass
- `is_circular_hole()` — Hole detection from ADV-07, reused for cylindrical feature detection
- `TriangleMesh::normals()` / `TriangleMesh::aabb()` — Mesh analysis primitives

### Established Patterns
- Error handling: `thiserror` enums in per-crate `error.rs` files
- Config: `PrintConfig` struct with `#[derive(ConfigSchema)]` for JSON schema generation
- Events: Pub/sub event system for progress/diagnostic output
- Testing: Unit tests in module, integration tests in `tests/` directory, golden file comparisons

### Integration Points
- `slicecore-slicer` crate: New VLH optimizer replaces/wraps existing adaptive functions
- `slicecore-engine` crate: PrintConfig gains new VLH weight fields; engine pipeline calls new optimizer
- Event system: New `VlhDiagnostic` event type for per-layer breakdowns

</code_context>

<specifics>
## Specific Ideas

- Wall ordering (inner-outer, outer-inner, inner-outer-inner) is unaffected by VLH — VLH operates at Z-scheduling level before perimeter generation. Each layer has one height, all features use it.
- Time-budgeted mode intentionally excluded — users get print time from slicer output and iterate via speed weight, not by specifying a target time. The weighted slider approach gives indirect time control.

</specifics>

<deferred>
## Deferred Ideas

### Future Phase Candidates
- **Simple FEA-lite stress estimation** — Beam theory approximations for strength zones (more accurate than geometric heuristics)
- **User-painted VLH regions** — Let users mark high-strength/high-quality regions (like support enforcers/blockers)
- **Time-budgeted VLH mode** — "Print in X hours" hard/soft constraint; framework supports adding time constraint later
- **VLH preview visualization** — Color gradient showing layer heights on model before slicing (thin=blue, thick=red)
- **Layer height quantization** — Snap heights to motor microstep multiples for Z accuracy
- **Multi-mesh VLH coherence** — Aligned VLH schedules at contact surfaces between assembly parts

### Research TODOs
- Research practical stress analysis algorithms for 3D printing that don't need supercomputer resources
- Investigate other feature types that benefit from VLH: text/lettering regions, support interface approach layers, top surface approach layers (2-3 layers before top solid), chamfers/fillets, scarf joint seam zones
- Survey academic VLH literature for optimization approaches beyond greedy/DP

</deferred>

---

*Phase: 47-variable-layer-height-algorithms*
*Context gathered: 2026-03-25*
