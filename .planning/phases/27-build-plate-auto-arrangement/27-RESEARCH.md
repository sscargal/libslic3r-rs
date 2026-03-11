# Phase 27: Build Plate Auto-Arrangement - Research

**Researched:** 2026-03-11
**Domain:** 2D bin packing, auto-orientation, sequential print planning
**Confidence:** HIGH

## Summary

This phase implements automatic part arrangement on the print bed -- a 2D irregular packing problem with domain-specific constraints (gantry clearance, material grouping, multi-plate splitting). The user has locked a bottom-left fill heuristic with convex hull footprints, which is a well-understood algorithm that avoids the complexity of NFP (no-fit polygon) approaches used by PrusaSlicer's libnest2d.

The implementation creates a new `slicecore-arrange` crate at Layer 2, with a sync API, JSON output, and CLI integration. The existing codebase provides strong foundations: `slicecore-geo::convex_hull()` for footprint computation, `offset_polygon()` for spacing expansion, polygon boolean operations for collision detection, and `slicecore-mesh` transforms for rotation/orientation. The existing `sequential.rs` module in slicecore-engine already handles basic sequential print collision detection and ordering, which will be enhanced with the new gantry clearance zone models.

Auto-orient (finding optimal print orientation to minimize support) uses face normal analysis against gravity, scoring each candidate orientation by summing overhang area. This leverages the existing `TriangleMesh::normals()` and `rotate()` from slicecore-mesh.

**Primary recommendation:** Build a standalone `slicecore-arrange` crate with bottom-left fill placement using convex hull footprints, integrated with existing geo/mesh primitives. Keep the algorithm simple and correct first -- NFP and optimization are explicitly deferred.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Speed-first approach: bottom-left fill heuristic for v1
- Convex hull footprints for part representation (project mesh to XY, compute convex hull)
- Arbitrary polygon bed shapes supported (use bed_shape from MachineConfig)
- Multi-plate splitting when parts don't all fit
- Height-aware grouping: group similar-height parts when splitting across plates
- Largest-first sorting heuristic (sort by convex hull area descending)
- Center arrangement on bed after packing for thermal balance
- Material/color-aware grouping: same-material parts placed on same plate
- Multi-head printer auto-detection from PrintConfig (skip material-based plate splitting)
- Body material only for classification (support/interface materials ignored)
- Sequential (by-object) printing support with gantry clearance zone collision avoidance
- Output includes print order for sequential mode (back-to-front)
- Default 45-degree rotation increments (8 orientations per part)
- Configurable rotation_step parameter
- Per-part rotation lock and orientation lock
- Optional mirroring per part (user opt-in)
- Best-fit-in-remaining-space strategy for rotation selection
- Auto-orient enabled by default, default criterion: minimize support volume
- Selectable criteria: minimize support volume, maximize flat face contact, multi-criteria scoring
- 2mm default part spacing, configurable via part_spacing
- Intelligent spacing adjustment based on nozzle diameter
- 5mm default bed edge margin, configurable via bed_margin
- Skirt/brim-aware footprint expansion
- Individual rafts per part (expand footprint by raft margin)
- Gantry clearance zone models: cylinder, rectangular, custom polygon
- New PrintConfig fields: extruder_clearance_radius, gantry_height, gantry_width, gantry_depth, extruder_clearance_polygon
- New standalone `slicecore-arrange` crate at Layer 2
- CLI: `arrange` subcommand and `--auto-arrange` flag on `slice`
- Output: JSON arrangement plan by default; `--apply` flag; `--format 3mf`
- Sync API: `arrange(parts, config) -> ArrangementResult` + `arrange_with_progress()` variant
- Integrates with Phase 23 progress/cancellation API

### Claude's Discretion
- Exact bottom-left fill implementation details
- Internal data structures for placement tracking
- Convex hull caching strategy
- Auto-orient sampling resolution (how many orientations to evaluate)
- Minimum support volume estimation approach for auto-orient
- JSON schema exact field names and nesting

### Deferred Ideas (OUT OF SCOPE)
- NFP (no-fit polygon) algorithm for higher-density packing
- Simulated annealing / genetic algorithm optimization
- Continuous rotation search (vs discrete increments)
- AI/ML-driven nesting and orientation optimization
- Reinforcement learning for placement strategy
- Bounding box footprint mode
- Exact 2D projection footprints (concave outlines)
- Part grouping constraints (must-share-plate for assemblies)
- Part priority ordering
- Thermal zone awareness
- Shared rafts across adjacent parts
- Additional auto-orient criteria beyond support/flat-face/multi-criteria
- Smart nozzle-material interaction table
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| slicecore-geo | workspace | Convex hull, polygon offset, boolean ops, point-in-polygon | Already implemented, i64 coordinates |
| slicecore-mesh | workspace | TriangleMesh transforms (rotate, translate), normals, connected_components | Already implemented |
| slicecore-math | workspace | BBox2, BBox3, Point2, Point3, Vec3, Matrix4x4, Coord | Foundation types |
| slicecore-engine | workspace | PrintConfig, MachineConfig, SequentialConfig, CancellationToken | Config and progress API |
| serde | 1.x | Serialization for ArrangementResult JSON output | Workspace dependency |
| serde_json | 1.x | JSON output format | Workspace dependency |
| thiserror | 2.x | Error types for ArrangeError | Workspace dependency |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| clap | existing | CLI `arrange` subcommand | In slicecore-cli |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-rolled BLF | rectangle-pack crate | Crate only handles rectangles, not convex polygons; we need polygon-aware placement |
| Hand-rolled BLF | binpack2d crate | Rectangle-only; our decision requires convex hull footprints |
| Custom convex hull | External crate | slicecore-geo already has Graham scan convex_hull() |

**Installation:**
No new external dependencies needed. The `slicecore-arrange` crate only depends on workspace crates.

```toml
[dependencies]
slicecore-math = { path = "../slicecore-math" }
slicecore-geo = { path = "../slicecore-geo" }
slicecore-mesh = { path = "../slicecore-mesh" }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
```

## Architecture Patterns

### Recommended Project Structure
```
crates/slicecore-arrange/
  src/
    lib.rs              # Public API: arrange(), arrange_with_progress()
    error.rs            # ArrangeError enum
    config.rs           # ArrangeConfig, ArrangePart, GantryModel
    footprint.rs        # Convex hull projection, footprint expansion (brim/raft/spacing)
    orient.rs           # Auto-orient: evaluate candidate orientations
    placer.rs           # Bottom-left fill placement algorithm
    bed.rs              # Bed shape parsing, point-in-bed validation
    grouper.rs          # Material/height-aware plate grouping
    sequential.rs       # Sequential print ordering with gantry collision
    result.rs           # ArrangementResult, PlateArrangement, PartPlacement
```

### Pattern 1: Bottom-Left Fill with Convex Hulls
**What:** For each part (sorted largest-first by convex hull area), try each allowed rotation at candidate positions along the bed, choosing the placement that minimizes wasted space. Candidate positions are generated by scanning from bottom-left, testing placement validity via polygon intersection checks.
**When to use:** Always (this is the core algorithm).
**Implementation approach:**

1. Project each mesh to XY plane (drop Z), compute convex hull via `slicecore_geo::convex_hull()`
2. Expand footprint by spacing/brim/raft margins via `offset_polygon()`
3. Sort parts by hull area descending (largest-first)
4. For each part, try all allowed rotations (8 at 45-degree increments by default)
5. For each rotation, scan candidate positions (bottom-left fill: raster scan or contour-following)
6. Place at first valid position (no overlap with placed parts, within bed boundary)
7. Track placed footprints for subsequent collision checks

### Pattern 2: Auto-Orient via Overhang Scoring
**What:** Evaluate candidate mesh orientations by rotating around X and Y axes, scoring each by overhang area (faces whose normal dot Z-up < cos(overhang_threshold)).
**When to use:** When auto-orient is enabled (default) and part is not orientation-locked.
**Implementation approach:**

1. Sample orientations: rotate mesh around X-axis and Y-axis in increments (e.g., 15 degrees = 24x24 = 576 candidates, or coarser 30 degrees = 12x12 = 144)
2. For each orientation, compute overhang score by iterating face normals:
   - `overhang_area += face_area` where `normal.dot(Vec3::Z_UP) < cos(45_degrees)`
3. For "maximize flat face contact" criterion: score by total area of faces nearly parallel to build plate (normal.z < -0.99)
4. Multi-criteria: weighted sum of support_score and contact_score
5. Return orientation with best score

### Pattern 3: Multi-Plate Splitting
**What:** When all parts don't fit on one plate, distribute across virtual plates.
**When to use:** When placement fails for remaining parts after first plate is full.
**Implementation approach:**

1. Run placement on first plate until no more parts fit
2. Remaining parts go to next virtual plate
3. Height-aware grouping: before splitting, sort parts into groups by height similarity
4. Material-aware grouping: group same-material parts together when enabled

### Pattern 4: Sequential Print with Gantry Zones
**What:** For by-object printing, compute gantry clearance zones and validate/order placement.
**When to use:** When sequential printing mode is requested.
**Implementation approach:**

1. Support three gantry models: cylinder (radius), rectangle (width x depth), custom polygon
2. Expand each part's footprint by the gantry clearance zone
3. Order parts back-to-front (largest Y first) to avoid gantry collisions
4. Validate no expanded footprints overlap

### Anti-Patterns to Avoid
- **Grid-based placement:** Wastes space by quantizing to grid cells. Use continuous coordinate placement with polygon intersection tests.
- **Center-to-center distance checks:** Use polygon overlap tests, not bounding box gap calculations, for convex hull footprints.
- **Modifying mesh in-place:** Follow project convention of immutable transforms (rotate/translate return new meshes). Cache convex hulls.
- **Floating-point bed shape parsing:** The bed_shape string uses "XxY" format with integer-ish values. Parse carefully, convert to i64 Coord via COORD_SCALE.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Convex hull computation | Custom algorithm | `slicecore_geo::convex_hull()` | Graham scan already implemented and tested |
| Polygon offsetting | Manual buffer expansion | `slicecore_geo::offset_polygon()` | Handles corner cases (acute angles, collinear edges) |
| Polygon overlap detection | AABB-only checks | `slicecore_geo::polygon_intersection()` | True polygon intersection for convex hull footprints |
| Mesh rotation | Manual matrix math | `slicecore_mesh::transform::rotate()` | Handles normal recalculation, winding reversal |
| Point-in-polygon | Ray casting from scratch | `slicecore_geo::point_in_polygon()` | Handles edge cases (on-edge, on-vertex) |
| JSON serialization | Manual string building | serde_json | Workspace standard |
| Cancellation support | Custom threading | CancellationToken from Phase 23 | Already integrated into engine |

**Key insight:** The existing slicecore-geo and slicecore-mesh crates provide all the geometric primitives needed. The arrange crate is primarily orchestration logic, not geometric computation.

## Common Pitfalls

### Pitfall 1: Coordinate System Mismatch
**What goes wrong:** Mixing f64 millimeter values with i64 Coord values (COORD_SCALE = 1_000_000).
**Why it happens:** Convex hull operates on IPoint2 (i64), but mesh vertices are f64 Point3, and config values (spacing, margin) are f64 mm.
**How to avoid:** Convert mesh vertices to IPoint2 early (multiply by COORD_SCALE, cast to i64). Convert config values to Coord at the boundary. Keep all internal placement math in i64 Coord space.
**Warning signs:** Off-by-million-x placement, parts at wrong scale.

### Pitfall 2: Bed Shape Parsing
**What goes wrong:** bed_shape is a string like "0x0,250x0,250x210,0x210" -- the 'x' separator looks like a dimension, not a coordinate separator.
**Why it happens:** Format inherited from PrusaSlicer/OrcaSlicer where bed_shape uses "XxY" pairs separated by commas.
**How to avoid:** Parse each comma-separated token, split on 'x', convert to f64, then to Coord. Handle both integer and float values. Handle circular beds (delta printers may use different format).
**Warning signs:** Assertion failures on non-rectangular beds, panic on float parsing.

### Pitfall 3: Convex Hull Degeneracy
**What goes wrong:** Meshes with all vertices at same XY (thin vertical pillars) produce degenerate or empty convex hulls.
**Why it happens:** XY projection of a vertical line is a point, not a polygon.
**How to avoid:** After computing convex hull, check that it has >= 3 points and non-zero area. Fall back to bounding box footprint for degenerate cases.
**Warning signs:** Empty hull, division by zero in area computation.

### Pitfall 4: Rotation Accumulation
**What goes wrong:** Applying discrete rotations to already-placed hulls causes floating-point drift.
**Why it happens:** Rotating polygon vertices by 45 degrees eight times doesn't return to exactly the original position.
**How to avoid:** Always rotate from the original (unrotated) convex hull. Pre-compute all rotation variants from the original and cache them.
**Warning signs:** Parts slowly drifting in position tests, non-deterministic placement.

### Pitfall 5: Sequential Back-to-Front vs Shortest-First Conflict
**What goes wrong:** The existing sequential.rs orders shortest-first (for general sequential printing), but the arrange phase specifies back-to-front (for gantry collision avoidance during arrangement).
**Why it happens:** Different ordering goals: arrangement wants to avoid gantry collisions during printing, while existing code minimizes height-based collisions.
**How to avoid:** The arrange crate computes its own print order specifically for sequential arrangement (back-to-front by Y coordinate). This is separate from the engine's existing shortest-first ordering.
**Warning signs:** Gantry collision despite passing validation.

### Pitfall 6: Offset Polygon Collapse
**What goes wrong:** Small parts with large brim/raft margins produce invalid expanded footprints.
**Why it happens:** offset_polygon with large positive delta on tiny polygons can produce self-intersecting results.
**How to avoid:** The existing offset_polygon handles collapse by returning empty Vec. Check for empty result and use original footprint + bounding box expansion as fallback.
**Warning signs:** Parts with brim disappearing from arrangement.

## Code Examples

### Projecting Mesh to XY Convex Hull
```rust
use slicecore_geo::convex_hull;
use slicecore_math::{Coord, IPoint2, COORD_SCALE};

fn compute_footprint(vertices: &[Point3]) -> Vec<IPoint2> {
    let xy_points: Vec<IPoint2> = vertices
        .iter()
        .map(|v| IPoint2::new(
            (v.x * COORD_SCALE as f64) as Coord,
            (v.y * COORD_SCALE as f64) as Coord,
        ))
        .collect();
    convex_hull(&xy_points)
}
```

### Expanding Footprint for Spacing
```rust
use slicecore_geo::{offset_polygon, JoinType, ValidPolygon};
use slicecore_math::COORD_SCALE;

fn expand_footprint(
    hull: &ValidPolygon,
    spacing_mm: f64,
    brim_width_mm: f64,
) -> Vec<ValidPolygon> {
    let total_expansion = spacing_mm / 2.0 + brim_width_mm; // half-spacing per side
    let delta = (total_expansion * COORD_SCALE as f64) as i64;
    offset_polygon(hull, delta, JoinType::Round).unwrap_or_default()
}
```

### Collision Detection Between Two Footprints
```rust
use slicecore_geo::polygon_intersection;

fn footprints_overlap(a: &ValidPolygon, b: &ValidPolygon) -> bool {
    match polygon_intersection(&[a.clone()], &[b.clone()]) {
        Ok(result) => !result.is_empty(),
        Err(_) => true, // Conservative: assume overlap on error
    }
}
```

### Overhang Area Scoring for Auto-Orient
```rust
use slicecore_math::Vec3;

fn overhang_score(normals: &[Vec3], face_areas: &[f64], threshold_cos: f64) -> f64 {
    let z_up = Vec3::new(0.0, 0.0, 1.0);
    normals.iter().zip(face_areas.iter())
        .filter(|(n, _)| n.dot(&z_up) < threshold_cos)
        .map(|(_, area)| area)
        .sum()
}
```

### Bed Shape Parsing
```rust
fn parse_bed_shape(bed_shape: &str) -> Vec<IPoint2> {
    bed_shape.split(',')
        .filter_map(|pair| {
            let parts: Vec<&str> = pair.trim().split('x').collect();
            if parts.len() == 2 {
                let x = parts[0].parse::<f64>().ok()?;
                let y = parts[1].parse::<f64>().ok()?;
                Some(IPoint2::new(
                    (x * COORD_SCALE as f64) as Coord,
                    (y * COORD_SCALE as f64) as Coord,
                ))
            } else {
                None
            }
        })
        .collect()
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Rectangle-based bounding box arrangement | Convex hull footprints with polygon overlap tests | Standard in modern slicers | Much denser packing for irregular shapes |
| Fixed circular gantry clearance | Multiple gantry models (cylinder/rect/polygon) | PrusaSlicer 2.9.1 (March 2025) | More accurate sequential print validation |
| Manual part orientation | Auto-orient via overhang analysis | Common in Cura/PrusaSlicer | Reduces support material automatically |
| Single plate only | Multi-plate virtual splitting | Bambu Studio popularized | Better batch printing workflow |

**Deprecated/outdated:**
- Simple rectangle bounding box packing: superseded by convex hull approaches
- Center-to-center distance collision: superseded by polygon intersection tests

## Open Questions

1. **Auto-orient sampling resolution**
   - What we know: Need to sample rotations around X and Y axes. More samples = better orientation but slower.
   - What's unclear: Optimal balance for v1 (30-degree increments = 144 candidates vs 15-degree = 576).
   - Recommendation: Default to 30-degree increments (144 candidates). This is fast enough for interactive use and provides reasonable coverage. Expose as configurable parameter.

2. **Support volume estimation precision**
   - What we know: Full support volume computation requires slicing. Face-normal overhang area is a fast proxy.
   - What's unclear: How well does overhang area correlate with actual support volume?
   - Recommendation: Use overhang area sum as proxy (fast, O(n) over faces). This matches PrusaSlicer's auto-orient approach and is sufficient for v1.

3. **Bottom-left fill candidate position generation**
   - What we know: Pure raster scanning (check every grid point) is O(n*m) per part. Contour-following approaches are faster.
   - What's unclear: Whether raster scanning at reasonable resolution (e.g., 1mm steps) is fast enough for typical bed sizes (220x220).
   - Recommendation: Start with raster scan at nozzle_diameter resolution. If too slow, switch to contour-following (place along edges of already-placed parts). Profile before optimizing.

4. **Bed shape format variations**
   - What we know: PrusaSlicer/OrcaSlicer use "XxY,XxY,..." format.
   - What's unclear: Whether BambuStudio or other sources use different formats.
   - Recommendation: Parse the "XxY" format. Fall back to rectangular bed from bed_x/bed_y if bed_shape is empty or unparseable.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in (#[test]) + cargo test |
| Config file | Cargo.toml [dev-dependencies] |
| Quick run command | `cargo test -p slicecore-arrange` |
| Full suite command | `cargo test --all-features --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ADV-02 | Sequential printing collision detection | unit | `cargo test -p slicecore-arrange -- sequential` | No - Wave 0 |
| N/A | Bottom-left fill places parts correctly | unit | `cargo test -p slicecore-arrange -- placer` | No - Wave 0 |
| N/A | Convex hull footprint computation | unit | `cargo test -p slicecore-arrange -- footprint` | No - Wave 0 |
| N/A | Multi-plate splitting | unit | `cargo test -p slicecore-arrange -- grouper` | No - Wave 0 |
| N/A | Auto-orient scoring | unit | `cargo test -p slicecore-arrange -- orient` | No - Wave 0 |
| N/A | Bed shape parsing | unit | `cargo test -p slicecore-arrange -- bed` | No - Wave 0 |
| N/A | JSON arrangement output | unit | `cargo test -p slicecore-arrange -- result` | No - Wave 0 |
| N/A | CLI arrange subcommand | integration | `cargo test -p slicecore-cli -- arrange` | No - Wave 0 |
| N/A | End-to-end arrangement | integration | `cargo test -p slicecore-arrange -- integration` | No - Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-arrange`
- **Per wave merge:** `cargo test --all-features --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/slicecore-arrange/` -- entire crate does not exist yet
- [ ] `crates/slicecore-arrange/Cargo.toml` -- workspace member setup
- [ ] All test files -- created alongside implementation

## Sources

### Primary (HIGH confidence)
- **Codebase inspection** -- slicecore-geo (convex_hull, offset_polygon, polygon_intersection, point_in_polygon), slicecore-mesh (rotate, translate, normals, connected_components), slicecore-engine (PrintConfig, MachineConfig, SequentialConfig, CancellationToken, sequential.rs)
- **CONTEXT.md** -- All decisions locked by user

### Secondary (MEDIUM confidence)
- [PrusaSlicer arrange / libnest2d](https://github.com/tamasmeszaros/libnest2d) -- Reference implementation for slicer arrangement, uses NFP (deferred for us)
- [PrusaSlicer 2.9.1 blog](https://blog.prusa3d.com/prusaslicer-2-9-1-smarter-sequential-printing-stronger-multi-material-interlocking_111458/) -- Smart sequential printing arrange feature
- [Bottom-left fill research paper](https://arxiv.org/pdf/2103.08739) -- Fast BLF algorithm with semi-discrete representation
- [Orientation analysis for minimal support](https://www.sciencedirect.com/science/article/abs/pii/S0097849315000564) -- Face normal overhang scoring approach

### Tertiary (LOW confidence)
- [rectangle-pack crate](https://crates.io/crates/rectangle-pack) -- Rectangle-only, not suitable for convex hull packing
- [binpack2d crate](https://crates.io/crates/binpack2d) -- Rectangle-only, not suitable

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries are existing workspace crates already in use
- Architecture: HIGH - Clear crate structure following established project patterns (Layer 2 crate)
- Pitfalls: HIGH - Based on direct codebase inspection of coordinate systems, bed_shape format, and existing sequential.rs code
- Algorithm: MEDIUM - Bottom-left fill is well-understood but implementation details for convex polygon variant require some experimentation

**Research date:** 2026-03-11
**Valid until:** 2026-04-11 (stable domain, no external dependency changes expected)
