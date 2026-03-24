# Phase 45: Global and Per-Object Settings Override System - Context

**Gathered:** 2026-03-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement a layered settings override system (global -> per-object -> per-region) with proper cascading, validation, serialization, and CLI tooling. Enables users to customize specific objects on multi-object plates with different infill, layer height, or other parameters. Includes: PlateConfig data model, 10-layer cascade resolution, override set management CLI, plate config TOML format, per-object Z scheduling, modifier mesh extension, layer-range overrides, 3MF import/export of per-object settings, override_safety annotations for all ~400 config fields, and multi-model CLI support.

**Not in scope:** Full 3MF project output with G-code/thumbnails/slice-preview embedding (deferred), override set inheritance, full adaptive per-object layer height, per-feature-type overrides, conditional overrides, dedicated `slicecore plate` management subcommand beyond from-3mf/to-3mf/init.

</domain>

<decisions>
## Implementation Decisions

### Override Field Coverage
- ALL ~400+ PrintConfig fields overridable per-object and per-region — no curated subset
- Same override scope for both per-object and per-region levels
- Replace existing `SettingOverrides` struct (8 fields) entirely with unified TOML partial merge approach
- Reuse `profile_compose.rs` infrastructure — per-object/per-region overrides are additional layers in the same merge pipeline with new FieldSource variants
- Nonsensical per-region overrides (e.g., bed_temperature on modifier mesh): warn but allow. --force suppresses warnings
- Per-object layer height fully supported — engine slices at union of all objects' Z heights with per-object Z schedules
- Preserve and extend existing `split_by_modifiers()` geometric intersection approach — modifier meshes carry TOML partial config tables instead of 8 Option fields

### Override Safety Metadata
- New `override_safety` attribute in `#[setting()]` derive macro: `safe`, `warn`, `ignored`
- Claude annotates ALL ~400 fields during this phase using domain knowledge
- Reviewable `OVERRIDE_SAFETY_MAP.md` artifact produced before annotation — user review gate (same pattern as Phase 35 TIER_MAP.md)
- Queryable via CLI: `slicecore schema --override-safety warn` filter added to existing schema command
- Warning messages include brief explanation of WHY the override is nonsensical
- Completeness test verifying every SettingRegistry field has override_safety classification

### 10-Layer Cascade Resolution
- Full cascade order:
  1. Defaults (compiled defaults)
  2. Machine profile
  3. Filament profile
  4. Process profile
  5. User overrides file
  6. CLI --set (global only, applies to base config)
  7. Default object overrides (applies to all objects)
  8. Per-object overrides (named set, file, or inline)
  9. Layer-range overrides (Z-height or layer-number ranges)
  10. Per-region (modifier mesh) overrides
- Per-region overrides inherit from per-object config, not global (natural cascading)
- Overlapping modifier meshes: last-defined wins (deterministic, matches PrusaSlicer)
- Eager resolution: all per-object configs resolved upfront before slicing starts
- Provenance: distinct FieldSource variants — DefaultObjectOverride, PerObjectOverride { object_id }, PerRegionOverride { object_id, modifier_id }
- Full provenance chain displayed: `infill_density = 80 [PerObject(1) > DefaultObject(30) > Process(20) > Default(15)]`

### Per-Object Z Scheduling
- Each object maintains its own Z-height schedule based on its resolved layer height
- Engine iterates through global union of all Z heights, only processes objects present at each Z
- Fixed layer height only for per-object override (full adaptive per-object deferred)
- Warn when Z-height union exceeds threshold (>2x max object's layer count) — proceed with --force
- Dedicated unit tests for Z-schedule computation

### PlateConfig Data Model
- Formal `PlateConfig` struct in slicecore-engine (alongside PrintConfig)
- Engine API changes: `Engine::new(plate_config: PlateConfig)` instead of `Engine::new(config: PrintConfig)`
- Single-object plates work via PlateConfig with one object (backwards compatible)
- PlateConfig includes: base config, default_object_overrides, Vec<ObjectConfig>, named override sets
- ObjectConfig includes: mesh, name (optional), overrides, modifier meshes, layer-range overrides, transform, copies
- Arc<PrintConfig> sharing for objects with no overrides (saves memory on large plates)
- Core resolution logic is WASM-safe — file operations are CLI-only

### Named Override Sets
- Stored in `~/.slicecore/override-sets/` as TOML partial config files
- No inheritance between sets in this phase
- Both referenced by name AND inline overrides supported
- Full CRUD via `slicecore override-set` subcommand:
  - `list` — show all sets with field counts
  - `show <name>` — display values with ConfigSchema metadata (units, description, safety)
  - `create <name> --set key=value` — create with schema validation + `--from-diff profile-a profile-b`
  - `edit <name>` — open in $EDITOR, re-validate after
  - `delete <name>` — with confirmation, --force skips
  - `rename <old> <new>`
  - `diff <set-a> <set-b>` — side-by-side table comparison
- All commands support --json output
- Schema-validated: field names checked against ConfigSchema, 'did you mean?' on typos

### Plate Config TOML Format
- Unified plate config: `slicecore slice --plate plate.toml`
- Sections: `[profiles]`, `[default_overrides]`, `[override_sets.<name>]`, `[[objects]]`
- Objects support: model path, name, override_set reference, inline [objects.overrides], copies, [objects.transform] (position, rotation, scale)
- Modifier meshes: geometric primitives (box, cylinder, sphere with position, size, rotation) AND STL file references
- Layer-range overrides: `[[objects.layer_overrides]]` with `z_range` or `layer_range` + `[objects.layer_overrides.overrides]`
- Override sets can be defined inline in plate config AND/OR reference ~/.slicecore/override-sets/
- --plate and positional model arguments are mutually exclusive

### CLI Override Specification
- `--object <id>:<source>` flag (repeatable) for per-object overrides in direct CLI mode
- Object identification: both index (1-indexed) and name (from 3MF or plate config)
- Source auto-detection: contains '=' -> inline k=v, contains '/' or '.toml' -> file, else -> named set lookup
- Stacking: `--object 1:high-detail+infill_density=80` applies named set then inline overrides on top
- Single override set per object (no multi-set stacking)
- --set remains global only (cascade layer 6)
- --config as base: compatible with --object (provides base config, per-object overrides layer on top)
- --plate and --set can combine (--set applies at layer 6 on top of plate's base config)

### Multi-Model Support
- Accept multiple model files: `slicecore slice a.stl b.stl --object 1:high-detail`
- Duplicate model files supported (same STL with different overrides for comparison)
- `copies = N` in plate config creates N identical instances (all copies identical, not individually addressable)
- Auto-arrange using Phase 27 system when no transforms specified
- Object names: optional `name` field in plate config, defaults to filename stem, 3MF names preserved

### Serialization & Provenance
- G-code header: per-object sections showing override diffs from base + reproduce command
- Reproduce command: uses --plate reference for complex plates, full CLI for simple ones
- --save-config: base config TOML. --save-plate: full plate config TOML (self-contained, override set contents inlined)
- --save-plate supports both TOML and 3MF output based on file extension
- --show-config extended: supports --object N and --at-z for per-object/per-region config inspection with full provenance chain
- JSON output: structured JSON with base config + per-object override diffs + provenance via --json
- Per-object statistics: separate filament usage, time estimate, layer count per object in slice summary
- Per-object log sections: config resolution, slicing progress, and statistics per object
- Plate config checksum (SHA-256) in G-code header for change detection
- Engine API returns per-object provenance maps alongside G-code output

### 3MF Import/Export
- Import OrcaSlicer/PrusaSlicer per-object settings: automatic (no flag needed), best-effort field mapping
- Import modifier meshes from 3MF with their override settings
- Unmapped fields: preserved as pass-through metadata (for round-tripping)
- Import summary printed to stderr: objects found, overrides imported, unmapped field warnings
- 3MF names preserved and used in output
- Export: write per-object overrides + modifier meshes + object transforms into 3MF (models embedded, self-contained)
- Best-effort OrcaSlicer/PrusaSlicer compatibility for basic settings; our own namespace for SliceCore-specific fields
- `slicecore plate from-3mf plate.3mf -o output/` — extracts objects as STLs + modifier meshes as STLs + plate.toml
- `slicecore plate to-3mf plate.toml -o output.3mf` — packages plate config into 3MF
- 3MF output: models + settings + modifiers only (no G-code/thumbnails in this phase)

### Layer-Range Overrides
- Specified in plate config: `[[objects.layer_overrides]]`
- Support both `z_range = [min, max]` (mm) and `layer_range = [start, end]` (layer numbers)
- Stack with modifier mesh overrides: layer-range applies at cascade layer 9, per-region at layer 10
- Plate config only (not via CLI flags — too complex)

### Error Handling & UX
- Reuse exit code 2 for override errors (config/profile error)
- --strict: all warnings become errors. --force: all errors become warnings. Default: warnings warn, errors error
- 'Did you mean?' fuzzy suggestions for override set names and field names
- Object index out-of-bounds: clear error with count and 1-indexing note
- Validation: collect all errors (missing files, invalid sets, bad fields) then report at once
- Profile-aware override validation: warn when overrides conflict with machine limits; error with --strict
- Active override summary in --dry-run output
- Fail entire plate if any object errors (mesh repair, config validation)
- Basic bounding box collision detection between objects (warn on overlap)
- Build plate size validation: verify all objects fit within machine bed dimensions
- Multi-progress bars: overall plate + per-object sub-progress (indicatif MultiProgress)
- No hard object limit; warn on >50 objects about memory/time implications

### Plate Init & Template
- `slicecore plate init [model files] [-m/-f/-p profiles] -o plate.toml`
- Generates fully commented template plate.toml with all sections explained
- Pre-populates [[objects]] and [profiles] from provided arguments
- `plate from-3mf` for importing existing 3MF projects

### Override Set Diff
- `slicecore override-set diff set-a set-b` — side-by-side table comparison
- Shows fields that differ + fields only in one set
- Supports --json output

### Performance
- Arc<PrintConfig> sharing for objects with identical (no-override) configs
- Parallel per-object slicing via rayon (natural parallelism, no shared mutable state)
- Let rayon decide thread scheduling (consistent with Phase 25)
- Cascade resolution computed once, stored (compute-once pattern)
- Modifier region caching: Claude's discretion during planning
- G-code merge: streamed layer-by-layer (bounded memory)
- No hard object limit; warn on large plates

### Testing
- Exhaustive unit tests for 10-layer cascade resolution (every layer interaction, provenance accuracy)
- Property-based tests (proptest) for cascade merge edge cases
- Per-object Z-schedule dedicated tests (union computation, membership, edge cases)
- Plate-level E2E integration tests (load plate config, resolve overrides, slice, verify G-code)
- Full CLI E2E tests for all override-set subcommands + plate init/from-3mf/to-3mf
- Override_safety annotation completeness test
- Plate TOML parsing test suite (valid configs, edge cases, error conditions)
- Criterion benchmarks: cascade resolution, config merge overhead, single vs multi-object slice time
- Regression tests: existing single-object workflow via PlateConfig with one object
- Reusable test fixtures: plate configs and override sets in tests/fixtures/

### Claude's Discretion
- Modifier region caching strategy (cache identical footprints across layers or always compute)
- Modifier mesh CLI association in direct mode (not plate config)
- --save-plate auto-save behavior alongside G-code
- PlateConfig internal data structures and field organization
- OVERRIDE_SAFETY_MAP.md production methodology
- 3MF namespace conventions for SliceCore-specific metadata
- Property-based test strategy and property definitions
- Exact plate.toml template content and comment wording
- Override set TOML file format details
- G-code merge implementation details for multi-object interleaving

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Override system requirements
- `designDocs/01-PRODUCT_REQUIREMENTS.md` — ADV-03: Modifier meshes (region-specific setting overrides)
- `designDocs/02-ARCHITECTURE.md` — Architecture patterns for config and plugin system

### Current config implementation
- `crates/slicecore-engine/src/config.rs` — PrintConfig struct (~line 1791), existing SettingOverrides struct (~line 3340), merge_into() method (~line 3360)
- `crates/slicecore-engine/src/modifier.rs` — ModifierMesh, ModifierRegion, split_by_modifiers() — existing geometric per-region override system
- `crates/slicecore-engine/src/profile_compose.rs` — 5-layer TOML merge with provenance tracking, FieldSource enum
- `crates/slicecore-engine/src/support/override_system.rs` — Support volume modifier system (enforcers/blockers)

### ConfigSchema system
- `crates/slicecore-config-derive/src/lib.rs` — Config derive macro for #[setting()] attributes
- `crates/slicecore-config-schema/src/` — SettingDefinition, SettingRegistry, ValueType

### Prior config decisions
- `.planning/phases/30-cli-profile-composition-and-slice-workflow/30-CONTEXT.md` — 5-layer profile merge, provenance tracking, --set, --config, --save-config, exit codes, --force
- `.planning/phases/35-configschema-system-with-setting-metadata-and-json-schema-generation/35-CONTEXT.md` — SettingSchema derive, tier system, override metadata pattern, TIER_MAP.md review gate

### 3MF and mesh handling
- `crates/lib3mf-core/` — Pure Rust 3MF parser, object metadata extraction
- `.planning/phases/24-mesh-export-stl-3mf-write/24-CONTEXT.md` — 3MF export capabilities

### Auto-arrangement
- `.planning/phases/27-build-plate-auto-arrangement/27-CONTEXT.md` — Auto-arrange system for multi-object plates

### Parallel slicing
- `.planning/phases/25-parallel-slicing-pipeline-rayon/25-CONTEXT.md` — Rayon parallelization patterns

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `SettingOverrides::merge_into()` (config.rs:3360) — existing partial merge pattern, to be replaced by TOML-level merge
- `split_by_modifiers()` (modifier.rs:80-117) — geometric intersection for modifier regions, to be extended
- `profile_compose.rs` — TOML deep merge + provenance tracking infrastructure, to be reused with new FieldSource variants
- `SettingRegistry` (slicecore-config-schema) — runtime metadata, to be extended with override_safety field
- `#[derive(SettingSchema)]` (slicecore-config-derive) — proc-macro, to be extended with override_safety attribute
- `VolumeModifier` (support/override_system.rs) — geometric modifier concept (box, cylinder, sphere), similar pattern for plate config modifiers
- Auto-arrange system (Phase 27) — for multi-object plate arrangement

### Established Patterns
- TOML partial deserialization + field-level merge (profile_compose.rs)
- FieldSource provenance tracking with HashMap<String, FieldSource>
- clap derive for CLI parsing with subcommands
- --json flag on all output commands
- ConfigSchema derive macro with #[setting()] attributes
- stderr for progress/warnings, stdout for output data
- 'Did you mean?' fuzzy matching for profile names and --set keys
- indicatif for progress bars, MultiProgress for parallel tasks

### Integration Points
- `Engine::new(config)` — needs to change to `Engine::new(plate_config: PlateConfig)`
- `cmd_slice()` in main.rs — needs plate config parsing, multi-model support, --object flag
- `FieldSource` enum — needs new variants for cascade layers 7-10
- `SettingDefinition` struct — needs override_safety field
- `#[derive(SettingSchema)]` — needs override_safety attribute support
- G-code generator — needs per-object sections and multi-object interleaving
- `slicecore schema` CLI — needs --override-safety filter
- lib3mf-core — needs per-object metadata extraction and writing

</code_context>

<specifics>
## Specific Ideas

- PlateConfig is the new top-level abstraction: Engine takes PlateConfig, single-model invocations auto-wrap in PlateConfig with one object
- Override sets are the "named partial config" concept — reusable across plates and invocations
- `--from-diff` on override-set create enables quick set generation from profile comparisons
- plate.toml is like a simplified 3MF project in TOML form — complete plate description
- 3MF round-tripping: from-3mf extracts to TOML + STLs, to-3mf packages back. Full interop cycle
- The 10-layer cascade is an extension of Phase 30's 5-layer model, not a replacement — layers 1-6 remain unchanged
- Per-object Z scheduling handles the complex case where objects with different layer heights share a build plate

</specifics>

<deferred>
## Deferred Ideas

- **Full adaptive per-object layer heights** — Each object gets its own adaptive layer height analysis based on its geometry + override settings. Most flexible but significantly more complex. Context: per-object Z scheduling in the cascade resolution system (crates/slicecore-engine/src/config.rs, modifier.rs). **HIGH PRIORITY** — extends the per-object layer height override system implemented in this phase
- **Full 3MF project output** — G-code + thumbnails + slice preview embedded in 3MF alongside models + settings. Completes the 3MF project file format. **HIGH PRIORITY** — must be implemented; this phase does models + settings only
- **Override set inheritance** — Sets extending other sets (single-level inherits pattern like profiles). Deferred to keep sets simple in v1
- **Per-feature-type overrides** — Override speeds/flow for specific feature types (perimeters, infill, bridges) per object
- **Conditional overrides** — Override sets that activate based on conditions (e.g., "if layer_height < 0.1, use slower speeds")
- **Visual modifier mesh preview** — ASCII or SVG visualization of modifier mesh placement relative to objects
- **Dedicated `slicecore plate` management subcommand** — Beyond from-3mf/to-3mf/init: validate, info, list objects, etc.
- **Object groups** — Named groups of objects that share behavior beyond override sets
- **Per-object --set targeting** — `--set 1:field=value` targeting specific objects from CLI

</deferred>

---

*Phase: 45-global-and-per-object-settings-override-system-implement-layered-settings-resolution-with-per-object-and-per-region-overrides*
*Context gathered: 2026-03-24*
