# Phase 3: Vertical Slice (STL to G-code) - Research

**Researched:** 2026-02-16
**Domain:** 3D printing slicing pipeline -- mesh slicing, perimeter generation, infill, G-code planning, CLI
**Confidence:** HIGH (domain well-understood from existing codebase + design docs + reference implementations)

## Summary

Phase 3 transforms the library from a collection of I/O and geometry primitives into a working slicer. The pipeline is: STL file in -> load mesh -> repair -> slice into layers -> generate perimeters -> classify solid surfaces -> generate infill -> plan toolpaths (retraction, speed, temperature, cooling) -> emit G-code. This phase creates 3-4 new crates (`slicecore-slicer`, `slicecore-engine`, and a CLI binary; perimeters, infill, and config may be modules within `slicecore-engine` or separate crates depending on scope).

The existing codebase provides excellent foundations: `slicecore-mesh` has BVH-accelerated `query_triangles_at_z()`, `slicecore-geo` has polygon boolean ops and offsetting via clipper2-rust, `slicecore-gcode-io` has a structured GcodeCommand enum with Marlin dialect support, and `slicecore-fileio` has STL/3MF/OBJ loaders with mesh repair. The core algorithmic work is: (1) triangle-plane intersection and segment chaining to produce contours, (2) polygon offsetting for perimeters, (3) rectilinear line generation clipped to infill regions, (4) extrusion math (E-axis values from cross-sectional area), (5) toolpath ordering with retraction/speed/temperature planning, and (6) a TOML-based config system for print profiles.

**Primary recommendation:** Build the slicing pipeline in the simplest possible form -- uniform layer heights, basic rectilinear infill, simple nearest-neighbor toolpath ordering, and a minimal config system. Correctness over optimization. The goal is a working end-to-end pipeline that prints a calibration cube, not a production slicer.

## Standard Stack

### Core (already in workspace)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| slicecore-math | workspace | Coordinates, points, vectors, bounding boxes | Foundation types (Phase 1) |
| slicecore-geo | workspace | Polygon booleans, offsetting, area, winding | clipper2-rust powered (Phase 1) |
| slicecore-mesh | workspace | TriangleMesh, BVH, repair, spatial queries | Phase 1-2 foundation |
| slicecore-fileio | workspace | STL/3MF/OBJ loading, format detection | Phase 2 |
| slicecore-gcode-io | workspace | GcodeCommand, GcodeWriter, Marlin dialect | Phase 2 |
| clipper2-rust | 1.0 | Polygon boolean ops and offsetting | Already locked, pure Rust, WASM-compatible |

### New Dependencies

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.5 | CLI argument parsing (derive macro) | De facto standard for Rust CLIs; 4.5.59 is latest |
| toml | 1.0 | TOML config file parsing/serialization | Standard serde-compatible TOML crate; used by Cargo itself |
| serde | 1 (workspace) | Serialization for config structs | Already in workspace |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| clap | argh (Google) | argh is simpler but lacks subcommands, shell completions; clap is more featureful for `slice`/`validate`/`analyze` subcommand pattern |
| toml | figment | figment supports layered config from multiple sources; overkill for Phase 3's minimal config; can adopt later |
| Separate crates per algorithm | Single slicecore-engine crate | Separate crates add compile-time overhead; for Phase 3 vertical slice, modules within a single engine crate are sufficient; refactor to separate crates in Phase 4+ if needed |

**Installation (Cargo.toml additions):**
```toml
# New CLI binary crate
[dependencies]
clap = { version = "4.5", features = ["derive"] }
toml = "1.0"
serde = { workspace = true }
```

## Architecture Patterns

### Recommended Crate/Module Structure

```
crates/
  slicecore-slicer/         # NEW: Mesh slicing (contour extraction)
    src/
      lib.rs                 # SliceLayer, slice_mesh()
      contour.rs             # Triangle-plane intersection, segment chaining
      layer_heights.rs       # Uniform layer height computation
  slicecore-engine/          # NEW: Full pipeline orchestrator
    src/
      lib.rs                 # Engine, SliceJob, SliceResult
      config.rs              # PrintConfig (TOML-backed)
      perimeter.rs           # Perimeter generation (polygon offset)
      infill.rs              # Rectilinear infill generation
      surface.rs             # Top/bottom solid layer classification
      toolpath.rs            # ExtrusionSegment, LayerToolpath
      planner.rs             # Speed, retraction, temperature, cooling, skirt/brim
      gcode_gen.rs           # Toolpath -> GcodeCommand conversion (extrusion math)
  slicecore-cli/             # NEW: CLI binary
    src/
      main.rs                # clap-based CLI with slice/validate/analyze subcommands
```

**Alternative (simpler):** Combine `slicecore-slicer` into `slicecore-engine` as a module. This reduces inter-crate dependencies and simplifies the build graph for Phase 3. The slicer can be extracted into its own crate in Phase 4 when it grows.

### Pattern 1: Layer-Parallel Pipeline

**What:** Each pipeline stage processes all layers, and within a stage, layers are independent and can be processed in parallel.

**When to use:** All per-layer operations (slicing, perimeter gen, infill gen, toolpath ordering).

**Example:**
```rust
// Phase 3 uses sequential processing for simplicity and determinism.
// Rayon parallelism can be added in Phase 4+ without changing the API.
pub fn slice_mesh(mesh: &TriangleMesh, config: &PrintConfig) -> Vec<SliceLayer> {
    let heights = compute_layer_heights(mesh.aabb(), config.layer_height, config.first_layer_height);
    heights.iter()
        .map(|&z| slice_at_height(mesh, z, config.layer_height))
        .collect()
}
```

### Pattern 2: Immutable Data Flow Between Stages

**What:** Each pipeline stage takes owned or borrowed input and returns new data. No mutation of previous stage output.

**When to use:** All pipeline stages. Aligns with existing codebase pattern (e.g., `repair()` takes owned vecs, returns new TriangleMesh).

**Example:**
```rust
// Pipeline stages are pure functions: input -> output
let layers: Vec<SliceLayer> = slice_mesh(&mesh, &config);
let perimetered: Vec<PerimeterLayer> = generate_perimeters(&layers, &config);
let infilled: Vec<InfilledLayer> = generate_infill(&perimetered, &config);
let toolpaths: Vec<LayerToolpath> = plan_toolpaths(&infilled, &config);
let gcode: Vec<GcodeCommand> = generate_gcode(&toolpaths, &config);
```

### Pattern 3: Config Struct with Serde Derive

**What:** A single `PrintConfig` struct with serde(default) for TOML deserialization. Minimal for Phase 3 -- only the settings needed for a calibration cube.

**When to use:** All configurable parameters.

**Example:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrintConfig {
    // Dimensions
    pub layer_height: f64,          // mm, default 0.2
    pub first_layer_height: f64,    // mm, default 0.3
    pub nozzle_diameter: f64,       // mm, default 0.4

    // Perimeters
    pub wall_count: u32,            // default 2
    pub wall_order: WallOrder,      // InnerFirst or OuterFirst

    // Infill
    pub infill_density: f64,        // 0.0 - 1.0, default 0.2
    pub top_solid_layers: u32,      // default 3
    pub bottom_solid_layers: u32,   // default 3

    // Speed (mm/s)
    pub perimeter_speed: f64,       // default 45.0
    pub infill_speed: f64,          // default 80.0
    pub travel_speed: f64,          // default 150.0
    pub first_layer_speed: f64,     // default 20.0

    // Retraction
    pub retract_length: f64,        // mm, default 0.8
    pub retract_speed: f64,         // mm/s, default 45.0
    pub retract_z_hop: f64,         // mm, default 0.0
    pub min_travel_for_retract: f64,// mm, default 1.5

    // Temperature
    pub nozzle_temp: f64,           // C, default 200.0
    pub bed_temp: f64,              // C, default 60.0
    pub first_layer_nozzle_temp: f64, // C, default 210.0
    pub first_layer_bed_temp: f64,  // C, default 65.0

    // Cooling
    pub fan_speed: u8,              // 0-255, default 255
    pub fan_below_layer_time: f64,  // seconds, default 60.0
    pub disable_fan_first_layers: u32, // default 1

    // Bed adhesion
    pub skirt_loops: u32,           // default 1
    pub skirt_distance: f64,        // mm, default 6.0
    pub brim_width: f64,            // mm, default 0.0 (disabled)

    // Bed size
    pub bed_x: f64,                 // mm, default 220.0
    pub bed_y: f64,                 // mm, default 220.0

    // Extrusion
    pub extrusion_multiplier: f64,  // default 1.0
    pub filament_diameter: f64,     // mm, default 1.75
}
```

### Anti-Patterns to Avoid

- **Premature parallelism:** Do NOT add rayon in Phase 3. Sequential processing is easier to debug and determinism is a success criterion. Parallelism is Phase 4+.
- **Floating-point polygon operations:** All polygon clipping, offsetting, and area calculations MUST use integer coordinates (IPoint2/Coord) via the existing slicecore-geo API. Float coordinates are only for mesh vertices, G-code output, and user-facing config values.
- **Stringly-typed G-code:** Use the existing GcodeCommand enum. Never concatenate G-code strings directly.
- **Monolithic slice function:** Split the pipeline into discrete, testable stages. Don't build a single 500-line function that does everything.
- **Hardcoded constants:** Every magic number (layer height, speed, temp, etc.) should be a config parameter with a sensible default.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Polygon offsetting for perimeters | Custom polygon offset | `slicecore_geo::offset_polygon()` with `JoinType::Miter` | Clipper2 handles all corner cases, concave polygons, collapse detection |
| Polygon boolean ops (infill clipping) | Custom boolean ops | `slicecore_geo::polygon_difference()` | Clipper2 is battle-tested for this exact use case |
| CLI argument parsing | Manual arg parsing | `clap` 4.5 derive macros | Handles help text, validation, subcommands, shell completion |
| TOML config parsing | Custom config parser | `toml` + `serde` derive | Standard, zero-effort deserialization |
| G-code formatting | String concatenation | `GcodeCommand` enum + `GcodeWriter` | Already built in Phase 2, type-safe, dialect-aware |
| Mesh loading | Custom STL parser | `slicecore_fileio::load_mesh()` | Already built in Phase 2, handles format detection + all formats |
| Mesh repair | Custom repair | `slicecore_mesh::repair()` | Already built in Phase 2, handles degenerate/normals/holes/stitching |
| Triangle-plane intersection | External library | Hand-roll (simple geometry) | This IS the core algorithm; 20 lines of linear interpolation, too simple for a dependency |

**Key insight:** Phase 3's algorithmic novelty is in the segment chaining (assembling line segments into closed contours), surface classification (identifying top/bottom solid layers), toolpath ordering, and extrusion math. Everything else is composition of existing primitives.

## Common Pitfalls

### Pitfall 1: Segment Chaining Failures

**What goes wrong:** Triangle-plane intersections produce line segments. Chaining them into closed contours fails when segments don't connect due to floating-point mismatches, T-junctions, or degenerate triangles.

**Why it happens:** Two triangles sharing an edge should produce segments that connect endpoint-to-endpoint, but floating-point interpolation may produce slightly different coordinates for the "same" point.

**How to avoid:**
1. Convert all intersection points to integer coordinates (IPoint2) immediately after computation. The quantization to nanometer precision absorbs floating-point jitter.
2. Use a spatial hash map (HashMap<IPoint2, ...>) for endpoint matching -- exact integer comparison, no epsilon.
3. After chaining, validate that all contours are closed (last point == first point, or within a small tolerance and then snap).
4. Log/warn about unchained segments rather than panicking.

**Warning signs:** Open contours, missing polygons at certain Z heights, zero-area layers where geometry exists.

### Pitfall 2: Winding Direction Inconsistency

**What goes wrong:** Contour polygons have the wrong winding direction (CW when it should be CCW or vice versa). This causes perimeters to offset outward instead of inward, infill to be generated outside the model, or holes to be treated as solids.

**Why it happens:** Segment chaining produces contours with arbitrary initial winding. The winding must be classified after chaining.

**How to avoid:**
1. After chaining segments into a closed polygon, compute signed area via `signed_area_2x()`.
2. Positive area = CCW = outer boundary. Negative area = CW = hole. This matches the convention established in Phase 1 (slicecore-geo).
3. Use `Polygon::validate()` to produce `ValidPolygon` with guaranteed winding before passing to boolean ops.
4. Nest holes inside their parent outer contours by point-in-polygon test.

**Warning signs:** Perimeters offsetting outward, infill outside the model boundary, missing hole detection.

### Pitfall 3: Extrusion Math Errors

**What goes wrong:** E-axis values in G-code are wrong, causing over-extrusion (blobs, stringing) or under-extrusion (gaps, weak layers).

**Why it happens:** Extrusion volume must match the cross-sectional area of the deposited bead times the move length. Errors in the cross-section model, filament diameter, or extrusion multiplier propagate directly to print quality.

**How to avoid:**
1. Use the Slic3r cross-section model: rectangle with semicircular ends.
   - `cross_section_area = (width - height) * height + PI * (height/2)^2`
   - Simplified: `cross_section_area = width * height - (4 - PI) * (height/2)^2`
2. E-axis (relative) = `cross_section_area * move_length / (PI * (filament_diameter/2)^2)`
3. Apply `extrusion_multiplier` as a final scaling factor.
4. Unit test the extrusion calculation with known values (e.g., 10mm move at 0.4mm width, 0.2mm height, 1.75mm filament diameter).
5. Use M83 relative extrusion (already the default in all 4 dialects from Phase 2).

**Warning signs:** G-code E values that are obviously too large (>1.0 for a short move) or too small (<0.001), printed walls that are too thick or have gaps.

### Pitfall 4: Non-Deterministic Output

**What goes wrong:** Same STL + same config produces different G-code on different runs. This violates success criterion 3.

**Why it happens:** HashMap iteration order is randomized in Rust (SipHash). If any stage iterates over a HashMap and the iteration order affects output, the result is non-deterministic.

**How to avoid:**
1. Use `BTreeMap` or `IndexMap` anywhere iteration order matters.
2. Or, collect HashMap results into a Vec and sort before processing.
3. Do NOT use rayon/parallel processing in Phase 3 (parallel reductions can have different summation order).
4. Use deterministic tie-breaking in all ordering decisions (e.g., sort by (x, y) coordinates, not by arbitrary order).
5. Add a determinism test: slice the same input twice and compare output byte-for-byte.

**Warning signs:** Tests that fail intermittently, G-code files that differ between runs.

### Pitfall 5: Top/Bottom Solid Layer Classification Mistakes

**What goes wrong:** The top or bottom surface of the model is not detected as solid, resulting in infill pattern showing through instead of solid layers.

**Why it happens:** Top/bottom detection requires looking at adjacent layers. A surface is "top" if the region has no corresponding region on the layer above (or the layer above has a smaller footprint). Similarly for "bottom".

**How to avoid:**
1. For each layer, compute the "above region" = intersection of current layer contours with layer above contours.
2. "Top surface" = current layer region MINUS the above region (the parts of this layer that have nothing above them).
3. "Bottom surface" = current layer region MINUS the below region.
4. Repeat for N layers deep (top_solid_layers, bottom_solid_layers config).
5. Start simple: for Phase 3, the top and bottom N layers can simply be marked as 100% solid infill.

**Warning signs:** Infill pattern visible on top/bottom surfaces, weak top layers.

### Pitfall 6: Skirt/Brim Colliding with Model

**What goes wrong:** Skirt or brim overlaps with the model's first-layer outline, causing adhesion issues or cosmetic defects.

**Why it happens:** Skirt distance is measured from the convex hull or bounding box of the first layer, but the offset direction or measurement is wrong.

**How to avoid:**
1. Skirt: Compute convex hull of all first-layer outer contours. Offset outward by `skirt_distance`. Generate `skirt_loops` outward offsets from there.
2. Brim: Offset the first-layer outer contours outward by `brim_width` with multiple passes. The brim must be attached to (touching) the model outline.
3. Validate: check that skirt polygons do not intersect model first-layer polygons.

**Warning signs:** Skirt printed on top of model outline, brim gaps, brim not connected to model.

## Code Examples

Verified patterns from the existing codebase and standard references:

### Triangle-Plane Intersection (Core Slicing Algorithm)

```rust
// Source: Standard computational geometry (Moller-Trumbore adapted for plane intersection)
// Given a triangle with vertices v0, v1, v2 and a Z-plane at height z,
// compute the intersection line segment (if any).

pub struct Segment2 {
    pub start: Point2,
    pub end: Point2,
}

pub fn intersect_triangle_z_plane(
    v0: Point3, v1: Point3, v2: Point3, z: f64
) -> Option<Segment2> {
    // Classify each vertex as above, on, or below the plane
    let d0 = v0.z - z;
    let d1 = v1.z - z;
    let d2 = v2.z - z;

    // Find edges that cross the plane (sign changes)
    let mut points = Vec::with_capacity(2);

    let edges = [(v0, v1, d0, d1), (v1, v2, d1, d2), (v2, v0, d2, d0)];
    for (va, vb, da, db) in &edges {
        if (da > 0.0 && db < 0.0) || (da < 0.0 && db > 0.0) {
            // Edge crosses plane -- interpolate
            let t = da / (da - db);
            let x = va.x + t * (vb.x - va.x);
            let y = va.y + t * (vb.y - va.y);
            points.push(Point2::new(x, y));
        } else if da.abs() < 1e-12 {
            // Vertex is ON the plane
            points.push(va.to_point2());
        }
    }

    points.dedup_by(|a, b| a.distance_to(b) < 1e-9);

    if points.len() == 2 {
        Some(Segment2 { start: points[0], end: points[1] })
    } else {
        None // Degenerate: triangle is coplanar, or only touches at a vertex
    }
}
```

### Segment Chaining (Contour Assembly)

```rust
// Source: PrusaSlicer algorithm reference + "An optimal algorithm for 3D triangle mesh slicing"
// Chain unsorted line segments into closed contours using endpoint matching.

use std::collections::HashMap;

pub fn chain_segments(segments: Vec<Segment2>) -> Vec<Vec<Point2>> {
    // Convert to integer space for exact matching
    let mut adj: HashMap<IPoint2, Vec<(IPoint2, usize)>> = HashMap::new();
    for (i, seg) in segments.iter().enumerate() {
        let start = seg.start.to_ipoint2();
        let end = seg.end.to_ipoint2();
        adj.entry(start).or_default().push((end, i));
        // Don't add reverse -- segments are directional from triangle winding
    }

    let mut used = vec![false; segments.len()];
    let mut contours = Vec::new();

    for start_idx in 0..segments.len() {
        if used[start_idx] { continue; }
        let mut contour = Vec::new();
        let first_point = segments[start_idx].start.to_ipoint2();
        let mut current = first_point;

        loop {
            // Find unused segment starting at current point
            let next = adj.get(&current).and_then(|neighbors| {
                neighbors.iter().find(|(_, idx)| !used[*idx])
            });

            match next {
                Some(&(end, idx)) => {
                    used[idx] = true;
                    contour.push(current);
                    current = end;
                    if current == first_point {
                        break; // Contour closed
                    }
                }
                None => break, // Open contour (mesh defect)
            }
        }

        if contour.len() >= 3 {
            contours.push(contour);
        }
    }
    contours
}
```

### Extrusion Calculation (E-Axis Values)

```rust
// Source: Slic3r flow math (https://manual.slic3r.org/advanced/flow-math)
// Cross-section model: rectangle with semicircular ends

/// Compute the cross-sectional area of an extrusion bead.
pub fn extrusion_cross_section(width: f64, height: f64) -> f64 {
    // Rectangle with semicircular ends:
    // area = (width - height) * height + PI * (height/2)^2
    let rect = (width - height) * height;
    let semicircles = std::f64::consts::PI * (height / 2.0) * (height / 2.0);
    rect + semicircles
}

/// Compute E-axis value for a linear move (relative extrusion, M83).
pub fn compute_e_value(
    move_length_mm: f64,
    extrusion_width: f64,
    layer_height: f64,
    filament_diameter: f64,
    extrusion_multiplier: f64,
) -> f64 {
    let cross_section = extrusion_cross_section(extrusion_width, layer_height);
    let volume = cross_section * move_length_mm; // mm^3
    let filament_area = std::f64::consts::PI * (filament_diameter / 2.0) * (filament_diameter / 2.0);
    let e = volume / filament_area;
    e * extrusion_multiplier
}
```

### Perimeter Generation (Polygon Offset)

```rust
// Source: Existing slicecore-geo API (Phase 1)
use slicecore_geo::{offset_polygon, JoinType, ValidPolygon};
use slicecore_math::mm_to_coord;

/// Generate N perimeter shells by repeated inward offsetting.
pub fn generate_perimeters(
    contour: &ValidPolygon,
    wall_count: u32,
    nozzle_width: f64,
    inner_first: bool,
) -> Vec<Vec<ValidPolygon>> {
    let mut shells = Vec::new();
    let half_width = mm_to_coord(nozzle_width / 2.0);
    let full_width = mm_to_coord(nozzle_width);

    for i in 0..wall_count {
        let offset = if i == 0 {
            -half_width // First perimeter: offset inward by half nozzle width
        } else {
            -(half_width + full_width * i as i64) // Subsequent: full width spacing
        };
        let polys = offset_polygon(contour, offset, JoinType::Miter)
            .unwrap_or_default();
        if polys.is_empty() { break; } // Polygon collapsed
        shells.push(polys);
    }

    if inner_first {
        shells.reverse(); // Print inner walls first, then outer
    }

    shells
}
```

### Rectilinear Infill

```rust
// Source: Standard slicer pattern (PrusaSlicer reference)
// Generate parallel lines at a given angle, clipped to infill region.

pub fn generate_rectilinear_infill(
    infill_region: &[ValidPolygon], // Region to fill (after subtracting perimeters)
    density: f64,                    // 0.0 - 1.0
    angle_degrees: f64,             // Rotation angle (alternates per layer)
    line_width: f64,                 // mm
) -> Vec<(IPoint2, IPoint2)> {       // Line segments
    if density <= 0.0 || infill_region.is_empty() {
        return Vec::new();
    }

    let spacing_mm = line_width / density; // At 20% density with 0.4mm width -> 2.0mm spacing
    let spacing = mm_to_coord(spacing_mm);

    // Compute bounding box of infill region
    let bbox = compute_ibbox(infill_region);

    // Generate parallel lines across the bounding box
    // (For Phase 3: axis-aligned lines, rotate by angle for alternating layers)
    let mut lines = Vec::new();
    let mut y = bbox.min.y;
    while y <= bbox.max.y {
        lines.push((
            IPoint2::new(bbox.min.x, y),
            IPoint2::new(bbox.max.x, y),
        ));
        y += spacing;
    }

    // Clip lines against infill region using polygon intersection
    clip_lines_to_region(&lines, infill_region)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Float polygon operations | Integer polygon operations (i64) | Phase 1 decision | Deterministic output, no float error accumulation in boolean ops |
| Raw G-code strings | Structured GcodeCommand enum | Phase 2 | Type-safe, testable, dialect-aware G-code generation |
| External mesh repair tools | Built-in repair pipeline | Phase 2 | Seamless load -> repair -> slice pipeline |
| Global mutable state | Immutable pipeline stages | Architecture decision | Each stage is a pure function; easy to test, easy to parallelize later |

**Deprecated/outdated:**
- Absolute extrusion (M82): Use M83 relative extrusion (locked in Phase 2, avoids E-axis overflow)
- Manual format detection: Use `load_mesh()` auto-detection (locked in Phase 2)

## Open Questions

1. **Crate structure: monolithic engine vs. separate crates?**
   - What we know: The design docs show separate crates for slicer, perimeters, infill, pathing, planner, gcode-gen. But Phase 3 is a vertical slice -- minimum viable pipeline.
   - What's unclear: How much crate separation to do now vs. later. Separate crates add CI time and dependency management complexity.
   - Recommendation: Create `slicecore-slicer` for the mesh-to-contours step (reusable across all future phases) and `slicecore-engine` for everything else (perimeters, infill, planning, G-code gen). The engine can be refactored into separate crates in Phase 4+. Create `slicecore-cli` as the binary crate.

2. **Config system scope: minimal vs. full schema?**
   - What we know: The design docs specify a full schema system with ~850 settings, tiers, validation, and dependency tracking. Phase 3 needs maybe 30 settings.
   - What's unclear: How much of the config infrastructure to build now.
   - Recommendation: A single `PrintConfig` struct with `#[serde(default)]` and TOML deserialization. No schema validation, no tiers, no dependency tracking. Just enough to configure a calibration cube print. Config system grows in later phases.

3. **Infill line clipping implementation**
   - What we know: clipper2-rust provides polygon boolean operations. Infill needs to clip infinite parallel lines against a polygon region.
   - What's unclear: Whether clipper2-rust's path intersection works for open polylines against closed polygons, or if we need a custom line-polygon clipping algorithm.
   - Recommendation: Use the standard approach: convert each infill line segment into a thin polygon (or use polyline clipping). If clipper2-rust doesn't support open path clipping, implement a simple Sutherland-Hodgman or scanline-based line-polygon clipper. This is a well-understood algorithm (~50 lines).

4. **Extrusion width: fixed or computed?**
   - What we know: Slic3r uses nozzle_diameter * 1.05 for external perimeters and a flow-matched width for others. Different features (perimeter, infill, first layer) use different widths.
   - What's unclear: How many different widths to support in Phase 3.
   - Recommendation: Use a single extrusion width for Phase 3: `nozzle_diameter * 1.1` (common default). Different widths per feature type can be added in Phase 4.

5. **Layer time estimation for fan control**
   - What we know: GCODE-09 requires layer-time-based cooling control. Fan speed should ramp up for fast (short) layers and be disabled for the first N layers.
   - What's unclear: How accurately layer time needs to be estimated at this stage.
   - Recommendation: Simple estimation: sum of (move_length / speed) for all moves in a layer. This gives a rough layer time. If layer time < `fan_below_layer_time`, set fan to max. Good enough for Phase 3.

## Sources

### Primary (HIGH confidence)
- Existing codebase: `slicecore-math`, `slicecore-geo`, `slicecore-mesh`, `slicecore-fileio`, `slicecore-gcode-io` -- all APIs verified by direct code reading
- Design documents: `designDocs/02-ARCHITECTURE.md`, `designDocs/04-IMPLEMENTATION-GUIDE.md` -- pipeline architecture, data types, algorithm descriptions
- `.planning/REQUIREMENTS.md` -- Phase 3 requirement definitions (SLICE-01, SLICE-03, SLICE-05, PERIM-01, PERIM-03, INFILL-01, INFILL-11, INFILL-12, GCODE-01, GCODE-05, GCODE-07, GCODE-08, GCODE-09, GCODE-10, API-02)
- `.planning/ROADMAP.md` -- Phase 3 success criteria and plan structure
- [Slic3r Flow Math](https://manual.slic3r.org/advanced/flow-math) -- Extrusion cross-section model, path spacing formulas

### Secondary (MEDIUM confidence)
- clap 4.5.59 -- verified via `cargo search clap`, standard Rust CLI crate
- toml 1.0.2 -- verified via `cargo search toml`, standard TOML parsing crate
- [An Optimal Algorithm for 3D Triangle Mesh Slicing](https://www.inf.ufpr.br/murilo/public/CAD-slicing.pdf) -- segment chaining in O(m) using hash table
- PrusaSlicer source code (GitHub) -- reference for pipeline stages (posSlice, posPerimeters, posPrepareInfill, posInfill)

### Tertiary (LOW confidence)
- Specific clap/toml API usage patterns -- based on training data, should be verified against current docs during implementation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries verified via cargo search or existing codebase; no novel dependencies
- Architecture: HIGH -- pipeline stages well-documented in design docs, data flow matches existing type system
- Algorithms: HIGH -- triangle-plane intersection is textbook geometry; perimeter generation uses existing offset API; infill is parallel lines + clipping
- Pitfalls: HIGH -- based on known slicing challenges documented in C++ slicer analysis and computational geometry literature
- Config/CLI: MEDIUM -- clap and toml are standard but exact version features not verified via Context7

**Research date:** 2026-02-16
**Valid until:** 2026-03-16 (stable domain, no fast-moving dependencies)
