# Phase 45: Global and Per-Object Settings Override System - Research

**Researched:** 2026-03-24
**Domain:** Layered settings resolution, multi-object plate config, per-object/per-region overrides
**Confidence:** HIGH

## Summary

Phase 45 extends the existing 5-layer profile composition system (profile_compose.rs) to a 10-layer cascade, introduces PlateConfig as the new top-level abstraction replacing direct PrintConfig on the Engine, and replaces the 8-field SettingOverrides struct with full TOML partial merge for all ~385 config fields. The existing infrastructure -- TOML deep merge with provenance tracking, FieldSource enum, SettingRegistry, ConfigSchema derive macro -- provides a solid foundation that needs extension rather than replacement.

The primary engineering challenges are: (1) extending the ProfileComposer to support per-object and per-region layers with distinct FieldSource variants, (2) implementing PlateConfig with per-object Z-schedule computation, (3) adding override_safety metadata to all ~385 annotated fields via the derive macro, (4) 3MF import/export of per-object settings, and (5) significant CLI expansion (override-set CRUD, plate init/from-3mf/to-3mf, --object flag, --plate flag, multi-model support).

**Primary recommendation:** Build incrementally from the existing profile_compose.rs infrastructure. PlateConfig wraps ProfileComposer per-object, each object gets its own compose pipeline with layers 7-10 appended. The SettingOverrides struct is retired in favor of TOML partial tables flowing through the same merge_layer() function.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- ALL ~400+ PrintConfig fields overridable per-object and per-region -- no curated subset
- Same override scope for both per-object and per-region levels
- Replace existing SettingOverrides struct (8 fields) entirely with unified TOML partial merge approach
- Reuse profile_compose.rs infrastructure -- per-object/per-region overrides are additional layers in same merge pipeline with new FieldSource variants
- Nonsensical per-region overrides (e.g., bed_temperature on modifier mesh): warn but allow. --force suppresses warnings
- Per-object layer height fully supported -- engine slices at union of all objects' Z heights with per-object Z schedules
- Preserve and extend existing split_by_modifiers() geometric intersection approach -- modifier meshes carry TOML partial config tables instead of 8 Option fields
- New override_safety attribute in #[setting()] derive macro: safe, warn, ignored
- Claude annotates ALL ~400 fields during this phase using domain knowledge
- Reviewable OVERRIDE_SAFETY_MAP.md artifact produced before annotation -- user review gate
- Queryable via CLI: slicecore schema --override-safety warn filter
- Completeness test verifying every SettingRegistry field has override_safety classification
- Full 10-layer cascade order (Defaults > Machine > Filament > Process > User > CLI --set > Default object > Per-object > Layer-range > Per-region)
- Per-region overrides inherit from per-object config, not global
- Overlapping modifier meshes: last-defined wins (deterministic)
- Eager resolution: all per-object configs resolved upfront before slicing starts
- Provenance: distinct FieldSource variants -- DefaultObjectOverride, PerObjectOverride { object_id }, PerRegionOverride { object_id, modifier_id }
- Full provenance chain displayed
- PlateConfig struct in slicecore-engine (alongside PrintConfig)
- Engine API changes: Engine::new(plate_config: PlateConfig) instead of Engine::new(config: PrintConfig)
- Single-object plates work via PlateConfig with one object (backwards compatible)
- Arc<PrintConfig> sharing for objects with no overrides
- Core resolution logic is WASM-safe -- file operations are CLI-only
- Named override sets stored in ~/.slicecore/override-sets/ as TOML partial config files
- Full CRUD via slicecore override-set subcommand (list, show, create, edit, delete, rename, diff)
- All commands support --json output
- Schema-validated: field names checked against ConfigSchema, 'did you mean?' on typos
- Unified plate config: slicecore slice --plate plate.toml
- Sections: [profiles], [default_overrides], [override_sets.<name>], [[objects]]
- Objects support: model path, name, override_set reference, inline overrides, copies, transform
- Modifier meshes: geometric primitives AND STL file references
- Layer-range overrides: [[objects.layer_overrides]] with z_range or layer_range
- --plate and positional model arguments are mutually exclusive
- --object <id>:<source> flag for per-object overrides in direct CLI mode
- Object identification: both index (1-indexed) and name
- Source auto-detection for --object values
- Stacking: --object 1:high-detail+infill_density=80
- Single override set per object
- --set remains global only (cascade layer 6)
- Multiple model files supported
- copies = N in plate config
- Auto-arrange using Phase 27 system when no transforms specified
- G-code header: per-object sections showing override diffs from base + reproduce command
- --save-config: base config TOML. --save-plate: full plate config TOML
- --show-config extended: supports --object N and --at-z
- JSON output with per-object override diffs + provenance
- Per-object statistics: separate filament usage, time estimate, layer count
- Plate config checksum (SHA-256) in G-code header
- Engine API returns per-object provenance maps alongside G-code output
- 3MF import OrcaSlicer/PrusaSlicer per-object settings (automatic, best-effort)
- Import modifier meshes from 3MF with their override settings
- Unmapped fields preserved as pass-through metadata
- 3MF export: per-object overrides + modifier meshes + object transforms
- slicecore plate from-3mf and to-3mf subcommands
- Layer-range overrides at cascade layer 9, per-region at layer 10
- Layer-range overrides in plate config only (not via CLI flags)
- Reuse exit code 2 for override errors
- --strict: all warnings become errors. --force: all errors become warnings
- 'Did you mean?' fuzzy suggestions
- Validation: collect all errors then report at once
- Profile-aware override validation
- Basic bounding box collision detection between objects
- Build plate size validation
- Multi-progress bars: overall plate + per-object sub-progress (indicatif MultiProgress)
- No hard object limit; warn on >50 objects
- slicecore plate init subcommand
- Override set diff subcommand
- Parallel per-object slicing via rayon
- Cascade resolution computed once, stored
- G-code merge: streamed layer-by-layer (bounded memory)
- Exhaustive unit tests for cascade resolution
- Property-based tests (proptest) for cascade merge edge cases
- Per-object Z-schedule dedicated tests
- Plate-level E2E integration tests
- Full CLI E2E tests for all subcommands
- Criterion benchmarks for cascade resolution and config merge overhead

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

### Deferred Ideas (OUT OF SCOPE)
- Full adaptive per-object layer heights
- Full 3MF project output (G-code + thumbnails + slice preview embedded)
- Override set inheritance
- Per-feature-type overrides
- Conditional overrides
- Visual modifier mesh preview
- Dedicated slicecore plate management subcommand beyond from-3mf/to-3mf/init
- Object groups
- Per-object --set targeting
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| ADV-03 | Modifier meshes (region-specific setting overrides) | Full 10-layer cascade with per-region overrides at layer 10, extended split_by_modifiers() with TOML partial merge replacing SettingOverrides, override_safety metadata, PlateConfig with modifier mesh support (geometric primitives + STL), 3MF import/export of modifier settings |
</phase_requirements>

## Standard Stack

### Core (already in workspace)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| toml | 0.8 | TOML parsing/serialization for plate config and override sets | Already used in profile_compose.rs for deep merge |
| serde | 1 | Serialization framework for PlateConfig, ObjectConfig structs | Already used throughout |
| serde_json | 1 | JSON output for --json flag on all commands | Already used throughout |
| sha2 | (workspace) | SHA-256 checksums for plate config | Already used in profile_compose.rs |
| strsim | (workspace) | Fuzzy matching for 'did you mean?' suggestions | Already used in validate_set_key() |
| proptest | 1 | Property-based testing for cascade merge edge cases | Already used in 6+ crates |
| criterion | 0.5 | Benchmarks for cascade resolution performance | Already in workspace |
| indicatif | 0.17 | Multi-progress bars for per-object slicing | Already used in slicecore-cli |
| rayon | (workspace) | Parallel per-object slicing | Already used in Phase 25 |
| clap | (workspace) | CLI parsing for new subcommands | Already used in slicecore-cli |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| lib3mf_core | workspace | 3MF parsing/writing for per-object metadata | 3MF import/export of per-object settings |
| slicecore-config-derive | workspace | Extend derive macro with override_safety attribute | Adding override_safety to #[setting()] |
| slicecore-config-schema | workspace | Extend SettingDefinition with override_safety field | Schema metadata for override safety |

**Installation:** No new dependencies required. Everything is already in the workspace.

## Architecture Patterns

### Recommended Module Structure
```
crates/slicecore-engine/src/
  plate_config.rs          # PlateConfig, ObjectConfig, ModifierConfig, LayerRangeOverride
  cascade.rs               # 10-layer cascade resolution using ProfileComposer
  z_schedule.rs            # Per-object Z-schedule computation + union
  config.rs                # (modify) Remove SettingOverrides, update PrintConfig
  profile_compose.rs       # (modify) Add new FieldSource variants, support per-object layers
  modifier.rs              # (modify) Replace SettingOverrides with toml::Value partial

crates/slicecore-config-schema/src/
  types.rs                 # (modify) Add OverrideSafety enum to SettingDefinition

crates/slicecore-config-derive/src/
  parse.rs                 # (modify) Parse override_safety attribute
  codegen.rs               # (modify) Generate override_safety in HasSettingSchema impl

crates/slicecore-cli/src/
  override_set.rs          # override-set CRUD subcommands
  plate_cmd.rs             # plate init/from-3mf/to-3mf subcommands
  main.rs                  # (modify) Add --plate, --object flags, multi-model support
```

### Pattern 1: Extended ProfileComposer for Per-Object Resolution
**What:** Each object gets its own compose pipeline. Layers 1-6 are shared (base config). Layers 7-10 are per-object.
**When to use:** Always -- this is the core resolution mechanism.
**Example:**
```rust
// Extend SourceType with new variants
pub enum SourceType {
    Default,
    Machine,
    Filament,
    Process,
    UserOverride,
    CliSet,
    // New variants for layers 7-10
    DefaultObjectOverride,
    PerObjectOverride { object_id: String },
    LayerRangeOverride { object_id: String, range: String },
    PerRegionOverride { object_id: String, modifier_id: String },
}

// PlateConfig resolution creates per-object composers
pub fn resolve_object_config(
    base_composer: &ProfileComposer,  // layers 1-6 already composed
    default_overrides: Option<&toml::Value>,
    object_overrides: Option<&toml::Value>,
) -> Result<ComposedConfig, EngineError> {
    let mut composer = base_composer.clone();  // or rebuild from base result
    if let Some(defaults) = default_overrides {
        composer.add_table_layer(SourceType::DefaultObjectOverride, defaults.clone());
    }
    if let Some(overrides) = object_overrides {
        composer.add_table_layer(
            SourceType::PerObjectOverride { object_id: "obj1".into() },
            overrides.clone(),
        );
    }
    composer.compose()
}
```

### Pattern 2: PlateConfig as Engine Input
**What:** PlateConfig wraps base config + per-object configs. Engine takes PlateConfig.
**When to use:** All engine construction.
**Example:**
```rust
pub struct PlateConfig {
    /// Base profile layers (machine, filament, process, user, cli-set)
    pub base_layers: Vec<ProfileLayer>,
    /// Default overrides applied to all objects (layer 7)
    pub default_object_overrides: Option<toml::Value>,
    /// Named override sets (inline or referenced)
    pub override_sets: HashMap<String, toml::Value>,
    /// Per-object configurations
    pub objects: Vec<ObjectConfig>,
}

pub struct ObjectConfig {
    pub mesh_source: MeshSource,
    pub name: Option<String>,
    pub override_set: Option<String>,
    pub inline_overrides: Option<toml::Value>,
    pub modifiers: Vec<ModifierConfig>,
    pub layer_overrides: Vec<LayerRangeOverride>,
    pub transform: Option<Transform>,
    pub copies: u32,
}

// Single-object backward compatibility
impl From<PrintConfig> for PlateConfig {
    fn from(config: PrintConfig) -> Self { /* wrap in single-object plate */ }
}
```

### Pattern 3: Per-Object Z-Schedule
**What:** Each object maintains its own Z-height list based on resolved layer_height. Engine processes the union of all Z-heights.
**When to use:** When objects have different layer heights.
**Example:**
```rust
pub struct ZSchedule {
    /// All unique Z heights across all objects, sorted
    pub z_heights: Vec<f64>,
    /// For each Z height, which object indices are present
    pub object_membership: Vec<Vec<usize>>,
}

impl ZSchedule {
    pub fn from_objects(objects: &[(f64, f64)]) -> Self {
        // objects: Vec<(layer_height, total_height)>
        // Compute per-object Z lists, merge into sorted union
        // Track which objects appear at each Z
    }
}
```

### Pattern 4: TOML Partial Merge Replacing SettingOverrides
**What:** Modifier meshes carry toml::Value (partial table) instead of SettingOverrides struct. split_by_modifiers() uses merge_layer() instead of merge_into().
**When to use:** All modifier region handling.
**Example:**
```rust
// Old:
pub struct ModifierMesh {
    pub mesh: TriangleMesh,
    pub overrides: SettingOverrides,  // 8 Option fields
}

// New:
pub struct ModifierMesh {
    pub mesh: TriangleMesh,
    pub overrides: toml::map::Map<String, toml::Value>,  // Any config fields
    pub modifier_id: String,
}
```

### Pattern 5: Override Safety Metadata
**What:** Each setting field gets an override_safety classification: safe (any context), warn (nonsensical in some contexts), ignored (has no effect as per-region override).
**When to use:** Validation during cascade resolution.
**Example:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverrideSafety {
    Safe,
    Warn,
    Ignored,
}

// In config.rs:
#[setting(tier = 1, description = "...", override_safety = "safe")]
pub layer_height: f64,

#[setting(tier = 2, description = "...", override_safety = "warn")]
pub bed_temperature: f64,  // Nonsensical per-region but allowed
```

### Anti-Patterns to Avoid
- **Separate merge code for per-object vs base:** Reuse merge_layer() for all cascade levels. Do not create a parallel merge path.
- **Mutable shared state during parallel slicing:** Each object gets its own resolved PrintConfig (Arc<PrintConfig>) before slicing starts. No shared mutable state.
- **Lazy cascade resolution:** Do not resolve per-object configs lazily during slicing. Resolve eagerly before slicing begins.
- **Custom serialization for plate config:** Use serde + toml for plate.toml parsing. Do not hand-roll TOML parsing.
- **Mixing file I/O into engine core:** PlateConfig resolution is WASM-safe. File reading (loading override set files, plate TOML) happens in CLI layer only.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TOML deep merge with provenance | New merge function | Existing merge_layer() from profile_compose.rs | Already handles recursive table merge, provenance, conflict warnings |
| Config field validation | Manual field name checking | Existing validate_set_key() + SettingRegistry | Already has fuzzy matching, valid key enumeration |
| 3MF parsing/writing | Custom XML parser | lib3mf_core + lib3mf_converters | Already handles 3MF spec, used in export.rs |
| Progress bars | Custom terminal output | indicatif MultiProgress (already used) | Already integrated in CLI output system |
| Parallel per-object slicing | Thread pool management | rayon par_iter (Phase 25 pattern) | Already established pattern |
| SHA-256 checksums | Custom hash | sha2 crate (already used in profile_compose.rs) | Already in workspace |
| Fuzzy string matching | Custom similarity | strsim Jaro-Winkler (already used) | Already in validate_set_key() |

**Key insight:** Nearly every infrastructure need is already present in the codebase. The work is extending existing systems (ProfileComposer, FieldSource, SettingDefinition, derive macro, CLI commands) rather than building new ones.

## Common Pitfalls

### Pitfall 1: SettingOverrides Removal Breaking modifier.rs
**What goes wrong:** Removing SettingOverrides breaks ModifierMesh, ModifierRegion, split_by_modifiers(), and all tests that use them.
**Why it happens:** SettingOverrides is used in modifier.rs, config.rs, and their tests.
**How to avoid:** Replace SettingOverrides with toml::Value partial tables in a single coordinated change. Update ModifierMesh and ModifierRegion to carry toml::map::Map<String, toml::Value>. Update split_by_modifiers() to use merge_layer() for applying overrides.
**Warning signs:** Compilation errors in modifier.rs tests after removing SettingOverrides.

### Pitfall 2: Engine::new() Signature Change Breaking Everything
**What goes wrong:** Changing Engine::new(config: PrintConfig) to Engine::new(plate_config: PlateConfig) breaks all existing callers (tests, CLI, benchmarks).
**Why it happens:** Engine::new is called throughout the codebase.
**How to avoid:** Implement From<PrintConfig> for PlateConfig so existing callers can wrap. Consider providing Engine::from_config(config: PrintConfig) as a convenience that auto-wraps. Update callers incrementally.
**Warning signs:** Massive compilation failures across the workspace.

### Pitfall 3: TOML Table Merge Ordering for Overlapping Modifier Meshes
**What goes wrong:** Non-deterministic behavior when multiple modifier meshes overlap at the same Z-height.
**Why it happens:** The order of modifier application determines which wins in overlapping regions.
**How to avoid:** Enforce definition order: last-defined modifier wins (consistent with PrusaSlicer). Process modifiers in reverse order in split_by_modifiers() so the last-defined has highest priority.
**Warning signs:** Different results depending on modifier order in config.

### Pitfall 4: Z-Schedule Union Explosion
**What goes wrong:** When objects have very different layer heights (e.g., 0.05mm and 0.3mm), the union of Z-heights can be enormous.
**Why it happens:** Fine layer height objects generate many Z-heights that coarse objects don't need.
**How to avoid:** Implement the warning threshold (>2x max object's layer count). Each object only processes Z-heights in its own schedule, not all union heights. Track object membership per Z-height.
**Warning signs:** Memory/time explosion with mismatched layer heights.

### Pitfall 5: WASM Compatibility of PlateConfig
**What goes wrong:** File I/O for loading override sets or plate TOML breaks WASM compilation.
**Why it happens:** PlateConfig resolution might include file reading.
**How to avoid:** PlateConfig struct and cascade resolution must be pure computation (no std::fs). File loading happens exclusively in CLI layer, which passes pre-loaded TOML values to PlateConfig. PlateConfig only operates on in-memory data.
**Warning signs:** WASM CI gate failures.

### Pitfall 6: Derive Macro Changes Requiring Full Rebuild
**What goes wrong:** Changes to slicecore-config-derive force recompilation of all crates using SettingSchema derive.
**Why it happens:** Proc-macro changes invalidate all downstream compilation.
**How to avoid:** Get the override_safety attribute parsing right the first time. Test the derive macro changes in isolation before applying annotations to config.rs.
**Warning signs:** Long rebuild times during iteration.

### Pitfall 7: Override Set File Path Handling
**What goes wrong:** Override set paths behave differently on Windows vs Unix, or relative vs absolute paths cause confusion.
**Why it happens:** ~/.slicecore/override-sets/ path resolution has platform-specific behavior.
**How to avoid:** Use dirs::config_dir() or home_dir() for cross-platform path resolution. Always validate paths before using them. Support --override-sets-dir for testing.
**Warning signs:** Tests failing on different platforms.

## Code Examples

### Extending FieldSource with New Variants
```rust
// In profile_compose.rs - extend the existing SourceType enum
pub enum SourceType {
    // Existing variants (layers 1-6)
    Default,
    Machine,
    Filament,
    Process,
    UserOverride,
    CliSet,
    // New variants (layers 7-10)
    DefaultObjectOverride,
    PerObjectOverride { object_id: String },
    LayerRangeOverride {
        object_id: String,
        z_min: f64,
        z_max: f64,
    },
    PerRegionOverride {
        object_id: String,
        modifier_id: String,
    },
}
```

### PlateConfig TOML Format
```toml
# plate.toml example

[profiles]
machine = "ender3-s1"
filament = "generic-pla"
process = "0.2mm-quality"

[default_overrides]
infill_density = 0.3
wall_count = 3

[override_sets.high_detail]
layer_height = 0.1
wall_count = 4
infill_density = 0.5

[[objects]]
model = "part-a.stl"
name = "Main Body"
override_set = "high_detail"
copies = 1

[objects.transform]
position = [100.0, 100.0, 0.0]
rotation = [0.0, 0.0, 45.0]
scale = [1.0, 1.0, 1.0]

[objects.overrides]
# Inline overrides applied AFTER override_set
infill_density = 0.8

[[objects.modifiers]]
shape = "box"
position = [50.0, 50.0, 10.0]
size = [20.0, 20.0, 20.0]

[objects.modifiers.overrides]
infill_density = 1.0

[[objects.layer_overrides]]
z_range = [0.0, 2.0]

[objects.layer_overrides.overrides]
speeds.perimeter = 20.0

[[objects]]
model = "part-b.stl"
name = "Support Bracket"
# No override_set -- uses default_overrides only
```

### Per-Object Z-Schedule Computation
```rust
pub fn compute_z_schedule(
    objects: &[ResolvedObject],
) -> ZSchedule {
    let mut all_z: BTreeSet<OrderedFloat<f64>> = BTreeSet::new();
    let mut per_object_z: Vec<BTreeSet<OrderedFloat<f64>>> = Vec::new();

    for obj in objects {
        let mut obj_z = BTreeSet::new();
        let layer_h = obj.config.layer_height;
        let first_h = obj.config.first_layer_height;
        let total_h = obj.mesh_height;

        let mut z = first_h;
        obj_z.insert(OrderedFloat(z));
        while z < total_h {
            z += layer_h;
            if z > total_h { z = total_h; }
            obj_z.insert(OrderedFloat(z));
        }
        all_z.extend(&obj_z);
        per_object_z.push(obj_z);
    }

    let z_heights: Vec<f64> = all_z.iter().map(|z| z.0).collect();
    let object_membership: Vec<Vec<usize>> = z_heights.iter().map(|z| {
        per_object_z.iter().enumerate()
            .filter(|(_, obj_z)| obj_z.contains(&OrderedFloat(*z)))
            .map(|(i, _)| i)
            .collect()
    }).collect();

    ZSchedule { z_heights, object_membership }
}
```

### Override Safety Annotation Example
```rust
// In SettingDefinition - add new field
pub struct SettingDefinition {
    // ... existing fields ...
    pub override_safety: OverrideSafety,
}

// Usage in config.rs with derive macro
#[derive(SettingSchema)]
pub struct PrintConfig {
    #[setting(tier = 1, override_safety = "safe", description = "Layer height")]
    pub layer_height: f64,

    #[setting(tier = 2, override_safety = "warn", description = "Bed temperature")]
    pub bed_temp: f64,  // warn: nonsensical per-region

    #[setting(tier = 4, override_safety = "ignored", description = "Machine bed X")]
    pub bed_x: f64,  // ignored: machine property, meaningless per-object
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| 8-field SettingOverrides struct | TOML partial merge (all fields) | This phase | Unlocks full per-object/per-region configurability |
| Engine::new(PrintConfig) | Engine::new(PlateConfig) | This phase | Multi-object plate support |
| 5-layer cascade (D/M/F/P/U/CLI) | 10-layer cascade (+DefObj/PerObj/LayerRange/PerRegion) | This phase | Complete override hierarchy |
| No override safety metadata | override_safety attribute in #[setting()] | This phase | Validates override context appropriateness |

## Open Questions

1. **G-code merge strategy for multi-object plates**
   - What we know: Layer-by-layer interleaving with bounded memory, per-object tool changes
   - What's unclear: Exact interleaving strategy (round-robin per object per layer? object-first per layer?)
   - Recommendation: Process all objects at each Z-height before moving to next Z. Within a layer, process objects in definition order. This matches PrusaSlicer behavior.

2. **3MF per-object metadata namespace**
   - What we know: PrusaSlicer uses `slic3rpe:` namespace, OrcaSlicer uses similar. Best-effort import.
   - What's unclear: Exact OrcaSlicer/PrusaSlicer XML attribute names for per-object settings
   - Recommendation: Research specific 3MF metadata format during implementation. Use `slicecore:` namespace for SliceCore-specific fields. Implement best-effort field name mapping with unmapped fields preserved as pass-through.

3. **Modifier region caching (Claude's discretion)**
   - What we know: Modifier meshes are sliced at each Z-height. If modifier mesh is a simple shape, the footprint may be identical across many layers.
   - What's unclear: Whether caching provides meaningful speedup vs. complexity cost
   - Recommendation: Start without caching. Profile with benchmarks. Add caching only if modifier slicing shows up as a bottleneck. Simple shapes (box, cylinder) are already fast to compute cross-sections for.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) + proptest 1.x + criterion 0.5 |
| Config file | Cargo.toml [dev-dependencies] in each crate |
| Quick run command | `cargo test -p slicecore-engine --lib` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ADV-03-a | 10-layer cascade resolution with correct priority | unit | `cargo test -p slicecore-engine cascade` | No - Wave 0 |
| ADV-03-b | Per-object Z-schedule computation + union | unit | `cargo test -p slicecore-engine z_schedule` | No - Wave 0 |
| ADV-03-c | PlateConfig TOML parsing (valid + invalid) | unit | `cargo test -p slicecore-engine plate_config` | No - Wave 0 |
| ADV-03-d | override_safety completeness (all fields classified) | unit | `cargo test -p slicecore-engine override_safety_complete` | No - Wave 0 |
| ADV-03-e | Modifier mesh with TOML partial overrides | unit | `cargo test -p slicecore-engine modifier` | Partial (existing tests use SettingOverrides) |
| ADV-03-f | Override set CRUD CLI commands | integration | `cargo test -p slicecore-cli override_set` | No - Wave 0 |
| ADV-03-g | Plate init/from-3mf/to-3mf CLI commands | integration | `cargo test -p slicecore-cli plate_cmd` | No - Wave 0 |
| ADV-03-h | Engine::new(PlateConfig) backward compat | unit | `cargo test -p slicecore-engine engine_plate` | No - Wave 0 |
| ADV-03-i | Provenance chain accuracy across all 10 layers | unit | `cargo test -p slicecore-engine provenance` | No - Wave 0 |
| ADV-03-j | Property-based cascade merge edge cases | unit | `cargo test -p slicecore-engine cascade_proptest` | No - Wave 0 |
| ADV-03-k | Per-object statistics in slice output | integration | `cargo test -p slicecore-cli per_object_stats` | No - Wave 0 |
| ADV-03-l | 3MF import/export of per-object settings | integration | `cargo test -p slicecore-fileio threemf_overrides` | No - Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-engine --lib && cargo test -p slicecore-cli --lib`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/slicecore-engine/src/plate_config.rs` -- PlateConfig struct + parsing tests
- [ ] `crates/slicecore-engine/src/cascade.rs` -- 10-layer cascade resolution + tests
- [ ] `crates/slicecore-engine/src/z_schedule.rs` -- Z-schedule computation + tests
- [ ] `tests/fixtures/plate-configs/` -- Reusable plate config TOML fixtures
- [ ] `tests/fixtures/override-sets/` -- Reusable override set TOML fixtures
- [ ] proptest dependency added to slicecore-engine Cargo.toml [dev-dependencies]

## Sources

### Primary (HIGH confidence)
- `crates/slicecore-engine/src/profile_compose.rs` -- Full source reviewed: ProfileComposer, merge_layer(), FieldSource, SourceType enum, provenance tracking, validate_set_key()
- `crates/slicecore-engine/src/modifier.rs` -- Full source reviewed: ModifierMesh, ModifierRegion, slice_modifier(), split_by_modifiers()
- `crates/slicecore-engine/src/config.rs` -- SettingOverrides struct (lines 3340-3385), merge_into() method, 385 #[setting()] annotations
- `crates/slicecore-config-derive/src/parse.rs` -- SettingAttrs struct with all current parseable attributes
- `crates/slicecore-config-schema/src/types.rs` -- SettingDefinition struct, Tier enum, ValueType, SettingCategory
- `crates/slicecore-config-schema/src/registry.rs` -- SettingRegistry with register/get/all/filter
- `crates/slicecore-engine/src/engine.rs` -- Engine struct (line 635), Engine::new(config: PrintConfig) (line 651)
- `crates/slicecore-engine/src/support/override_system.rs` -- VolumeModifier with geometric shapes (Box, Cylinder, Sphere)
- `crates/slicecore-cli/src/main.rs` -- Commands enum (line 171), cmd_slice() function (line 1107)
- `crates/slicecore-fileio/src/export.rs` -- 3MF export using lib3mf_core Model
- `crates/slicecore-fileio/src/threemf.rs` -- 3MF import, multi-object parsing

### Secondary (MEDIUM confidence)
- 45-CONTEXT.md -- All locked decisions and deferred items reviewed
- Phase 30 context (referenced) -- 5-layer profile merge, provenance tracking patterns
- Phase 35 context (referenced) -- SettingSchema derive, TIER_MAP.md review gate pattern

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in workspace, no new dependencies needed
- Architecture: HIGH -- extending existing ProfileComposer/SettingOverrides/SettingDefinition patterns with clear extension points
- Pitfalls: HIGH -- identified from direct source code analysis of existing integration points
- Test infrastructure: HIGH -- proptest, criterion, cargo test all already established in workspace

**Research date:** 2026-03-24
**Valid until:** 2026-04-24 (stable -- internal project, no external API changes expected)
