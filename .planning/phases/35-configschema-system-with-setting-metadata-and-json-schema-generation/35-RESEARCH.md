# Phase 35: ConfigSchema System with Setting Metadata and JSON Schema Generation - Research

**Researched:** 2026-03-17
**Domain:** Rust proc-macro derive system, runtime setting registry, JSON Schema generation
**Confidence:** HIGH

## Summary

Phase 35 builds a per-field metadata system for all config settings (~387 fields across config.rs + support/config.rs) using a custom `#[derive(SettingSchema)]` proc-macro. This requires two new crates (`slicecore-config-derive` for the proc-macro, `slicecore-config-schema` for runtime types), annotation of every field with tier/description/units/constraints/affects metadata, a `SettingRegistry` global singleton, JSON Schema 2020-12 output, flat metadata JSON output, a CLI `schema` subcommand, and replacement of the existing ad-hoc `config_validate.rs` with schema-driven validation.

The project uses Rust edition 2021, MSRV 1.75, but the installed toolchain is rustc 1.93.1. `std::sync::LazyLock` was stabilized in Rust 1.80, so it is available on the installed toolchain but NOT within the declared MSRV 1.75. The project should either bump MSRV to 1.80+ or use `once_cell::sync::Lazy` for the global singleton. Given 34 phases have already been built, bumping MSRV is the cleaner choice.

**Primary recommendation:** Build the proc-macro using syn 2.x + quote 1.x + proc-macro2 1.x (standard Rust proc-macro stack). Generate JSON Schema manually via serde_json rather than using the `schemars` crate, because the schema requires extensive custom `x-` extensions and a specific nested structure matching the Rust struct hierarchy. The existing `config_validate.rs` (~200 lines of ad-hoc checks) is small enough to replace cleanly with schema-driven validation.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Custom `#[derive(SettingSchema)]` proc-macro with `#[setting(...)]` attributes on each field
- New crate: `slicecore-config-derive` (proc-macro crate)
- New crate: `slicecore-config-schema` (runtime types: SettingDefinition, SettingRegistry, ValueType, etc.)
- `slicecore-engine` depends on both
- Explicit `description = "..."` attribute required (NOT auto-extracted from doc comments)
- `display_name` auto-generated from field name (snake_case to Title Case), overridable
- `#[setting(flatten)]` on sub-struct fields auto-prefixes child keys with parent field name
- Enums also derive `SettingSchema` with variant metadata
- `ValueType` inferred from Rust types (f64->Float, bool->Bool, Vec<f64>->FloatVec, Option<T>->optional)
- Fields WITHOUT `#[setting()]` still get registered with defaults (tier=4, empty description, inferred type)
- `#[setting(skip)]` excludes fields from schema
- Struct-level `#[setting(category = "...")]` sets default category
- 5 tiers: 0=AI Auto, 1=Simple (~15), 2=Intermediate (~60), 3=Advanced (~200), 4=Developer (rest)
- Constraint types: Range { min, max }, Enum { variants }, DependsOn { key, condition }
- `depends_on` uses simple equality checks
- Full `affects` dependency graph populated for ALL fields
- `affected_by` auto-generated as inverse of `affects`
- `SettingKey` is a newtype around String with dotted path format
- Both compile-time AND runtime validation
- Additional metadata: since_version, deprecated, tags
- JSON Schema draft 2020-12 with nested structure + `$ref` to `$defs`
- Custom metadata as `x-` extensions in JSON Schema
- Flat metadata JSON format for UI/AI consumption
- `slicecore schema` CLI subcommand with --format, --tier, --category, --search
- `SettingRegistry.search(query)` with case-insensitive substring matching
- Lazy global singleton via `std::sync::LazyLock` (or `once_cell::sync::Lazy`)
- Replace Phase 30's ad-hoc validation with schema-driven validation
- ALL ~400+ fields annotated in this phase
- TIER_MAP.md reviewable artifact produced before annotation work

### Claude's Discretion
- Exact SettingCategory enum variant list and naming
- Internal proc-macro implementation details (syn/quote patterns)
- TIER_MAP.md production methodology
- Exact search ranking algorithm
- Compile warning implementation approach
- LazyLock vs once_cell choice based on MSRV
- JSON Schema structure details (how to represent depends_on as if/then/else)
- Test strategy for the derive macro
- Order of annotation work across config structs

### Deferred Ideas (OUT OF SCOPE)
- Auto-generated markdown docs (`--format markdown`)
- Schema diff tool (`--diff`)
- Interactive schema explorer (TUI/web)
- OpenAPI integration
- AI prompt schema / Tier 0 population / Smart defaults
- Config migration system
- UI form generator
- WASM-compatible registry (alternative initialization)
- Engine behavior wiring for mapped fields
- Profile management CLI
- Upstream sync workflow
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| syn | 2.0.117 | Parse Rust source in proc-macro | Universal proc-macro parser, no alternative |
| quote | 1.0.45 | Generate Rust token streams | Standard companion to syn |
| proc-macro2 | 1.0.106 | Token stream wrapper for testing | Required by syn 2.x |
| serde_json | 1.x (workspace) | JSON Schema output generation | Already in workspace |
| serde | 1.x (workspace) | Serialize SettingDefinition | Already in workspace |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| once_cell | 1.x | Lazy global singleton (if MSRV stays 1.75) | Only if MSRV not bumped |
| clap | 4.5 (already in CLI) | `schema` subcommand | Already a dependency |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom JSON Schema gen | schemars 1.x | schemars can't produce custom `x-` extensions in the way needed; nested struct hierarchy with flattened dotted keys requires manual control |
| Custom derive macro | Build-script codegen | Derive macro is more ergonomic and gives compile-time attribute validation; build scripts would require separate metadata files |

**Installation (new crates):**
```bash
# slicecore-config-derive/Cargo.toml
cargo add syn --features="full,derive,extra-traits,parsing"
cargo add quote
cargo add proc-macro2

# slicecore-config-schema/Cargo.toml
# Uses workspace serde, serde_json only
```

## Architecture Patterns

### Recommended Project Structure
```
crates/
├── slicecore-config-schema/     # Runtime types (no proc-macro dependency)
│   └── src/
│       ├── lib.rs               # Re-exports
│       ├── types.rs             # SettingDefinition, SettingKey, ValueType, Tier, SettingCategory, Constraint
│       ├── registry.rs          # SettingRegistry, global singleton, search, validate, filter
│       ├── json_schema.rs       # JSON Schema 2020-12 generation
│       └── metadata_json.rs     # Flat metadata JSON output
├── slicecore-config-derive/     # Proc-macro crate
│   └── src/
│       ├── lib.rs               # #[derive(SettingSchema)] entry point
│       ├── parse.rs             # Parse #[setting(...)] attributes
│       ├── codegen.rs           # Generate registration code
│       └── validate.rs          # Compile-time validation (min<max, key references)
├── slicecore-engine/            # Existing, gains dependency on both new crates
│   └── src/
│       ├── config.rs            # Add #[derive(SettingSchema)] + #[setting()] to all structs
│       └── config_validate.rs   # Replaced by registry.validate() (keep G-code template resolution)
└── slicecore-cli/               # Existing, add schema subcommand
    └── src/
        └── schema_command.rs    # New: --format, --tier, --category, --search
```

### Pattern 1: Proc-Macro Derive with Registration
**What:** The `#[derive(SettingSchema)]` macro generates an implementation of a `HasSettingSchema` trait that returns `Vec<SettingDefinition>` for each struct/enum. A global `SettingRegistry` collects all definitions on first access.
**When to use:** Every config struct and config enum.
**Example:**
```rust
// In slicecore-config-schema/src/types.rs
pub trait HasSettingSchema {
    /// Returns setting definitions for this type.
    fn setting_definitions(prefix: &str) -> Vec<SettingDefinition>;
}

// Generated by derive macro for SpeedConfig:
impl HasSettingSchema for SpeedConfig {
    fn setting_definitions(prefix: &str) -> Vec<SettingDefinition> {
        let prefix = if prefix.is_empty() {
            "speed".to_string()
        } else {
            format!("{prefix}.speed")
        };
        vec![
            SettingDefinition {
                key: SettingKey(format!("{prefix}.perimeter")),
                display_name: "Perimeter".to_string(),
                description: "Perimeter print speed".to_string(),
                tier: Tier::Simple,
                category: SettingCategory::Speed,
                value_type: ValueType::Float,
                default_value: serde_json::json!(45.0),
                min: Some(1.0),
                max: Some(1000.0),
                units: Some("mm/s".to_string()),
                affects: vec![SettingKey("quality".into())],
                // ... other fields
            },
            // ... more fields
        ]
    }
}
```

### Pattern 2: Flatten with Prefix Propagation
**What:** When PrintConfig has `pub speed: SpeedConfig`, the flatten attribute generates keys like `speed.perimeter`, `speed.infill`, etc. The macro passes the parent prefix to the child's `setting_definitions()`.
**When to use:** All sub-struct fields in PrintConfig.
**Example:**
```rust
#[derive(SettingSchema)]
#[setting(category = "general")]
pub struct PrintConfig {
    #[setting(flatten)]
    pub speed: SpeedConfig,           // generates speed.perimeter, speed.infill, etc.

    #[setting(flatten, prefix = "cooling")]
    pub cooling: CoolingConfig,       // generates cooling.fan_speed, etc.

    #[setting(skip)]
    pub passthrough: BTreeMap<String, String>,  // excluded from schema
}
```

### Pattern 3: Enum Schema Derivation
**What:** Enums derive `SettingSchema` to generate `ValueType::Enum { variants }` with display names and descriptions per variant.
**When to use:** All config enums (WallOrder, InfillPattern, BedType, etc.).
**Example:**
```rust
#[derive(SettingSchema)]
pub enum WallOrder {
    #[setting(display = "Inner First", description = "Print inner walls first")]
    InnerFirst,
    #[setting(display = "Outer First", description = "Print outer wall first")]
    OuterFirst,
}
// Generates: ValueType::Enum { variants: vec![
//     EnumVariant { value: "inner_first", display: "Inner First", description: "..." },
//     EnumVariant { value: "outer_first", display: "Outer First", description: "..." },
// ]}
```

### Pattern 4: Global Registry Singleton
**What:** A `LazyLock<SettingRegistry>` (or `once_cell::sync::Lazy`) that collects all definitions on first access.
**When to use:** `SettingRegistry::global()` for all runtime access.
**Example:**
```rust
use std::sync::LazyLock;

static GLOBAL_REGISTRY: LazyLock<SettingRegistry> = LazyLock::new(|| {
    let mut registry = SettingRegistry::new();
    // PrintConfig::setting_definitions("") recursively includes all sub-structs
    for def in PrintConfig::setting_definitions("") {
        registry.register(def);
    }
    registry.compute_affected_by(); // Inverse of affects
    registry
});

impl SettingRegistry {
    pub fn global() -> &'static SettingRegistry {
        &GLOBAL_REGISTRY
    }
}
```

### Anti-Patterns to Avoid
- **Circular dependency between schema crate and engine crate:** The schema crate defines types only. The engine crate derives the trait. The derive macro crate is proc-macro only. The registry is populated by engine code, not by the schema crate itself.
- **Putting SettingRegistry in the proc-macro crate:** Proc-macro crates can only export proc-macros. Runtime types must be in a separate crate.
- **Using schemars derive alongside custom derive:** Would create conflicting schema generation. Generate JSON Schema manually from the registry.
- **Hardcoding affected_by:** Only `affects` is annotated. `affected_by` is computed at registry initialization time.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Rust source parsing | Custom token parser | syn 2.x | Proc-macro parsing is extraordinarily complex |
| Token generation | String concatenation | quote! macro | Hygiene, span preservation, correctness |
| CLI argument parsing | Manual arg parsing | clap 4.x (existing) | Already in use, derive-based subcommands |
| JSON serialization | Manual JSON string building | serde_json | Already in workspace, handles escaping/nesting |

**Key insight:** The proc-macro ecosystem (syn + quote + proc-macro2) is the only viable approach for custom derive macros in Rust. There are no shortcuts or alternatives.

## Common Pitfalls

### Pitfall 1: Proc-Macro Crate Type Configuration
**What goes wrong:** Proc-macro crates MUST have `proc-macro = true` in Cargo.toml `[lib]` section and cannot export anything other than proc-macros.
**Why it happens:** Developers try to put runtime types in the same crate as the derive macro.
**How to avoid:** Two crates: `slicecore-config-derive` (proc-macro = true, exports only the derive) and `slicecore-config-schema` (normal lib, exports runtime types). The derive macro generates code that references types from the schema crate.
**Warning signs:** Compile errors about "can't find type" from generated code.

### Pitfall 2: Generated Code Must Use Full Paths
**What goes wrong:** Generated code references types like `SettingDefinition` without the crate path, fails if the user hasn't imported it.
**Why it happens:** Proc-macro output runs in the caller's scope.
**How to avoid:** Always use fully qualified paths in quote! output: `::slicecore_config_schema::SettingDefinition` instead of `SettingDefinition`.
**Warning signs:** "unresolved import" errors when using the derive macro.

### Pitfall 3: Compile-Time Key Validation Scope
**What goes wrong:** The proc-macro wants to validate that `affects = ["speed.perimeter"]` references a key that exists, but at compile time the macro only sees one struct at a time.
**Why it happens:** Proc-macros process items individually, no cross-struct visibility.
**How to avoid:** Do compile-time validation for local properties only (min < max, variant names). Defer cross-struct key validation to a runtime `SettingRegistry.validate_integrity()` check that runs in tests. Add a test that calls `SettingRegistry::global()` and checks for dangling key references.
**Warning signs:** Wanting to read other files from a proc-macro (this is fragile and non-idiomatic).

### Pitfall 4: Default Value Extraction
**What goes wrong:** The derive macro needs to know each field's default value, but `Default::default()` is a runtime operation.
**Why it happens:** Proc-macros operate at compile time; they cannot call runtime functions.
**How to avoid:** Two approaches: (a) require explicit `default = X` in the `#[setting()]` attribute, or (b) generate code that calls `Default::default()` at runtime and extracts the value via serde serialization. Option (b) is more maintainable since defaults are already defined in `impl Default` blocks. The registry initialization code can create a `PrintConfig::default()` and serialize it to extract all default values.
**Warning signs:** Default values drifting out of sync between `#[setting(default = X)]` and `impl Default`.

### Pitfall 5: MSRV and LazyLock
**What goes wrong:** `std::sync::LazyLock` requires Rust 1.80+, but workspace MSRV is 1.75.
**Why it happens:** MSRV was set early in the project.
**How to avoid:** Either bump MSRV to 1.80+ (recommended, since installed toolchain is 1.93.1 and no known consumers require 1.75) or use `once_cell::sync::Lazy` which works on any edition.
**Warning signs:** CI failures on older toolchains.

### Pitfall 6: Annotation Volume
**What goes wrong:** Annotating ~387 fields with full metadata (description, tier, units, min, max, affects) is tedious and error-prone. Typos in affects keys or wrong tier assignments are likely.
**Why it happens:** Scale of the task.
**How to avoid:** (1) Produce TIER_MAP.md first for user review. (2) Add a comprehensive integration test that loads the registry and validates: all keys referenced in `affects`/`depends_on` exist, all tiers have expected field counts, no empty descriptions for tier < 4. (3) Work struct-by-struct systematically.
**Warning signs:** Test failures on dangling key references.

### Pitfall 7: JSON Schema 2020-12 Nested Structure
**What goes wrong:** The nested struct hierarchy needs `$defs` for reusable sub-schemas, but the flattened dotted-key format for the registry conflicts with the nested JSON Schema structure.
**Why it happens:** JSON Schema naturally represents object nesting, but SettingKey uses flat dotted paths.
**How to avoid:** Generate two separate outputs: (1) JSON Schema uses nested `properties` matching the Rust struct hierarchy with `$ref` to `$defs` for sub-schemas. (2) Flat metadata JSON uses the dotted SettingKey directly. These are independent output formats, not derived from each other.
**Warning signs:** Trying to flatten the JSON Schema or nest the metadata JSON.

## Code Examples

### SettingDefinition Type
```rust
// slicecore-config-schema/src/types.rs

use serde::{Deserialize, Serialize};

/// A dotted-path key identifying a setting (e.g., "speed.perimeter").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SettingKey(pub String);

/// Progressive disclosure tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Tier {
    AiAuto = 0,
    Simple = 1,
    Intermediate = 2,
    Advanced = 3,
    Developer = 4,
}

/// Setting category for grouping in UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SettingCategory {
    Quality,
    Speed,
    LineWidth,
    Cooling,
    Retraction,
    Support,
    Infill,
    Adhesion,
    Advanced,
    Machine,
    Filament,
    Acceleration,
    PostProcess,
    Timelapse,
    MultiMaterial,
    Calibration,
}

/// Value type for a setting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValueType {
    Bool,
    Int,
    Float,
    String,
    Percent,
    Enum { variants: Vec<EnumVariant> },
    FloatVec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumVariant {
    pub value: String,
    pub display: String,
    pub description: String,
}

/// A constraint on a setting value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Constraint {
    Range { min: f64, max: f64 },
    DependsOn { key: SettingKey, condition: String },
}

/// Full metadata for a single setting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingDefinition {
    pub key: SettingKey,
    pub display_name: String,
    pub description: String,
    pub tier: Tier,
    pub category: SettingCategory,
    pub value_type: ValueType,
    pub default_value: serde_json::Value,
    pub constraints: Vec<Constraint>,
    pub affects: Vec<SettingKey>,
    pub affected_by: Vec<SettingKey>,  // computed, not annotated
    pub units: Option<String>,
    pub tags: Vec<String>,
    pub since_version: String,
    pub deprecated: Option<String>,
}
```

### Proc-Macro Attribute Parsing
```rust
// slicecore-config-derive/src/parse.rs
// Parse #[setting(tier = 1, description = "...", units = "mm/s", min = 0.0, max = 1000.0)]

use syn::{Attribute, Expr, Lit, Meta, MetaNameValue};

pub struct SettingAttrs {
    pub tier: Option<u8>,
    pub description: Option<String>,
    pub display_name: Option<String>,
    pub category: Option<String>,
    pub units: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub affects: Vec<String>,
    pub depends_on: Option<String>,
    pub tags: Vec<String>,
    pub since_version: Option<String>,
    pub deprecated: Option<String>,
    pub skip: bool,
    pub flatten: bool,
    pub prefix: Option<String>,
}
```

### Registry Search Implementation
```rust
// slicecore-config-schema/src/registry.rs

impl SettingRegistry {
    /// Search settings by case-insensitive substring across key, display_name,
    /// description, and tags.
    pub fn search(&self, query: &str) -> Vec<&SettingDefinition> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<(&SettingDefinition, u8)> = self
            .definitions
            .values()
            .filter_map(|def| {
                let mut score = 0u8;
                if def.key.0.to_lowercase().contains(&query_lower) {
                    score += 3; // key match highest priority
                }
                if def.display_name.to_lowercase().contains(&query_lower) {
                    score += 2;
                }
                if def.description.to_lowercase().contains(&query_lower) {
                    score += 1;
                }
                if def.tags.iter().any(|t| t.to_lowercase().contains(&query_lower)) {
                    score += 2;
                }
                if score > 0 { Some((def, score)) } else { None }
            })
            .collect();
        results.sort_by(|a, b| b.1.cmp(&a.1));
        results.into_iter().map(|(def, _)| def).collect()
    }
}
```

### JSON Schema Generation Skeleton
```rust
// slicecore-config-schema/src/json_schema.rs

impl SettingRegistry {
    pub fn to_json_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://slicecore.dev/config-schema.json",
            "title": "SliceCore Print Configuration",
            "type": "object",
            "properties": self.generate_nested_properties(),
            "$defs": self.generate_defs(),
        })
    }

    // Each property includes x- extensions:
    // "perimeter": {
    //     "type": "number",
    //     "minimum": 1.0,
    //     "maximum": 1000.0,
    //     "default": 45.0,
    //     "x-tier": 1,
    //     "x-category": "speed",
    //     "x-units": "mm/s",
    //     "x-display-name": "Perimeter",
    //     "x-affects": ["quality"],
    //     "x-tags": ["speed", "perimeter"],
    //     "x-since-version": "0.1.0"
    // }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `once_cell::sync::Lazy` | `std::sync::LazyLock` | Rust 1.80 (2024-07) | No external dep for lazy statics |
| syn 1.x | syn 2.x | 2023 | Better error reporting, spans |
| JSON Schema draft-07 | JSON Schema 2020-12 | 2020 | `$defs` replaces `definitions`, `$dynamicRef` |

**Deprecated/outdated:**
- `lazy_static!` macro: Replaced by `LazyLock` or `once_cell`. Do not use.
- syn 1.x: Still works but syn 2.x is current and actively maintained.

## Existing Code Inventory

### Config Structs (need `#[derive(SettingSchema)]`)
| Struct | File | Approx Fields | Category |
|--------|------|:-------------:|----------|
| PrintConfig | config.rs:1018 | ~90 (incl sub-struct refs) | General |
| SpeedConfig | config.rs:313 | 21 | Speed |
| CoolingConfig | config.rs:397 | ~15 | Cooling |
| RetractionConfig | config.rs:455 | ~12 | Retraction |
| MachineConfig | config.rs:500 | ~60 | Machine |
| AccelerationConfig | config.rs:746 | ~25 | Acceleration |
| FilamentPropsConfig | config.rs:845 | ~40 | Filament |
| LineWidthConfig | config.rs:272 | 7 | LineWidth |
| FuzzySkinConfig | config.rs:121 | 3 | Advanced |
| BrimSkirtConfig | config.rs:152 | ~8 | Adhesion |
| InputShapingConfig | config.rs:186 | ~5 | Advanced |
| ToolChangeRetractionConfig | config.rs:209 | 2 | Retraction |
| DimensionalCompensationConfig | config.rs:234 | 3 | Advanced |
| PostProcessConfig | config.rs:1332 | ~15 | PostProcess |
| TimelapseConfig | config.rs:1395 | ~10 | Timelapse |
| ScarfJointConfig | config.rs:1479 | ~12 | Quality |
| ToolConfig | config.rs:1788 | ~5 | Machine |
| MultiMaterialConfig | config.rs:1814 | ~25 | MultiMaterial |
| SequentialConfig | config.rs:1929 | ~10 | Advanced |
| PaCalibrationConfig | config.rs:1979 | ~10 | Calibration |
| SupportConfig | support/config.rs | ~27 | Support |
| BridgeConfig | support/config.rs | ~8 | Support |
| TreeSupportConfig | support/config.rs | ~10 | Support |

### Config Enums (need `#[derive(SettingSchema)]`)
| Enum | File | Variants |
|------|------|:--------:|
| WallOrder | config.rs:27 | 2 |
| SurfacePattern | config.rs:42 | 6 |
| BedType | config.rs:65 | 6 |
| InternalBridgeMode | config.rs:87 | 3 |
| BrimType | config.rs:103 | 4 |
| CustomGcodeTrigger | config.rs:1442 | ~5 |
| SlicingTolerance | config.rs:1554 | ~3 |
| ScarfJointType | config.rs:1567 | ~3 |
| SupportType | support/config.rs:24 | 4 |
| SupportPattern | support/config.rs:39 | 5 |
| InterfacePattern | support/config.rs:56 | 3 |
| TreeBranchStyle | support/config.rs:69 | 3 |
| TaperMethod | support/config.rs:84 | ~3 |
| QualityPreset | support/config.rs:102 | ~3 |
| ConflictResolution | support/config.rs:114 | ~3 |

Additional enums from other modules: InfillPattern, SeamPosition, GcodeDialect, IroningConfig fields. These also need `#[derive(SettingSchema)]` since they are used as config field types.

### Existing Validation (to be replaced)
- `config_validate.rs`: ~200 lines of hardcoded range checks
- Checks: layer_height > 0, nozzle_diameter > 0, temperature limits, extreme speed warnings
- Also contains `resolve_template_variables()` which is NOT validation -- keep this function
- The validation issues use `ValidationSeverity::Warning` and `ValidationSeverity::Error` -- the new schema validation should preserve this distinction

**Total estimated field count:** ~387 fields across all structs (333 in config.rs + 54 in support/config.rs)

## Dependency Graph Considerations

The `affects` relationship captures domain knowledge about which settings influence each other. Key patterns:

- **layer_height** affects: quality, print_time, strength, nearly everything
- **speed.\*** affects: quality, print_time, cooling requirements
- **cooling.\*** affects: quality, bridging, overhangs
- **retraction.\*** affects: stringing, oozing, print_quality
- **infill_density** affects: strength, print_time, filament_usage
- **support.enable** is a depends_on target for all support sub-settings
- **machine.\*** mostly standalone (physical constraints)

The full affects graph should be informed by OrcaSlicer's setting interactions. The TIER_MAP.md artifact should include a rationale column showing which OrcaSlicer tab (Simple/Advanced/Expert) each setting appears in.

## Open Questions

1. **MSRV bump to 1.80+**
   - What we know: LazyLock requires 1.80, installed toolchain is 1.93.1, declared MSRV is 1.75
   - What's unclear: Whether any downstream consumer depends on MSRV 1.75
   - Recommendation: Bump to 1.80 (conservative) or 1.85 (current stable-ish). Alternative: use once_cell.

2. **Default value extraction strategy**
   - What we know: Defaults are defined in `impl Default` blocks, proc-macro cannot call runtime code
   - What's unclear: Whether to duplicate defaults in attributes or extract at runtime
   - Recommendation: Extract at runtime via `PrintConfig::default()` + serde serialization during registry initialization. Avoids duplication.

3. **Cross-module enum derives**
   - What we know: InfillPattern is in `crates/slicecore-engine/src/infill.rs`, SeamPosition in `seam.rs`, etc.
   - What's unclear: Whether these enums should also derive SettingSchema or be handled differently
   - Recommendation: Add `#[derive(SettingSchema)]` to all enums used as config field types. The macro generates ValueType::Enum with variant metadata. The config field then references the enum's generated schema.

4. **Compile-time key validation feasibility**
   - What we know: Proc-macros process one item at a time, cannot see other structs
   - What's unclear: How to validate affects/depends_on key references at compile time
   - Recommendation: Validate at test time via `SettingRegistry::global().validate_integrity()` rather than compile time. The CONTEXT.md says "compile-time: macro validates... referenced depends_on/affects keys exist" but this is infeasible in a proc-macro. Use a mandatory integration test instead.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | N/A |
| Quick run command | `cargo test -p slicecore-config-schema` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| N/A-01 | Derive macro generates valid setting definitions | unit | `cargo test -p slicecore-config-derive` | Wave 0 |
| N/A-02 | Registry contains all ~387 fields | integration | `cargo test -p slicecore-engine -- registry` | Wave 0 |
| N/A-03 | JSON Schema output is valid 2020-12 | unit | `cargo test -p slicecore-config-schema -- json_schema` | Wave 0 |
| N/A-04 | Flat metadata JSON contains all fields | unit | `cargo test -p slicecore-config-schema -- metadata` | Wave 0 |
| N/A-05 | CLI schema subcommand works | integration | `cargo test -p slicecore-cli -- schema` | Wave 0 |
| N/A-06 | Schema-driven validation replaces ad-hoc | integration | `cargo test -p slicecore-engine -- validate` | Wave 0 |
| N/A-07 | Search returns ranked results | unit | `cargo test -p slicecore-config-schema -- search` | Wave 0 |
| N/A-08 | All affects/depends_on keys resolve | integration | `cargo test -p slicecore-engine -- integrity` | Wave 0 |
| N/A-09 | Tier counts match expectations | integration | `cargo test -p slicecore-engine -- tier_counts` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-config-schema -p slicecore-config-derive`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/slicecore-config-schema/` -- entire new crate
- [ ] `crates/slicecore-config-derive/` -- entire new crate
- [ ] `crates/slicecore-config-derive/tests/` -- trybuild or expansion tests for the derive macro
- [ ] Integration test in slicecore-engine verifying registry completeness and integrity

## Sources

### Primary (HIGH confidence)
- Project codebase: `crates/slicecore-engine/src/config.rs` (2794 lines, ~333 fields)
- Project codebase: `crates/slicecore-engine/src/support/config.rs` (~54 fields)
- Project codebase: `crates/slicecore-engine/src/config_validate.rs` (existing validation)
- Project design docs: `designDocs/CONFIG_PARITY_AUDIT.md` Section 5-6 (schema system design)
- Project design docs: `designDocs/02-ARCHITECTURE.md` Section 6.2 (ConfigSchema spec)
- Project design docs: `designDocs/01-PRODUCT_REQUIREMENTS.md` Section 7.2 (SettingDefinition)
- `cargo search`: syn 2.0.117, quote 1.0.45, proc-macro2 1.0.106, schemars 1.2.1
- Rust toolchain: rustc 1.93.1 installed (LazyLock available)
- Workspace: edition 2021, MSRV 1.75, resolver = "2"

### Secondary (MEDIUM confidence)
- JSON Schema 2020-12 spec (well-established standard, `x-` extension pattern from OpenAPI)
- syn/quote proc-macro patterns (well-documented ecosystem, standard approach)

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - syn/quote/proc-macro2 is the only viable proc-macro stack in Rust
- Architecture: HIGH - Two-crate pattern (types + derive) is universal for derive macros in Rust
- Pitfalls: HIGH - Based on extensive proc-macro development experience and codebase analysis
- Annotation scope: HIGH - Exact field count verified from source (333 + 54 = 387)

**Research date:** 2026-03-17
**Valid until:** 2026-04-17 (stable domain, no fast-moving dependencies)
