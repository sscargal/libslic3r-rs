# Phase 4: Perimeter and Infill Completeness - Research

**Researched:** 2026-02-17
**Domain:** Advanced perimeter generation (Arachne, gap fill, seam control, scarf seam), infill pattern algorithms (grid, honeycomb, gyroid, adaptive cubic, cubic, lightning, monotonic), adaptive layer heights, and slicing preview data
**Confidence:** MEDIUM-HIGH (algorithms well-documented in literature and open-source slicers; Arachne/lightning are the most complex and least verified)

## Summary

Phase 4 expands the minimal vertical-slice pipeline from Phase 3 into a production-quality slicer covering the full range of perimeter and infill features. The phase has three major workstreams: (1) advanced perimeter generation including Arachne variable-width walls, gap fill, seam placement strategies, and scarf joint seams; (2) seven new infill patterns beyond the existing rectilinear; and (3) adaptive layer heights based on surface curvature, plus slicing preview data output.

The most technically complex requirement is PERIM-02 (Arachne), which requires computing a medial axis / Voronoi diagram of polygon contours to generate variable-width perimeter paths. This is the one area where a new dependency (the `boostvoronoi` crate for line-segment Voronoi diagrams) would be valuable, as hand-rolling a robust Voronoi implementation is a multi-thousand-line endeavor. The infill patterns range from straightforward (grid = two-direction rectilinear) to moderately complex (gyroid = TPMS implicit surface sampling, lightning = top-down tree generation with branch growth). Adaptive layer heights require sampling triangle normals at candidate Z heights to estimate surface curvature, then applying a dynamic-programming or greedy optimization to choose non-uniform heights within min/max bounds.

The existing codebase is well-structured for this expansion. The `generate_rectilinear_infill()` function in `slicecore-engine/src/infill.rs` demonstrates the pattern (generate lines, clip to region, return `Vec<InfillLine>`). New infill patterns should follow the same interface. The perimeter module (`perimeter.rs`) and toolpath assembly (`toolpath.rs`) already handle multi-shell perimeters and nearest-neighbor ordering. Seam placement adds a new decision point (where to start each perimeter loop), and gap fill adds a new post-perimeter step to detect and fill narrow voids.

**Primary recommendation:** Implement infill patterns first (lowest risk, most parallelizable, directly extends existing infill.rs), then adaptive layer heights (requires slicer crate changes but well-bounded), then seam placement (touches perimeter + toolpath modules), then gap fill (moderate complexity), and finally Arachne + scarf seam last (highest complexity, most novel code).

## Standard Stack

### Core (already in workspace)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clipper2-rust | 1.0 | Polygon boolean ops, offsetting | Already locked; used by perimeter gen, infill clipping, gap fill detection |
| slicecore-math | workspace | Coord, IPoint2, Point2/3, Vec3, BBox, mm_to_coord | Foundation types |
| slicecore-geo | workspace | ValidPolygon, polygon_difference, offset_polygons, convex_hull | All polygon operations |
| slicecore-mesh | workspace | TriangleMesh with per-face normals, BVH, query_triangles_at_z | Normals needed for adaptive layers |
| slicecore-slicer | workspace | slice_mesh, SliceLayer, compute_layer_heights | Must be extended for adaptive heights |
| slicecore-engine | workspace | Engine, infill, perimeter, surface, toolpath, planner, gcode_gen | All Phase 4 target modules |
| serde / serde_json | 1 | Serialization for preview data and config | Already in workspace |

### New Dependencies (Recommended)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| boostvoronoi | latest | Line-segment Voronoi diagrams for Arachne medial axis | PERIM-02 only; pure Rust port of Boost.Voronoi; supports line segment input with integer coordinates |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| boostvoronoi | centerline crate | centerline wraps boostvoronoi with higher-level API; may be sufficient but adds another abstraction layer |
| boostvoronoi | Hand-rolled medial axis | Medial axis from polygon is a multi-thousand-line algorithm; hand-rolling would delay Arachne significantly |
| boostvoronoi | voronoice/voronator | These only support point sites, not line segments; line segment support is essential for polygon medial axis |
| New infill module patterns | Plugin system | Phase 7 adds plugins; for now, infill patterns are built-in modules; can be refactored to plugins later |

## Architecture Patterns

### Recommended Module Structure

```
crates/
  slicecore-slicer/
    src/
      lib.rs
      contour.rs           # existing
      layer.rs             # existing -- extend compute_layer_heights for adaptive
      adaptive.rs          # NEW: adaptive layer height computation
  slicecore-engine/
    src/
      lib.rs               # extend re-exports
      config.rs            # extend with new config params
      perimeter.rs         # existing -- extend for seam placement
      arachne.rs           # NEW: Arachne variable-width perimeter generator
      gap_fill.rs          # NEW: gap fill between perimeters
      seam.rs              # NEW: seam placement strategies
      scarf.rs             # NEW: scarf joint seam implementation
      infill.rs            # existing -- refactor to dispatch to pattern modules
      infill/              # NEW: directory for pattern implementations
        mod.rs             # InfillPattern trait/enum dispatch
        rectilinear.rs     # move existing code here
        grid.rs            # NEW
        honeycomb.rs        # NEW
        gyroid.rs           # NEW
        cubic.rs            # NEW
        adaptive_cubic.rs   # NEW
        lightning.rs        # NEW
        monotonic.rs        # NEW
      surface.rs           # existing
      toolpath.rs          # existing -- extend for variable-width extrusion
      preview.rs           # NEW: slicing preview data generation (SLICE-04)
      engine.rs            # existing -- extend for adaptive layers, pattern selection
```

### Pattern 1: Infill Pattern Dispatch

**What:** A common interface for all infill patterns, with the engine selecting the appropriate pattern based on config.

**When to use:** All infill generation.

```rust
/// Enum of all supported infill patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InfillPattern {
    Rectilinear,
    Grid,
    Honeycomb,
    Gyroid,
    AdaptiveCubic,
    Cubic,
    Lightning,
    Monotonic,
}

/// Generate infill lines for a given pattern, region, and parameters.
pub fn generate_infill(
    pattern: InfillPattern,
    infill_region: &[ValidPolygon],
    density: f64,
    layer_index: usize,
    layer_z: f64,
    line_width: f64,
) -> Vec<InfillLine> {
    match pattern {
        InfillPattern::Rectilinear => rectilinear::generate(infill_region, density, layer_index, line_width),
        InfillPattern::Grid => grid::generate(infill_region, density, layer_index, line_width),
        InfillPattern::Honeycomb => honeycomb::generate(infill_region, density, layer_index, line_width),
        InfillPattern::Gyroid => gyroid::generate(infill_region, density, layer_index, layer_z, line_width),
        InfillPattern::AdaptiveCubic => adaptive_cubic::generate(infill_region, density, layer_z, line_width),
        InfillPattern::Cubic => cubic::generate(infill_region, density, layer_index, layer_z, line_width),
        InfillPattern::Lightning => lightning::generate(infill_region, density, layer_index, line_width),
        InfillPattern::Monotonic => monotonic::generate(infill_region, density, layer_index, line_width),
    }
}
```

### Pattern 2: Seam Placement Strategy

**What:** A separate seam selection step that runs after perimeter shells are generated but before toolpath assembly. The seam function chooses the start vertex for each perimeter polygon.

**When to use:** All perimeter generation.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeamPosition {
    Aligned,
    Random,
    Rear,
    NearestCorner, // "smart hiding" -- PrusaSlicer's "Nearest" strategy
}

/// Select the start point for a perimeter polygon based on strategy.
pub fn select_seam_point(
    polygon: &ValidPolygon,
    strategy: SeamPosition,
    previous_seam: Option<IPoint2>,
    layer_index: usize,
    bed_max_y: Coord,
) -> usize {
    // Returns the index into polygon.points() where printing should start.
    match strategy {
        SeamPosition::NearestCorner => find_best_corner(polygon, previous_seam),
        SeamPosition::Aligned => find_aligned_point(polygon, previous_seam),
        SeamPosition::Random => deterministic_random_point(polygon, layer_index),
        SeamPosition::Rear => find_rear_point(polygon, bed_max_y, previous_seam),
    }
}
```

### Pattern 3: Adaptive Layer Heights

**What:** Replace uniform `compute_layer_heights()` with an adaptive version that samples mesh surface curvature to determine per-layer thickness.

**When to use:** When `adaptive_layer_height` is enabled in config.

```rust
/// Compute adaptive layer heights based on surface curvature.
/// Returns non-uniform (z, layer_height) pairs.
pub fn compute_adaptive_layer_heights(
    mesh: &TriangleMesh,
    min_height: f64,
    max_height: f64,
    quality: f64,  // 0.0 = max speed, 1.0 = max quality
) -> Vec<(f64, f64)> {
    // 1. Sample normals at candidate Z heights (fine grid)
    // 2. Compute curvature estimate = change in normal direction between adjacent Z
    // 3. Map curvature to layer height: high curvature -> min_height, low -> max_height
    // 4. Smooth to avoid large jumps between adjacent layers
    // 5. Return non-uniform heights
}
```

### Pattern 4: Scarf Joint Seam

**What:** Instead of an abrupt seam where the perimeter loop closes, gradually ramp the extrusion height and flow over a configurable length. The scarf creates a smooth wedge overlap.

**When to use:** When `scarf_joint_seam` is enabled in config.

```rust
/// Apply scarf joint to the seam region of a perimeter polygon.
/// Modifies the toolpath segments near the seam to ramp Z and flow.
pub fn apply_scarf_joint(
    segments: &mut Vec<ToolpathSegment>,
    seam_index: usize,
    config: &ScarfJointConfig,
    layer_height: f64,
) {
    // 1. Identify the scarf region (scarf_length mm around the seam point)
    // 2. Split segments in the scarf region into sub-segments (scarf_steps count)
    // 3. For the leading ramp: gradually increase Z from (z - scarf_start_height) to z
    // 4. Adjust E values proportionally (flow ramps up with Z)
    // 5. For the trailing ramp at the next layer: gradually decrease to create overlap
}
```

### Anti-Patterns to Avoid

- **Monolithic infill function:** Do NOT put all 8 infill pattern algorithms in a single file. Each pattern gets its own module/file.
- **Floating-point infill coordinates:** All infill line endpoints MUST be IPoint2 (integer coordinates). Float math for generating the pattern is fine, but convert to IPoint2 before clipping and returning.
- **Ignoring polygon holes in infill:** When generating infill for a region, the region may contain holes (e.g., bolt holes). The line-polygon clipping must handle holes correctly (even-odd or nonzero fill rule).
- **Non-deterministic seam placement:** The "random" seam placement MUST be deterministic (use layer_index as seed, not `rand`). Same input = same output.
- **Arachne without Voronoi:** Do NOT try to approximate variable-width perimeters without a proper medial axis. The results will be incorrect for complex geometries.
- **Scarf seam modifying Z during perimeter:** The scarf joint modifies Z height within a single layer's perimeter. This is NOT the same as moving to the next layer. The Z ramp happens over the scarf_length, not the full perimeter.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Voronoi diagram for medial axis | Custom Voronoi implementation | `boostvoronoi` crate | Fortune's algorithm with line segment support is ~3000+ lines; boostvoronoi is a mature port of Boost.Voronoi |
| Polygon boolean ops for infill clipping | Custom line-polygon clipper | `slicecore_geo::polygon_intersection/difference` | Already battle-tested in Phase 3; handles holes, degeneracies |
| Polygon offsetting for gap detection | Custom offset | `slicecore_geo::offset_polygons` | Clipper2's offset handles all corner cases |
| Hexagonal grid generation | Manual hex coordinate math | Standard axial hex coordinate formulas | Well-documented in "Red Blob Games" hex grid guide; straightforward math but easy to get wrong |
| Gyroid implicit surface | Custom TPMS solver | Standard formula: `cos(x)*sin(y) + cos(y)*sin(z) + cos(z)*sin(x) = 0` | Published mathematical formula; just evaluate and threshold |
| Octree for adaptive cubic | Custom spatial subdivision | Standard octree with distance-to-surface query | Well-known data structure; simple recursive implementation |

**Key insight:** The algorithmic complexity in Phase 4 is in correctly combining these primitives (e.g., Arachne = Voronoi + medial axis filtering + width assignment + toolpath conversion), not in implementing the primitives themselves.

## Common Pitfalls

### Pitfall 1: Arachne Width Transitions Causing Artifacts

**What goes wrong:** Variable-width perimeters have abrupt width changes at Voronoi diagram vertices, causing over/under-extrusion blobs.

**Why it happens:** The medial axis has branch points where three or more edges meet. At these junctions, the extrusion width can change discontinuously.

**How to avoid:**
1. Smooth width transitions by linearly interpolating over a minimum distance (e.g., 2-3x nozzle width).
2. Clamp width to the range [min_extrusion_width, max_extrusion_width] where min is typically 0.1mm and max is 2x nozzle width.
3. Filter out very short medial axis segments (< min_extrusion_width) to avoid micro-extrusions.
4. Start with simple cases (rectangles, simple thin walls) and validate before testing complex geometry.

**Warning signs:** Blobs at perimeter junctions, gaps in thin wall coverage, extrusion widths below printer capability.

### Pitfall 2: Gyroid Pattern Discontinuities Between Layers

**What goes wrong:** The gyroid infill shows gaps or disconnections between adjacent layers because the iso-surface sampling is not aligned between layers.

**Why it happens:** Gyroid is a 3D surface defined by `cos(x)*sin(y) + cos(y)*sin(z) + cos(z)*sin(x) = 0`. Each layer's Z is different, producing a different 2D cross-section. If the sampling resolution (grid step) is too coarse, the cross-sections look disconnected.

**How to avoid:**
1. Sample the gyroid implicit function on a fine 2D grid at each layer's Z height.
2. Use marching squares to extract the iso-contour at the desired density threshold.
3. Ensure the grid spacing is smaller than the line width to capture all features.
4. The iso-level determines density: threshold closer to 0 = lower density, farther from 0 = higher density.

**Warning signs:** Infill lines that don't connect between layers when viewed in a G-code preview, visible gaps in printed parts.

### Pitfall 3: Lightning Infill Structural Weakness

**What goes wrong:** Lightning infill fails to adequately support top surfaces, leading to drooping or gaps in top layers.

**Why it happens:** The tree-branching algorithm grows support structures from below, but if branch density is too low or branches terminate too far from top surfaces, support is insufficient.

**How to avoid:**
1. Lightning infill is NOT traditional infill -- it's internal support. Generate it by analyzing which regions of the top surface need support.
2. Start from the top surface regions and grow branches downward (not upward from the bed).
3. Branches should merge when close to each other (within a configurable distance) to minimize material.
4. The branch angle from vertical should be limited (e.g., max 45 degrees) to ensure printability.
5. Always validate that every point on the top surface has a support path to either the bottom surface or a wall.

**Warning signs:** Top surfaces sagging, incomplete infill patterns under overhangs, branches that terminate mid-air.

### Pitfall 4: Adaptive Layer Heights Causing Layer Adhesion Issues

**What goes wrong:** Rapid changes in layer height between adjacent layers cause poor interlayer adhesion at the transitions.

**Why it happens:** Going from a 0.05mm layer to a 0.3mm layer in one step means the thicker layer's flow rate is 6x higher, which can cause under-bonding with the thin layer below.

**How to avoid:**
1. Limit the maximum height change between adjacent layers (e.g., max 50% change: a 0.2mm layer can be followed by at most 0.3mm or at least 0.1mm).
2. Smooth the height profile using a moving average or by iterating the DP optimization with a change constraint.
3. The quality parameter should control how aggressively heights vary (quality=1.0 uses thinner layers more, quality=0.0 uses thicker layers more).

**Warning signs:** Visible horizontal lines at height transitions, weak horizontal shear strength at transition points.

### Pitfall 5: Seam Scoring Producing Suboptimal Placements on Smooth Models

**What goes wrong:** On cylindrical or spherical models with no sharp corners, the "NearestCorner" strategy falls back to arbitrary vertex selection, producing visible seams.

**Why it happens:** The corner-detection algorithm finds no concave vertices on smooth curves (all vertices are convex with nearly identical angles).

**How to avoid:**
1. For NearestCorner/smart hiding: when all vertices have similar angles (no strong concavity), fall back to the "Aligned" strategy to at least keep seams in a vertical line.
2. Score vertices by: (a) concavity angle, (b) overhang status, (c) proximity to previous layer's seam point. Weigh these factors.
3. For cylindrical models, the "Rear" strategy often produces the best visual result.
4. The scarf joint seam (PERIM-06) is specifically designed for smooth models where traditional seam hiding is impossible.

**Warning signs:** Seam zigzagging across the surface of cylinders, seams placed on overhangs.

### Pitfall 6: Gap Fill Producing Excessive Small Extrusions

**What goes wrong:** Gap fill generates thousands of tiny extrusion segments that increase print time and cause nozzle vibration without meaningfully improving part quality.

**Why it happens:** Every narrow region between perimeters triggers gap fill, even when the gap is only slightly wider than zero.

**How to avoid:**
1. Set a minimum gap fill area threshold (e.g., ignore gaps smaller than `line_width * line_width * 2`).
2. Set a minimum gap fill length threshold (e.g., ignore segments shorter than `2 * line_width`).
3. The gap fill extrusion width should be at least `min_extrusion_width` (typically 0.1mm) -- below this, the printer cannot physically extrude.
4. Merge adjacent thin gap fill segments into longer paths to reduce retractions.

**Warning signs:** Print time increasing dramatically with gap fill enabled, tiny segments causing printer vibration, blobs at gap fill starts.

## Code Examples

### Grid Infill (INFILL-02): Two-Direction Rectilinear

```rust
// Grid = rectilinear at 0 degrees AND 90 degrees on the SAME layer.
// This produces a crosshatch pattern that is stronger than alternating-layer rectilinear.
pub fn generate_grid_infill(
    infill_region: &[ValidPolygon],
    density: f64,
    layer_index: usize,
    line_width: f64,
) -> Vec<InfillLine> {
    // Generate horizontal lines
    let mut lines = generate_rectilinear_at_angle(infill_region, density, 0.0, line_width);
    // Generate vertical lines on the same layer
    lines.extend(generate_rectilinear_at_angle(infill_region, density, 90.0, line_width));
    lines
}
```

### Honeycomb Infill (INFILL-03): Hexagonal Grid

```rust
// Honeycomb generates a hexagonal tiling.
// Implementation: generate three sets of parallel lines at 0, 60, and 120 degrees,
// then selectively remove segments to form hexagons.
// Alternative simpler approach: generate a repeating hexagonal profile as a polyline
// and clip to the infill region.
pub fn generate_honeycomb_infill(
    infill_region: &[ValidPolygon],
    density: f64,
    layer_index: usize,
    line_width: f64,
) -> Vec<InfillLine> {
    let spacing = line_width / density;
    // Hex cell size: side = spacing / sqrt(3)
    let hex_side = spacing / 3.0_f64.sqrt();

    // Generate zigzag polylines that form hexagonal boundaries
    // Each row alternates between forward-zigzag and backward-zigzag
    // Clip the resulting polyline segments to infill_region
    todo!("Honeycomb generation")
}
```

### Gyroid Infill (INFILL-04): TPMS Cross-Section

```rust
// Gyroid: cos(x)*sin(y) + cos(y)*sin(z) + cos(z)*sin(x) = 0
// At a fixed z, this becomes a 2D implicit function:
//   f(x,y) = cos(x)*sin(y) + cos(y)*sin(z_layer) + cos(z_layer)*sin(x)
// Sample on a grid, use marching squares to extract the iso-contour.
pub fn generate_gyroid_infill(
    infill_region: &[ValidPolygon],
    density: f64,
    layer_index: usize,
    layer_z: f64,
    line_width: f64,
) -> Vec<InfillLine> {
    let spacing = line_width / density;
    // Scale factor: frequency = 2*PI / spacing (one full period per spacing)
    let freq = std::f64::consts::TAU / spacing;

    let bbox = compute_bounding_box(infill_region);

    // Sample the gyroid function on a 2D grid
    // Use marching squares to extract contour lines at f(x,y) = 0
    // Convert contour segments to IPoint2 and clip to infill_region
    todo!("Gyroid marching squares")
}
```

### Adaptive Layer Heights (SLICE-02)

```rust
// Source: Slic3r adaptive layer height algorithm + PrusaSlicer refinements
pub fn compute_adaptive_layer_heights(
    mesh: &TriangleMesh,
    min_height: f64,
    max_height: f64,
    quality: f64,
    first_layer_height: f64,
) -> Vec<(f64, f64)> {
    let aabb = mesh.aabb();
    let z_max = aabb.max.z;

    // Step 1: Sample surface curvature at fine Z intervals
    let sample_step = min_height / 2.0;
    let mut curvatures: Vec<(f64, f64)> = Vec::new(); // (z, curvature)

    let mut z = first_layer_height / 2.0;
    while z <= z_max {
        // Query triangles at this Z and compute average normal Z-component
        // High |normal.z| = flat/low curvature (horizontal surface) -> can use thick layers
        // Low |normal.z| = steep/high curvature (near-vertical, curved) -> need thin layers
        let triangles = query_triangles_at_z(mesh, z);
        let avg_steepness = compute_average_steepness(&triangles, mesh);
        curvatures.push((z, avg_steepness));
        z += sample_step;
    }

    // Step 2: Map curvature to desired layer height
    // Step 3: Smooth to avoid sudden changes (max 50% change between adjacent)
    // Step 4: Generate actual (z, height) pairs with first layer as given
    todo!("DP or greedy layer height optimization")
}
```

### Seam Placement: Nearest Corner / Smart Hiding (PERIM-05)

```rust
// Source: PrusaSlicer seam placement algorithm
pub fn find_best_corner(
    polygon: &ValidPolygon,
    previous_seam: Option<IPoint2>,
) -> usize {
    let pts = polygon.points();
    let n = pts.len();

    let mut best_idx = 0;
    let mut best_score = f64::MIN;

    for i in 0..n {
        let prev = pts[(i + n - 1) % n];
        let curr = pts[i];
        let next = pts[(i + 1) % n];

        // Compute angle at this vertex
        let angle = compute_corner_angle(prev, curr, next);

        // Score: prefer concave corners (angle > PI), then convex corners
        // Penalize overhang vertices
        let mut score = if angle > std::f64::consts::PI {
            // Concave: excellent seam hiding spot
            200.0 + angle
        } else {
            // Convex: acceptable but visible
            angle
        };

        // Bonus for proximity to previous seam (alignment)
        if let Some(prev_seam) = previous_seam {
            let dist_sq = (curr.x - prev_seam.x).pow(2) + (curr.y - prev_seam.y).pow(2);
            let dist_mm = (dist_sq as f64).sqrt() / 1_000_000.0;
            // Closer to previous seam = higher score (up to a point)
            score += 50.0 / (1.0 + dist_mm);
        }

        if score > best_score {
            best_score = score;
            best_idx = i;
        }
    }

    best_idx
}
```

### Gap Fill Detection (PERIM-04)

```rust
// Gap fill detects narrow regions between the innermost perimeter shell
// and the infill boundary that are too narrow for a full-width extrusion
// but wide enough for a thin extrusion.
pub fn detect_and_fill_gaps(
    inner_perimeter: &[ValidPolygon],
    infill_boundary: &[ValidPolygon],
    min_width: f64,   // minimum extrusion width (typically 0.1mm)
    max_width: f64,   // maximum gap width to fill (typically nozzle_diameter)
) -> Vec<GapFillPath> {
    // 1. Compute the gap region: inner_perimeter MINUS infill_boundary
    //    This gives the region between the last perimeter and the infill area
    let gap_region = polygon_difference(inner_perimeter, infill_boundary);

    // 2. For each gap polygon, compute its medial axis (centerline)
    //    The medial axis gives the path along the center of the gap
    //    with distance-to-boundary at each point (= half the gap width)

    // 3. Filter: only keep paths where width >= min_width
    // 4. Generate extrusion segments along the medial axis with variable width

    todo!("Gap fill implementation")
}
```

### Slicing Preview Data (SLICE-04)

```rust
// Preview data is a serializable representation of each layer
// for visualization by external tools (web viewers, desktop GUIs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlicePreview {
    pub layers: Vec<LayerPreview>,
    pub bounding_box: [f64; 6], // min_x, min_y, min_z, max_x, max_y, max_z
    pub total_layers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerPreview {
    pub z: f64,
    pub layer_height: f64,
    pub contours: Vec<Vec<[f64; 2]>>,       // outer boundaries as point arrays
    pub perimeters: Vec<Vec<[f64; 2]>>,     // perimeter paths
    pub infill_lines: Vec<[[f64; 2]; 2]>,   // infill as line segment pairs
    pub travel_moves: Vec<[[f64; 2]; 2]>,   // travel paths
    pub feature_types: Vec<String>,          // feature type per path for coloring
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Fixed-width perimeters only | Arachne variable-width (Kuipers et al. 2020) | PrusaSlicer 2.5 (2022), Cura 5.0 (2022) | Eliminates thin-wall gaps; standard in all major slicers |
| Uniform layer heights | Adaptive layer heights | Slic3r (2014), refined in Cura/PrusaSlicer | 20-50% faster prints with equal quality on curved models |
| Abrupt seam | Scarf joint seam | OrcaSlicer 2.0 (2024) | Nearly invisible seams on smooth surfaces |
| Rectilinear-only infill | 8+ pattern library | Evolution 2015-2024 | Different patterns optimized for different use cases |
| Grid-based supports as infill | Lightning infill (Tricard et al. 2019) | Cura 4.12 (2021) | 40-70% less material for purely structural infill |

**Deprecated/outdated:**
- Classic perimeter generator (fixed-width): Still available in PrusaSlicer as fallback but Arachne is default since 2.5
- Hilbert curve infill: Removed from most slicers due to poor strength and excessive retractions

## Algorithm Details for Each Requirement

### SLICE-02: Adaptive Layer Heights

**Algorithm:** Sample mesh surface normals at fine Z intervals. Compute a "steepness" metric at each Z: if triangles at that Z are mostly horizontal (normal Z-component near 1), the surface is flat and thick layers are fine. If normals are mostly horizontal (Z-component near 0), the surface is nearly vertical and thin layers aren't needed either. The key insight is that thin layers are needed where the surface curvature is high -- where the normal direction changes rapidly between adjacent Z heights.

**Implementation approach:**
1. Fine-sample normals at every `min_height/2` Z interval
2. Compute curvature as the rate of change of average normal direction
3. Map curvature to layer height: `height = max_height - (max_height - min_height) * curvature * quality`
4. Apply smoothing: maximum 50% height change between adjacent layers
5. Requires changes to `slicecore-slicer/src/layer.rs` and a new `adaptive.rs` module

### SLICE-04: Slicing Preview Data

**Algorithm:** After slicing is complete, serialize layer data (contours, perimeters, infill lines, travel moves) into a JSON-serializable struct. This is a post-processing step that does not affect the slicing pipeline itself.

**Implementation approach:**
1. New `preview.rs` module in `slicecore-engine`
2. `SlicePreview` struct with `#[derive(Serialize)]`
3. Extract from `LayerToolpath` data after pipeline completes
4. Expose as `Engine::generate_preview()` method
5. Convert IPoint2 coordinates to f64 mm for the preview output

### PERIM-02: Arachne Variable-Width Perimeters

**Algorithm (from Kuipers et al. 2020):**
1. Compute the medial axis (skeleton) of the polygon using Voronoi diagram of the polygon edges
2. At each point on the medial axis, the distance to the nearest polygon edge gives half the local thickness
3. Where thickness < 2 * nozzle_width: generate a single variable-width perimeter centered on the medial axis
4. Where thickness >= 2 * nozzle_width: generate standard fixed-width perimeters as usual
5. Connect variable-width segments with smooth width transitions

**Implementation approach:**
1. Use `boostvoronoi` to compute Voronoi diagram of polygon line segments
2. Filter Voronoi edges to extract the medial axis (internal edges only)
3. At each medial axis point, compute distance to nearest polygon edge = half gap width
4. If gap < 2 * nozzle_width: use single variable-width path
5. Otherwise: fall back to standard offsetting
6. Requires `ToolpathSegment` to support variable `extrusion_width` field

### PERIM-04: Gap Fill

**Algorithm:**
1. Compute the region between the innermost perimeter shell and the infill boundary
2. Identify narrow regions where width < 2 * min_extrusion_width (too narrow for a standard extrusion)
3. Compute the centerline of each narrow region (medial axis or simplified center)
4. Generate thin extrusion paths along the centerlines with width matching the gap

**Note:** If Arachne (PERIM-02) is implemented properly, it handles most gap-fill cases natively. PERIM-04 may be simplified to handle only the gaps between the Arachne perimeter output and the infill boundary.

### PERIM-05: Seam Placement Strategies

**Strategies:**
- **Aligned:** Choose the vertex closest to the previous layer's seam point. For the first layer, choose the vertex at rear-center.
- **Random:** Deterministic pseudo-random: `seam_index = hash(layer_index) % num_vertices`. Use a simple hash, not cryptographic.
- **Rear:** Choose the vertex with maximum Y coordinate. Break ties by proximity to previous seam.
- **NearestCorner (Smart Hiding):** Score each vertex: concave angle > convex angle > straight. Among candidates of equal quality, choose nearest to previous seam.

### PERIM-06: Scarf Joint Seam (12 Parameters)

**Parameters (from OrcaSlicer):**
1. `scarf_joint_type`: Contour / ContourAndHole
2. `conditional_scarf`: bool -- only on smooth perimeters
3. `scarf_speed`: mm/s or percentage of wall speed
4. `scarf_start_height`: mm or percentage of layer height (Z offset at ramp start)
5. `scarf_around_entire_wall`: bool
6. `scarf_length`: mm (horizontal distance of the ramp)
7. `scarf_steps`: integer (number of discrete segments in the ramp)
8. `scarf_flow_ratio`: percentage (extrusion multiplier during scarf)
9. `scarf_inner_walls`: bool
10. `role_based_wipe_speed`: bool
11. `wipe_speed`: mm/s
12. `wipe_on_loop`: bool (inward wipe at seam)

**Algorithm:**
At the seam point of each perimeter, replace the abrupt start/end with a gradual ramp:
- Trailing ramp: over `scarf_length` mm before the seam point, gradually reduce Z from layer Z to (Z - scarf_start_height)
- Leading ramp: over `scarf_length` mm after the seam point, gradually increase Z from (Z - scarf_start_height) to layer Z
- Flow adjusts proportionally with the Z change
- Split the ramp region into `scarf_steps` discrete G-code segments

### Infill Pattern Algorithms

**INFILL-02 Grid:** Generate rectilinear lines at both 0 and 90 degrees on the same layer. Crosshatch pattern.

**INFILL-03 Honeycomb:** Generate a repeating hexagonal grid. Three approaches: (a) three sets of parallel lines at 0/60/120 degrees with selective removal, (b) zigzag polylines that form hex cells, (c) hexagonal tile template repeated across the region. Approach (b) is most common in slicers.

**INFILL-04 Gyroid:** Evaluate the TPMS function `cos(fx)*sin(fy) + cos(fy)*sin(fz) + cos(fz)*sin(fx)` where `f = 2*PI/spacing` on a 2D grid at the layer's Z height. Use marching squares to extract iso-contours at the threshold that gives the desired density. Convert contour segments to InfillLine.

**INFILL-05 Adaptive Cubic:** Build an octree of the infill region. Refine cells that are near surfaces (within a distance threshold). Generate cubic infill lines within each octree cell. Denser cells near surfaces, sparser deep inside.

**INFILL-06 Cubic:** Three sets of parallel lines at specific angles that, when stacked across layers with per-layer Z-dependent offsets, form interlocking cubes. Each layer uses lines at one of three angles (0, 60, 120 degrees), cycling every 3 layers. The Z-dependent offset shifts the line phase to create the 3D cube structure.

**INFILL-07 Lightning:** Top-down tree-branching algorithm:
1. Identify top surface regions (from surface classification)
2. For each top surface point, grow a branch downward
3. Branches prefer vertical paths, with limited lateral spread (max angle)
4. Merge branches that come within a configurable distance
5. At each layer, output the branch cross-sections as infill lines
6. Result: tree-like internal support only where needed

**INFILL-08 Monotonic:** Rectilinear lines printed strictly in one direction (left-to-right). The ordering algorithm ensures no line is printed to the left of an already-printed line. Use a sweep-line approach: sort lines by their leftmost X coordinate, print in left-to-right order, inserting travel moves as needed. This eliminates the ridges caused by bidirectional printing on top surfaces.

## Open Questions

1. **boostvoronoi WASM compatibility**
   - What we know: boostvoronoi is pure Rust (ported from Boost C++), which suggests WASM compatibility
   - What's unclear: Whether it compiles to wasm32-unknown-unknown without issues
   - Recommendation: Add boostvoronoi as a dependency, verify WASM compilation early; if it fails, scope Arachne as a non-WASM feature behind a feature flag

2. **Lightning infill cross-layer dependencies**
   - What we know: Lightning grows branches from top surfaces downward, meaning it has cross-layer dependencies (unlike other infill patterns which are per-layer)
   - What's unclear: How to integrate this with the current per-layer pipeline in engine.rs
   - Recommendation: Lightning requires a pre-pass that analyzes all layers' top surfaces before generating infill on any layer. Add a `LightningContext` struct that is built once and queried per-layer.

3. **Adaptive cubic octree memory usage**
   - What we know: For large models, a fine-grained octree could use significant memory
   - What's unclear: What octree depth is needed for good results vs. acceptable memory
   - Recommendation: Limit octree depth to 8 levels (256 cells per axis at finest); profile memory on a large model during implementation

4. **Scarf joint interaction with Arachne variable-width perimeters**
   - What we know: Scarf modifies Z and flow at the seam. Arachne already varies width along the perimeter.
   - What's unclear: Whether the two features compose correctly or need special handling
   - Recommendation: Implement scarf first with fixed-width perimeters. Test with Arachne later. If width variation at the scarf point causes issues, fall back to fixed width for the scarf region.

5. **Gyroid density control**
   - What we know: The gyroid iso-surface has a single parameter (the iso-level threshold) that controls density
   - What's unclear: The exact mapping from user-specified density percentage to iso-level threshold
   - Recommendation: Experimentally calibrate: generate gyroid at various thresholds, measure the fill fraction (area of infill / area of region), build a lookup table for density -> threshold mapping

6. **Preview data format compatibility**
   - What we know: SLICE-04 requires "layer-by-layer visualization" data
   - What's unclear: What external tools/formats should be targeted (custom JSON, SVG, GCode viewer format)
   - Recommendation: Start with a custom JSON format (serde_json serialization of the SlicePreview struct). This is the simplest to implement and can be adapted later. Optionally add SVG export per-layer for debugging.

## Sources

### Primary (HIGH confidence)
- Existing codebase: `slicecore-engine/src/infill.rs`, `perimeter.rs`, `toolpath.rs`, `surface.rs`, `config.rs`, `engine.rs` -- all APIs verified by direct code reading
- Existing codebase: `slicecore-slicer/src/layer.rs` -- current `compute_layer_heights()` and `slice_mesh()` functions
- Existing codebase: `slicecore-mesh/src/triangle_mesh.rs` -- per-face normals available for adaptive layer height
- Existing codebase: `slicecore-geo/src/offset.rs`, `boolean.rs` -- clipper2-rust wrappers for offset and boolean ops
- `.planning/REQUIREMENTS.md` -- Phase 4 requirement definitions
- [Slic3r Variable Layer Height](https://manual.slic3r.org/expert-mode/variable-layer-height) -- adaptive layer height reference
- [Slic3r Flow Math](https://manual.slic3r.org/advanced/flow-math) -- extrusion cross-section model

### Secondary (MEDIUM confidence)
- [OrcaSlicer Scarf Joint Seam Wiki](https://github.com/OrcaSlicer/OrcaSlicer/wiki/quality_settings_seam) -- 12 scarf parameters and their descriptions
- [Prusa KB: Arachne Perimeter Generator](https://help.prusa3d.com/article/arachne-perimeter-generator_352769) -- Arachne overview and behavior
- [Prusa KB: Infill Patterns](https://help.prusa3d.com/article/infill-patterns_177130) -- pattern descriptions and characteristics
- [Prusa KB: Seam Position](https://help.prusa3d.com/article/seam-position_151069) -- seam placement algorithm descriptions
- [Ultimaker: Lightning Infill](https://ultimaker.com/learn/how-to-print-like-a-flash-with-lightning-infill/) -- lightning infill concept and behavior
- [boostvoronoi crate](https://crates.io/crates/boostvoronoi) -- pure Rust Voronoi for line segments
- [centerline crate](https://crates.io/crates/centerline) -- medial axis extraction using boostvoronoi

### Tertiary (LOW confidence -- needs validation during implementation)
- Kuipers et al. 2020 paper on Arachne -- referenced but not directly read; algorithm details inferred from implementations and descriptions
- Tricard et al. 2019 paper on lightning infill -- referenced but not directly read; algorithm details inferred from implementations
- Gyroid TPMS formula -- mathematical formula is well-known, but the specific density-to-threshold mapping needs experimental calibration
- Marching squares implementation for gyroid -- algorithm is textbook but specific integration with the infill pipeline needs validation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all core libraries already in workspace; only boostvoronoi is new
- Architecture: HIGH -- clear extension of existing Phase 3 module structure
- Infill algorithms (grid, honeycomb, cubic, monotonic): MEDIUM-HIGH -- well-documented patterns, straightforward to implement
- Infill algorithms (gyroid, adaptive cubic, lightning): MEDIUM -- more complex, require specific sub-algorithms (marching squares, octree, tree growth)
- Arachne: MEDIUM -- algorithm is complex, requires Voronoi/medial axis tooling; boostvoronoi needs validation
- Seam placement: HIGH -- well-documented strategies with clear algorithms
- Scarf joint seam: MEDIUM -- parameters well-documented in OrcaSlicer wiki, but implementation details (G-code segment splitting for Z ramp) need careful work
- Adaptive layer heights: MEDIUM-HIGH -- algorithm concept is clear, but curvature estimation and height smoothing need tuning
- Preview data: HIGH -- straightforward serde serialization of existing data

**Research date:** 2026-02-17
**Valid until:** 2026-03-17 (stable domain, no fast-moving dependencies)
