# Phase 6: G-code Completeness and Advanced Features - Research

**Researched:** 2026-02-17
**Domain:** G-code firmware dialects, multi-material, arc fitting, modifier meshes, dimensional accuracy, advanced print features
**Confidence:** HIGH (codebase fully analyzed, firmware docs verified, algorithms well-understood)

## Summary

Phase 6 transforms the slicing engine from a single-dialect proof-of-concept into a production-complete slicer. The phase covers 17 requirements spanning five major domains: (1) firmware dialect completeness for Klipper/RepRap/Bambu with dialect-specific commands, (2) advanced G-code features including acceleration control, arc fitting, and time/filament estimation, (3) multi-material support with tool changes and purge towers, (4) geometry-level features including modifier meshes, sequential printing, and polyhole conversion, and (5) two new TPMS infill patterns.

The existing codebase is well-structured for this expansion. The `GcodeDialect` enum already has all four variants, and basic start/end sequences exist for all dialects. The `GcodeCommand` enum needs new variants for G2/G3 arcs, acceleration commands, and tool changes. The `FeatureType` enum needs extension for ironing and purge tower. The `PrintConfig` needs substantial additions for multi-material, acceleration, and per-feature flow control. The engine pipeline needs hooks for modifier meshes, sequential printing, and custom G-code injection.

**Primary recommendation:** Organize work into sub-phases by dependency: (A) firmware dialect enrichment + acceleration/jerk, (B) arc fitting + time/filament estimation, (C) per-feature flow + custom G-code + ironing, (D) TPMS infill patterns, (E) modifier meshes + polyhole, (F) multi-material + purge tower + sequential printing, (G) pressure advance calibration pattern.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| slicecore-gcode-io | local | G-code commands, writer, dialect modules, validator | Already exists, extend with new commands |
| slicecore-engine | local | Pipeline orchestrator, toolpath assembly, planner | Already exists, extend with new features |
| slicecore-geo | local | Polygon operations, point-in-polygon, offsets | Already exists, used for modifier mesh region detection |
| slicecore-math | local | Coordinate types, points, vectors, matrices | Already exists, used for arc fitting geometry |
| slicecore-mesh | local | TriangleMesh, BVH spatial queries | Already exists, used for modifier mesh intersection |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde | existing | Serialization of new config types | All new config structs |
| toml | existing | TOML config file parsing | PrintConfig extensions |
| thiserror | existing | Error types | New error variants |

### No New External Dependencies
Phase 6 requires no new external crate dependencies. All algorithms (arc fitting, TPMS surfaces, polyhole, ironing, purge tower) are implemented as pure Rust within existing crates. This maintains the project's pure-Rust, no-FFI constraint and WASM compatibility.

## Architecture Patterns

### Recommended Module Organization

```
crates/slicecore-gcode-io/src/
  commands.rs          # Extended GcodeCommand enum (G2/G3, M204/M205, Txx)
  dialect.rs           # Extended StartConfig/EndConfig for dialect-specific features
  marlin.rs            # Unchanged (already complete)
  klipper.rs           # Extended: SET_PRESSURE_ADVANCE, SET_VELOCITY_LIMIT
  reprap.rs            # Extended: M572, M593 input shaping
  bambu.rs             # Extended: AMS commands, filament slot selection
  validate.rs          # Extended: G2/G3 arc validation, tool change validation
  arc.rs               # NEW: Arc fitting algorithm (line segments -> G2/G3)
  writer.rs            # Extended: multi-tool state tracking

crates/slicecore-engine/src/
  config.rs            # Extended: acceleration, multi-material, per-feature flow
  toolpath.rs          # Extended: FeatureType::Ironing, FeatureType::PurgeTower
  estimation.rs        # NEW: Print time estimation with acceleration model
  filament.rs          # NEW: Filament usage estimation (length, weight, cost)
  ironing.rs           # NEW: Ironing pass generation
  modifier.rs          # NEW: Modifier mesh region detection + setting overrides
  multimaterial.rs     # NEW: Multi-material support, tool changes, purge tower
  sequential.rs        # NEW: Sequential printing (object-by-object)
  polyhole.rs          # NEW: Hole-to-polyhole conversion
  calibration.rs       # NEW: Pressure advance calibration pattern
  custom_gcode.rs      # NEW: Custom G-code injection hooks
  flow_control.rs      # NEW: Per-feature flow rate control
  infill/
    tpms_d.rs          # NEW: TPMS-D (Schwarz Diamond) infill pattern
    tpms_fk.rs         # NEW: TPMS-FK (Fischer-Koch S) infill pattern
```

### Pattern 1: Dialect-Specific Command Emission via Trait Dispatch

**What:** Each firmware dialect emits different commands for the same logical operation (e.g., pressure advance: Marlin=M900, Klipper=SET_PRESSURE_ADVANCE, RepRap=M572).
**When to use:** Any command that varies by firmware dialect.
**Example:**
```rust
// In commands.rs - extend the GcodeCommand enum
pub enum GcodeCommand {
    // ... existing variants ...

    /// Arc move clockwise: G2 X Y I J [E] [F]
    ArcMoveCW {
        x: Option<f64>,
        y: Option<f64>,
        i: f64,  // center offset X relative to start
        j: f64,  // center offset Y relative to start
        e: Option<f64>,
        f: Option<f64>,
    },

    /// Arc move counter-clockwise: G3 X Y I J [E] [F]
    ArcMoveCCW {
        x: Option<f64>,
        y: Option<f64>,
        i: f64,
        j: f64,
        e: Option<f64>,
        f: Option<f64>,
    },

    /// Set acceleration: M204 P{print} T{travel}
    SetAcceleration { print_accel: f64, travel_accel: f64 },

    /// Set jerk/junction: M205 X{x} Y{y} Z{z}
    SetJerk { x: f64, y: f64, z: f64 },

    /// Tool change: T{n}
    ToolChange(u8),
}
```

### Pattern 2: Modifier Mesh as Region-Specific Config Override

**What:** A modifier mesh defines a 3D volume where different print settings apply. The engine checks each layer's contour against modifier volumes and generates separate toolpaths with overridden settings.
**When to use:** ADV-03 modifier meshes.
**Example:**
```rust
/// A modifier mesh that overrides settings within its volume.
pub struct ModifierMesh {
    /// The mesh defining the modification volume.
    pub mesh: TriangleMesh,
    /// Settings to override within this volume.
    pub overrides: SettingOverrides,
}

/// Settings that can be overridden per-region.
pub struct SettingOverrides {
    pub infill_density: Option<f64>,
    pub infill_pattern: Option<InfillPattern>,
    pub wall_count: Option<u32>,
    pub perimeter_speed: Option<f64>,
    // ... other overridable settings
}
```

### Pattern 3: Multi-Material State Machine

**What:** Track the active tool/extruder and emit tool change sequences (retract, park, change, prime, wipe) at material boundaries.
**When to use:** ADV-01 multi-material support.
**Example:**
```rust
/// Multi-material configuration.
pub struct MultiMaterialConfig {
    /// Number of extruders/tools.
    pub tool_count: u8,
    /// Per-tool filament settings.
    pub tools: Vec<ToolConfig>,
    /// Purge tower position [x, y] in mm.
    pub purge_tower_position: [f64; 2],
    /// Purge tower width in mm.
    pub purge_tower_width: f64,
    /// Purge volume per tool change in mm^3.
    pub purge_volume: f64,
}
```

### Pattern 4: Post-Processing Arc Fitting

**What:** Arc fitting is a post-processing step applied to the final G-code command stream. It scans consecutive G1 moves for colinear-arc patterns and replaces them with G2/G3 arcs.
**When to use:** GCODE-11 arc fitting.
**Example:**
```rust
/// Fits arcs to consecutive linear moves within tolerance.
pub fn fit_arcs(
    commands: &[GcodeCommand],
    tolerance: f64,        // max deviation in mm (typically 0.05)
    min_arc_points: usize, // minimum points to form an arc (typically 3)
) -> Vec<GcodeCommand> {
    // Scan windows of consecutive G1 moves
    // Test if points lie on a circular arc within tolerance
    // If yes, replace with G2 or G3 command
    // Preserve E-values by summing extrusion across replaced segments
}
```

### Anti-Patterns to Avoid

- **Monolithic dialect switch:** Do not add giant match blocks in the engine. Keep dialect-specific logic in the gcode-io crate's dialect modules.
- **Hardcoded purge volumes:** Do not hardcode purge tower dimensions or volumes. Make them configurable per material pair.
- **Time estimation from feedrate alone:** Do not estimate time as distance/feedrate. Must model acceleration ramps (trapezoid motion profile) for accuracy within 15%.
- **Arc fitting in toolpath assembly:** Do not fit arcs during toolpath generation. Fit arcs as a post-processing step on the final GcodeCommand stream, after all other processing is complete.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TPMS implicit surfaces | Custom surface evaluator | Standard TPMS formulas + marching squares (same as existing Gyroid) | The formulas are well-established; follow the existing Gyroid pattern |
| Circle fitting | Custom least-squares | Algebraic circle fit (Taubin method or simple 3-point circumcircle) | 3-point circumcircle is exact and O(1) per test |
| Polygon containment | Custom ray-casting | Existing `point_in_poly` from slicecore-geo | Already implemented and tested |
| Convex hull for collision | Custom algorithm | Existing `convex_hull` from slicecore-geo | Already implemented |
| Config serialization | Custom parsers | serde + toml (already in use) | Battle-tested, already integrated |

**Key insight:** Phase 6 is primarily about extending existing well-structured code, not building new foundational systems. The patterns established in Phases 1-5 (typed enums, config-driven behavior, modular dispatch) should be followed consistently.

## Common Pitfalls

### Pitfall 1: Arc Fitting Producing Invalid Extrusion
**What goes wrong:** When replacing multiple G1 moves with a single G2/G3 arc, the total extrusion (E-value) must be preserved. If E-values are simply summed, the extrusion rate along the arc may be wrong because the arc length differs from the sum of line segment lengths.
**Why it happens:** The arc is geometrically different from the polyline it replaces.
**How to avoid:** Compute the arc length geometrically (r * theta) and redistribute the E-value proportionally: `arc_e = total_e * (arc_length / total_line_length)`. Or simply sum E-values since the deviation is within tolerance.
**Warning signs:** Blobs or gaps at arc/line transitions in test prints.

### Pitfall 2: Firmware Dialect Differences in Acceleration Commands
**What goes wrong:** M204 parameters differ between Marlin and RepRapFirmware. Marlin uses `M204 P T R`, RepRapFirmware uses `M204 S` (single value). Klipper uses `SET_VELOCITY_LIMIT ACCEL=`.
**Why it happens:** G-code is not standardized across firmware.
**How to avoid:** Route acceleration commands through the dialect system. Each dialect module formats its own acceleration command.
**Warning signs:** Printer ignores acceleration commands or throws errors.

### Pitfall 3: Print Time Estimation Ignoring Acceleration
**What goes wrong:** Naive time = distance / feedrate gives estimates 30-50% too low because it ignores acceleration and deceleration ramps.
**Why it happens:** Each segment has a ramp-up, cruise, and ramp-down phase (trapezoid profile).
**How to avoid:** Model each move as a trapezoidal velocity profile: time = accel_time + cruise_time + decel_time. Account for junction speed at segment transitions.
**Warning signs:** Estimates consistently under-report actual print times.

### Pitfall 4: Purge Tower Height Synchronization
**What goes wrong:** The purge tower must be printed at every layer where the main model exists, even layers without tool changes, to maintain structural integrity.
**Why it happens:** Without "sparse" purge tower layers, the tower collapses at its first use.
**How to avoid:** Generate sparse purge tower infill on every layer, dense purge on tool-change layers.
**Warning signs:** Purge tower falling over mid-print.

### Pitfall 5: Modifier Mesh Z-Slicing Mismatch
**What goes wrong:** If the modifier mesh is sliced at different Z heights than the model, the region boundaries don't align with layer contours.
**Why it happens:** Modifier mesh uses its own slicing independent of model layers.
**How to avoid:** Slice modifier meshes at the same Z heights as the model layers. Use the model's layer stack, not an independent slice.
**Warning signs:** Modifier regions shift between layers or have gaps.

### Pitfall 6: Sequential Printing Ignoring Gantry Height
**What goes wrong:** Collision detection only checks XY clearance but ignores that the print head assembly has a finite Z clearance height above the nozzle.
**Why it happens:** The extruder carriage, fan duct, and cable bundle create a collision envelope much larger than the nozzle tip.
**How to avoid:** Model the full extruder clearance as a configurable cylinder (radius + height). Objects taller than the clearance height cannot be adjacent.
**Warning signs:** Print head crashes into previously completed objects.

### Pitfall 7: Polyhole Side Count Calculation
**What goes wrong:** Using too few polygon sides makes holes look polygonal; too many sides defeats the purpose.
**Why it happens:** The optimal side count depends on hole diameter and nozzle diameter.
**How to avoid:** Use the Nophead formula: `sides = max(3, ceil(PI / acos(1 - nozzle_diameter / hole_diameter)))`. This ensures each flat segment is shorter than the nozzle, preventing over-extrusion into the hole.
**Warning signs:** Small holes still undersized, large holes look obviously polygonal.

## Code Examples

### Arc Fitting: Three-Point Circle Test

```rust
/// Tests if three points lie on a circle and returns the center and radius.
/// Returns None if points are collinear (infinite radius).
fn circumcircle(p1: Point2, p2: Point2, p3: Point2) -> Option<(Point2, f64)> {
    let ax = p1.x;
    let ay = p1.y;
    let bx = p2.x;
    let by = p2.y;
    let cx = p3.x;
    let cy = p3.y;

    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-10 {
        return None; // Collinear
    }

    let ux = ((ax * ax + ay * ay) * (by - cy)
        + (bx * bx + by * by) * (cy - ay)
        + (cx * cx + cy * cy) * (ay - by))
        / d;
    let uy = ((ax * ax + ay * ay) * (cx - bx)
        + (bx * bx + by * by) * (ax - cx)
        + (cx * cx + cy * cy) * (bx - ax))
        / d;

    let center = Point2::new(ux, uy);
    let radius = ((ax - ux).powi(2) + (ay - uy).powi(2)).sqrt();

    Some((center, radius))
}

/// Tests if a sequence of points fits a circular arc within tolerance.
fn points_fit_arc(
    points: &[Point2],
    tolerance: f64,
) -> Option<(Point2, f64, bool)> {
    if points.len() < 3 {
        return None;
    }

    let first = points[0];
    let mid = points[points.len() / 2];
    let last = points[points.len() - 1];

    let (center, radius) = circumcircle(first, mid, last)?;

    // Check all intermediate points lie within tolerance of the arc.
    for pt in points {
        let dist = ((pt.x - center.x).powi(2) + (pt.y - center.y).powi(2)).sqrt();
        if (dist - radius).abs() > tolerance {
            return None;
        }
    }

    // Determine arc direction (CW or CCW) via cross product.
    let v1x = mid.x - first.x;
    let v1y = mid.y - first.y;
    let v2x = last.x - first.x;
    let v2y = last.y - first.y;
    let cross = v1x * v2y - v1y * v2x;
    let is_ccw = cross > 0.0;

    Some((center, radius, is_ccw))
}
```

### TPMS-D (Schwarz Diamond) Implicit Surface

```rust
/// Evaluates the Schwarz Diamond TPMS implicit function.
/// Returns > 0 inside, < 0 outside, 0 on the surface.
///
/// Formula: sin(x)*sin(y)*sin(z) + sin(x)*cos(y)*cos(z)
///        + cos(x)*sin(y)*cos(z) + cos(x)*cos(y)*sin(z) = 0
fn schwarz_diamond(x: f64, y: f64, z: f64) -> f64 {
    x.sin() * y.sin() * z.sin()
        + x.sin() * y.cos() * z.cos()
        + x.cos() * y.sin() * z.cos()
        + x.cos() * y.cos() * z.sin()
}

/// Evaluates the Fischer-Koch S TPMS implicit function.
///
/// Formula: cos(2x)*sin(y)*cos(z) + cos(2y)*sin(z)*cos(x)
///        + cos(2z)*sin(x)*cos(y) = 0
fn fischer_koch_s(x: f64, y: f64, z: f64) -> f64 {
    (2.0 * x).cos() * y.sin() * z.cos()
        + (2.0 * y).cos() * z.sin() * x.cos()
        + (2.0 * z).cos() * x.sin() * y.cos()
}
```

### Polyhole Side Count (Nophead Formula)

```rust
/// Computes the number of polygon sides for a polyhole.
///
/// Uses the Nophead formula to determine the optimal number of sides
/// such that each flat segment is shorter than the nozzle diameter.
fn polyhole_sides(hole_diameter: f64, nozzle_diameter: f64) -> u32 {
    let ratio = nozzle_diameter / hole_diameter;
    if ratio >= 1.0 {
        return 3; // Minimum polygon
    }
    let sides = std::f64::consts::PI / (1.0 - ratio).acos();
    sides.ceil().max(3.0) as u32
}

/// Computes the outer radius of a polyhole that produces the desired
/// inner diameter (the inscribed circle diameter).
fn polyhole_radius(desired_diameter: f64, sides: u32) -> f64 {
    // The inscribed circle radius of a regular polygon with circumradius R
    // is R * cos(PI/n). So: desired_radius = R * cos(PI/n)
    // => R = desired_radius / cos(PI/n)
    let desired_radius = desired_diameter / 2.0;
    let angle = std::f64::consts::PI / sides as f64;
    desired_radius / angle.cos()
}
```

### Print Time Estimation with Trapezoid Model

```rust
/// Estimates the time to traverse a segment with trapezoidal velocity profile.
///
/// The segment starts at `entry_speed`, accelerates to `cruise_speed`
/// (capped by feedrate), then decelerates to `exit_speed`.
fn trapezoid_time(
    distance: f64,
    entry_speed: f64,     // mm/s
    cruise_speed: f64,    // mm/s (feedrate)
    exit_speed: f64,      // mm/s
    acceleration: f64,    // mm/s^2
) -> f64 {
    if distance <= 0.0 || acceleration <= 0.0 {
        return 0.0;
    }

    // Distance to accelerate from entry to cruise speed
    let accel_dist = (cruise_speed * cruise_speed - entry_speed * entry_speed)
        / (2.0 * acceleration);
    // Distance to decelerate from cruise to exit speed
    let decel_dist = (cruise_speed * cruise_speed - exit_speed * exit_speed)
        / (2.0 * acceleration);

    if accel_dist + decel_dist > distance {
        // Cannot reach cruise speed -- triangular profile
        // Peak speed: v_peak^2 = entry^2 + 2*accel*d_accel
        // where d_accel + d_decel = distance
        let v_peak_sq = (2.0 * acceleration * distance
            + entry_speed * entry_speed
            + exit_speed * exit_speed)
            / 2.0;
        let v_peak = v_peak_sq.sqrt();
        let t_accel = (v_peak - entry_speed) / acceleration;
        let t_decel = (v_peak - exit_speed) / acceleration;
        t_accel + t_decel
    } else {
        // Full trapezoidal profile
        let cruise_dist = distance - accel_dist - decel_dist;
        let t_accel = (cruise_speed - entry_speed) / acceleration;
        let t_cruise = cruise_dist / cruise_speed;
        let t_decel = (cruise_speed - exit_speed) / acceleration;
        t_accel + t_cruise + t_decel
    }
}
```

### Ironing Pass Generation

```rust
/// Generates ironing passes for a top surface region.
///
/// Ironing creates a second pass over top surfaces with very low flow
/// and a zigzag pattern offset 45 degrees from the primary infill.
fn generate_ironing_passes(
    top_surface: &[ValidPolygon],
    config: &IroningConfig,
    layer_z: f64,
) -> Vec<ToolpathSegment> {
    // Ironing uses the existing rectilinear infill generator
    // but with different parameters:
    // - angle = primary_infill_angle + 45 degrees
    // - spacing = nozzle_diameter * iron_spacing_fraction (e.g., 0.1)
    // - flow = normal_flow * iron_flow_rate (e.g., 0.1 = 10%)
    // - speed = iron_speed (e.g., 15 mm/s)
    // - feature = FeatureType::Ironing

    let ironing_lines = rectilinear::generate(
        top_surface,
        1.0, // 100% density for ironing pattern
        config.angle,
        config.spacing, // very tight spacing
    );

    // Convert to toolpath segments with minimal flow
    ironing_lines
        .iter()
        .map(|line| {
            let (sx, sy) = line.start.to_mm();
            let (ex, ey) = line.end.to_mm();
            ToolpathSegment {
                start: Point2::new(sx, sy),
                end: Point2::new(ex, ey),
                feature: FeatureType::Ironing,
                e_value: compute_e_value(/* ... */) * config.flow_rate,
                feedrate: config.speed * 60.0,
                z: layer_z,
                extrusion_width: None,
            }
        })
        .collect()
}
```

## Firmware Dialect Reference

### Klipper-Specific Commands
| Command | Purpose | Syntax |
|---------|---------|--------|
| SET_PRESSURE_ADVANCE | Pressure advance | `SET_PRESSURE_ADVANCE ADVANCE={value}` |
| SET_VELOCITY_LIMIT | Acceleration/jerk | `SET_VELOCITY_LIMIT ACCEL={value} SQUARE_CORNER_VELOCITY={value}` |
| BED_MESH_CALIBRATE | Bed leveling | `BED_MESH_CALIBRATE` (already in start_gcode) |
| TURN_OFF_HEATERS | Shutdown | `TURN_OFF_HEATERS` (already in end_gcode) |

### RepRapFirmware-Specific Commands
| Command | Purpose | Syntax |
|---------|---------|--------|
| M572 | Pressure advance | `M572 D{extruder} S{value}` |
| M593 | Input shaping | `M593 F{freq} S{damping}` |
| M204 | Acceleration | `M204 S{value}` (single value, not P/T split) |
| M563 | Tool definition | `M563 P{tool} D{drive} H{heater} F{fan}` |
| M0 H1 | Halt | `M0 H1` (already in end_gcode) |

### Bambu Lab-Specific Commands
| Command | Purpose | Syntax |
|---------|---------|--------|
| M620 | AMS filament slot | `M620 S{slot}A` |
| M621 | AMS filament unload | `M621 S{slot}A` |
| M204 | Acceleration | Standard `M204 P{print} T{travel}` |
| M205 | Jerk | Standard `M205 X{x} Y{y}` |

### Marlin-Specific Commands (already complete, for reference)
| Command | Purpose | Syntax |
|---------|---------|--------|
| M900 | Linear advance | `M900 K{value}` |
| M204 | Acceleration | `M204 P{print} R{retract} T{travel}` |
| M205 | Jerk | `M205 X{x} Y{y} Z{z} E{e}` |

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single dialect output | Multi-dialect with firmware-specific commands | PrusaSlicer 2.4+ (2022) | Essential for Klipper/RRF users |
| Time = distance/speed | Trapezoid motion model with acceleration | Cura 4.x (2020) | 15-30% more accurate time estimates |
| Manual polyhole STLs | Automatic hole-to-polyhole in slicer | OrcaSlicer 2.0 (2024) | Better dimensional accuracy for functional parts |
| No arc fitting | G2/G3 arc post-processing | BambuStudio/OrcaSlicer (2023) | 20-40% G-code file size reduction on curves |
| Simple ironing | Configurable ironing with spacing/flow/speed | PrusaSlicer 2.3 (2021) | Standard feature in all major slicers |
| TPMS = Gyroid only | TPMS-D + TPMS-FK patterns | OrcaSlicer 2.0+ (2024) | More infill choices for functional parts |

**Deprecated/outdated:**
- M205 jerk control: Being replaced by junction deviation (M205 J) in newer Marlin versions, but M205 X/Y/Z is still widely supported
- M900 K-factor: Klipper uses SET_PRESSURE_ADVANCE instead; RepRap uses M572
- G28 without conditional: Modern Klipper setups prefer conditional homing macros, but standard G28 is still valid

## Open Questions

1. **Bambu AMS G-code specifics**
   - What we know: Bambu uses M620/M621 for AMS slot selection, T-codes for tool selection
   - What's unclear: Exact purge sequence, filament buffer commands, and whether Bambu G-code must be in 3MF to work with AMS
   - Recommendation: Implement standard T-code tool changes with configurable tool change G-code macros; Bambu-specific AMS sequences go in the dialect module. Note that Bambu printers may require 3MF format for full AMS support -- document this limitation.

2. **Arc fitting tolerance selection**
   - What we know: OrcaSlicer uses the "resolution" setting as fitting tolerance (typically 0.05mm). Bambu Studio uses similar approach.
   - What's unclear: Optimal default tolerance for balancing file size reduction vs. dimensional accuracy
   - Recommendation: Default tolerance of 0.05mm, configurable. Minimum 3 consecutive G1 moves to form an arc. Skip arcs with radius < 0.5mm or > 1000mm.

3. **Print time estimation accuracy target**
   - What we know: Success criteria requires within 15% of actual. Trapezoid model needed.
   - What's unclear: Whether junction speed calculation (lookahead) is needed for 15% accuracy
   - Recommendation: Start with per-segment trapezoid model using configurable acceleration. Add fixed time overhead per retraction and tool change. This should achieve 15% accuracy for typical prints. Junction speed optimization can be added later if needed.

4. **Per-feature flow control granularity**
   - What we know: Requirement says 10+ feature types. Current FeatureType enum has 12 variants.
   - What's unclear: Whether every feature type needs independent flow multiplier
   - Recommendation: Add per-feature flow multipliers for all existing 12+ feature types. Use the existing FeatureType enum as the key. Default all to 1.0.

## Detailed Requirement Analysis

### GCODE-02/03/04: Firmware Dialect Enrichment
**Current state:** All four dialect modules exist with basic start/end sequences. Validator accepts Klipper extended commands.
**Gap:** Dialect-specific inline commands (pressure advance, acceleration, input shaping) are not emitted during print body. The engine hardcodes `GcodeDialect::Marlin` in `engine.rs` line 736.
**Work needed:**
- Make dialect configurable in Engine (add to PrintConfig or Engine::new)
- Add acceleration/jerk commands to GcodeCommand enum
- Each dialect module gets a `format_acceleration()` and `format_pressure_advance()` method
- Validator extended for G2/G3 and new M-codes

### GCODE-06: Acceleration and Jerk Control
**Current state:** No acceleration or jerk commands emitted. Speeds are set per-segment via F parameter only.
**Work needed:**
- Add acceleration/jerk config fields to PrintConfig
- Add M204/M205 (or equivalent) command emission at feature transitions
- Dialect-aware formatting (Marlin M204 P/T/R vs. Klipper SET_VELOCITY_LIMIT)

### GCODE-11: Arc Fitting
**Current state:** No G2/G3 support. All moves are G0/G1.
**Work needed:**
- New ArcMoveCW/ArcMoveCCW variants in GcodeCommand
- New `arc.rs` module in gcode-io with fitting algorithm
- Post-processing step in generate_full_gcode or as separate pass
- Arc validation in validator (I/J parameters, radius bounds)

### GCODE-12/13: Time and Filament Estimation
**Current state:** Basic time estimation exists via `LayerToolpath::estimated_time_seconds()` (distance/feedrate). No filament estimation.
**Work needed:**
- Replace naive estimation with trapezoid model
- Add filament length computation (sum of E-values)
- Add filament weight computation (length * cross-section * density)
- Add filament cost computation (weight * cost_per_kg)
- Add these to SliceResult

### ADV-01: Multi-Material Support
**Current state:** No multi-material support. Single extruder assumed.
**Work needed:**
- MultiMaterialConfig in PrintConfig
- ToolChange variant in GcodeCommand
- Purge tower geometry generation
- Tool change sequence generation (retract, park, change, prime, wipe)
- Per-region tool assignment in modifier mesh system

### ADV-02: Sequential Printing
**Current state:** All objects printed simultaneously (layer-by-layer across all objects).
**Work needed:**
- Object separation and independent slicing
- Collision detection using extruder clearance envelope
- Print ordering (shortest-first heuristic)
- Per-object start/end sequences within a single print

### ADV-03: Modifier Meshes
**Current state:** No modifier mesh support. Single config for entire print.
**Work needed:**
- Modifier mesh loading and slicing at model layer heights
- Per-layer region detection (point-in-polygon against modifier contours)
- Config override merging for affected regions
- Separate toolpath generation per region

### ADV-04: Custom G-code Injection
**Current state:** No custom G-code hooks.
**Work needed:**
- Hook points: before_layer, after_layer, before_feature_change, after_feature_change, at_z_height, tool_change_before, tool_change_after
- Custom G-code strings in PrintConfig
- Placeholder substitution ({layer_z}, {layer_num}, {tool}, etc.)

### ADV-05: Per-Feature Flow Control
**Current state:** Single extrusion_multiplier applies globally.
**Work needed:**
- Per-feature flow multiplier map in PrintConfig
- Apply multiplier in toolpath assembly based on FeatureType
- At minimum: outer_perimeter, inner_perimeter, solid_infill, sparse_infill, support, bridge, gap_fill, top_surface, bottom_surface, ironing

### ADV-06: Pressure Advance Calibration Pattern
**Current state:** No calibration pattern generation.
**Work needed:**
- Generate a test pattern with varying PA values
- Alternating blocks with different speeds to reveal PA artifacts
- Output includes SET_PRESSURE_ADVANCE commands at calibration steps

### ADV-07: Polyhole Conversion
**Current state:** No hole detection or polyhole conversion.
**Work needed:**
- Detect circular holes in layer contours (fit circle to polygon)
- Replace with regular polygon (polyhole) using Nophead formula
- Configurable enable/disable and minimum hole diameter

### ADV-08: Ironing
**Current state:** No ironing support.
**Work needed:**
- Detect top surface regions (already done by surface classification)
- Generate ironing pass toolpath (zigzag at 45 degrees, very low flow)
- New FeatureType::Ironing variant
- Ironing config (flow rate, speed, spacing, pattern, enabled)

### INFILL-09/10: TPMS-D and TPMS-FK
**Current state:** Gyroid TPMS pattern exists as reference implementation.
**Work needed:**
- New tpms_d.rs and tpms_fk.rs modules following Gyroid pattern
- Use implicit surface formulas with marching squares (same approach as Gyroid)
- Add to InfillPattern enum and generate_infill dispatch

## Sources

### Primary (HIGH confidence)
- Codebase analysis: All source files in slicecore-gcode-io and slicecore-engine crates
- Klipper G-code documentation: https://www.klipper3d.org/G-Codes.html
- Marlin firmware G-code reference: https://marlinfw.org/docs/gcode/M204.html, https://marlinfw.org/docs/gcode/M205.html
- RepRapFirmware G-code dictionary: https://docs.duet3d.com/User_manual/Reference/Gcodes

### Secondary (MEDIUM confidence)
- TPMS mathematical formulas: https://xyzdims.com/2023/02/09/3d-printing-parametric-generative-3d-infill-geometries/
- OrcaSlicer wiki (infill patterns): https://github.com/SoftFever/OrcaSlicer/wiki/strength_settings_patterns
- OrcaSlicer wiki (precision/polyhole): https://github.com/OrcaSlicer/OrcaSlicer/wiki/quality_settings_precision
- PrusaSlicer ironing documentation: https://help.prusa3d.com/article/ironing_177488
- PrusaSlicer sequential printing: https://help.prusa3d.com/article/sequential-printing_124589
- Prusa wipe tower documentation: https://help.prusa3d.com/article/wipe-tower_125010

### Tertiary (LOW confidence)
- Bambu Lab AMS G-code specifics: Forum posts and wiki fragments; Bambu's proprietary protocol not fully documented publicly
- Fischer-Koch S exact formula: Cross-verified with multiple sources but minor variations exist in the constant offset term

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new external dependencies, all extensions to existing crates
- Firmware dialects: HIGH - official firmware documentation verified for Klipper, Marlin, RepRapFirmware; MEDIUM for Bambu (partially proprietary)
- Arc fitting: HIGH - well-understood algorithm, multiple reference implementations in other slicers
- TPMS patterns: HIGH - mathematical formulas verified, follows existing Gyroid implementation pattern
- Multi-material: MEDIUM - purge tower geometry is well-documented but Bambu AMS specifics are partially proprietary
- Sequential printing: MEDIUM - collision detection concept is clear but implementation details vary across slicers
- Polyhole: HIGH - Nophead formula is well-established and simple to implement
- Ironing: HIGH - well-documented feature with clear parameters
- Time estimation: MEDIUM - trapezoid model is straightforward but achieving 15% accuracy depends on accurate acceleration parameters

**Research date:** 2026-02-17
**Valid until:** 2026-03-17 (stable domain, 30 days)
