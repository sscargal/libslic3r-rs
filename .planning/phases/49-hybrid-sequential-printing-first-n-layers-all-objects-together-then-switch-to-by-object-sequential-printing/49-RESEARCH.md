# Phase 49: Hybrid Sequential Printing - Research

**Researched:** 2026-03-26
**Domain:** Slicing engine -- hybrid print mode (shared layers + per-object sequential)
**Confidence:** HIGH

## Summary

Phase 49 extends the existing sequential printing system (`sequential.rs`) with a hybrid mode that prints the first N layers of all objects together (normal by-layer ordering) before switching to per-object sequential printing. The codebase already has all the foundation pieces: `SequentialConfig` for configuration, `plan_sequential_print()` for object ordering, `connected_components()` for mesh splitting, the event system for progress reporting, `CustomGcodeHooks` for G-code injection points, and profile import pipelines for field mapping.

The primary engineering challenge is the two-phase slicing pipeline in `Engine::slice()`. Currently, sequential mode only validates feasibility (lines 1253-1329 in `engine.rs`) but still slices the mesh as one piece. This phase must implement actual per-component slicing for layers above the transition point, while keeping shared layers as normal by-layer slicing. The `slice_plate()` method already handles multi-object slicing with per-object configs, providing a pattern to follow.

**Primary recommendation:** Extend `SequentialConfig` with hybrid fields, refactor `Engine::slice_to_writer_with_events()` to support two-phase output (shared layers via normal pipeline, then per-object via component-isolated slicing), and add a `HybridPlan` struct that captures the transition point and object ordering for both G-code generation and dry-run preview.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Two threshold modes: layer count (primary, default 5) and height-based (fallback, only if layer count is 0)
- Extend existing `SequentialConfig` with `hybrid_enabled` (bool), `transition_layers` (u32, default 5), `transition_height` (f64, default 0.0) -- not a separate sub-struct
- `hybrid_enabled` requires `sequential.enabled = true`
- All hybrid fields at Tier 3, `depends_on = "sequential.enabled"`
- Shared layers use normal by-layer ordering; sequential object order uses existing `order_objects()` shortest-first
- At transition: retract, raise to safe Z (above clearance height), begin first object from layer N+1
- No pause at transition by default
- Object markers: `; OBJECT_START id=N name="..."` / `; OBJECT_END id=N` inserted between objects in sequential phase only
- Post-processor or firmware macro handles skip via markers; no skip during shared layers
- No collision re-evaluation when objects are skipped; order fixed at slice time
- Shared layers (1..N): slice full combined mesh; layers above N: per-component slicing
- Per-object settings overrides (Phase 45) apply in sequential phase only; shared layers use global settings
- Brim/skirt generated once during shared layers around all objects; no per-object brim in sequential
- Per-object progress events during sequential phase via event system (API-05)
- Object names from 3MF metadata or filename in comment markers
- Dry-run CLI flag to preview hybrid plan without slicing
- Profile import: map `complete_objects`/`print_sequence` to `sequential.enabled` (existing); no invented hybrid mappings

### Claude's Discretion
- Safe Z calculation details for transition (margin above clearance height)
- G-code comment marker exact format and placement details
- Per-component slicing implementation (how to split/rejoin G-code streams)
- Progress event frequency and granularity during sequential phase
- Dry-run output format and time estimation approach
- Error handling for edge cases (single object with hybrid enabled, objects with zero shared layers)
- Test fixture design for hybrid mode validation

### Deferred Ideas (OUT OF SCOPE)
- Transition layer blending (gradual reduction of inter-object travel)
- Collision re-check at transition using actual printed heights
- Pause-at-transition option (M0/M1 insertion)
- Back-to-front ordering option for bed-slinger printers
- User-configurable custom object order
- Klipper EXCLUDE_OBJECT integration
- Per-object brim in sequential phase
- Firmware variable-based skip (Klipper macros / Marlin M808)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| ADV-02 | Sequential printing (object-by-object with collision detection) | Already marked complete. This phase extends ADV-02 with hybrid mode -- config fields, two-phase slicing, object markers, per-object progress, dry-run preview. All foundation code exists in `sequential.rs`, `config.rs`, `engine.rs`. |
</phase_requirements>

## Standard Stack

### Core (already in project)
| Library | Purpose | Why Standard |
|---------|---------|--------------|
| `serde` + `toml` | Config serialization for hybrid fields | Already used for `SequentialConfig` |
| `thiserror` | Error variants for hybrid mode failures | Project-wide error handling pattern |
| `slicecore-config-derive` (`SettingSchema`) | Setting metadata (tier, depends_on) for hybrid fields | All config structs use this derive |
| `sha2` | Plate checksum in G-code header | Already used in `gcode_gen.rs` |

### No New Dependencies
This phase requires zero new crate dependencies. All functionality builds on existing infrastructure:
- Config: `SequentialConfig` extension with derive macros already in use
- Slicing: `connected_components()` + `Engine::slice()` already exist
- G-code: `GcodeCommand::Comment` and `GcodeCommand::Raw` for object markers
- Events: `SliceEvent` enum extension for per-object progress
- CLI: `clap` already in use for CLI flags

## Architecture Patterns

### Recommended Changes by File

```
crates/slicecore-engine/src/
  config.rs           # Add hybrid fields to SequentialConfig
  sequential.rs       # Add HybridPlan struct, plan_hybrid_print() function
  engine.rs           # Refactor slice_to_writer_with_events() for two-phase
  gcode_gen.rs        # Add generate_hybrid_gcode() or extend generate_full_gcode()
  event.rs            # Add ObjectProgress variant to SliceEvent
  error.rs            # Add HybridConfigError variant if needed
  profile_import.rs   # Map complete_objects -> sequential.enabled (already done)
  profile_import_ini.rs  # Same INI mapping (already done)
  lib.rs              # Re-export new public types

crates/slicecore-cli/src/
  main.rs             # Add --hybrid-dry-run CLI flag
  slice_workflow.rs   # Integrate dry-run output
```

### Pattern 1: Config Extension
**What:** Add hybrid fields directly to `SequentialConfig`
**When:** Config definition phase
**Example:**
```rust
// In config.rs, extend SequentialConfig:
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Advanced")]
pub struct SequentialConfig {
    // ... existing fields ...

    /// Enable hybrid sequential mode (shared first layers + per-object sequential).
    #[setting(
        tier = 3,
        description = "Enable hybrid sequential mode",
        depends_on = "sequential.enabled",
        override_safety = "warn"
    )]
    pub hybrid_enabled: bool,

    /// Number of shared layers before switching to sequential.
    #[setting(
        tier = 3,
        description = "Number of shared first layers in hybrid mode",
        depends_on = "sequential.enabled",
        override_safety = "warn"
    )]
    pub transition_layers: u32,

    /// Height threshold for transition (only if transition_layers is 0).
    #[setting(
        tier = 3,
        description = "Z height threshold for hybrid transition",
        units = "mm",
        depends_on = "sequential.enabled",
        override_safety = "warn"
    )]
    pub transition_height: f64,
}
```

### Pattern 2: Hybrid Plan Struct
**What:** Captures the computed transition point and object ordering for reuse across G-code gen and dry-run
**When:** Planning phase (after config validation, before slicing)
**Example:**
```rust
// In sequential.rs:
/// Pre-computed plan for hybrid sequential printing.
#[derive(Debug, Clone)]
pub struct HybridPlan {
    /// Number of shared layers (printed by-layer for all objects).
    pub shared_layer_count: u32,
    /// Z height at which transition occurs.
    pub transition_z: f64,
    /// Ordered object indices for sequential phase (shortest-first).
    pub object_order: Vec<usize>,
    /// Safe Z height for travel between objects.
    pub safe_z: f64,
    /// Object metadata (index, name, bounds) for markers.
    pub objects: Vec<HybridObjectInfo>,
}

/// Metadata for a single object in hybrid mode.
#[derive(Debug, Clone)]
pub struct HybridObjectInfo {
    pub index: usize,
    pub name: String,
    pub bounds: ObjectBounds,
}
```

### Pattern 3: Two-Phase Slicing
**What:** Split the slicing pipeline at the transition layer
**When:** Inside `Engine::slice_to_writer_with_events()`
**Approach:**
1. Detect hybrid mode from config
2. Split mesh into components via `connected_components()`
3. Slice full combined mesh for layers 0..N (shared phase)
4. For each component in `object_order`, slice independently for layers N+1..end
5. Generate G-code: shared layers normally, then per-object with markers

### Pattern 4: Object Markers in G-code
**What:** Insert comment markers around each object's sequential G-code block
**When:** During G-code generation for sequential phase
**Example output:**
```gcode
; === HYBRID TRANSITION at layer 5 (Z=1.000) ===
G1 E-0.8 F2400     ; retract
G0 Z45.000          ; safe Z
; OBJECT_START id=0 name="bracket_left"
; Layer 5 at Z=1.000
G0 X10.0 Y10.0
...
; OBJECT_END id=0
G0 Z45.000          ; safe Z between objects
; OBJECT_START id=1 name="bracket_right"
...
; OBJECT_END id=1
```

### Pattern 5: Per-Object Progress Events
**What:** New `SliceEvent` variant for object-level progress
**Example:**
```rust
// In event.rs:
/// Per-object progress during hybrid sequential phase.
ObjectProgress {
    /// Object index (matches OBJECT_START id).
    object_index: usize,
    /// Total number of objects.
    total_objects: usize,
    /// Human-readable object name.
    object_name: String,
    /// Progress within this object (0.0 to 100.0).
    object_percent: f32,
    /// Current layer within this object's sequential section.
    object_layer: usize,
    /// Total layers for this object's sequential section.
    object_total_layers: usize,
},
```

### Anti-Patterns to Avoid
- **Creating a separate HybridConfig struct:** The decision is to extend `SequentialConfig` directly. Do not create a nested sub-struct.
- **Modifying shared layer ordering:** Shared layers must use normal by-layer ordering. Do not attempt to group by object during shared layers.
- **Per-object brim/skirt in sequential phase:** Explicitly deferred. Brim/skirt is generated once during shared layers only.
- **Re-validating collisions after skip:** Order is fixed at slice time. Skipping an object just skips its G-code block.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Mesh splitting | Custom triangle partitioning | `mesh.connected_components()` | Already handles vertex/triangle index mapping |
| Object ordering | Custom sort + collision check | `sequential::order_objects()` | Handles shortest-first with all-pairs collision validation |
| Safe Z calculation | Ad-hoc Z value | `plan_sequential_print()` pattern (clearance_height + margin) | Consistent with existing sequential behavior |
| G-code comment formatting | String concatenation | `GcodeCommand::Comment(...)` | Consistent with existing comment patterns |
| Config TOML parsing | Manual deserialization | `#[derive(Serialize, Deserialize)]` + `#[serde(default)]` | All config structs use this pattern |
| Setting metadata | Manual tier/depends_on | `#[setting(tier = 3, depends_on = "...")]` derive | SettingSchema derive handles all metadata |
| Profile import field mapping | New mapping functions | Extend existing `map_field_name()` + `apply_field()` | Follows established import pipeline |

## Common Pitfalls

### Pitfall 1: Transition Layer Off-by-One
**What goes wrong:** Shared layers counted from 0 or 1 inconsistently, causing either a missing layer or duplicated layer at transition.
**Why it happens:** Layer indices are 0-based in the engine, but `transition_layers = 5` means "print 5 layers shared" (layers 0-4), with sequential starting at layer 5.
**How to avoid:** Define clearly: shared layers are `0..transition_layers` (exclusive upper bound). Sequential starts at layer index `transition_layers`. Add test that verifies no gap or overlap.
**Warning signs:** Total layer count != shared layers + sum of per-object sequential layers.

### Pitfall 2: Z Height Mismatch at Transition
**What goes wrong:** Per-object sequential slicing starts at a different Z than where shared layers ended, causing layer height discontinuity.
**Why it happens:** Shared layers slice the combined mesh; per-object slicing starts fresh and may compute different Z heights (especially with adaptive layers).
**How to avoid:** Pass the transition Z height to per-object slicing as a starting Z. Do not let per-object slicing re-compute from Z=0; start from `transition_z`.
**Warning signs:** Visible seam/gap/overlap at the transition layer in the printed object.

### Pitfall 3: Component Slicing Ignores Shared Layers
**What goes wrong:** Per-object slicing accidentally re-slices layers 0..N, producing duplicate G-code for the shared portion.
**Why it happens:** `Engine::slice()` always starts from the bottom of the mesh.
**How to avoid:** Either (a) filter out layers below `transition_z` from per-object results, or (b) pass a `start_z` parameter to the slicing pipeline. Option (a) is simpler.
**Warning signs:** G-code file is larger than expected; print time estimate is inflated.

### Pitfall 4: Single Object with Hybrid Enabled
**What goes wrong:** Hybrid mode is enabled but the mesh has only one connected component. The transition is meaningless.
**Why it happens:** User enables hybrid without multiple objects on the plate.
**How to avoid:** Emit a warning (like existing single-object sequential warning at line 1258 of `engine.rs`) and fall through to normal slicing. Hybrid with one object should degrade gracefully to normal slicing.
**Warning signs:** No error, but user confusion about why hybrid had no effect.

### Pitfall 5: Object Names Missing
**What goes wrong:** Object markers have `name=""` because 3MF metadata was not propagated.
**Why it happens:** Object names come from 3MF metadata or plate config; not all meshes have names.
**How to avoid:** Use `object_name.unwrap_or_else(|| format!("object_{}", index))` as fallback. Check `ResolvedObject.name` in `slice_plate()` path.
**Warning signs:** G-code markers all show empty or generic names.

### Pitfall 6: Safe Z Too Low
**What goes wrong:** Travel between objects in sequential phase hits a previously printed object.
**Why it happens:** Safe Z calculated without accounting for all object heights, or margin too small.
**How to avoid:** Safe Z = `clearance_height + margin` (existing pattern in `plan_sequential_print()` uses 5mm margin). For hybrid, the safe Z only needs to clear objects that have been printed so far in the sequential ordering (shortest-first helps here).
**Warning signs:** Collision during print at transition between objects.

## Code Examples

### Extending SequentialConfig Default
```rust
// In config.rs Default impl:
impl Default for SequentialConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            extruder_clearance_radius: 35.0,
            extruder_clearance_height: 40.0,
            gantry_width: 0.0,
            gantry_depth: 0.0,
            extruder_clearance_polygon: Vec::new(),
            // New hybrid fields:
            hybrid_enabled: false,
            transition_layers: 5,
            transition_height: 0.0,
        }
    }
}
```

### Computing Transition Point
```rust
/// Determines the transition layer index for hybrid mode.
///
/// If `transition_layers > 0`, uses that directly.
/// If `transition_layers == 0` and `transition_height > 0.0`, finds the
/// layer index closest to that Z height.
pub fn compute_transition_layer(
    config: &SequentialConfig,
    layer_heights: &[f64], // Z heights for each layer
) -> u32 {
    if config.transition_layers > 0 {
        return config.transition_layers;
    }
    if config.transition_height > 0.0 {
        // Find first layer at or above transition_height
        for (i, &z) in layer_heights.iter().enumerate() {
            if z >= config.transition_height {
                return i as u32;
            }
        }
    }
    // Fallback: 5 layers
    5
}
```

### Inserting Object Markers
```rust
/// Generates G-code comment markers for object boundaries.
fn emit_object_start(cmds: &mut Vec<GcodeCommand>, index: usize, name: &str) {
    cmds.push(GcodeCommand::Comment(format!(
        "OBJECT_START id={index} name=\"{name}\""
    )));
}

fn emit_object_end(cmds: &mut Vec<GcodeCommand>, index: usize) {
    cmds.push(GcodeCommand::Comment(format!("OBJECT_END id={index}")));
}
```

### Dry-Run Output (Recommended Format)
```
=== Hybrid Sequential Print Plan ===

Transition: after layer 5 (Z=1.000mm)

Phase 1 (Shared Layers):
  Layers 0-4, all objects printed together
  Estimated time: ~2m 30s

Phase 2 (Sequential):
  Object order (shortest first):
    1. bracket_left  (height: 15.0mm, layers 5-80)
    2. bracket_right (height: 22.0mm, layers 5-115)
    3. mount         (height: 35.0mm, layers 5-180)
  Safe Z: 45.0mm
  Estimated time: ~18m 45s

Total estimated time: ~21m 15s
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Sequential validates only (no actual per-object slicing) | Hybrid mode with actual per-component slicing | Phase 49 | Enables real sequential printing, not just validation |
| No object markers in G-code | OBJECT_START/OBJECT_END comment markers | Phase 49 | Enables post-processor object skipping |
| Global progress events only | Per-object progress during sequential phase | Phase 49 | Better UX for multi-object prints |

**Key codebase note:** The comment at `engine.rs:1325-1328` explicitly acknowledges that "full object-by-object slicing requires API changes" -- this phase is that API change.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | `Cargo.toml` per-crate `[dev-dependencies]` |
| Quick run command | `cargo test -p slicecore-engine --lib sequential` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ADV-02.hybrid-config | Hybrid fields parse from TOML, defaults correct | unit | `cargo test -p slicecore-engine sequential::tests::hybrid -- -x` | No -- Wave 0 |
| ADV-02.hybrid-plan | `plan_hybrid_print()` computes correct transition and ordering | unit | `cargo test -p slicecore-engine sequential::tests::hybrid_plan -- -x` | No -- Wave 0 |
| ADV-02.transition-layer | Off-by-one check: shared layers 0..N, sequential starts at N | unit | `cargo test -p slicecore-engine sequential::tests::transition_layer -- -x` | No -- Wave 0 |
| ADV-02.object-markers | G-code contains OBJECT_START/OBJECT_END markers | unit | `cargo test -p slicecore-engine gcode_gen::tests::hybrid_markers -- -x` | No -- Wave 0 |
| ADV-02.per-object-progress | ObjectProgress events emitted during sequential phase | unit | `cargo test -p slicecore-engine event::tests::object_progress -- -x` | No -- Wave 0 |
| ADV-02.single-object-graceful | Hybrid with 1 object degrades to normal slicing with warning | unit | `cargo test -p slicecore-engine sequential::tests::single_object_hybrid -- -x` | No -- Wave 0 |
| ADV-02.dry-run | Dry-run produces plan output without slicing | integration | `cargo test -p slicecore-cli -- hybrid_dry_run` | No -- Wave 0 |
| ADV-02.profile-import | `print_sequence = "by object"` maps to sequential.enabled | unit | `cargo test -p slicecore-engine profile_import::tests::sequential -- -x` | Exists (partial) |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-engine --lib sequential`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `sequential.rs` -- hybrid config tests, hybrid plan tests, transition layer tests, single-object edge case
- [ ] `gcode_gen.rs` -- object marker tests
- [ ] `event.rs` -- ObjectProgress event serialization test
- [ ] CLI integration test for dry-run flag

## Open Questions

1. **Per-component slicing Z alignment**
   - What we know: Shared layers slice the combined mesh; per-object slicing needs to start at `transition_z`
   - What's unclear: Whether to pass `start_z` into `Engine::slice()` or filter layers post-slicing
   - Recommendation: Filter post-slicing (simpler, no API change to `slice()`). Slice each component from Z=0 but discard layers below `transition_z`.

2. **G-code stream merging vs. sequential generation**
   - What we know: Each object produces its own `Vec<LayerToolpath>` after per-component slicing
   - What's unclear: Whether to merge into one `Vec<GcodeCommand>` or write sequentially to the writer
   - Recommendation: Generate shared layers G-code first, then for each object generate its G-code with markers. Single output stream, no merging needed.

3. **Dry-run time estimation accuracy**
   - What we know: `SliceResult` already has `estimated_print_time_seconds`
   - What's unclear: How to estimate per-phase time without actually slicing
   - Recommendation: For dry-run, use rough estimation based on layer count * average layer time (from config speeds + object bounds). Accuracy is not critical for preview.

## Sources

### Primary (HIGH confidence)
- `crates/slicecore-engine/src/sequential.rs` -- Full sequential printing implementation (418 lines)
- `crates/slicecore-engine/src/config.rs` lines 3922-4014 -- `SequentialConfig` struct definition
- `crates/slicecore-engine/src/engine.rs` lines 1253-1329 -- Sequential validation in slice pipeline
- `crates/slicecore-engine/src/engine.rs` lines 817-880 -- `slice_plate()` multi-object slicing pattern
- `crates/slicecore-engine/src/event.rs` -- Full event system implementation
- `crates/slicecore-engine/src/gcode_gen.rs` -- G-code generation pipeline
- `crates/slicecore-engine/src/custom_gcode.rs` -- Custom G-code hooks system
- `crates/slicecore-engine/src/error.rs` -- EngineError enum
- `crates/slicecore-engine/src/profile_import.rs` -- Profile import field mappings (sequential fields already mapped)

### Secondary (MEDIUM confidence)
- Phase 49 CONTEXT.md -- User decisions and implementation constraints

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- No new dependencies, all patterns established in codebase
- Architecture: HIGH -- All integration points identified and verified in source code
- Pitfalls: HIGH -- Based on direct code reading of transition points and edge cases

**Research date:** 2026-03-26
**Valid until:** 2026-04-25 (stable -- internal architecture, no external dependencies)
