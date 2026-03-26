# Phase 49: Hybrid Sequential Printing - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement hybrid print mode where the first N layers of all objects print together (normal by-layer ordering) for adhesion verification, then switch to sequential by-object printing for quality. Includes early failure detection via comment markers for object skipping, per-object progress events, and object naming in G-code. Extends existing SequentialConfig with hybrid fields.

</domain>

<decisions>
## Implementation Decisions

### Phase Transition Logic
- Two threshold modes: layer count (primary) and height-based (fallback)
- Layer count is the main control; height threshold only activates if layer count is 0 (disabled)
- Default transition layer count: 5 layers
- Default height threshold: 0.0 (disabled)
- At transition: retract, raise to safe Z (above clearance height), then begin first object from layer N+1
- No pause at transition by default (no M0/M1 insertion)
- Shared layers use normal by-layer ordering across all objects (no special grouping)
- Sequential object order after transition: shortest-first (reuses existing `order_objects()` from `sequential.rs`)

### Failure Detection & Object Skipping
- User-triggered skip via custom G-code comment markers
- Comment markers inserted between objects during sequential phase: `; OBJECT_START id=N name="..."` / `; OBJECT_END id=N`
- Post-processor or firmware macro can use markers to skip failed objects
- No skip hooks during shared layers — only between objects in sequential phase
- No collision re-evaluation when objects are skipped — order is fixed at slice time
- Skipping an object just skips its G-code block; remaining objects print in same order

### Object-by-Object Slicing Strategy
- Split mesh into components using existing `connected_components()`, slice each independently for layers above N
- Shared layers (1 through N): slice the full combined mesh as-is (normal multi-object slicing)
- After layer N: switch to per-component slicing for each object independently
- Per-object settings overrides (from Phase 45) apply in sequential phase only; shared layers use global settings
- Brim/skirt generated once during shared layers around all objects together — no per-object brim in sequential phase

### Per-Object Progress Events
- Emit progress events per-object during sequential phase through event system (API-05)
- Reports "Object M/N: X% complete" in addition to overall progress
- Object names included in progress events when available

### Object Naming in G-code
- Human-readable object names (from 3MF metadata or filename) included in comment markers alongside numeric indices
- Format: `; OBJECT_START id=0 name="bracket_left"`

### Hybrid Mode Dry-Run
- CLI flag to preview hybrid plan without slicing
- Shows: object order, transition point (layer count and height), estimated time per phase
- Quick validation before committing to a long print

### Config Structure
- Extend existing `SequentialConfig` with hybrid fields (not a separate sub-struct)
- New fields: `hybrid_enabled` (bool), `transition_layers` (u32, default 5), `transition_height` (f64, default 0.0)
- `hybrid_enabled` requires `sequential.enabled = true` — hybrid is a modifier on sequential mode
- All hybrid fields at Tier 3 (Advanced), same as sequential.enabled
- `depends_on = "sequential.enabled"` for all hybrid fields

### Profile Import
- Map known OrcaSlicer/Bambu sequential fields (`complete_objects`, etc.) to `sequential.enabled`
- No invented mappings for hybrid fields that don't exist upstream yet
- Add hybrid field mappings when/if OrcaSlicer adds hybrid mode support

### Claude's Discretion
- Safe Z calculation details for transition (margin above clearance height)
- G-code comment marker exact format and placement details
- Per-component slicing implementation (how to split/rejoin G-code streams)
- Progress event frequency and granularity during sequential phase
- Dry-run output format and time estimation approach
- Error handling for edge cases (single object with hybrid enabled, objects with zero shared layers)
- Test fixture design for hybrid mode validation

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing Sequential Implementation
- `crates/slicecore-engine/src/sequential.rs` — `ObjectBounds`, `detect_collision()`, `order_objects()`, `plan_sequential_print()` — all reusable for hybrid mode
- `crates/slicecore-engine/src/config.rs` — `SequentialConfig` struct (line ~3940), `PrintOrder` enum with ByLayer/ByObject variants
- `crates/slicecore-engine/src/engine.rs` — Sequential validation in `slice()` (line ~1253), `connected_components()` usage for object splitting

### Arrange Crate Sequential Support
- `crates/slicecore-arrange/src/sequential.rs` — `expand_for_gantry()`, `validate_sequential()`, `order_back_to_front()` — arrangement-level sequential support
- `crates/slicecore-arrange/src/config.rs` — `GantryModel` enum for clearance modeling

### Per-Object Overrides (Phase 45)
- `crates/slicecore-engine/src/config.rs` — Per-object settings override system from Phase 45

### Custom G-code & Events
- `crates/slicecore-engine/src/custom_gcode.rs` — Custom G-code injection system (hook points for object markers)
- `crates/slicecore-engine/src/event.rs` — Event system (API-05) for progress reporting

### Design Documents
- `designDocs/04-IMPLEMENTATION-GUIDE.md` line 499 — Sequential printing: object ordering and collision checks
- `designDocs/01-PRODUCT_REQUIREMENTS.md` line 153 — FR-020: Sequential printing with collision avoidance

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `sequential::order_objects()` — Shortest-first ordering with collision validation, directly reusable for hybrid post-transition ordering
- `sequential::detect_collision()` — Bounding box collision detection with clearance envelope, validates hybrid feasibility
- `sequential::plan_sequential_print()` — Returns ordered `(object_index, safe_z)` pairs, extend for hybrid transition
- `Engine::slice()` connected_components() call — Already splits mesh for sequential validation, extend to actually slice per-component
- `SequentialConfig` — Extend with hybrid fields rather than creating new struct
- `custom_gcode.rs` — Hook system for inserting object markers in G-code stream
- Event system (API-05) — Pub/sub for per-object progress events

### Established Patterns
- Config sub-structs: `SequentialConfig`, `ZHopConfig`, `CoolingConfig` all use `#[setting(flatten)]` in `PrintConfig`
- Config tiers: `#[setting(tier = 3)]` for advanced features, `depends_on` for conditional visibility
- Error handling: `thiserror` enums in per-crate `error.rs`
- G-code comments: Feature labels already inserted via `feature_label()` in `gcode_gen.rs`
- Profile import: `profile_import.rs` and `profile_import_ini.rs` pipelines for field mapping

### Integration Points
- `SequentialConfig` — Gains `hybrid_enabled`, `transition_layers`, `transition_height` fields
- `Engine::slice()` — Major refactor: two-phase slicing (shared layers + per-object sequential layers)
- `gcode_gen.rs` — Object start/end comment markers, safe-Z transitions between objects
- `event.rs` — New per-object progress event variant
- `profile_import.rs` / `profile_import_ini.rs` — Map `complete_objects` to sequential.enabled (existing fields only)

</code_context>

<specifics>
## Specific Ideas

- Hybrid mode solves the key weakness of pure sequential printing: bed adhesion uncertainty. By printing first N layers of all objects together, users verify adhesion before committing to the sequential quality benefits.
- Object comment markers (`OBJECT_START`/`OBJECT_END`) serve dual purpose: enable runtime skip via post-processor AND provide clear G-code readability for debugging.
- The engine note "full object-by-object slicing requires API changes" (in engine.rs line ~1326) is exactly what this phase addresses — extending beyond validation to actual per-component slicing.
- Dry-run preview lets users validate the hybrid plan (object order, transition point) without waiting for a full slice — important for large multi-object plates.

</specifics>

<deferred>
## Deferred Ideas

### Future Enhancements
- **Transition layer blending** — Gradually reduce inter-object travel over 2-3 layers for smoother thermal transition instead of hard cut
- **Collision re-check at transition** — Re-validate clearances using actual printed heights (elephant's foot compensation)
- **Pause-at-transition option** — M0/M1 pause after shared layers for visual adhesion inspection before sequential phase
- **Back-to-front ordering option** — Alternative object ordering for bed-slinger printers (Y-axis based)
- **User-configurable object order** — Custom ordering via `ObjectOrder::Custom(Vec<usize>)` enum variant
- **Klipper EXCLUDE_OBJECT integration** — Native EXCLUDE_OBJECT_DEFINE/START/END markers for runtime object exclusion
- **Per-object brim in sequential phase** — Optional per-object brim generation for extra adhesion safety
- **Firmware variable-based skip** — Klipper macros / Marlin M808 for automated skip without post-processor

</deferred>

---

*Phase: 49-hybrid-sequential-printing*
*Context gathered: 2026-03-26*
