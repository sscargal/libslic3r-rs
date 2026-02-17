# Phase 5: Support Structures - Research

**Researched:** 2026-02-17
**Domain:** FDM support structure generation (overhang detection, traditional supports, tree supports, bridge handling, interface layers, manual overrides)
**Confidence:** MEDIUM-HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Support generation strategy
- **Detection approach:** Hybrid — layer-by-layer analysis for speed, downward raycast validation for accuracy on complex geometry
- **Support type selection:** Automatic based on geometry (small contact areas -> tree, large areas -> traditional) with user override via config to force specific type
- **Overhang angle threshold:** Claude's discretion — research common slicer patterns for sensible defaults
- **Tiny support regions:** Claude's discretion — determine appropriate minimum area filtering or single-point pillar conversion based on printability research

#### Tree support architecture
- **Growth direction:** Bottom-up from build plate — start at bed, branch upward toward overhangs for stability
- **Branch merging:** Claude's discretion — research tree support best practices for optimal merging rules (distance threshold with stability checking)
- **Branch structure:** Support both organic (curved, natural-looking) and geometric (angular, easier to slice) tree structures
  - Automatic selection based on model geometry (may need both types in one model)
  - User override to force organic or geometric style
- **Diameter progression:** Implement all taper methods (linear, exponential, load-based)
  - Automatic selection based on model geometry or default to single best option
  - User override to select specific taper method

#### Bridge detection and overrides
- **Bridge detection criteria:** Combined approach — angle threshold (<10 degrees from horizontal) + endpoint-based (both ends supported) + minimum span length (>5mm) for accurate classification
- **Bridge-specific settings:** All settings configurable — speed, fan, flow, acceleration, line width
  - Allows full control for advanced users
  - Research common bridge parameter adjustments for smart defaults
- **Manual override system:** Support all non-GUI options in Phase 5
  - **Mesh-based enforcers/blockers:** Import separate STL meshes marking enforce/block regions (PrusaSlicer approach)
  - **Volume modifier approach:** Define 3D volumes (boxes, cylinders, spheres) with enforce/block properties (programmatic and GUI-compatible)
  - **Painted regions:** Deferred to future GUI phase (not supported in CLI/API)
- **Override priority rules:**
  - **Default:** "Warn on conflicts" — manual overrides win, but log warnings when blocking removes critical supports
  - **Optional:** "Smart merge" can be set as default — combine auto + manual intelligently
  - **Interactive:** When conflicts detected, show smart merge results and allow user to accept/decline

#### Interface layers and surface quality
- **Interface thickness:** Configurable with smart default (recommend 2 layers = ~0.4mm as default)
- **Interface density:** Configurable ratio between interface and body density
  - Research common ratios (e.g., 100% interface / 15% body vs 50% interface / 15% body)
  - Allow user to specify custom ratio
- **Separation gap:** Configurable with material-specific defaults
  - Different materials have different adhesion properties (PLA vs PETG vs ABS)
  - Default values per material (e.g., PLA = 0.2mm, PETG = 0.25mm)
  - User can override for specific models
- **Quality vs removability balance:** User-selectable quality preset system
  - Low/Medium/High quality presets that adjust multiple interface parameters together
  - Low = easier removal, rougher surface (larger gaps, fewer interface layers)
  - High = best surface quality, harder removal (tighter gaps, more interface layers)
  - Presets provide simple tuning while advanced users can adjust individual parameters

### Claude's Discretion
- Exact overhang angle threshold defaults (research 40-50 degree range for various materials)
- Minimum support area filtering approach (area threshold vs single-point pillar conversion)
- Tree branch merging rules and distance thresholds
- Interface layer pattern (solid vs sparse vs specific pattern types)
- Default bridge parameter adjustments (speed/fan/flow values)
- Default interface:body density ratios
- Material-specific separation gap distances

### Deferred Ideas (OUT OF SCOPE)
- **GUI-based painted support regions:** Paint enforce/block areas directly on 3D model surface — requires GUI, deferred to future UI phase
- **Support structure preview/visualization:** 3D rendering of generated supports — useful but GUI-focused, deferred to visualization phase
- **Automatic support material switching:** Multi-material support with different materials for interface vs body — Phase 6 multi-material work
- **Support raft generation:** Raft beneath supports for bed adhesion — could be combined with build plate adhesion features in Phase 6
</user_constraints>

## Summary

Phase 5 adds support structure generation to the slicing pipeline, enabling users to print models with overhangs, bridges, and unsupported geometry. The implementation covers five major subsystems: (1) overhang detection using a hybrid layer-by-layer + raycast approach, (2) traditional grid/line support generation, (3) bottom-up tree supports with both organic and geometric branching, (4) bridge detection and specialized print settings, and (5) a manual override system with mesh-based enforcers/blockers and volume modifiers.

The existing codebase provides strong foundations for this phase. The `slicecore-mesh` crate already has a BVH with ray intersection (`intersect_ray`) needed for raycast validation. The `slicecore-geo` crate provides all required polygon boolean operations (union, intersection, difference) and polygon offsetting via `clipper2-rust`. The `slicecore-slicer` crate produces `SliceLayer` with contour polygons, and the `slicecore-engine` crate has the pipeline orchestrator with `FeatureType` enum and `ToolpathSegment` types that need to be extended for support features. The `TriangleMesh` already computes per-face normals, which is the starting point for overhang detection.

The recommended architecture is to add support logic primarily within `slicecore-engine` as a new `support` module (similar to how `infill`, `perimeter`, `surface` modules exist), with some utilities potentially in `slicecore-geo` and `slicecore-mesh`. This matches the existing pattern where algorithm crates are dependencies but the engine module orchestrates the pipeline. A new `slicecore-supports` crate is envisioned by the architecture doc, but given the current codebase pattern where perimeters, infill, and surface classification all live in `slicecore-engine`, it is pragmatic to start with a `support` module in `slicecore-engine` and extract to a separate crate later if needed.

**Primary recommendation:** Build the support pipeline as modules within `slicecore-engine`, following the existing pattern. Implement in order: overhang detection -> traditional supports -> bridge detection -> tree supports -> interface layers -> manual overrides. Use existing BVH ray intersection and clipper2-rust polygon operations throughout.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clipper2-rust | 1.0 | Polygon boolean ops, offsetting for support region computation | Already used in slicecore-geo; provides union/difference/intersection/offset needed for support region calculation |
| slicecore-mesh (BVH) | internal | Ray intersection for overhang validation | Already has `intersect_ray` with Moller-Trumbore; needed for raycast validation of overhangs |
| slicecore-geo | internal | Polygon booleans, offset, area, point-in-polygon | All support region computation relies on 2D polygon operations |
| serde | 1.x | Config serialization for support parameters | Already used throughout for PrintConfig |
| toml | 0.8 | Config deserialization | Already used for PrintConfig |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| ordered-float | 4.x | Reliable f64 comparison in spatial data structures | Tree support node merging requires distance comparisons; consider for BTreeMap keys |
| rayon | 1.x | Parallel per-layer support computation | Future optimization; not needed for initial implementation but worth structuring for |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| In-engine support module | Separate `slicecore-supports` crate | Architecture doc recommends separate crate, but current code pattern keeps algorithms in engine. Start in-engine, extract later if modularity needed |
| Custom tree data structure | petgraph | petgraph adds dependency for graph operations; custom tree struct is simpler and more cache-friendly for this use case |
| BVH raycast for all detection | Layer-only polygon comparison | Layer comparison alone misses complex geometry (cantilevers with internal voids); hybrid approach per user decision |

**Installation:**
No new external crates strictly required for initial implementation. The existing workspace dependencies suffice. `ordered-float` is optional.

## Architecture Patterns

### Recommended Module Structure
```
crates/slicecore-engine/src/
├── support/
│   ├── mod.rs              # Module root, public API, SupportResult type
│   ├── config.rs           # SupportConfig struct, quality presets, material defaults
│   ├── detect.rs           # Overhang detection (layer diff + raycast validation)
│   ├── traditional.rs      # Grid/line support pattern generation
│   ├── tree.rs             # Tree support generation (bottom-up branching)
│   ├── tree_node.rs        # TreeNode, Branch, merging logic
│   ├── bridge.rs           # Bridge detection and bridge-specific settings
│   ├── interface.rs        # Interface layer generation (dense contact layers)
│   ├── override_system.rs  # Enforcer/blocker mesh and volume modifier processing
│   └── conflict.rs         # Override conflict detection, smart merge, warnings
├── config.rs               # Extended PrintConfig with support fields
├── engine.rs               # Pipeline integration point
├── toolpath.rs             # Extended FeatureType enum
└── ...existing modules...
```

### Pattern 1: Layer-by-Layer Overhang Detection
**What:** Compare adjacent layer contours using polygon difference to find unsupported overhang regions. For each layer N, compute `layer[N] MINUS offset_inward(layer[N-1])` to find regions that extend beyond the layer below.
**When to use:** Primary fast-path detection for all models.
**Example:**
```rust
/// Detects overhang regions at a given layer by comparing with the layer below.
///
/// Returns polygons representing areas that need support.
pub fn detect_overhangs_layer(
    current_contours: &[ValidPolygon],
    below_contours: &[ValidPolygon],
    overhang_angle: f64,       // radians from vertical
    layer_height: f64,
    extrusion_width: f64,
) -> Vec<ValidPolygon> {
    if below_contours.is_empty() {
        // First layer or empty below -> no overhang (supported by bed)
        return Vec::new();
    }

    // Compute the maximum horizontal offset allowed at this angle.
    // tan(overhang_angle) = horizontal_offset / layer_height
    let max_offset = layer_height * overhang_angle.tan();

    // Expand the below-layer contours by the max offset.
    let expanded_below = offset_polygons(
        below_contours,
        mm_to_coord(max_offset),
        JoinType::Miter,
    ).unwrap_or_default();

    // Overhang = current layer regions NOT covered by expanded below.
    polygon_difference(current_contours, &expanded_below)
        .unwrap_or_default()
}
```

### Pattern 2: Raycast Validation
**What:** For detected overhang regions, cast rays downward from sample points to verify they truly lack support in 3D (not just from layer comparison).
**When to use:** Secondary validation pass for complex geometry where layer comparison may be inaccurate (e.g., models with internal voids).
**Example:**
```rust
/// Validates overhang regions via downward raycasting against the mesh.
///
/// For each candidate overhang point, casts a ray downward. If the ray
/// hits the model surface within a short distance, the point is actually
/// supported (internal feature) and should be excluded.
pub fn validate_overhangs_raycast(
    candidate_regions: &[ValidPolygon],
    mesh: &TriangleMesh,
    layer_z: f64,
    sample_spacing: f64,  // mm between sample points
) -> Vec<ValidPolygon> {
    let bvh = mesh.bvh();
    let down = Vec3::new(0.0, 0.0, -1.0);

    // Sample points within candidate regions
    // Cast ray downward from each point
    // If ray hits model within a few layer heights -> actually supported
    // Return only truly unsupported regions
    // ...
}
```

### Pattern 3: Bottom-Up Tree Support Growth
**What:** Start from build plate, grow tree trunks upward, branch toward overhang contact points. Merge nearby branches for stability and material savings.
**When to use:** When support type selection chooses tree (small contact areas) or user forces tree mode.
**Example:**
```rust
/// A node in the tree support structure.
#[derive(Clone, Debug)]
pub struct TreeNode {
    /// Position in XY plane (mm).
    pub position: Point2,
    /// Z height of this node (mm).
    pub z: f64,
    /// Radius/diameter at this node.
    pub radius: f64,
    /// Children (branches going upward).
    pub children: Vec<TreeNode>,
    /// Whether this node contacts the model.
    pub is_contact: bool,
}

/// Generates tree supports bottom-up from build plate to overhangs.
pub fn generate_tree_supports(
    overhang_points: &[(Point2, f64)],  // (xy, z) contact points
    mesh: &TriangleMesh,
    config: &SupportConfig,
) -> Vec<TreeNode> {
    // 1. Identify contact points on overhang surfaces
    // 2. Project contact points downward to find viable trunk positions
    // 3. Start trunks at build plate (z=0)
    // 4. Grow trunks upward, branching toward contact points
    // 5. Merge nearby branches within distance threshold
    // 6. Apply taper (linear/exponential/load-based)
    // 7. Return tree structure for slicing into per-layer polygons
}
```

### Pattern 4: Support-to-Toolpath Conversion
**What:** Convert support structure geometry into per-layer toolpath segments with appropriate `FeatureType` variants.
**When to use:** After support generation, during toolpath assembly.
**Example:**
```rust
// New FeatureType variants needed:
pub enum FeatureType {
    // ...existing variants...
    Support,              // Main support body
    SupportInterface,     // Dense interface layer near model
    Bridge,               // Bridge extrusion (specialized speed/flow)
}
```

### Anti-Patterns to Avoid
- **Computing supports after toolpaths:** Supports must be computed BEFORE toolpath assembly because they add geometry that gets perimeters/infill. The correct order is: slice -> detect overhangs -> generate supports -> add support contours to layer data -> then run perimeters/infill/toolpath on combined geometry.
- **Top-down tree generation for bottom-up growth:** The user locked in bottom-up growth. Do not implement top-down (PrusaSlicer's approach) even though it is better documented. Bottom-up requires different data structures (growing upward, merging at higher Z).
- **Ignoring XY gap between support and model:** Without an XY gap, supports fuse to the model and cannot be removed. Always offset support regions inward by `support_xy_gap` before generating toolpaths.
- **Dense support everywhere:** Support body should be sparse (15-25% density). Only interface layers near the model should be dense. Dense body wastes material and is harder to remove.
- **Single-pass bridge detection:** Bridge detection needs both geometric analysis (angle + endpoints) AND minimum span length filtering. Single-criterion detection produces too many false positives.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Polygon boolean operations | Custom polygon clipping | `clipper2-rust` via `slicecore-geo` | Polygon clipping is notoriously difficult to get right with all edge cases (self-intersection, touching edges, collinear vertices). clipper2 handles all of these. |
| Polygon offsetting | Custom offset algorithm | `clipper2-rust` `inflate_paths_64` via `slicecore-geo::offset` | Offset with correct corner handling (miter/round/square) is complex. Already proven in perimeter generation. |
| Ray-triangle intersection | Custom ray-tri test | `slicecore-mesh::bvh::intersect_ray` (Moller-Trumbore + BVH) | BVH acceleration with SAH already implemented and tested. Adding custom raycasting would be slower and buggier. |
| Point-in-polygon test | Custom containment test | `slicecore-geo::point_in_polygon` (winding number) | Already implemented with proper edge-case handling. Needed for checking if support sample points are inside regions. |
| Signed area computation | Manual shoelace | `slicecore-geo::signed_area_i64` | Already implemented. Needed for minimum area filtering of support regions. |
| Convex hull | Custom hull algorithm | `slicecore-geo::convex_hull` (Graham scan) | May be useful for tree support trunk placement optimization. Already implemented. |

**Key insight:** The existing `slicecore-geo` and `slicecore-mesh` crates provide nearly all the geometric primitives needed. The support phase is primarily an algorithmic layer that composes these existing operations, not a geometric foundation phase.

## Common Pitfalls

### Pitfall 1: Support-Model Adhesion (Z-Gap)
**What goes wrong:** Supports that touch the model surface directly become impossible to remove without damaging the model.
**Why it happens:** Missing or insufficient Z-distance gap between the top of the support interface and the bottom of the model.
**How to avoid:** Always insert a configurable Z-gap (default: 1 layer height worth of space) between the topmost support interface layer and the model. Material-specific defaults: PLA=0.2mm, PETG=0.25mm, ABS=0.2mm.
**Warning signs:** Test prints where support cannot be removed; support leaves significant marks on model surface.

### Pitfall 2: XY-Gap Missing
**What goes wrong:** Support material fuses to the model walls, making lateral removal impossible.
**Why it happens:** Support regions are generated exactly at the overhang boundary without an XY offset.
**How to avoid:** Inward-offset all support regions by `support_xy_gap` (default: 0.5-1.0 extrusion widths) before generating toolpaths. This creates a small clearance between support and model walls.
**Warning signs:** Support structure contours that exactly match model contours on any edge.

### Pitfall 3: Tree Support Collision with Model
**What goes wrong:** Tree branches grow through or into the model geometry.
**Why it happens:** Bottom-up growth without collision checking against the model volume.
**How to avoid:** At each growth step, check that the branch cylinder does not intersect the model. Use polygon difference (branch cross-section minus model contour at that Z) to verify clearance. Cache model contours per layer for efficiency.
**Warning signs:** Tree branches that appear inside the model in layer previews.

### Pitfall 4: Bridge Detection False Positives
**What goes wrong:** Normal overhang regions are classified as bridges, getting incorrect speed/fan/flow settings.
**Why it happens:** Using angle-only detection without verifying both-end support and minimum span length.
**How to avoid:** Require all three criteria: (1) near-horizontal angle (<10 degrees from horizontal), (2) both ends of the span must be supported, (3) span length >= 5mm. Regions that fail any criterion are overhangs, not bridges.
**Warning signs:** Tiny horizontal faces incorrectly classified as bridges; model surfaces receiving bridge settings when they should get normal overhang settings.

### Pitfall 5: Performance Death by Polygon Booleans
**What goes wrong:** Support generation takes minutes or crashes on complex models with many layers.
**Why it happens:** Running polygon difference/union on every layer without caching or early-exit optimization.
**How to avoid:** (1) Skip layers where contours are identical to the layer below (common in vertical walls). (2) Use bounding box pre-check before running full boolean operations. (3) Cache expanded contours across layers when the expansion is the same. (4) Process layers in parallel when possible.
**Warning signs:** Support generation time growing super-linearly with layer count; memory spikes during boolean operations.

### Pitfall 6: Tiny Support Regions
**What goes wrong:** Generating support for extremely small overhang areas that are unprintable (too small for nozzle to deposit material).
**Why it happens:** No minimum area filtering after overhang detection.
**How to avoid:** Filter out support regions smaller than `min_support_area` (recommended: 2x extrusion_width squared, ~0.4mm squared for 0.4mm nozzle). Optionally convert very small regions to single-pillar supports if they are critical contact points.
**Warning signs:** Support structures with tiny isolated pillars that topple during printing or clog nozzle movement.

### Pitfall 7: Enforcer/Blocker Priority Conflicts
**What goes wrong:** User-placed enforcer and auto-generated supports conflict, producing either redundant supports or dangerous gaps.
**Why it happens:** No clear priority system or conflict detection.
**How to avoid:** Implement priority: (1) Blockers always remove support. (2) Enforcers always add support. (3) When a blocker removes auto-generated support that is structurally critical, emit a warning. (4) "Smart merge" mode tries to reconcile both.
**Warning signs:** User placing blocker over critical overhang without warning; enforcer duplicating existing auto-support.

## Code Examples

Verified patterns from the existing codebase:

### Existing Polygon Difference (used for overhang detection)
```rust
// Source: slicecore-geo/src/boolean.rs
// Already used in surface.rs for top/bottom surface detection.
// Same pattern applies to overhang detection.
use slicecore_geo::{polygon_difference, offset_polygons, JoinType};

let overhang_regions = polygon_difference(
    current_layer_contours,
    &expanded_below_contours,
).unwrap_or_default();
```

### Existing BVH Ray Intersection (used for raycast validation)
```rust
// Source: slicecore-mesh/src/bvh.rs
// BVH already supports intersect_ray with Moller-Trumbore.
let bvh = mesh.bvh();
let origin = Point3::new(x, y, layer_z);
let direction = Vec3::new(0.0, 0.0, -1.0); // downward
let hit = bvh.intersect_ray(&origin, &direction, mesh.vertices(), mesh.indices());
// hit.map(|h| h.t) gives distance to first surface below
```

### Extending FeatureType for Support Features
```rust
// Source: slicecore-engine/src/toolpath.rs
// Current FeatureType enum needs these additions:
pub enum FeatureType {
    OuterPerimeter,
    InnerPerimeter,
    SolidInfill,
    SparseInfill,
    Skirt,
    Brim,
    GapFill,
    VariableWidthPerimeter,
    Travel,
    // New for Phase 5:
    Support,           // Support body extrusion
    SupportInterface,  // Dense interface near model
    Bridge,            // Bridge extrusion
}
```

### Extending PrintConfig for Support Parameters
```rust
// Source: slicecore-engine/src/config.rs
// Following the existing pattern of nested config structs (ScarfJointConfig).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SupportConfig {
    /// Enable support generation.
    pub enabled: bool,
    /// Support type: auto, traditional, tree, or none.
    pub support_type: SupportType,
    /// Overhang angle threshold in degrees (from vertical).
    /// Surfaces steeper than this from vertical get support.
    pub overhang_angle: f64,
    /// Minimum support region area in mm^2.
    pub min_support_area: f64,
    /// Support body density (0.0-1.0).
    pub support_density: f64,
    /// Support body pattern.
    pub support_pattern: SupportPattern,
    /// Number of interface layers between support and model.
    pub interface_layers: u32,
    /// Interface layer density (0.0-1.0).
    pub interface_density: f64,
    /// Interface layer pattern.
    pub interface_pattern: InterfacePattern,
    /// Z separation gap between support top and model bottom (mm).
    pub z_gap: f64,
    /// XY clearance between support and model walls (mm).
    pub xy_gap: f64,
    /// Support on build plate only (no support on model).
    pub build_plate_only: bool,
    /// Bridge detection enabled.
    pub bridge_detection: bool,
    /// Bridge settings.
    pub bridge: BridgeConfig,
    /// Tree support specific settings.
    pub tree: TreeSupportConfig,
    /// Quality preset (overrides individual settings).
    pub quality_preset: Option<QualityPreset>,
}
```

### Pipeline Integration Point
```rust
// Source: slicecore-engine/src/engine.rs
// Support generation inserts between slicing and per-layer processing.
// The pipeline becomes:
// 1. Slice mesh into layers
// 2. ** Detect overhangs across all layers **
// 3. ** Generate support structures **
// 4. ** Merge support contours into layer data **
// 5. Per-layer processing: perimeters, surface classify, infill, toolpath
// 6. G-code generation

// Support regions are added as additional contours per layer,
// tagged so they receive support-specific perimeter/infill settings.
```

## Claude's Discretion Recommendations

### Overhang Angle Threshold Defaults
**Recommendation:** Default overhang angle = 45 degrees from vertical (equivalently, 45 degrees from horizontal).
**Rationale:** The 45-degree rule is the universal industry standard default. PrusaSlicer, Cura, OrcaSlicer, and BambuStudio all use approximately 45 degrees as the default. Material-specific research shows:
- **PLA:** Can handle 55-60 degrees from vertical with good cooling. Conservative support threshold: 50 degrees.
- **PETG:** Handles 45-50 degrees from vertical. Keep default at 45 degrees.
- **ABS:** Handles 40-45 degrees from vertical due to poor cooling. Consider 40 degrees for ABS profiles.

**Implementation:** Default = 45 degrees. Material-specific profiles can override (PLA=50, PETG=45, ABS=40). Configurable per-model via `SupportConfig.overhang_angle`.

**Confidence:** HIGH — multiple sources confirm 45 degrees as universal default.

### Minimum Support Area Filtering
**Recommendation:** Use area threshold filtering with optional single-point pillar conversion.
- **Default minimum area:** `(2.0 * extrusion_width)^2` = approximately 0.77 mm^2 for 0.4mm nozzle (about 0.88mm x 0.88mm square equivalent).
- **Regions below threshold but above 1 extrusion_width squared:** Convert to single-pillar supports (a thin column of 1 extrusion width diameter).
- **Regions below 1 extrusion_width squared:** Discard entirely (unprintable).
**Rationale:** Very small support regions are unprintable and waste material. But some small overhangs (tips, pointed features) genuinely need a single support pillar. Two-tier filtering handles both cases.

**Confidence:** MEDIUM — based on practical printability analysis. No standard values found in slicer documentation; these are engineering judgments.

### Tree Branch Merging Rules
**Recommendation:**
- **Merge distance threshold:** 3x trunk diameter at the merge point (or 5mm, whichever is larger).
- **Stability check:** After merging, verify the combined branch can support the total load above it. If the merged trunk would need to be wider than `max_trunk_diameter` (configurable, default 10mm), don't merge.
- **Merge priority:** Merge closest branches first (greedy nearest-neighbor).
- **Minimum branch angle:** Branches must diverge by at least 15 degrees from the trunk to avoid sharp turns that are hard to print.
**Rationale:** Literature on tree support algorithms (Cura, PrusaSlicer) suggests merging nearby branches reduces material and print time significantly. The 3x diameter threshold provides a good balance between merging aggressively (saves material) and maintaining structural separation (each branch has room to print).

**Confidence:** MEDIUM — values are engineering estimates based on available research. Will need tuning with real-world print tests.

### Interface Layer Pattern
**Recommendation:** Use **rectilinear** as the default interface pattern.
- **Rectilinear (default):** Straight parallel lines, alternating direction per layer. Best for non-soluble supports — easy to detect separation layer.
- **Concentric (alternative):** Following model contours. Better surface finish but harder to remove.
- **Grid (alternative):** Cross-hatched rectilinear. Very dense, good for large flat surfaces.
**Rationale:** PrusaSlicer uses rectilinear as default for non-soluble interface. Concentric is recommended only for soluble supports. Rectilinear provides the best balance of surface quality and removability for single-material printing.

**Confidence:** HIGH — PrusaSlicer and OrcaSlicer documentation explicitly recommend rectilinear for non-soluble support interfaces.

### Default Bridge Parameter Adjustments
**Recommendation:**
- **Bridge speed:** 30 mm/s (reduced from typical 45-80 mm/s print speeds). Slower printing allows filament to stretch and solidify before sagging.
- **Bridge fan speed:** 100% (maximum cooling). Rapid cooling is critical for bridge solidification.
- **Bridge flow ratio:** 0.85 (85% of normal flow). Slightly reduced flow prevents excess material weight that causes sag. PrusaSlicer uses 0.7 as internal default, but 0.85 is more forgiving for users.
- **Bridge acceleration:** 500 mm/s^2 (reduced). Gentle acceleration prevents filament from snapping at bridge start.
- **Bridge line width:** 100% of normal extrusion width (no change needed by default).
**Rationale:** Based on PrusaSlicer bridge defaults (flow=0.7), Cura experimental bridge settings, and community testing. The 0.85 flow is more conservative than PrusaSlicer's 0.7 to provide better results for users who don't fine-tune.

**Confidence:** MEDIUM-HIGH — values are well-documented in slicer defaults and community guides. The specific flow ratio (0.85 vs 0.7) is an engineering judgment.

### Default Interface:Body Density Ratios
**Recommendation:**
- **Support body density:** 15% (sparse, easy to remove)
- **Interface density:** 80% (dense, good surface contact)
- **Ratio:** Interface is approximately 5.3x denser than body
**Rationale:** PrusaSlicer uses ~15% body density. OrcaSlicer suggests 50-70% interface density for easier removal, but higher density (80%) produces better surface finish. Users can adjust via the quality preset system.

Quality presets:
- **Low (easy removal):** body=10%, interface=50%, z_gap=0.3mm, interface_layers=1
- **Medium (balanced):** body=15%, interface=80%, z_gap=0.2mm, interface_layers=2
- **High (best surface):** body=20%, interface=100%, z_gap=0.15mm, interface_layers=3

**Confidence:** MEDIUM — ratios are based on practical experience. Exact values will need print testing.

### Material-Specific Separation Gap Distances
**Recommendation:**
- **PLA:** z_gap = 0.2mm (1 layer height at 0.2mm). PLA separates cleanly with minimal gap.
- **PETG:** z_gap = 0.25mm. PETG is stickier than PLA, needs slightly more gap.
- **ABS:** z_gap = 0.2mm. ABS with enclosed printing separates similarly to PLA.
- **TPU:** z_gap = 0.3mm. Flexible materials need more gap to prevent bonding.
- **Nylon/PA:** z_gap = 0.25mm. Similar to PETG adhesion characteristics.

XY gaps (apply universally):
- **xy_gap:** 0.4mm default (approximately 1 extrusion width for 0.4mm nozzle)

**Confidence:** MEDIUM — based on community guides and slicer defaults. The 0.2mm PLA default is well-established. PETG and others are less standardized.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Top-down tree supports (grow from overhang down) | Bottom-up tree supports (grow from bed up) | Cura 5.x / OrcaSlicer 2.x (2023-2024) | Better stability, branches are self-supporting during print |
| Uniform support density | Separate body/interface density | PrusaSlicer 2.4+ (2022) | Dramatically improved removability while maintaining surface quality |
| Binary support (yes/no per region) | 4-tier overhang angle control | OrcaSlicer (2023) | Graduated speed/fan control produces better surfaces on mild overhangs |
| Manual angle threshold only | Auto-detect support type (tree vs traditional) | BambuStudio (2023) | Selects optimal support strategy per-region automatically |
| Fixed bridge settings | Per-bridge configurable settings | OrcaSlicer 2.x (2024) | Internal vs external bridges can have different parameters |

**Deprecated/outdated:**
- **Uniform dense supports:** Modern slicers all use sparse body + dense interface. Dense-everywhere supports waste material and are hard to remove.
- **Grid-only support pattern:** Line pattern is now preferred for single-material supports as it is easier to remove (lines peel off, grid must be broken).
- **Top-down-only tree generation:** Bottom-up approaches produce more stable, self-supporting structures.

## Open Questions

1. **Exact tree branch collision detection performance**
   - What we know: BVH ray intersection is available and fast for single rays. Checking branch-model collision requires testing a cylindrical volume against the mesh.
   - What's unclear: Whether per-layer polygon difference (branch circle minus model contour) is fast enough for hundreds of branches, or if we need spatial caching.
   - Recommendation: Start with per-layer polygon difference (simple, correct). Profile on test models. Add caching if needed.

2. **Support region merging across layers**
   - What we know: Each layer independently produces overhang regions. But support pillars need to be continuous across layers.
   - What's unclear: The optimal strategy for connecting support regions between layers. PrusaSlicer uses top-down projection; we're doing bottom-up.
   - Recommendation: Project overhang regions downward to build plate. At each layer, the support cross-section is the union of all projected overhang regions above that reach down to this layer. Intersect with model contours to avoid collision.

3. **Organic vs geometric branch selection heuristic**
   - What we know: User decided both should be supported, with automatic selection based on geometry.
   - What's unclear: What heuristic selects organic vs geometric. No clear industry standard for automatic selection.
   - Recommendation: Default to geometric (easier to implement, more predictable). Use organic when the overhang surface is curved (high curvature) and geometric when the overhang surface is flat/angular. Curvature can be estimated from triangle normals in the overhang region.

4. **4-tier overhang control integration**
   - What we know: SUPP-07 requires 4-tier angle control (0-25%, 25-50%, 50-75%, 75%+). This affects speed and fan rather than support placement.
   - What's unclear: Whether this belongs in the support module or the planner/toolpath module.
   - Recommendation: The overhang angle tiers are a print parameter adjustment, not support placement. The support module detects overhangs and generates supports. The 4-tier speed/fan adjustment should be applied during toolpath assembly or G-code generation for perimeters near overhangs (without support). Implement as a separate overhang perimeter marking pass.

5. **Volume modifier geometry parsing**
   - What we know: User decided to support box, cylinder, sphere volumes as enforcers/blockers.
   - What's unclear: Whether these are defined in config files, in a separate mesh file, or via API.
   - Recommendation: Define volume modifiers in the `SupportConfig` as structured data (center, dimensions, type, role). API/CLI creates them programmatically. This is separate from mesh-based enforcers which import STL files.

## Sources

### Primary (HIGH confidence)
- Existing codebase: `slicecore-mesh/src/bvh.rs` — BVH with `intersect_ray`, Moller-Trumbore, SAH construction
- Existing codebase: `slicecore-geo/src/boolean.rs` — polygon union/intersection/difference via clipper2-rust
- Existing codebase: `slicecore-geo/src/offset.rs` — polygon offsetting via clipper2-rust
- Existing codebase: `slicecore-engine/src/surface.rs` — existing layer-comparison pattern for surface classification
- Existing codebase: `slicecore-engine/src/toolpath.rs` — FeatureType enum and toolpath assembly pattern
- Existing codebase: `slicecore-engine/src/config.rs` — PrintConfig pattern with nested config structs
- Design doc: `designDocs/02-ARCHITECTURE.md` — SupportStrategy trait, slicecore-supports crate design
- Design doc: `designDocs/04-IMPLEMENTATION-GUIDE.md` — Support tasks, tree support implementation notes (3 C++ approaches)

### Secondary (MEDIUM confidence)
- [PrusaSlicer Support Material Knowledge Base](https://help.prusa3d.com/article/support-material_1698) — Overhang threshold, support patterns, interface layers
- [Bambu Lab Support Wiki](https://wiki.bambulab.com/en/software/bambu-studio/support) — Support Z distance, material-specific gaps
- [PrusaSlicer Bridging Knowledge Base](https://help.prusa3d.com/article/poor-bridging_1802) — Bridge fan speed 100%, detect bridging perimeters
- [Ultimaker Cura Bridge Settings](https://community.ultimaker.com/topic/22195-introducing-the-experimental-bridging-settings/) — Bridge speed, flow, fan parameters
- [OrcaSlicer Support Guide](https://orcaslicerpro.com/supports-in-orca-slicer/) — Tree support settings, interface density 50-70%
- [Cura Tree Support Guide](https://www.wevolver.com/article/cura-tree-support) — Tree branch angle, diameter, merging concepts
- [3D Printing Overhang Material Strategies](https://3dx.info/material-specific-overhang-strategies-optimizing-angles-for-pla-petg-and-abs/) — PLA 55-60 degrees, PETG 45-50, ABS 40-45
- [Slicer Settings for Perfect Bridges](https://3dx.info/mastering-slicer-settings-for-perfect-bridges-a-deep-dive-into-flow-speed-and-fan-control/) — Bridge flow 0.7-0.9, speed 20-40mm/s
- [PrusaSlicer Support Settings Guide](https://clevercreations.org/prusaslicer-support-settings/) — 7 critical support settings

### Tertiary (LOW confidence)
- Material-specific Z-gap values (PLA=0.2, PETG=0.25): Derived from forum discussions and wiki entries; no single authoritative source. Will need validation through print testing.
- Tree branch merging distance (3x diameter): Engineering estimate. No authoritative source found for specific threshold values.
- Organic vs geometric selection heuristic: No industry standard found. Custom recommendation.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all required libraries are already in the codebase
- Architecture: MEDIUM-HIGH — patterns follow existing codebase conventions; support pipeline integration point is clear
- Algorithms (overhang detection, traditional support): HIGH — well-documented, layer diff approach matches existing surface.rs pattern
- Algorithms (tree support): MEDIUM — bottom-up tree growth is less documented than top-down; will need iterative refinement
- Algorithms (bridge detection): MEDIUM-HIGH — criteria are well-defined by user; implementation is straightforward
- Pitfalls: HIGH — common issues are well-documented in 3D printing community
- Default values: MEDIUM — overhang angle (45 deg) is well-established; material-specific gaps and tree parameters need print validation

**Research date:** 2026-02-17
**Valid until:** 2026-03-17 (30 days — stable domain, algorithms are well-established)
