# Phase 27: Build Plate Auto-Arrangement - Context

**Gathered:** 2026-03-11
**Status:** Ready for planning

<domain>
## Phase Boundary

Automatically position multiple parts on the print bed to maximize utilization and minimize wasted space. Includes auto-orientation, material-aware grouping, sequential (by-object) print mode with gantry collision avoidance, and multi-plate splitting. Does NOT include mesh boolean operations, advanced AI optimization, or thermal simulation.

</domain>

<decisions>
## Implementation Decisions

### Packing Algorithm
- Speed-first approach: bottom-left fill heuristic for v1
- Convex hull footprints for part representation (project mesh to XY, compute convex hull)
- Arbitrary polygon bed shapes supported (use bed_shape from MachineConfig, handles delta circles, Voron skirts, etc.)
- Multi-plate splitting when parts don't all fit — automatically distribute across virtual plates
- Height-aware grouping: when splitting across plates, prefer grouping similar-height parts to minimize total print time
- Largest-first sorting heuristic (sort by convex hull area descending)
- Center arrangement on bed after packing for thermal balance
- Material/color-aware grouping: same-material parts placed on same plate when grouping enabled
- Multi-head printer auto-detection from PrintConfig (if multiple extruders, skip material-based plate splitting). User can override via config
- Body material only for classification — support/interface materials are ignored for grouping purposes
- Sequential (by-object) printing support with gantry clearance zone collision avoidance
- Output includes print order for sequential mode (back-to-front to avoid gantry collisions)

### Rotation Handling
- Default 45° rotation increments (try 8 orientations per part)
- Configurable rotation_step parameter so users can try finer or coarser increments
- Per-part rotation lock (rotation_locked: true prevents arranger from rotating)
- Optional mirroring per part (user opt-in, useful for symmetric parts)
- Best-fit-in-remaining-space strategy — try each allowed rotation at the placement position, pick the one that minimizes wasted space
- Auto-orient enabled by default — tilt parts to find optimal print orientation before arrangement
- Auto-orient default criterion: minimize support volume
- Selectable criteria: minimize support volume, maximize flat face contact, multi-criteria scoring
- Per-part orientation lock (orientation_locked: true prevents tilting, keeps user-set orientation)

### Spacing & Clearance
- 2mm default part spacing, configurable via part_spacing
- Intelligent spacing adjustment based on nozzle diameter (ensure spacing >= nozzle size thresholds to avoid warping/elephants foot issues with larger nozzles or problematic materials)
- 5mm default bed edge margin, configurable via bed_margin
- Skirt/brim-aware footprint expansion — if brim enabled, expand each part's footprint by brim width; if skirt enabled, reserve space around outermost parts
- Individual rafts per part — expand footprint by raft margin (no shared rafts in v1)
- Gantry clearance zone models: all three supported and selectable
  - Cylinder approximation (extruder_clearance_radius)
  - Rectangular zone (gantry_width x gantry_depth)
  - Custom polygon (user-defined clearance shape)
- New PrintConfig fields added in this phase: extruder_clearance_radius, gantry_height, gantry_width, gantry_depth, extruder_clearance_polygon

### API & Invocation
- New standalone `slicecore-arrange` crate at Layer 2 (alongside slicer, perimeters, infill)
- CLI: both dedicated `arrange` subcommand and `--auto-arrange` flag on `slice`
  - `slicecore arrange model1.stl model2.stl --bed-shape ...` for standalone planning
  - `slicecore slice --auto-arrange ...` for integrated workflow
- Output: JSON arrangement plan by default; `--apply` flag writes transformed files; `--format 3mf` outputs positioned 3MF
  - JSON plan contains: plates array, each with parts {id, position, rotation, orientation, plate_index}
- Sync API with optional progress: `arrange(parts, config) -> ArrangementResult` + `arrange_with_progress()` variant with callback
- Integrates with Phase 23 progress/cancellation API via the progress callback

### Claude's Discretion
- Exact bottom-left fill implementation details
- Internal data structures for placement tracking
- Convex hull caching strategy
- Auto-orient sampling resolution (how many orientations to evaluate)
- Minimum support volume estimation approach for auto-orient
- JSON schema exact field names and nesting

</decisions>

<specifics>
## Specific Ideas

- Material grouping should avoid long print times from constant color changes on single-head printers (by-layer mode). Group same-color parts on same plate when possible
- For multi-material prints, only consider main body material for grouping. Support/interface materials may deliberately differ (e.g., PLA body with PETG support interface for easy separation)
- Multi-head printers (Bambu H2C, Snapmaker Ultra) should auto-detect and skip material grouping since color changes are free — but user can override
- "By-object" vs "by-layer" print mode awareness is important — sequential mode needs gantry collision avoidance
- Innovative algorithms (AI-driven nesting, reinforcement learning for placement, etc.) should be researched and added to TODO list for future phases

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `slicecore-geo::convex_hull()` — Graham scan convex hull, directly usable for part footprint computation
- `slicecore-geo::offset_polygon()` — polygon offsetting for spacing/margin expansion
- `slicecore-geo` boolean operations via `i-overlay` — for collision detection between footprints
- `slicecore-mesh::TriangleMesh` — has bounding_box(), transform support (scale, rotate, translate)
- `slicecore-math::BBox` — bounding box primitives
- `bed_shape` field in `MachineConfig` — serialized polygon string for bed boundary

### Established Patterns
- Crate structure: Layer 0 (math/geo/mesh) → Layer 1 (fileio/config) → Layer 2 (slicer/perimeters/infill) → Layer 3+ (engine/cli)
- New crate follows workspace conventions: `crates/slicecore-arrange/`
- CLI subcommands use clap derive with `Commands` enum in main.rs
- JSON output via serde_json, consistent with existing `--json` flag pattern
- Progress callback pattern from Phase 23 progress/cancellation API

### Integration Points
- `slicecore-engine` would call `slicecore-arrange` when `--auto-arrange` is active in the pipeline
- `slicecore-cli` adds `arrange` subcommand and `--auto-arrange` flag on `slice`
- `PrintConfig::MachineConfig` gains new gantry/clearance fields
- Profile import (INI/JSON) needs to handle new config fields

</code_context>

<deferred>
## Deferred Ideas

### Future Algorithm Research (TODO)
- NFP (no-fit polygon) algorithm for higher-density packing
- Simulated annealing / genetic algorithm optimization
- Continuous rotation search (vs discrete increments)
- AI/ML-driven nesting and orientation optimization
- Reinforcement learning for placement strategy
- Bounding box footprint mode (simplest/fastest, useful for quick estimates)
- Exact 2D projection footprints (concave outlines for maximum accuracy)

### Future Features (TODO)
- Part grouping constraints (must-share-plate for assemblies)
- Part priority ordering
- Thermal zone awareness (heat-sensitive material spacing)
- Shared rafts across adjacent parts
- Additional auto-orient criteria (e.g., minimize print time, maximize strength along load axis, surface finish optimization)
- Smart nozzle-material interaction table (specific spacing recommendations per material pair)

</deferred>

---

*Phase: 27-build-plate-auto-arrangement*
*Context gathered: 2026-03-11*
