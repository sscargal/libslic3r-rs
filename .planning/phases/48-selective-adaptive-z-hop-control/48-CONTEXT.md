# Phase 48: Selective Adaptive Z-Hop Control - Context

**Gathered:** 2026-03-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Replace global z-hop with intelligent surface-type-based z-hop that activates only on top solids and ironing surfaces. Implement four z-hop motion types (Normal, Slope, Spiral, Auto), height-proportional lift mode, distance-gated activation, Z-range filters, dedicated z-hop speed, retract-on-layer-change, and OrcaSlicer/Bambu profile import mapping for z-hop fields.

</domain>

<decisions>
## Implementation Decisions

### Surface Activation Rules
- Z-hop activates only when departing from top solid surfaces or ironing passes
- Trigger is based on departure surface FeatureType, not crossed-surface detection
- Crossed-surface detection deferred to future enhancement

### Z-Hop Motion Types
- Four types implemented: Normal (vertical lift), Slope (diagonal), Spiral (helical approximation), Auto
- Auto mode: uses Spiral when departing from top/ironing surfaces, Normal elsewhere
- Slope/Spiral implemented as short diagonal line segments (4-8 G0 moves), not true G2/G3 arcs
- Slope/Spiral diagonal move happens AFTER retraction completes (retract first, then lift)
- Configurable travel angle for Slope/Spiral (degrees, default ~45°; 90° degrades to Normal)

### Z-Hop Height Modes
- Two modes: Fixed (mm value) and Proportional (multiplier × layer height)
- Fixed mode: user specifies z-hop in mm (matches OrcaSlicer behavior)
- Proportional mode: multiplier range 1.0-3.0×, default 1.5× layer height
- z_hop = 0.0 means disabled (no separate enable/disable boolean, matches OrcaSlicer/Bambu convention)
- Configurable min/max clamps: z_hop_min default 0.1mm, z_hop_max default 2.0mm
- Clamps only apply when z-hop is enabled (height > 0). Negative values never allowed

### Z-Range Filters
- `z_hop_above`: z-hop only activates above this absolute Z (mm). Default 0.0 (no filter)
- `z_hop_below`: z-hop only activates below this absolute Z (mm). Default 0.0 (no filter, meaning unlimited)
- Matches OrcaSlicer's `retract_lift_above` / `retract_lift_below`

### Z-Hop Speed
- Dedicated `z_hop_speed` field (mm/s), separate from travel speed
- Default 0.0 = use travel speed (backward compatible)
- Z-axis motors are often slower than XY; allows independent tuning

### Distance Gating
- Separate `z_hop_min_travel` threshold for z-hop activation, independent of retraction `min_travel`
- Default slightly higher than retraction min_travel (e.g., 2.0mm)
- No maximum travel distance — z-hop always activates above min threshold when surface conditions are met
- Z-hop type does NOT interact with distance — type is a fixed choice, distance is a simple on/off gate

### Retract on Layer Change
- Add `retract_when_changing_layer` boolean to RetractionConfig
- When true, forces retraction at every layer change Z-move
- Z-hop on that retraction follows normal z-hop rules (surface type, distance, etc.)

### Config Structure
- New `ZHopConfig` sub-struct under PrintConfig (like CoolingConfig, RetractionConfig)
- Fields: height, hop_type, height_mode, proportional_multiplier, min_height, max_height, surface_enforce, travel_angle, speed, min_travel, above, below
- `retraction.z_hop` migrated to `z_hop.height` via serde alias for backward compatibility
- `retract_when_changing_layer` lives in RetractionConfig (controls retraction trigger, not z-hop behavior)

### Backward Compatibility & Migration
- `#[serde(alias = "z_hop")]` on ZHopConfig.height — old profiles with `retraction.z_hop: 0.4` auto-deserialize
- New profiles use `z_hop: { height: 0.4, type: "Normal", ... }` structure
- Zero breakage for existing configs

### Profile Import Mapping
- OrcaSlicer/PrusaSlicer/Bambu profile import maps z-hop fields to new ZHopConfig:
  - `z_hop_types` → `hop_type`
  - `retract_lift_enforce` → `surface_enforce`
  - `travel_slope` → `travel_angle`
  - `retract_lift_above` / `retract_lift_below` → `above` / `below`
  - `z_hop` → `height`
- Extends existing profile_import.rs and profile_import_ini.rs pipelines

### Claude's Discretion
- Exact number of line segments for Spiral approximation (4-8 range)
- Default travel angle for Slope vs Spiral (both use travel_angle, but may ship different defaults)
- Internal representation of ZHopType enum variants
- G-code generation sequencing details for Slope/Spiral moves
- Test fixture design for z-hop type validation

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing Z-Hop Implementation
- `crates/slicecore-engine/src/config.rs` — `RetractionConfig` with current `z_hop: f64` field (line ~898), `PrintConfig` struct
- `crates/slicecore-engine/src/gcode_gen.rs` — Current z-hop G-code generation (lines ~155-180), z-hop up/down during retraction
- `crates/slicecore-engine/src/planner.rs` — `plan_retraction()` function, `RetractionMove` struct with `z_hop` field

### Toolpath & Feature Types
- `crates/slicecore-engine/src/toolpath.rs` — `FeatureType` enum (line ~26) with `SolidInfill`, `Ironing`, `Travel`, etc.
- `crates/slicecore-engine/src/gcode_gen.rs` — `feature_label()` function, feature type comment insertion

### Profile Import
- `crates/slicecore-engine/src/profile_import.rs` — Profile import pipeline
- `crates/slicecore-engine/src/profile_import_ini.rs` — INI format profile import (OrcaSlicer/PrusaSlicer)

### Design Documents
- `designDocs/04-IMPLEMENTATION-GUIDE.md` line 397 — Z-hop in G-code generation pipeline
- `designDocs/CONFIG_PARITY_AUDIT.md` line 138 — z_hop field parity status
- `designDocs/08-GLOSSARY.md` line 184 — Z-hop definition

### OrcaSlicer Reference (External)
- OrcaSlicer z-hop wiki: z_hop_types (Normal/Slope/Spiral/Auto), retract_lift_enforce (surface filtering), travel_slope (angle), retract_lift_above/below (Z-range filters)
- OrcaSlicer retraction wiki: retraction_length, retraction_speed, retraction_minimum_travel, retract_when_changing_layer, wipe settings

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `RetractionConfig` — Existing retraction config struct to extend with `retract_when_changing_layer`
- `plan_retraction()` in `planner.rs` — Returns `RetractionMove`, needs to include z-hop type and surface context
- `generate_layer_gcode()` in `gcode_gen.rs` — Already handles z-hop up/down, needs to support Slope/Spiral/Auto types
- `FeatureType` enum in `toolpath.rs` — Already distinguishes `SolidInfill` and `Ironing` for departure surface detection
- `SettingSchema` derive macro — Used for config structs, apply to new `ZHopConfig`
- Profile import pipelines — `profile_import.rs` and `profile_import_ini.rs` for OrcaSlicer field mapping

### Established Patterns
- Config sub-structs: `CoolingConfig`, `RetractionConfig`, `SpeedConfig` — all use `#[setting(flatten)]` in `PrintConfig`
- Error handling: `thiserror` enums in per-crate `error.rs`
- Config tiers: `#[setting(tier = N)]` for progressive disclosure (z-hop type would be tier 2-3)
- Serde aliases: `#[serde(alias = "...")]` used elsewhere for backward-compatible field renames

### Integration Points
- `PrintConfig` — gains new `ZHopConfig` flattened sub-struct
- `gcode_gen.rs` — z-hop generation logic refactored to support 4 types + surface gating
- `planner.rs` — `plan_retraction()` or new `plan_z_hop()` function incorporates surface context and distance gating
- `profile_import.rs` / `profile_import_ini.rs` — new field mappings for z-hop parameters

</code_context>

<specifics>
## Specific Ideas

- OrcaSlicer z-hop types and surface enforcement studied as reference implementation — our approach matches their feature set plus adds proportional height mode as a novel feature
- Spiral z-hop approximated with line segments for firmware universality (no G2/G3 dependency)
- Auto z-hop type is surface-aware (Spiral on top/ironing, Normal elsewhere) rather than OrcaSlicer's overhang-crossing detection — simpler since we use departure-surface triggering
- z_hop=0.0 means disabled, matching industry convention — no separate enable/disable toggle

</specifics>

<deferred>
## Deferred Ideas

### Future Z-Hop Enhancements
- **Configurable surface set** — Let users pick which FeatureTypes trigger z-hop via config list (not just top solid + ironing)
- **OrcaSlicer-style surface enum** — All Surfaces / Top Only / Bottom Only / Top and Bottom options
- **Crossed-surface detection** — Analyze travel path to detect if it crosses top surfaces (more accurate but requires spatial queries)
- **Distance-based type switching** — Short travels use Normal, long travels use Spiral/Slope
- **True G2/G3 arc Spiral** — Use arc commands for firmware that supports them
- **Maximum travel distance gate** — Skip z-hop on very long travels that clear the part

### Research TODOs
- Investigate optimal Spiral segment count for different travel distances
- Study real-world z-hop speed impact on print quality across printer types

</deferred>

---

*Phase: 48-selective-adaptive-z-hop-control*
*Context gathered: 2026-03-25*
