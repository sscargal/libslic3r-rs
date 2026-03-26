# Phase 48: Selective Adaptive Z-Hop Control - Research

**Researched:** 2026-03-25
**Domain:** Surface-type-based z-hop, G-code generation, config sub-structs, profile import
**Confidence:** HIGH

## Summary

Phase 48 replaces the current global z-hop mechanism (a single `z_hop: f64` on `RetractionConfig`) with an intelligent surface-type-based z-hop system. The current implementation is straightforward: `plan_retraction()` in `planner.rs` returns a `RetractionMove` with z_hop taken directly from config, and `generate_layer_gcode()` in `gcode_gen.rs` emits a single vertical G0 Z move up/down. The new system needs: a `ZHopConfig` sub-struct, four motion types (Normal/Slope/Spiral/Auto), height modes (Fixed/Proportional), surface-type gating, distance gating, Z-range filters, and profile import mapping.

The key architectural challenge is surface-type detection. The current `FeatureType` enum has `SolidInfill` (covers both top AND bottom) and `Ironing` (top only). To trigger z-hop only on top solid departures, the implementation must either: (a) add a `TopSolidInfill` variant to `FeatureType`, or (b) add an `is_top_surface: bool` metadata field to `ToolpathSegment`, or (c) propagate layer-position context (is this a top layer?) into the gcode generation. Option (a) is cleanest for this use case since it naturally flows through the existing `last_feature` tracking in gcode_gen.rs.

The existing codebase provides strong patterns for every aspect: config sub-structs (CoolingConfig, SpeedConfig, RetractionConfig all use `#[setting(flatten)]`), profile import field mapping (two match-arm functions: `upstream_to_config_field` for path mapping and `apply_field_mapping` for value assignment), serde aliases for backward compatibility, and the SettingSchema derive macro.

**Primary recommendation:** Add `TopSolidInfill` variant to `FeatureType`, create `ZHopConfig` sub-struct following `CoolingConfig` pattern, refactor `plan_retraction()` to accept departure feature context, extend gcode_gen.rs z-hop emission to support Slope/Spiral motion types via short G0 segment sequences.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Z-hop activates only when departing from top solid surfaces or ironing passes
- Trigger is based on departure surface FeatureType, not crossed-surface detection
- Four z-hop motion types: Normal (vertical lift), Slope (diagonal), Spiral (helical approximation), Auto
- Auto mode: uses Spiral when departing from top/ironing surfaces, Normal elsewhere
- Slope/Spiral implemented as short diagonal line segments (4-8 G0 moves), not true G2/G3 arcs
- Slope/Spiral diagonal move happens AFTER retraction completes (retract first, then lift)
- Configurable travel angle for Slope/Spiral (degrees, default ~45 degrees; 90 degrees degrades to Normal)
- Two height modes: Fixed (mm value) and Proportional (multiplier x layer height)
- Fixed mode: user specifies z-hop in mm (matches OrcaSlicer behavior)
- Proportional mode: multiplier range 1.0-3.0x, default 1.5x layer height
- z_hop = 0.0 means disabled (no separate enable/disable boolean, matches OrcaSlicer/Bambu convention)
- Configurable min/max clamps: z_hop_min default 0.1mm, z_hop_max default 2.0mm (only apply when z-hop enabled)
- Z-range filters: z_hop_above (default 0.0 = no filter), z_hop_below (default 0.0 = no filter)
- Dedicated z_hop_speed field (mm/s), separate from travel speed; default 0.0 = use travel speed
- Separate z_hop_min_travel threshold (default 2.0mm), independent of retraction min_travel
- No maximum travel distance gate -- z-hop always activates above min threshold
- Z-hop type does NOT interact with distance -- type is fixed choice, distance is simple on/off gate
- retract_when_changing_layer already exists in RetractionConfig (verified in codebase)
- New ZHopConfig sub-struct under PrintConfig (like CoolingConfig, RetractionConfig)
- Fields: height, hop_type, height_mode, proportional_multiplier, min_height, max_height, surface_enforce, travel_angle, speed, min_travel, above, below
- retraction.z_hop migrated to z_hop.height via serde alias for backward compatibility
- OrcaSlicer/PrusaSlicer/Bambu profile import maps z-hop fields to new ZHopConfig

### Claude's Discretion
- Exact number of line segments for Spiral approximation (4-8 range)
- Default travel angle for Slope vs Spiral
- Internal representation of ZHopType enum variants
- G-code generation sequencing details for Slope/Spiral moves
- Test fixture design for z-hop type validation

### Deferred Ideas (OUT OF SCOPE)
- Configurable surface set (let users pick which FeatureTypes trigger z-hop)
- OrcaSlicer-style surface enum (All/Top/Bottom/TopAndBottom)
- Crossed-surface detection (spatial queries on travel path)
- Distance-based type switching (short=Normal, long=Spiral)
- True G2/G3 arc Spiral
- Maximum travel distance gate
- Optimal Spiral segment count research
- Real-world z-hop speed impact studies
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| GCODE-03 | RepRapFirmware dialect G-code output | Z-hop G-code must be dialect-agnostic (G0 rapid moves are universal across Marlin/Klipper/RRF/Bambu). The z-hop implementation uses G0 X/Y/Z moves which are standard across all supported dialects. No dialect-specific z-hop behavior needed. |
</phase_requirements>

## Standard Stack

### Core (already in crate)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde | 1.x | Serialization/deserialization for ZHopConfig | Already used for all config structs |
| thiserror | 2.x | Error types if needed | Project standard per rust-senior-dev skill |
| slicecore-settings-derive | workspace | SettingSchema derive macro for ZHopConfig | Used by all config sub-structs |

### No new dependencies needed
This phase is entirely internal refactoring. All z-hop motion types (Normal/Slope/Spiral) produce standard G0 rapid moves. No external libraries required.

## Architecture Patterns

### Recommended Changes Structure
```
crates/slicecore-engine/src/
  config.rs          # Add ZHopConfig struct, ZHopType enum, ZHopHeightMode enum
  toolpath.rs        # Add TopSolidInfill variant to FeatureType
  planner.rs         # Extend plan_retraction() or add plan_z_hop() with surface context
  gcode_gen.rs       # Refactor z-hop emission to support 4 motion types + surface gating
  profile_import.rs  # Add z-hop field mappings (z_hop_types, retract_lift_enforce, etc.)
  profile_import_ini.rs  # Add INI z-hop field mappings (retract_lift_above, etc.)
  statistics.rs      # Update z_hop_count tracking if needed
```

### Pattern 1: Config Sub-Struct (follow CoolingConfig/RetractionConfig pattern)

**What:** New `ZHopConfig` struct with `#[derive(SettingSchema)]` and `#[setting(flatten)]` in PrintConfig.
**When to use:** All new config groupings in this project.
**Example:**
```rust
// Source: crates/slicecore-engine/src/config.rs (existing pattern)
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Z-Hop")]
pub struct ZHopConfig {
    /// Z-hop height in mm. 0.0 = disabled.
    #[serde(alias = "z_hop")]  // backward compat with old retraction.z_hop
    #[setting(tier = 2, description = "Z-hop height", units = "mm", min = 0.0, max = 5.0)]
    pub height: f64,

    /// Z-hop motion type.
    #[setting(tier = 3, description = "Z-hop motion type")]
    pub hop_type: ZHopType,

    /// Height calculation mode.
    #[setting(tier = 3, description = "Z-hop height mode")]
    pub height_mode: ZHopHeightMode,

    // ... remaining fields
}
```

### Pattern 2: FeatureType Extension for Surface Discrimination

**What:** Add `TopSolidInfill` variant to `FeatureType` to distinguish top solid from bottom solid infill.
**When to use:** When z-hop surface gating needs to know if a departure is from a top surface.
**Rationale:** Currently `SolidInfill` covers both top and bottom. The gcode generator tracks `last_feature` before travel moves -- if we add `TopSolidInfill`, the departure surface check becomes a simple match arm.

**Impact analysis:** `FeatureType` is used in:
- `gcode_gen.rs` -- `feature_label()`, acceleration selection, z-hop triggering
- `toolpath.rs` -- toolpath assembly (line 490: `if infill.is_solid { SolidInfill }`)
- `config.rs` -- speed config per-feature
- `statistics.rs` -- per-feature statistics
- `profile_import.rs` -- (not directly)

Adding `TopSolidInfill` requires updating:
1. `toolpath.rs`: assembly logic to emit `TopSolidInfill` when layer is a top layer
2. `gcode_gen.rs`: `feature_label()` match, z-hop departure check
3. `config.rs`: `SpeedConfig` if top solid needs different speed (already has `top_solid_infill_speed` in some slicers, but can map to same speed initially)
4. Any exhaustive match arms on `FeatureType`

**Alternative:** Add `is_top_surface: bool` to `ToolpathSegment`. This avoids enum variant proliferation but requires propagating a new field through all toolpath construction sites. Given the CONTEXT.md says departure-based triggering on FeatureType, `TopSolidInfill` is the natural fit.

### Pattern 3: Slope/Spiral G-code Emission

**What:** Emit 4-8 short G0 segments that approximate a diagonal or helical lift path.
**When to use:** When `hop_type` is Slope, Spiral, or Auto (on top/ironing surfaces).
**Example:**
```rust
// Slope: diagonal line from (x0, y0, z) to (x0 + dx, y0 + dy, z + z_hop)
// Split into N segments for smooth motion
fn emit_slope_z_hop(
    start_x: f64, start_y: f64,
    z_base: f64, z_hop: f64,
    travel_angle_deg: f64,
    segments: usize,  // 4-8
    feedrate: Option<f64>,
) -> Vec<GcodeCommand> {
    let angle_rad = travel_angle_deg.to_radians();
    let horizontal_dist = z_hop / angle_rad.tan();
    // ... emit N G0 moves with interpolated X, Y, Z
}

// Spiral: helical approximation via short line segments
// Each segment rotates around the start point while ascending
fn emit_spiral_z_hop(
    center_x: f64, center_y: f64,
    z_base: f64, z_hop: f64,
    radius: f64,  // small, ~1-2mm
    segments: usize,
    feedrate: Option<f64>,
) -> Vec<GcodeCommand> {
    // N segments around a circle with linearly increasing Z
}
```

### Pattern 4: Z-Hop Decision Function

**What:** A dedicated function that decides whether z-hop should activate and what parameters to use.
**When to use:** Called from gcode_gen.rs before emitting z-hop moves.
```rust
pub struct ZHopDecision {
    pub height: f64,       // Computed z-hop height (after proportional calc + clamping)
    pub hop_type: ZHopType, // Resolved type (Auto -> Normal or Spiral)
    pub speed: Option<f64>, // Z-hop speed (None = use travel speed)
}

/// Decides whether z-hop should activate for this travel move.
pub fn plan_z_hop(
    departure_feature: FeatureType,
    travel_distance: f64,
    current_z: f64,
    layer_height: f64,
    config: &ZHopConfig,
) -> Option<ZHopDecision> {
    // 1. Check if z-hop enabled (height > 0)
    // 2. Check surface_enforce: departure must be TopSolidInfill or Ironing
    // 3. Check distance gate: travel_distance >= min_travel
    // 4. Check Z-range filters: current_z >= above && (below == 0 || current_z <= below)
    // 5. Compute height (fixed or proportional with clamping)
    // 6. Resolve Auto type based on departure feature
}
```

### Anti-Patterns to Avoid
- **Mixing z-hop config into RetractionConfig:** The whole point is separating z-hop into its own sub-struct. Don't add new fields to RetractionConfig (except retract_when_changing_layer which is about retraction triggering).
- **Hardcoding surface types:** Use the `surface_enforce` config field so the surface check is data-driven, even though we initially only support top solid + ironing.
- **Computing z-hop in plan_retraction():** Keep retraction planning and z-hop planning as separate concerns. plan_retraction() returns retraction parameters; plan_z_hop() returns z-hop parameters.
- **Using G2/G3 arcs for Spiral:** The decision is locked -- use line segments for firmware universality.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Config serialization | Manual serde impl | `#[derive(Serialize, Deserialize)]` + `#[serde(default, alias)]` | Backward compat aliases are built-in |
| Setting metadata | Manual schema generation | `#[derive(SettingSchema)]` with `#[setting(...)]` attributes | Project-wide pattern, auto-generates UI metadata |
| Enum string conversion | Manual FromStr/Display | `serde` rename variants (`#[serde(rename_all = "snake_case")]`) | Consistent with existing enum patterns |

## Common Pitfalls

### Pitfall 1: Breaking Existing z_hop Deserialization
**What goes wrong:** Old config files have `retraction.z_hop: 0.4` which must still work after migration.
**Why it happens:** Moving z_hop from RetractionConfig to ZHopConfig changes the JSON/TOML path.
**How to avoid:** Use `#[serde(alias = "z_hop")]` on `ZHopConfig.height`. Keep `RetractionConfig.z_hop` as a deprecated field that gets migrated during deserialization, OR use a custom deserializer that handles both locations.
**Warning signs:** Tests with old-format config files fail to parse.

### Pitfall 2: TopSolidInfill Breaking Exhaustive Matches
**What goes wrong:** Adding a new `FeatureType` variant breaks every exhaustive `match` in the codebase.
**Why it happens:** Rust requires exhaustive matching on enums.
**How to avoid:** Search for all `match` on `FeatureType` before adding the variant. Most can treat `TopSolidInfill` same as `SolidInfill` (same speed, same E-value computation, same statistics category).
**Warning signs:** Compilation errors across multiple files after adding the variant.

### Pitfall 3: Spiral Z-Hop Travel Direction
**What goes wrong:** Spiral/Slope z-hop moves the nozzle horizontally during lift, but the subsequent travel move expects to start from the pre-hop position.
**Why it happens:** The current code emits z-hop up, then rapid travel XY, then z-hop down. With Slope/Spiral, the nozzle moves in XY during lift, so the travel start position shifts.
**How to avoid:** After Slope/Spiral lift completes, the nozzle is at a new XY. The subsequent G0 travel to destination is still correct (it goes to the target XY), but the z-hop-down at the destination should also use Slope/Spiral back down. Account for this asymmetry.
**Warning signs:** Toolpath visualization shows unexpected horizontal jumps.

### Pitfall 4: Proportional Height with Variable Layer Heights
**What goes wrong:** Proportional z-hop mode uses `multiplier * layer_height`, but with VLH (Phase 47), layer heights vary per layer.
**Why it happens:** The current `plan_retraction()` doesn't receive layer height.
**How to avoid:** Pass per-layer height into `plan_z_hop()`. The `LayerToolpath` already stores `z` per segment; layer height can be computed from adjacent layers or passed as a parameter.
**Warning signs:** Uniform z-hop height across variable-height layers when proportional mode is active.

### Pitfall 5: Z-Hop on First/Last Layer Edge Cases
**What goes wrong:** Z-hop on first layer can cause the nozzle to catch on clips/brims. Z-hop above/below filters may not cover all edge cases.
**Why it happens:** `z_hop_above` default is 0.0 (no filter), meaning z-hop activates even on first layer.
**How to avoid:** Document that users should set `z_hop_above` to at least first_layer_height to avoid first-layer issues. The defaults are safe (z_hop height defaults to 0.0 = disabled).
**Warning signs:** Print failures on first layer with z-hop enabled.

### Pitfall 6: Profile Import Field Name Collisions
**What goes wrong:** OrcaSlicer's `z_hop` field currently maps to `retraction.z_hop`. After migration, it must map to `z_hop.height`.
**Why it happens:** The profile import has existing mappings that need updating.
**How to avoid:** Update both `upstream_to_config_field()` (path mapping) and `apply_field_mapping()` (value assignment) in profile_import.rs. Also update profile_import_ini.rs (`retract_lift` mapping at line 344/960).
**Warning signs:** Imported profiles have z_hop=0 despite source profile having it set.

## Code Examples

### Current Z-Hop Implementation (to be refactored)
```rust
// Source: crates/slicecore-engine/src/planner.rs:36-46
pub fn plan_retraction(travel_distance: f64, config: &PrintConfig) -> Option<RetractionMove> {
    if travel_distance >= config.retraction.min_travel {
        Some(RetractionMove {
            retract_length: config.retraction.length,
            retract_speed: config.retraction.speed,
            z_hop: config.retraction.z_hop,
        })
    } else {
        None
    }
}
```

### Current Z-Hop G-code Emission (to be refactored)
```rust
// Source: crates/slicecore-engine/src/gcode_gen.rs:157-165
if ret.z_hop > 0.0 {
    cmds.push(GcodeCommand::RapidMove {
        x: None,
        y: None,
        z: Some(seg.z + ret.z_hop),
        f: None,
    });
}
```

### Current FeatureType Enum (to be extended)
```rust
// Source: crates/slicecore-engine/src/toolpath.rs:26-55
pub enum FeatureType {
    OuterPerimeter,
    InnerPerimeter,
    SolidInfill,       // Covers BOTH top and bottom -- needs TopSolidInfill variant
    SparseInfill,
    Skirt,
    Brim,
    GapFill,
    VariableWidthPerimeter,
    Support,
    SupportInterface,
    Bridge,
    Ironing,           // Already distinguishable -- triggers z-hop
    PurgeTower,
    Travel,
}
```

### Existing Config Sub-Struct Pattern
```rust
// Source: crates/slicecore-engine/src/config.rs:2475-2477
/// Retraction configuration (includes length, speed, z_hop, min_travel).
#[setting(flatten)]
pub retraction: RetractionConfig,
```

### Existing Profile Import Pattern
```rust
// Source: crates/slicecore-engine/src/profile_import.rs:550-552
"retraction_length" => Some("retraction.length"),
"retraction_speed" => Some("retraction.speed"),
"z_hop" => Some("retraction.z_hop"),
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Global z_hop on RetractionConfig | Surface-gated ZHopConfig (this phase) | Phase 48 | Eliminates unnecessary z-hops on interior layers |
| Single vertical lift | 4 motion types (Normal/Slope/Spiral/Auto) | Phase 48 | Reduces stringing with diagonal/helical paths |
| Fixed z-hop height | Fixed + Proportional modes | Phase 48 | Adapts to variable layer heights from Phase 47 |

**OrcaSlicer reference features (that we match or exceed):**
- `z_hop_types`: Normal/Slope/Spiral/Auto -- we match
- `retract_lift_enforce`: surface filtering -- we match (departure-based)
- `travel_slope`: angle config -- we match via `travel_angle`
- `retract_lift_above` / `retract_lift_below`: Z-range filters -- we match
- Proportional height mode: **novel feature** not in OrcaSlicer

## Open Questions

1. **TopSolidInfill vs is_top_surface metadata**
   - What we know: `FeatureType::SolidInfill` doesn't distinguish top from bottom. Ironing is already its own variant.
   - What's unclear: Whether adding `TopSolidInfill` or an `is_top_surface` flag on `ToolpathSegment` is more maintainable long-term.
   - Recommendation: Use `TopSolidInfill` variant. It's consistent with how `Ironing` is already a separate variant, and it flows naturally through the `last_feature` tracking in gcode_gen.rs. The toolpath assembly already has the `is_top_layer` check in engine.rs (line 599) that can be propagated to set the feature type.

2. **Backward-compat serde migration strategy**
   - What we know: Old configs have `retraction: { z_hop: 0.4 }`. New configs need `z_hop: { height: 0.4 }`.
   - What's unclear: Whether to keep `RetractionConfig.z_hop` as a deprecated passthrough or remove it entirely and handle migration in a custom deserializer.
   - Recommendation: Remove `z_hop` from `RetractionConfig`, add `ZHopConfig` to `PrintConfig` with `#[serde(alias = "z_hop")]` on the `height` field. For the nested `retraction.z_hop` path in old configs, add a `#[serde(deserialize_with = "...")]` or post-deserialization migration step that copies `retraction.z_hop` into `z_hop.height` if the latter is 0.

3. **Layer height availability for proportional mode**
   - What we know: `generate_layer_gcode()` receives `LayerToolpath` which has `z` per segment but not explicit layer height. Phase 47 adds VLH with per-layer heights.
   - What's unclear: How to get per-layer height into the z-hop decision function.
   - Recommendation: Compute layer height as `current_z - previous_layer_z` or add a `layer_height` field to `LayerToolpath`. The latter is cleaner and already available from the Z-schedule.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml workspace |
| Quick run command | `cargo test -p slicecore-engine --lib` |
| Full suite command | `cargo test --all-features --workspace` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| GCODE-03 | Z-hop G-code uses dialect-agnostic G0 moves | unit | `cargo test -p slicecore-engine z_hop -- --exact` | Existing z_hop_during_retraction test needs extension |
| GCODE-03 | ZHopConfig deserialization (new + backward compat) | unit | `cargo test -p slicecore-engine zhop_config` | Wave 0 |
| GCODE-03 | Surface-gated z-hop (only on TopSolidInfill/Ironing) | unit | `cargo test -p slicecore-engine z_hop_surface_gate` | Wave 0 |
| GCODE-03 | Distance-gated z-hop activation | unit | `cargo test -p slicecore-engine z_hop_distance_gate` | Wave 0 |
| GCODE-03 | Z-range filter (above/below) | unit | `cargo test -p slicecore-engine z_hop_z_range` | Wave 0 |
| GCODE-03 | Proportional height mode with clamping | unit | `cargo test -p slicecore-engine z_hop_proportional` | Wave 0 |
| GCODE-03 | Slope z-hop emits correct G0 segments | unit | `cargo test -p slicecore-engine z_hop_slope` | Wave 0 |
| GCODE-03 | Spiral z-hop emits correct G0 segments | unit | `cargo test -p slicecore-engine z_hop_spiral` | Wave 0 |
| GCODE-03 | Auto mode resolves to Spiral on top/ironing, Normal elsewhere | unit | `cargo test -p slicecore-engine z_hop_auto` | Wave 0 |
| GCODE-03 | Profile import maps OrcaSlicer z-hop fields | unit | `cargo test -p slicecore-engine profile_import` | Existing tests need extension |
| GCODE-03 | Old config format backward compatibility | unit | `cargo test -p slicecore-engine z_hop_backward_compat` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-engine --lib`
- **Per wave merge:** `cargo test --all-features --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/slicecore-engine/src/config.rs` -- ZHopConfig struct tests (deserialization, defaults, backward compat)
- [ ] `crates/slicecore-engine/src/planner.rs` -- plan_z_hop() tests (surface gate, distance gate, Z-range, proportional)
- [ ] `crates/slicecore-engine/src/gcode_gen.rs` -- z-hop motion type emission tests (Slope segments, Spiral segments, Auto resolution)
- [ ] Update existing `z_hop_during_retraction` test to work with new ZHopConfig

## Sources

### Primary (HIGH confidence)
- `crates/slicecore-engine/src/config.rs` lines 874-970 -- RetractionConfig with z_hop field, Default impl
- `crates/slicecore-engine/src/gcode_gen.rs` lines 130-210 -- Current z-hop G-code generation
- `crates/slicecore-engine/src/planner.rs` lines 22-46 -- RetractionMove struct, plan_retraction()
- `crates/slicecore-engine/src/toolpath.rs` lines 24-55 -- FeatureType enum (no TopSolidInfill)
- `crates/slicecore-engine/src/profile_import.rs` lines 550-552, 1167-1170 -- z_hop field mapping
- `crates/slicecore-engine/src/profile_import_ini.rs` lines 344, 960-962 -- retract_lift INI mapping
- `crates/slicecore-engine/src/infill/mod.rs` line 51 -- `LayerInfill.is_solid` (no is_top flag)
- `crates/slicecore-engine/src/engine.rs` line 599 -- `is_top_layer` computation in engine

### Secondary (MEDIUM confidence)
- OrcaSlicer z-hop feature set (z_hop_types, retract_lift_enforce, travel_slope, retract_lift_above/below) -- referenced from CONTEXT.md canonical refs

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, all patterns already established in codebase
- Architecture: HIGH -- direct extension of existing config/planner/gcode_gen patterns with clear integration points
- Pitfalls: HIGH -- identified from direct codebase analysis (serde migration, FeatureType exhaustive matches, profile import updates)

**Research date:** 2026-03-25
**Valid until:** 2026-04-25 (stable domain, internal refactoring)
